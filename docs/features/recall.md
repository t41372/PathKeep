# RECALL — 召回

> 從 [vision-and-requirements.md](../vision-and-requirements.md) 抽出。Recall 建立在穩定的 Archive 之上。

---

## 0. 2026-10 介面

M18 redesign（2026-10-02）後，下文的「Explorer」就是 **History**（`src/features/history/`），「Audit Ledger」是 **Backup → 最近執行**。後端的搜尋、分頁、favicon 與回滾行為不變。畫面細節見 [screens-and-nav.md](../design/screens-and-nav.md)。

### 現在的 History

- **三個視圖**：時間線（依天分組，同一天內相隔 30 分鐘以上切成 session）、網站（所選範圍內最常去的 200 個網站，每列帶該網站最近一頁的連結預覽縮圖，點一個切回時間線並篩選）、已加星（頁面與整個網站，附連結預覽縮圖、最多三個標籤與筆記預覽）。搜尋時改為結果列表，同一頁面只列一行並標出符合的瀏覽次數。
- **搜尋框**：一個輸入框，打字 250 ms 後才寫進 URL 並查詢。三種方式：
  - **全文**（預設）：`query_history`（`groupByUrl`），依相關度排序；§1 的進階語法（`site:`、`-詞`、`"片語"`、`OR`、`intitle:`、`inurl:`、`filetype:`、`after:` / `before:`、`note:` / `tag:`）由後端解析。搜尋框裡的「?」（點擊、鍵盤或滑鼠停留）打開語法速查（`search-help.tsx`）：每個運算子一個例子，點一下就填進搜尋框並切回全文；日期例子固定是上個月。只有 `tag:` 而沒有關鍵字的搜尋從 `url_tags` 找候選，不掃整個 `urls`（[ipc-performance.md](../architecture/ipc-performance.md) §6）。
    - **排序窗口**：一個詞（或只有運算子的搜尋）在篩選之內命中超過 25,000 個網址時，只排序、分組、計數最近封存的 25,000 個（`oldest` 排序取最早封存的）。回應帶 `windowed: true`、`totalExact: false`，結果數顯示「25,000+ 個頁面」這類下限，下面多一行「沒有為每個結果排序，只排了最近保存的那些。再加一個詞可以縮小範圍。」。篩選（瀏覽器、profile、日期、網域、`site:` 等）先於窗口；窗口邊界寫進 cursor，翻頁期間有新備份也不會跳行或重複。未超過的詞結果與以前完全一樣。只有 `tag:` 的搜尋不走窗口（標籤集是手寫的、很小）。實作與失效模式見 `vault-core/src/archive/history/grouped/window.rs`，數字見 [ipc-performance.md](../architecture/ipc-performance.md) §6。
    - 其他呼叫者：AI 搜尋（混合與依日期排序）在被窗口截斷時多一個 `lexicalWindowed` 註記（給模型的英文說明，MCP `search-history` 的 `notes` 也帶），`export_history` 的結果多 `windowed`；⌘K 只列前 6 個頁面、不顯示數字，不受影響。
  - **Regex**：`query_history` 加 `regexMode`，依時間新到舊。把查詢用 `/…/` 包起來時，不管選了哪種方式都當 regex。前端用 Rust `regex` 的規則檢查（`regex-dialect.ts`：群組、巢狀字元類、跳脫字元、重複次數），擋下 look-around、反向參照、atomic / 條件群組、Rust 不認得的跳脫字元與不是重複次數的 `{`，用白話說明哪裡不支援，不送出查詢，畫面保留上一批結果（淡化）。不再用 JavaScript `RegExp`：它會擋下 Rust 接受的寫法（`a++`、`(?i)`、`(?P<name>…)`）。檢查器與後端共用一張案例表（`regex-dialect-cases.json`；Rust 測試 `regex_dialect_cases` 與 vitest 各跑一次）。只有編譯時才知道的錯誤（不存在的 Unicode 屬性、反向範圍、大小上限）仍會送到後端，畫面顯示「這個正規表示式無法執行」與 Rust 的訊息，不再是「搜尋沒有執行成功」。
    - **分段掃描整個 archive**：每個請求從上次停下的地方往舊掃最多 200 ms，回傳找到的結果、續掃 cursor 與 `regexScan`（掃到哪個時間、是否掃完）。捲動列表會續掃；列表不滿一頁時自己接著要下一段，之後顯示「目前找到 48+ 個頁面 · 已搜索到 2025年3月」與「繼續搜索」（掃描中是「停止」）。掃完之前數字都是「目前」，`totalExact` 只有一次掃完整個篩選範圍時才為 true。分組時每段的列只算該段的訪問，同一頁面在後段再出現時由列表相加。篩選是掃描條件的一部分，日期篩選會縮短掃描。中途離開不會留下任何在跑的工作（cursor 就是全部狀態）。實作與失效模式見 `vault-core/src/archive/history/regex_scan.rs`，數字見 [ipc-performance.md](../architecture/ipc-performance.md) §7。
  - **語意**：`search_ai_history`（AI 搜尋），只有 `ai.enabled`、`semanticIndexEnabled` 都開而且索引就緒時可選；否則自動退回全文。不可選時，按鈕的提示與結果數下方一行說明原因（未開啟、建立中、等待建立、已暫停、索引是空的、建立失敗、無法使用、archive 未設定），並連到 Settings → AI；可用時同一行寫出 provider 與模型、已索引頁數、更新時間，索引過期時提示去重建。後端只能依網域篩選，日期與瀏覽器篩選在前端對已載入的結果做。
- **篩選**：日期（今天、昨天、最近 7 天、最近 30 天、自訂起訖日）、瀏覽器（依瀏覽器種類，不是單一 profile）、網站（從網站視圖或 Insights 帶入的 chip）。全部寫在 URL（`date`、`browser`、`domain`），可一鍵清除。
- **列表**：虛擬化、固定列高；cursor 分頁，捲到接近底部自動載下一頁（每頁 100 筆，語意每頁 50 筆）。結果數先顯示「100+」，旁邊另一個 `limit: 1, includeTotal: true` 的查詢算完精確總數後再換成精確數字；全文搜尋被窗口截斷時一直是「N+」。
- **詳情面板**：標題、網址；在瀏覽器打開、加星 / 取消、複製連結；連結預覽（og:image，固定 1.91 : 1 的框，見 [og-images.md](og-images.md) §4）；總瀏覽次數、第一次與最近一次瀏覽、來源瀏覽器（`get_url_detail`）；最近 12 週每週瀏覽數；同一 session 的其他頁面；標籤（點標籤就搜尋 `tag:名稱`，見 [annotations.md](annotations.md) §4）；備註。
- **鍵盤**：↑↓ 移動選取、Enter 在瀏覽器打開、Esc 關閉面板。
- **回滾**：介面上只有 Takeout 匯入批次能「復原」與「還原」（Backup → 匯入卡的近期匯入）。

### 已接受要求中 2026-10 介面沒有做到的

以下要求沒有被改掉，只是新介面沒做；要補還是改需求，需要用戶決定：

- §1 互動式時間軸 rail（拖動、年 → 月 → 週 → 天縮放、密度可視化、輸入日期跳轉、回到今天）。現在只有日期篩選。
- §1 分頁列（「第 N / 共 M 頁」、跳頁、每頁筆數、偏好保存）與 Settings 的背景預取窗口。現在是連續捲動，見 [ui-review-guardrails.md](../design/ui-review-guardrails.md) §8。
- §1 依頁面類型、來源途徑、run / 匯入批次篩選；依單一 profile 篩選（現在只能依瀏覽器種類）。
- §1 單條記錄的可選顯示欄位（訪問次數、來源途徑、分類、provenance、metadata versions…）與其設定。
- §1 詳情面板的 typed count、來源途徑與 referrer、provenance（run id）、標題歷史版本與 diff。
- §1 從 History 直接匯出目前篩選結果（Settings → Storage 只能匯出整個歷史）。
- §2 Audit Ledger 的 run timeline 篩選、與上一筆的 summary delta、展開預覽某次 run 寫入的記錄、回滾 / 取消回滾**備份** run（現在只有匯入批次能復原）、archive 快照的手動觸發。Settings → Storage「從安全副本還原」可列出並還原整個 archive 快照。

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
  - Explorer 搜尋欄旁必須提供 hover / focus 可見的語法速查浮窗，讓使用者不必先讀文檔才知道這些 operators 存在；浮窗內容需同步 `en` / `zh-CN` / `zh-TW`。
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
- regex mode 在 canonical filter（profile / browser / domain / date range / visibility）之後做 post-filter：先由後端以 canonical filter 縮小結果集，再對縮小後的結果執行 regex 匹配。2026-10 起 regex 會掃完整個 archive，但每個請求只掃一段（約 200 ms），由 cursor 續掃，介面顯示目前掃到哪裡；見上方 §0。
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
