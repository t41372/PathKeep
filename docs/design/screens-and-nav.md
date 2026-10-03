# 畫面與導航結構

> 從 [vision-and-requirements.md](../vision-and-requirements.md) 抽出。
> **2026-10-02 起描述 M18 redesign 後的前端**：視覺與畫面來自 `docs/design/prototype-2026-10/`（見該目錄 README，`python3 -m http.server` 開），實作在 `src/app/` 與 `src/features/`。prototype 是參考，不是規格；prototype 沒畫到、但產品需要的狀態，以本頁與 [ux-principles.md](ux-principles.md) 為準。
> 舊版（Dashboard / Explorer / Intelligence / Assistant / Import / Audit / Jobs / Integrations / Maintenance / Schedule / Security 獨立頁、sidebar 三分區、topbar、profile scope switcher）已在 2026-10 刪除。要看舊規格請看本檔在 `main` 上的版本。
> token 見 [design-tokens.md](design-tokens.md)；長期審查紅線見 [ui-review-guardrails.md](ui-review-guardrails.md)。

---

## 整體結構

```
SessionGate（src/app/index.tsx）
├─ booting        → BootScreen
├─ error          → BootErrorScreen（錯誤全文、重試、顯示 log）
├─ app-locked     → LockScreen kind="app"（App Lock passcode）
├─ archive-locked → LockScreen kind="archive"（加密 archive 的密碼）
├─ recovery       → RecoveryScreen（archive 打不開，從已驗證快照還原）
├─ upgrade        → UpgradeScreen（一次性 archive 升級，含進度）
├─ onboarding     → Onboarding 路由（七步設定）
└─ ready          → AppFrame：導航列 + 當前畫面 + ⌘K
```

- 狀態由 `src/app/session.tsx` 決定，順序是：App Lock 鎖著 → `app-locked`；加密 archive 沒有金鑰 → 先試鑰匙圈（只試一次），失敗就 `archive-locked`；明文 archive 打不開 → `recovery`；有待跑的 migration → `upgrade`；`config.initialized` 為 false → `onboarding`；其餘 → `ready`。
- 只有 `ready` 會掛主路由；鎖定、升級、onboarding 都是全螢幕，主畫面完全不渲染。鎖定時清空整個查詢快取，archive 資料不留在記憶體。
- 路由是 hash router（`src/app/router.tsx`）。每個畫面是 lazy chunk，載入時顯示 skeleton；每個畫面有自己的 `RouteError`，一個畫面崩潰不會拖垮整個 app。

## 導航

- **左側導航列**（76 px，`src/app/shell/nav-rail.tsx`）：Home、History、Insights、Ask、Backup 五個主畫面（圖示加文字，當前項目是白色卡片底）；下方依序是 ⌘K、鎖定（⌘L）、語言（A / EN / 简 / 繁）、深淺色切換、Settings。macOS overlay titlebar 下，導航列頂端 46 px 是可拖動區；其他平台顯示品牌標誌。
- 語言從導航列或 Settings 改，兩者都寫進 config（`useChooseLanguage`），重啟後保留，排程與選單列圖示也用它。
- 沒有 topbar、沒有麵包屑、沒有上一頁 / 下一頁按鈕、沒有通知中心、沒有共享 profile scope。
- 全域快捷鍵（`app-frame.tsx`）：⌘K / Ctrl+K 開關命令面板；⌘L / Ctrl+L 鎖定。沒設 passcode 時鎖定不會發生，改為提示「先設定 passcode」並打開 Settings → Security。

### URL 契約

| 路徑                   | 參數                                                                                                                                                                                                                                                                          |
| ---------------------- | ----------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `#/`                   | —                                                                                                                                                                                                                                                                             |
| `#/history`            | `q`（搜尋字串）、`mode=full\|regex\|semantic`（預設 full）、`view=timeline\|sites\|starred`（預設 timeline）、`date=today\|yesterday\|7d\|30d\|YYYY-MM-DD\|YYYY-MM-DD..YYYY-MM-DD`、`browser=<browser kind>`（如 `chrome`）、`domain`、`visit=<visit id>`。預設值不寫進 URL。 |
| `#/insights`           | `range=d7\|d30\|d90\|y1`（預設 d30）                                                                                                                                                                                                                                          |
| `#/ask`                | `new=1` 開新對話                                                                                                                                                                                                                                                              |
| `#/backup`             | —                                                                                                                                                                                                                                                                             |
| `#/settings/:section?` | `general`（預設）、`security`、`ai`、`storage`、`about`                                                                                                                                                                                                                       |

未知路徑導回 `#/`。History 的狀態全部在 URL 裡，所以重新整理、從別處連過來都會還原同一個畫面。

### 畫面之間的連結

- Home：統計卡分別連到 History、Insights、Settings → Storage、Backup；年度熱力圖點某天 → `/history?date=YYYY-MM-DD`；On this day 的日期 → 同上；「常回來的主題」→ `/history?q=…`，「全部」→ Insights；來源卡「管理」→ Backup；備份狀態膠囊 → Backup。
- Insights：熱門網站 → `/history?domain=…`；常搜尋 → `/history?q=…`；常重開的頁面 → `/history?q=<標題>`。
- Ask：回答引用的頁面 → `/history?q=<標題>&visit=<id>`；未設定 AI → Settings → AI。
- ⌘K：拜訪結果 → `/history?q=…&visit=<id>`；「搜尋歷史『…』」→ `/history?q=…`。
- History：無資料時「前往 Backup」；semantic 未開時提示去 Settings → AI。

---

## 畫面清單

| 畫面           | 檔案                            | 內容                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                       |
| -------------- | ------------------------------- | -------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| **Home**       | `src/features/home/`            | 依時段問候與今天日期；右上角備份狀態膠囊（上次備份多久前、下次排程時間或「自動備份已關」、進行中時顯示第幾個來源）與「立即備份」。四張統計卡：已保存瀏覽數（本週新增）、時間跨度（自何時起）、archive 大小（是否加密）、來源（幾個瀏覽器、幾個暫停）。最近 30 天趨勢（面積圖，與前 30 天比較）、On this day（往年同一天）、「你的一年」日曆熱力圖（可翻年份）、常回來的主題（90 天內的 reopened investigations）、來源（每個瀏覽器的瀏覽數，暫停的標灰）。                                                                                                                                                                                                                                                                                                                                                                 |
| **History**    | `src/features/history/`         | 頂部工具列：搜尋框（標題、網址或內容；`/regex/` 包起來就是 regex）、搜尋方式（全文 / Regex / 語意）、視圖（時間線 / 網站 / 已加星；搜尋時三個都不選中，改顯示搜尋結果）、日期篩選（今天、昨天、7 天、30 天、自訂範圍）、瀏覽器篩選、網站篩選 chip、清除篩選。**時間線**：依天分組，同一天內間隔 30 分鐘以上切成 session，每個 session 有標頭（時間範圍、頁數、主要網站）；捲到底自動載下一頁。**搜尋結果**：上方一行「N 筆結果 · 方式」。**網站**：所選日期與瀏覽器內最常去的 200 個網站，點一個就切回時間線並篩選該網站。**已加星**：加星的頁面與整個網站，有筆記的顯示預覽。選一列在右側打開 320 px 詳情面板：標題、網址、在瀏覽器打開 / 加星 / 複製連結；總瀏覽次數、第一次與最近一次、來源瀏覽器；最近 12 週每週瀏覽數；同一 session 的其他頁面；筆記（自動存）。鍵盤：↑↓ 移動選取、Enter 在瀏覽器打開、Esc 關閉面板。 |
| **Insights**   | `src/features/insights/`        | 全部從本機 archive 計算，不用 AI。右上角範圍 7 天 / 30 天 / 90 天 / 1 年。KPI：頁面瀏覽、不同網站數、搜尋次數、活躍時間（各與前一段同長期間比較）；每日活動（頁面與搜尋的長條圖，1 年時按週）；熱門網站（前 6）；節奏（星期 × 小時熱力圖）；常搜尋（前 10，同一句不同搜尋引擎合併）；常重開的頁面（前 5，在幾天裡重開幾次）。背景重算中會顯示「備份後更新中…」並每 5 秒重拉。                                                                                                                                                                                                                                                                                                                                                                                                                                              |
| **Ask**        | `src/features/ask/`             | 沒有可用的 AI 服務時：說明這是可選功能、PathKeep 不需要它也能完整使用，按鈕連到 Settings → AI。有服務時：左側對話清單（依日分組，可改名、刪除），右側對話。空對話顯示四個範例問題，並說明用的是本機模型還是線上服務（線上服務時明講會把需要的片段送出去）。回答串流顯示，可展開思考過程與搜尋 archive 的次數，下方列出引用的頁面（連回 History）、複製、重試、token 用量；可隨時停止。輸入框 Enter 送出、Shift+Enter 換行。                                                                                                                                                                                                                                                                                                                                                                                                |
| **Backup**     | `src/features/backup/`          | 標題說明 PathKeep 不讀即時資料庫、先複製再追加。備份進行時頂端有進度卡。**來源**：每個瀏覽器 profile 一列（瀏覽數、最近一次瀏覽、狀態：正常 / 需要授權 / 已暫停 / 未找到）與開關；Safari 缺 Full Disk Access 時顯示說明、「授權」（直開系統設定）與「再檢查一次」；重新掃描。**自動備份**：每小時 / 每 6 小時 / 每天 / 關，下次執行時間、排程器狀態（已安裝 / 未安裝 / 需修復 / 需授權 / 發現舊版 / 手動設定），需要處理時有「修復」；無法自動安裝的平台列出手動步驟。**匯入**：拖入或選擇 Google Takeout `.zip`（或貼路徑）→ 檢查（筆數、時間範圍、會略過的檔案、已有的會跳過）→ 匯入；下方列出近期匯入，可復原（隱藏該批）與還原。**最近執行**：備份、匯入、重新加密、健康檢查、還原、清理的紀錄，點一列在側邊 sheet 看詳情（開始時間、耗時、誰觸發、新增瀏覽與頁面、來源、錯誤、警告）。                                |
| **Settings**   | `src/features/settings/`        | 左側分頁，右側一列一列的設定（標題、說明、控制項）。見下表。                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                               |
| **Onboarding** | `src/features/onboarding/`      | 見下方「Onboarding」。                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                     |
| **鎖定畫面**   | `src/app/shell/lock-screen.tsx` | 全螢幕模糊背景、品牌標誌、標題與一句說明、密碼框與解鎖按鈕；輸錯會清空輸入框並提示。`kind="archive"` 多一個「記在系統鑰匙圈」勾選（預設勾）。沒有 Touch ID、沒有 config 路徑、沒有上次解鎖時間、沒有 recovery 提示，見下方 App Lock 一節。                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                 |

### Settings

| 分頁         | 列                                                                                                                                                                                                                                                                                                                                            |
| ------------ | --------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| **General**  | 語言；外觀（淺色 / 深色 / 跟隨系統）；開機時啟動；選單列圖示（Windows / Linux 叫系統匣圖示；讀不到 OS 狀態時顯示錯誤列）；「線上」分組：連結預覽（下載每頁提供的預覽圖）、網頁摘要（抓取造訪過頁面的摘要供搜尋），並說明只有這兩項會連到網站。                                                                                                |
| **Security** | 「Archive」分組：加密 archive（開關，開 / 關都走重新加密對話框）、加密時才出現的「把密碼存在鑰匙圈」與「更改密碼」。「App Lock」分組（說明它只是擋在視窗前的 passcode，不加密任何東西）：App Lock 開關（沒有 passcode 時先要求設定）、Passcode（更改 / 移除）、啟用後才出現的自動鎖定（1 / 5 / 15 / 30 / 60 分鐘）與「立即鎖定」（標示 ⌘L）。 |
| **AI**       | 開頭一句「這些都是可選的」。本機語意搜尋（開關、一次性模型下載、建索引進度）；AI 服務（選擇、新增、測試、移除）；MCP 伺服器（開關，開啟後顯示可複製的指令）；重建搜尋索引（先估計時間）。                                                                                                                                                     |
| **Storage**  | Archive 位置（在 Finder / 資料夾中顯示）；磁碟用量（archive / 搜尋索引 / 快照 / 其他）；釋放空間；連結預覽圖快取（數量、大小、清除）；匯出歷史（HTML / Markdown / 純文字 / JSON Lines，整個歷史一個檔）。「搬移與還原」分組：搬到另一台電腦（打包成一個 `.pathkeep` 檔 / 打開一個）、從安全副本還原、刪除所有資料。                           |
| **About**    | 版本（產品名、版本號、commit，有未提交修改時標示）；更新（檢查、下載安裝、重啟）；記錄檔（顯示資料夾）；檢查存檔（唯讀健康檢查，失敗時可修復）；重建洞察。                                                                                                                                                                                    |

### Onboarding

`archive` 尚未初始化時出現，左邊是步驟清單，右邊是內容，上方有細進度條；視窗內可切換語言與深淺色。

1. **歡迎**：各瀏覽器預設保留多久的歷史（Chrome / Arc 約 90 天…）對比 PathKeep 的永久保存，加上三個承諾（只在本機、不碰原始檔、可搜尋）。
2. **瀏覽器**：勾選要備份的 profile（預設勾所有可讀的）；Safari 缺 Full Disk Access 時說明並提供授權與重新掃描。按繼續時把選擇存進 config，中途關掉 app 下次會帶著它繼續。
3. **存放位置**：archive 會建在 app 資料夾（不能改，提供「在 Finder 中顯示」）與依檔案大小估算的體積。
4. **加密**：加密（預設）或不加密；密碼與強度；是否存進系統鑰匙圈（鑰匙圈可用時預設開，排程備份需要它）。
5. **排程**：每小時（預設）/ 每 6 小時 / 每天 / 手動，並顯示將安裝的排程檔內容（可展開）；無法自動安裝時說明。可「略過」（= 手動）。
6. **AI**：先不要（預設）/ 本機語意搜尋 / 連接 AI 服務（共用 Settings → AI 的表單）。可略過。
7. **完成**：列出所有選擇，按「開始第一次備份」後依序執行：建立 archive → 存密碼到鑰匙圈 → 安裝排程 → 第一次備份（即時進度）→ 設定 AI。每階段顯示狀態，失敗時就地顯示錯誤並可重試；除了建立 archive 以外都能略過（略過的階段顯示橫線與刪除線，不打勾）。完成後進入主畫面。

前六步不寫入任何東西（瀏覽器選擇除外），可自由返回修改。沒有「離開設定」按鈕：archive 建立之前沒有可去的主畫面，關掉 app 下次會回到 onboarding。

### ⌘K 命令面板（`src/app/shell/command-palette.tsx`）

- **前往**：Home、History、Insights、Ask、Backup、Settings。
- **動作**：立即備份、切換深淺色、鎖定 PathKeep（⌘L）、新對話。
- 頁面與動作依輸入文字過濾。輸入兩個字以上時多一組 **拜訪**：用全文搜尋取 30 筆、同一網址只留一筆、顯示前 6 筆（favicon、標題、時間），最後一項是「搜尋歷史『…』」打開 History。
- 底部提示 ↑↓ 移動、↵ 打開。

### 選單列圖示

Settings → General 開啟後出現在 macOS 選單列（Windows / Linux 系統匣）。選單：備份狀態（不可點）、立即備份、搜尋歷史…、打開 PathKeep、結束 PathKeep。

- 「搜尋歷史…」叫出視窗並送 `pathkeep://open-command-palette`，視窗收到就打開 ⌘K。
- 任何 app 內備份結束都送 `pathkeep://backup-finished`；來源是選單列時，視窗顯示與自己發起的備份相同的 toast 並刷新資料（`src/app/backup-runner.tsx`）。
- 開著圖示時關閉視窗只會隱藏；登入啟動不會把視窗推到前面。完整行為見 [archive.md](../features/archive.md) §2「開機啟動與選單列圖示」與 [desktop-command-surface.md](../architecture/desktop-command-surface.md)。

---

## 共同規則

- 長流程（重新加密、匯入、還原、刪除資料、搬家、釋放空間、健康修復）都在對話框裡走 Preview → 確認 → 執行，結果就地顯示；完成後如需重新開機（刪除資料、還原、匯入 `.pathkeep` 檔）會呼叫 `session.restart()`，從頭跑一次開機流程。
- 寫入 archive 的動作完成後（備份、匯入、還原）呼叫 `refreshAfterArchiveChange()`：重拉 snapshot，並讓所有 `archive` 開頭的查詢失效，Home / History / Insights 一起更新。
- 同時只會有一個手動備份：`BackupRunnerProvider` 持有唯一的執行狀態，Home、Backup、⌘K、onboarding 共用。
- On this day 只回看往年同一天，不含今年今天；日期都用使用者目前時區的本地日曆日。
- Profile / 瀏覽器在 UI 上一律顯示瀏覽器名稱與 profile 名稱，不露出 `chrome:Default` 這類內部 id。

---

## App Lock 畫面與導航規則

> **2026-10 實作與下列已接受要求不一致。** 新的 Settings → Security 沒有 biometric 開關、recovery hint、config 路徑與上次解鎖時間；新的鎖定畫面沒有 Touch ID、沒有 config 路徑與 recovery 動作；鎖定也不是 `/lock` 路由，而是 session 狀態 `app-locked`（效果相同：主畫面完全不渲染）。要補上還是改需求，選項與建議見 [app-lock-panel-tradeoff.md](app-lock-panel-tradeoff.md)，**待用戶決定**。以下要求在決定之前不變。

- App Lock 是獨立的 utility route（`/lock`），不常駐 sidebar。
- 啟動時若 App Lock 已啟用，PathKeep 先顯示 lock screen，通過驗證後才載入主 shell。
- 閒置逾時（idle timeout）觸發時，自動導向 `/lock`，主 shell chrome 完全不渲染。
- Topbar 在 App Lock 已啟用時提供 `Lock now` 動作；手動鎖定也走同一個 `/lock` route。
- Lock screen 顯示 PathKeep branding、鎖定原因、config path、上次解鎖時間、passcode input、recovery hint callout，以及打開 config path 的 recovery 動作。
- 若平台是 macOS 且 Touch ID 目前不可用，lock screen 仍顯示 Touch ID CTA / note，但按鈕必須 disabled，並明講會回退到 passcode。
- 若使用者已在 Settings 關閉 biometric unlock，lock screen 不得再顯示 Touch ID / biometric CTA；capability 可用不代表可以繞過設定直接解鎖。
- 若平台不是 macOS，lock screen 繼續顯示 generic biometric honesty copy，不假裝有 native parity。
- 鎖定狀態下不僅 UI 隱藏，後端 query 與 MCP history query 也必須被阻擋 — 避免透過 dev tools / MCP 繞過。
- Settings 的 App Lock panel：enable / disable toggle、idle timeout duration、biometric toggle、passcode set / update / clear、recovery hint、`Lock now`、config path、last unlocked timestamp。
- manual update check / install（release availability、notes、install progress、restart CTA）屬於 Maintenance。
- App Lock 與 archive encryption 是**獨立的兩層保護**：App Lock 保護 UI session，encryption 保護資料庫檔案。兩者可獨立啟用。
- 設計規格 → `docs/features/archive.md` §8
