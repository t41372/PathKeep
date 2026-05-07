# RECALL — 召回

> 從 [vision-and-requirements.md](../vision-and-requirements.md) 抽出。Recall 建立在穩定的 Archive 之上。

---

## 1. 歷史紀錄瀏覽器

**作為**用戶，**我想要**用直覺的方式瀏覽和搜尋我的所有歷史紀錄，**以便**能快速找到過去看過的內容。

### 互動式時間軸

這是 Explorer 的核心導航元素。用戶面對的可能是跨越 20 年以上、數千萬筆的海量歷史紀錄，需要一種直覺、流暢的方式在時間中穿梭。

- **可拖動的時間軸控件**：
  - 水平或垂直的時間軸 rail，用戶可以拖動、滾動、或點擊來快速定位。
  - 支援多個縮放級別：年 → 月 → 週 → 天。
  - 拖動時有即時的視覺反饋 — 顯示當前指向的日期和該時段的記錄密度。
  - 記錄密度的高低應該在時間軸上有視覺表達（例如色彩深淺、柱狀高度），讓用戶一眼看出哪些時間段比較活躍。
- **快速跳轉**：
  - 點擊年份 → 展開該年的月份 → 點擊月份 → 展開天數。
  - 也可以直接輸入日期跳轉。
- **回到今天**：一鍵回到最新的紀錄。
- 時間軸應當有流暢的動畫和過渡效果，拖動手感要好。

### 大數據量下的效能設計

考慮到 20 年以上的重度使用者可能累積數千萬筆歷史紀錄（按 2,500 visits/天計算，20 年約 1800 萬筆），時間軸和列表**必須**在這個量級下保持流暢互動。

- **預聚合統計表**：後端維護增量更新的 `daily_visit_counts`、`domain_daily_counts` 等聚合表。時間軸拖動時查聚合表（20 年 ≈ 7,300 行），不查主表。
- **虛擬滾動**：列表使用虛擬化（只渲染可見區域的 DOM），無論總記錄數多大，渲染成本恆定。
- **分頁加載**：列表和搜尋結果走 cursor-based pagination，永遠只加載一頁。UI 必須在結果列表上方與底部分頁列都直接顯示「當前頁 / 總頁數」，提供第一頁 / 上一頁 / 下一頁 / 最後一頁、跳頁與每頁筆數控制（例如 25 / 50 / 100 / 200），避免用戶在跳頁時失去定位。每頁筆數選擇屬於 Explorer 偏好，離開頁面或重啟 app 後都要保留。
- **時間軸不觸發全表掃描**：拖動時的密度可視化來自聚合表，點擊展開某一天才查詢該天的具體記錄。
- **搜尋走 FTS5 索引**：全文搜尋不走 `LIKE`，走 FTS5 倒排索引，查詢速度不隨數據量線性增長。M14 後 keyword recall 會先做 ICU4X NFKC、官方 OpenCC 字典資產的繁簡 folding、lowercase、compact normalization，再查 unicode61 term/prefix、CJK gram 與 trigram compact projection。短別名（`gh` / `yt` / `pr`）會先展開成可審查的固定詞表；Latin typo tolerance 只在正常 FTS/trigram 無結果時啟動，且先由 trigram top-N 產生 bounded candidate，再做 Rust-side edit-distance scoring。
- **Favicon 不進主列表 payload**：Explorer 的主 `query_history` response 只回 row metadata；列表先用 placeholder reveal，再以 page-scoped batched lookup 補 icon，避免 favicon bytes / base64 序列化卡住首屏與翻頁。
- **Favicon 導入去重**：新導入的 favicon image bytes 必須在 ingest 階段去重存放，避免同一張 icon 在 archive 裡被重複寫入多次，拖高磁盤與後續 recall payload 成本。
- **Favicon domain fallback 必須是 lazy、indexed、time-aware**：當某筆 history row 沒有可用的 exact page icon 時，hydration 允許依序嘗試同 profile / 跨 profile 的同 host、同 registrable domain 已保存 icon；exact page lookup 必須維持快路徑，host / registrable-domain fallback 只能在 miss 後分級嘗試，每級都必須用 indexed `LIMIT 1` 先選出候選後才讀取 `favicon_blobs`。fallback 查詢必須只走 `favicons` 的 page / host / registrable-domain 索引，不得把 favicon bytes 拉進主列表查詢，也不得做全表掃描或把多級候選合成會擴大排序面的 monolithic SQL。只要 lookup 帶 visit time，所有候選 icon 的 `last_updated_ms` 都必須早於或等於該 visit time，避免網站多年後換 icon 時把舊訪問紀錄在讀取層刷成新 icon。
- `WORK-M4-G` 已將 Explorer day-one keyword recall 收斂到 canonical FTS5 projection；`WORK-M14-A` 將該 projection 升級為 versioned `history_search_terms` + `history_search_trigram`，索引 raw / normalized URL、title、search term、compact text 與 CJK grams；regex mode 仍維持 post-filter 邊界。

### 搜尋與篩選

- **全文搜尋**（基於 FTS5）：搜尋 URL、標題、搜尋關鍵詞。Keyword mode 預設以 relevance 排序，明確選擇 newest / oldest 時才回到時間排序；relevance 只來自本機 lexical FTS/BM25，不是 embedding 或 AI ranking。
- **高級關鍵詞語法**：Keyword mode 支援本地歷史紀錄可誠實落地的 Google-like operators。這些語法只讀 PathKeep 已歸檔的 URL、title、search terms 與 visit time，不推斷網頁正文，也不依賴網路。
  - `site:github.com`：限制 URL/site/domain；也可繼續使用獨立 Domain 篩選欄。
  - `-pathkeep`、`-"release candidate"`：排除 URL、title、search terms / normalized compact projection 中包含指定詞或片語的結果。常見用法是 Domain 篩選 `github.com` + query `-pathkeep`。
  - `"release notes"`：要求 URL、title 或 search terms 中出現 exact phrase，同時仍用 lexical FTS 建候選。
  - `manual OR youtube`：任一側詞組命中即可返回，對應 Google Advanced Search 的「any of these words」。
  - `intitle:manual` / `inurl:pull-request`：要求詞出現在 title 或 URL 欄位。
  - `filetype:pdf` / `ext:pdf`：依 URL 副檔名篩選本地歷史結果。
  - `after:2026-05-01` / `before:2026-05-07`：依 visit time 收窄；會與 UI date filters 取交集。
  - 不支援 Google 的 language / region / usage rights / related / image-only operators，因為 v0.1 archive 不保存或不能本地證明這些 web-index 屬性。
- **Regex 搜尋**：Explorer 提供顯式的正則模式，用於 URL / title 的手動進階檢索。
  - **切換按鈕**：搜尋列旁有 toggle button，讓用戶在 FTS5 keyword 模式和 regex 模式之間切換。切換時保留目前輸入的搜尋字串，但清楚更新 placeholder 提示（如 "Search keywords…" ↔ "Regex pattern…"）。
  - **Client-side regex 驗證**：每次輸入變更時即時驗證 pattern 合法性。Regex dialect 以後端 Rust `regex` crate 為準；look-around / backreference 這類 JavaScript 可接受但 Rust 不支援的語法必須在 UI 先擋下。Invalid regex 直接在 UI 阻止查詢並顯示錯誤訊息（如 "Invalid regex: unterminated group"），同時保留目前可見的搜尋結果不被清空。
  - **URL 參數**：regex 模式透過 `?regex=1` query string 持久化，讓搜尋結果可分享、可書籤、可重新載入。與其他 Explorer filter 參數（`q`、`profileId`、`domain` 等）正交組合。
  - 這個模式必須清楚標示自己不是 day-one 快速路徑；UI 先驗證 pattern，再執行 scoped query。
- **複合篩選**，可疊加使用：
  - 按瀏覽器 / Profile
  - 按 Domain（支援子域名匹配）
  - 按時間範圍（可與時間軸聯動）
  - 按頁面類型（如果有分類數據：docs, forum, video, news 等）
  - 按來源途徑（typed, link, redirect, bookmark 等）
  - 按 run ID / 導入批次
- Explorer、Export、Dashboard、AI search、Insights 等 read models 都只能讀取**當前可見** facts；已 rollback 的 visits / downloads / search terms 不能漏出，restore 後則要重新可見。
- 篩選狀態在 UI 上有清晰的標籤式展示，可逐個移除或一鍵清除。
- shell chrome 提供共享的 profile viewing scope。Explorer 預設繼承這個 scope，但 route 上若有明確 `profileId` filter，頁面級 filter 必須優先。

### Regex 搜尋的效能邊界

- FTS5 仍是 day-one keyword recall 的正式快速路徑；regex 不是它的替代品。
- regex mode 在 canonical filter（profile / browser / domain / date range / visibility）之後做 post-filter：先由後端以 canonical filter 縮小結果集，再對縮小後的結果執行 regex 匹配。這確保 regex 永遠只跑在受限的 working set 上，不觸發全表掃描。
- 對用戶的 UX 含義：regex 搜尋在已縮窄的結果集上通常足夠快，但在無任何 canonical filter 的情況下對大型 archive 可能較慢。UI 應在這種情境下顯示適當的載入指示。
- 若未來要把 regex 升級成大數據量下也可接受的正式 fast path，必須先新增獨立 research / benchmark，再改文檔與實作。

### Regex 搜尋的已實現狀態

- M1 已交付：Explorer 的 regex toggle、client-side validation、`?regex=1` URL 參數、post-filter 執行路徑。
- 目前 regex 支援 URL 和 title 欄位的匹配；不支援 page content 或 enrichment 欄位。
- regex 搜尋結果與 FTS5 搜尋結果共用相同的 list / detail / export 介面，切換模式不改變下游 UX。

### 單條記錄顯示

用戶瀏覽歷史紀錄時，每條記錄默認顯示的信息以及可選顯示的信息，**用戶可以在設定中自定義**。

**預設顯示：**

- Favicon + 頁面標題（若該 history row 沒有 exact page icon payload，lazy hydration 先嘗試 visit-time aware 的同 host / 同 registrable-domain icon fallback；仍沒有可用 icon 時，UI 顯示 deterministic placeholder，不顯示 broken image）
- 列表中的 favicon 允許在 row 已經顯示後再批量補齊；icon hydration 不能阻塞首屏 skeleton 消失或翻頁後的 row reveal。
- URL（可展開/摺疊長 URL）
- 訪問時間
- 來源瀏覽器 / Profile 標識

**可選顯示（用戶在設定中開關）：**

- 訪問次數
- 來源途徑（typed, link, redirect 等）
- Domain 分類標籤
- Provenance 信息（哪次 run 寫入的）
- 所有 metadata versions（如果該 URL 的 title 等信息有過變化）
- 搜尋關鍵詞（如果這次訪問來自搜尋）
- Transition / referrer 信息

用戶的顯示偏好持久保存，跨 session 保留。

### 記錄詳情面板

點擊任一條記錄，展開詳情面板，顯示該條記錄的完整信息：

- 完整 URL
- 頁面標題（所有歷史版本）
- 所有訪問時間（如果同一 URL 被多次訪問）
- 訪問次數、typed count
- 來源途徑和 referrer
- Favicon（若 archive 目前沒有這筆 row 可用的 icon payload，detail 仍保留同樣的 placeholder fallback）
- 來源瀏覽器 / Profile
- Provenance：寫入的 run ID、run 時間、run 來源
- 如果有 metadata 變化歷史，顯示 version diff

Detail rail 屬於持續參照面板：當用戶已經捲到列表底部、再選擇一筆新記錄時，右側 detail 必須仍然留在可視區，不可因為左側列表變長就被推回頁面頂端。

### 通用

- 支援 keyboard shortcut 和快速導航（上下鍵切換記錄、Enter 展開詳情、Esc 關閉）。
- 支援從 Explorer 直接匯出當前篩選結果。
- 大數據量下保持流暢（虛擬滾動 / 分頁加載）；切換頁碼時不得強制把整個 Explorer scroll container 拉回頁首，用戶視角應保持在原本位置。
- time-view 的首屏與翻頁以 skeleton-first 為主：主結果頁一旦可顯示就先畫出來，前後相鄰頁可以在背景預取，但不得因為等待相鄰頁或 favicon 補齊而阻塞目前頁面打開。
- Explorer 的背景預取窗口屬於 user-configurable performance preference，入口在 Settings > General；shipping default 是每側 `5` 頁，並且必須保持有上限的 bounded range，避免在超大 archive 上把相鄰頁 warmup 退化成無上限背景掃描。

---

## 2. 版本管理與回滾

**作為**用戶，**我想要**在發現任何誤操作後能回滾到之前的狀態，**以便**不用擔心「試一下會不會搞壞數據」。

**用戶必須有信心操作這個工具，知道任何操作都是可撤銷的。**

### Run 級別的回滾

- 每次寫入操作（定時備份 run、手動備份 run、Takeout 導入、瀏覽器直接導入）都有唯一 run ID。
- 用戶能在 Audit Ledger 中檢視每次 run 的：
  - 執行時間、來源類型、來源 profile
  - 寫入記錄數量（新增 / 更新 / 跳過 / 失敗）
  - 當前狀態（completed / reverted / partial）
- Audit Ledger / Dashboard recent runs 必須直接反映 unified run ledger 的真實 `run_type`，至少涵蓋 `backup`、`import`、`rollback`、`doctor`；不得再用 trigger-only 或 backup-only 的近似資料冒充 run type。
- Audit Ledger 必須支援至少按 run type、severity、profile / source scope、artifact type 篩選，並能把目前選中的 run 和上一筆可見 run 做 summary delta，避免使用者在回滾或信任某次 run 前失去比較基準。
- Audit Ledger 的主入口必須是 run timeline，而不是只有 manifest 路徑或 hash；使用者要能一眼看出「這次是哪種 run、何時發生、改了多少資料、接下來可以做什麼」。
- 用戶能展開某次 run，預覽它寫入的所有記錄。
- Audit / import batch detail 必須顯示 visible / reverted item 數量、warnings 與 audit artifact 路徑，讓使用者能先確認再 rollback 或 restore。
- import / rollback / restore 類 run 若已有對應 import batch preview，Audit Ledger 必須直接顯示 record-level change preview，並提供回到 `/import?batch=<id>` 的 review deep-link。
- 用戶能**回滾整次 run**：
  - 該 run 寫入的所有記錄標記為 reverted（軟刪除，不物理刪除）。
  - Reverted 的記錄從正常搜尋和瀏覽中隱藏，但保留在底層以備審計。
  - 回滾操作本身記入審計日誌，產生新的 manifest。
  - 回滾是可逆的 — 用戶可以「取消回滾」，重新恢復那次 run 的記錄；恢復後 Explorer / Export / AI / Insights 必須回到一致的可見狀態。

### 典型誤操作場景

| 場景                          | 恢復方式                         |
| ----------------------------- | -------------------------------- |
| 導入了格式錯誤的 Takeout 檔案 | 回滾該次 import run              |
| 錯誤的 profile 被選中並備份了 | 回滾該次 backup run              |
| 同一份數據被重複導入          | 去重機制自動處理；如有異常可回滾 |
| 導入了別人的歷史紀錄          | 回滾該次 import run              |
| Schema migration 後發現問題   | Archive DB 自動備份可恢復        |

### Archive 快照（Safety Net）

- 除了 run-level 的回滾，archive 在以下時機自動保存完整快照：
  - Archive schema migration 前
  - 大型導入（超過設定閾值的記錄數）前
  - 用戶手動觸發
- 用戶可以在 UI 中查看所有可用的快照，查看快照的時間和大小。
- 用戶可以從快照恢復整個 archive（全局回滾到某個時間點）。
- **快照有保留上限**（見 [data-model.md](../architecture/data-model.md)）。

### 設計約束

- 回滾不依賴 Git — Git 只管理審計工件，不管理主 archive DB。
- 回滾在 archive DB 層面實現（軟刪除 + 快照），不是 Git revert。
- 回滾操作要足夠快 — O(records in run)，不需要重建整個 archive。
- UI 中回滾必須有確認步驟和影響預覽（將會隱藏多少筆記錄）。
- rollback / restore 後若衍生狀態（FTS、AI embeddings、insights）失真，doctor repair 必須能偵測並清理，讓系統回到可重建狀態。
