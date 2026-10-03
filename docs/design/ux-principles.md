# UX 設計原則

> 從 [vision-and-requirements.md](../vision-and-requirements.md) 抽出。
> 2026-10-02 起依 M18 redesign 改寫：視覺來自 `docs/design/prototype-2026-10/`，實作在 shadcn/ui 上。色票與 token 見 [design-tokens.md](./design-tokens.md)，字體見 [typography-and-font-fallback.md](./typography-and-font-fallback.md)，畫面與導航見 [screens-and-nav.md](./screens-and-nav.md)。
> 2026-05 到 2026-09 的「Paper + Archival」方向（米色紙感、Newsreader serif、3 px 圓角、紙噪、Density 切換）已退役。

---

## 1. 視覺設計方向

PathKeep 看起來是一個安靜的 macOS 原生工具：半透明的視窗浮在柔和的漸層上，內容放在圓角面板裡，資料本身是畫面的重心。

- **視窗**：`body` 是一層徑向漸層（淺色偏藍到暖橘，深色偏紫黑），app frame 是 62 % 不透明的玻璃層加 40 px 背景模糊，左邊是導航列，右邊是放當前畫面的圓角面板。macOS 上標題列與導航列頂部融為一體（overlay titlebar，可拖動）。
- **導航列**：76 px 寬，五個主畫面（Home / History / Insights / Ask / Backup）是圖示加文字的直排按鈕；下方是 ⌘K、鎖定、語言、深淺色切換與 Settings。
- **卡片**：白色 78 % 不透明（深色 5.5 %），1 px 細邊框，極淡陰影，12–16 px 圓角。一頁通常是一個大標題加一格格卡片。
- **單一品牌色**：橘色 `--brand`。用在進行中的狀態、圖表主序列、熱力圖、焦點框與選取；主要按鈕是中性黑白，不是橘色。藍 / 綠 / 紫 / 紅只做第二序列與狀態。
- **字體**：Geist（UI 與標題）與 Geist Mono（路徑、指令、版本）。中文走系統字體。數字用 tabular figures。
- **主題**：淺色、深色、跟隨系統，三者等價；沒有「預設紙感」之類的主題命名。
- **密度**：只有一種。頁面寬度上限 1040–1080 px，左右 40 px padding；Settings 內容欄 680 px。沒有 Density 切換、沒有材質開關。
- **動畫**：克制。畫面與 Settings 分頁進場用 220 ms 的 `animate-rise`（6 px 上浮加淡入），History 詳情面板 200 ms 滑入；hover / 顏色變化 100–200 ms。沒有彈簧、沒有裝飾性循環動畫。
- **圖表**：EvilCharts（Recharts）加我們自己的熱力圖，橘色色階一致。圖表回答「量什麼、哪段時間」，不當裝飾。
- 需要操作透明的地方（加密、刪除資料、排程、還原）照樣清楚嚴謹，但用對話框與一行一行的設定列表達，不做成帳本頁。

---

## 2. 操作透明性

- 每一個涉及系統/數據的操作，都要讓用戶看到：
  - 我們在做什麼
  - 為什麼做這件事
  - 具體執行了什麼命令/步驟
  - 已經做了哪些、還剩哪些
  - 如何撤銷
- 自動模式和手動模式都是 step-by-step 的 UI。
- 對於自動模式：逐步顯示進度，每步可展開查看詳情。
- 對於手動模式：每步有指南、理由、可複製的命令、完成後的確認。

### PME 共用 grammar

- **Preview**：先顯示邊界、影響範圍、generated artifact / visible query / profile scope，再出現真正的 execute CTA。
- **Manual**：所有需要碰檔案系統、排程或匯出物的流程，都要有可檢視的 artifact viewer 與 open / copy path 動作，不要求使用者自行去資料夾猜位置。
- **Execute**：執行按鈕文案必須直接說明會做什麼，例如 first backup、run backup、copy path、open path；不要把高風險操作藏在模糊 CTA 裡。
- **Execute paint-first**：凡是會觸發大量 SQLite / Rust / derived-state 工作的流程，前端必須先讓 busy 狀態、skeleton 或畫面外殼成功 repaint，再開始真正的重工作業（`src/lib/wait-for-next-paint.ts`）；不能讓動畫或轉場因 foreground work 而卡死。
- **Verify**：完成後要在原頁面留下可見的結果訊號，例如 toast 加「在 Finder 中顯示」、Recent runs 新增一列、onboarding 每個階段的打勾或「已略過」。
- **Rollback hint**：凡是會寫入 archive 的流程，都要讓使用者知道之後去哪裡檢查或回滾，而不是只回報「成功」。
- 高風險流程至少要能完成 keyboard-only walkthrough，current step / selected filter / status chip 要有可朗讀的 label，而不是只靠顏色或位置辨識。
- 高風險流程在 reduced motion 模式下要降低動畫和 loading shimmer，避免把「透明」做成另一種視覺負擔。
- 長時間流程若超過單次短暫 spinner 的合理範圍，必須持續回報 phase、current/total、percent；「正在處理」不是可接受的最終 honesty copy。

2026-10 的實際落點：

| 流程                   | 在哪裡                       | Preview                                                                  | Execute                                                                                 |
| ---------------------- | ---------------------------- | ------------------------------------------------------------------------ | --------------------------------------------------------------------------------------- |
| 首次設定               | Onboarding 第 7 步           | 前 6 步只收集選擇；第 5 步顯示將安裝的 launchd / 工作排程器 / systemd 檔 | 「開始第一次備份」後依序建立 archive、存鑰匙圈、裝排程、備份、設 AI，每階段可重試或略過 |
| 加密 / 改密碼 / 解密   | Settings → Security → 對話框 | `preview_rekey_archive`：會先存一份安全副本、寫新檔、成功才替換          | `rekey_archive`                                                                         |
| 釋放空間               | Settings → Storage → 對話框  | `preview_retention_prune` 列出可刪的類別與大小，自己勾選                 | `run_retention_prune`                                                                   |
| 搬到另一台電腦（匯入） | Settings → Storage → 對話框  | `preview_app_data_import` 檢查檔案                                       | 確認覆蓋後 `apply_app_data_import`，重新開機流程                                        |
| 從安全副本還原         | Settings → Storage → 對話框  | 列出 `list_recovery_snapshots`                                           | `run_full_archive_restore`；目前 archive 移到一旁，不刪                                 |
| 刪除所有資料           | Settings → Storage → 對話框  | `preview_wipe_all_data` 列出每個路徑與大小                               | 輸入 `DELETE` 才執行 `wipe_all_data`                                                    |
| Takeout 匯入           | Backup → 匯入卡              | `inspect_takeout`：筆數、時間範圍、會被略過的檔案                        | `import_takeout`；之後可在同一張卡「復原」                                              |
| 健康檢查與修復         | Settings → About             | `doctor_report`（唯讀）                                                  | 確認後 `repair_health`                                                                  |

### Trust warning grammar

- **Info**：能力存在但仍需閱讀說明，例如可選便利功能、手動安裝入口。
- **Needs attention**：排程與設定不符、舊版排程、部分權限缺失等需要人工確認的狀態（Backup → 自動備份卡的「需要處理」）。
- **Blocked / degraded**：Full Disk Access 未授予、鑰匙圈不可用、無法自動安裝排程之類會改變能力邊界的情境。
- **Success**：已驗證、可追蹤、可回滾的狀態，而不是單純「看起來沒錯」。
- warning 不能只是說明文字；要直接附上下一步，例如「授權」按鈕直開系統設定、「修復」按鈕、「再檢查一次」、或連到 Settings 對應分頁。

---

## 3. 狀態清晰

- 所有可交互元素的狀態必須清晰可辨：
  - 勾選/未勾選有明顯視覺差異和動畫反饋。
  - 操作進行中有明確的 loading / progress 狀態。
  - 錯誤有明確的錯誤信息和修復建議。
- 空狀態友好：沒有數據時告訴用戶該怎麼開始。
- 降級狀態友好：AI 未配置時明確提示，但不阻礙核心備份功能。

### 四種狀態在哪裡

| 狀態        | 做法                                                                                                                                                                                                                                                                      |
| ----------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| **Loading** | 見 §4。                                                                                                                                                                                                                                                                   |
| **Empty**   | shadcn `Empty`（History 的 `state-message.tsx`）或卡片內一句話。每個空狀態說下一步：History 無資料 →「前往 Backup」；篩選後無結果 →「清除篩選」；搜尋無結果 → 建議少打幾個字或換搜尋方式；Ask 未設定 → 連到 Settings → AI。                                               |
| **Error**   | 整個 app 起不來：`BootErrorScreen`（錯誤全文、重試、顯示 log）。單一畫面崩潰：路由的 `RouteError`（錯誤內容、重新載入），其他畫面不受影響。列表載入失敗：`StateMessage` 帶錯誤與重試。操作失敗：sonner toast，標題說哪件事失敗，description 放 `describeError()` 的內容。 |
| **Stale**   | Insights 與 Home 的統計來自背景重算。區段回傳 `meta.state === 'stale'` 時查詢每 5 秒重新拉（`pollWhileStale`），空的卡片顯示「備份後更新中…」而不是「這段時間沒有資料」；改日期範圍時保留上一份結果（`placeholderData`），不閃回 skeleton。                               |

已知缺口：Home 與 Insights 的卡片查詢失敗時沒有獨立的錯誤畫面，會顯示成空狀態（只有 Home 的來源卡會說「統計暫時拿不到」）。

### 國際化是 UX 契約

- 所有 user-visible copy 一開始就同時交付 `en` / `zh-CN` / `zh-TW`，包含 aria-label、placeholder、toast、空/錯/載入/停用狀態。
- 文案放在 `src/lib/i18n/messages/<feature>.ts`，每個檔案三種語言並排。`defineMessages` 以英文為形狀，中文少一個 key 或多一個 key 都會讓 `tsc` 失敗。
- 複數用 `_one` / `_other` 後綴，由 `Intl.PluralRules` 選；參數用 `{name}`，數字依語系格式化；找不到的 key 回退英文，dev 模式在 console 警告。
- `bun run check:i18n`（`scripts/i18n-progress.ts`）擋編譯器擋不住的：三語 `{placeholder}` 必須一致、不得留 TODO、中文不得夾未翻譯的英文術語（profile、archive、visit、worker、schema…）、繁中必須用台灣用語（不得出現「會話、視圖、訪問、刷新、智能」等）、英文不得用內部術語（shell state、app data、all profiles…）。
- 語言偏好：導航列與 Settings → General 都能改，兩者都寫進 `config.preferredLanguage`（排程與選單列圖示也讀它）。「跟隨系統」依 `navigator.languages` 判斷：`zh` 含 Hant / TW / HK / MO → zh-TW，其他 `zh` → zh-CN，否則 en。
- `html[lang]` 隨目前語言同步（`src/main.tsx` 首屏、`src/lib/i18n/provider.tsx` 之後）。
- 設計要預留中文與長錯誤訊息的換行；錯誤列與路徑允許任意斷行，不能把對話框撐寬。

---

## 4. Loading States & Skeleton Screens

所有畫面必須提供有意義的載入狀態，~100 ms 內要有視覺回饋，不允許空白或凍結的 spinner。

### 各畫面 Loading 規範

| 畫面 / 情境      | Loading 表現                                                                                                                                                                                                                             |
| ---------------- | ---------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| **啟動**         | `BootScreen`：品牌標誌加「正在打開你的 archive…」。                                                                                                                                                                                      |
| **切換畫面**     | 每個畫面是 lazy chunk，載入時顯示 `ScreenFallback`：標題、四格統計卡、一大塊卡片的 skeleton。                                                                                                                                            |
| **Home**         | 統計卡數字位置是 skeleton；趨勢圖、On this day、年度熱力圖、常回來的主題、來源各自 skeleton，各自到齊。                                                                                                                                  |
| **History**      | 第一頁載入時 12 列遞減透明的 skeleton；捲到底載入下一頁時列表尾端多 3 列 loading；改篩選時保留舊結果並淡化（`dimmed`），新結果到了再換。搜尋結果數先顯示「100+」，精確總數算完再補上。                                                   |
| **Insights**     | KPI、每日活動、熱門網站、節奏、常搜尋、常重開頁面各自 skeleton；切換 7d / 30d / 90d / 1y 時保留上一份結果直到新結果到達。                                                                                                                |
| **Ask**          | 送出後回答區立即出現「處理中…」，搜尋 archive 時「正在搜尋你的 archive…」，思考過程與工具呼叫可展開；停止或中斷會留下說明。                                                                                                              |
| **Backup**       | 備份進行時頁面頂端出現進度卡：第幾個來源、來源名稱、已處理 / 總筆數、百分比；沒有百分比時是脈動的短條。按鈕本身也顯示「備份中… N%」。                                                                                                    |
| **Settings**     | 需要查詢的列（磁碟用量、鑰匙圈、開機啟動、MCP 指令、預覽圖快取…）先顯示 skeleton 列。長操作（重新加密、匯出、模型下載、建索引、更新）在列內或對話框內顯示進度與說明（例如重新加密時提醒「大型 archive 要幾分鐘，請讓 PathKeep 開著」）。 |
| **Archive 升級** | `UpgradeScreen`：階段名稱加「N / M」與進度條。                                                                                                                                                                                           |

### 視覺規範

- Skeleton 用 shadcn `Skeleton`（`bg-accent` 加 `animate-pulse`），尺寸要接近最終內容，避免載入完成後 layout shift。
- `prefers-reduced-motion`：History 詳情面板的滑入在 reduced motion 下關閉；其他循環動畫（skeleton pulse、進度條脈動）目前沒有 reduced-motion 替代，屬已知缺口。
- 進度條要有數字（百分比或筆數）與可讀的狀態說明。shadcn `Progress` 在沒有 `value` 時畫的是空軌，不是不確定動畫；需要不確定進度的地方（升級畫面的不確定階段）目前只會看到空軌，屬已知缺口。
- 同一份背景狀態只能有一個資料來源：snapshot 由 session 開機時載入後放進 TanStack Query 快取，各畫面只讀不重拉；備份進度由 `BackupRunnerProvider` 統一持有，Home、Backup、⌘K 與 onboarding 共用。
- 大列表一律虛擬化（`@tanstack/react-virtual`，固定列高），列數再多 DOM 成本不變。
- AI 長操作（模型下載、建索引）要同時顯示目前階段文字與進度。
