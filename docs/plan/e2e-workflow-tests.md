# E2E Workflow Test Plan

> 以真實使用者操作流程為基礎的端對端工作流測試計劃。  
> 每個 workflow 模擬一位完整的使用者旅程，驗證跨頁面、跨功能的整合行為。  
> 與 `m4-full-polish/e2e-workflow-rehearsal.md` 互補：該文檔聚焦 release polish rehearsal，本文檔覆蓋完整的功能驗收測試矩陣。

---

## 測試通則

- **環境假設**：測試環境已安裝至少一個 Chromium 系瀏覽器，且有可偵測的 profile。
- **語言**：預設 `en`，Workflow 5 包含語言切換驗證。
- **loading state 驗證**：所有頁面切換時必須觀察到 skeleton / loading state（參見 `docs/design/ux-principles.md` §4），不得出現空白畫面或系統預設轉圈。
- **scope 驗證**：涉及 profile scope 的步驟必須確認 topbar badge、Explorer inheritance、Insights callout 的一致性。
- **PME grammar**：凡涉及 Preview / Manual / Execute / Verify 的操作，每個階段都要驗證對應的 UI 狀態與產出物。
- **accessibility baseline**：每個 workflow 至少走一遍 keyboard-only 路徑確認可及性。

---

## Workflow 1: First-Time Setup（首次啟動設定）

### 前置條件

- PathKeep 首次安裝，archive 尚未初始化。
- 本機至少安裝一個 Chromium 系瀏覽器（含 profile）。

### 步驟

| #   | 動作                               | 預期結果                                                                                                                       | 驗證重點                                                                        |
| --- | ---------------------------------- | ------------------------------------------------------------------------------------------------------------------------------ | ------------------------------------------------------------------------------- |
| 1   | 啟動 PathKeep                      | 自動進入 Onboarding wizard；Dashboard / Explorer 等主路由**不可進入**                                                          | 視覺：Onboarding 頁面有 PathKeep branding、步驟指示器                           |
| 2   | Onboarding step 1：瀏覽器偵測      | 自動掃描並顯示已安裝的瀏覽器和 profiles 清單，每個 profile 有勾選控件                                                          | 資料：至少偵測到一個瀏覽器和 profile；Safari 若缺權限顯示 needs-access guidance |
| 3   | 選擇要備份的 profiles              | 勾選 ≥1 個 profile，已選 / 未選狀態有明確視覺差異和動畫反饋                                                                    | 視覺：checkbox 動畫、選中計數更新                                               |
| 4   | Onboarding step 2：設定存儲位置    | 顯示預設 archive 路徑，允許自定義路徑                                                                                          | 資料：路徑可寫入且顯示可用空間                                                  |
| 5   | Onboarding step 3：加密選擇        | 提供「啟用加密」或「明文模式」兩個選項；選擇加密時要求輸入主密碼並顯示密碼遺失 = 資料丟失的醒目警告                            | 視覺：加密警告的 severity 層級正確                                              |
| 6   | 完成 Onboarding → 進入 Dashboard   | Onboarding 完成後導向 Dashboard；sidebar、topbar、archive status footer 全部可見                                               | 視覺：Dashboard skeleton → 載入完成                                             |
| 7   | 驗證 Dashboard "no data" 狀態      | Dashboard 顯示 zero-state：stat cards 顯示 0 / empty、recent runs 為空、On This Day 顯示 no-data fallback；提供「開始備份」CTA | 視覺：zero-state 文案友好、不出現 raw i18n key                                  |
| 8   | 驗證可從 Dashboard 返回 Onboarding | Dashboard zero-state 或 topbar 有回到 setup 的入口                                                                             | 導航：路由跳轉正確                                                              |

### 退出條件

- Archive 已初始化但尚無備份資料。
- 所有 Onboarding 決策（storage / profile / security）已持久保存。

---

## Workflow 2: First Backup + Exploration（首次備份與探索）

### 前置條件

- Workflow 1 已完成（archive 已初始化、至少一個 profile 已選擇）。
- 選中的瀏覽器 profile 中有歷史紀錄。

### 步驟

| #   | 動作                                             | 預期結果                                                                                      | 驗證重點                                              |
| --- | ------------------------------------------------ | --------------------------------------------------------------------------------------------- | ----------------------------------------------------- |
| 1   | 從 Dashboard 點擊「Backup Now」                  | 觸發備份流程，顯示 progress overlay（含已處理筆數、進度百分比）                               | 視覺：progress overlay 不是空轉圈；包含可取消按鈕     |
| 2   | 備份進行中觀察                                   | 進度指示器持續更新；profile name、browser kind 在進度中可見                                   | 資料：進度數字遞增、不卡死                            |
| 3   | 備份完成                                         | Dashboard stats 即時更新：total visits > 0、latest run 時間更新、recent runs table 出現新 run | 資料：stat cards 數值一致                             |
| 4   | 導航到 Explorer                                  | Explorer 載入（先 skeleton → 再顯示真實資料）；顯示歷史紀錄列表與時間軸                       | 視覺：skeleton timeline → 真實 timeline；虛擬滾動正常 |
| 5   | 在 Explorer 搜尋一個已知 URL 關鍵字              | FTS5 搜尋觸發，結果列表更新，highlight 匹配文字                                               | 資料：搜尋結果包含預期記錄                            |
| 6   | 切換到 Regex mode                                | 搜尋框旁出現 regex toggle（明確的 affordance）；切換後搜尋框 placeholder 提示變更             | 視覺：regex mode 有顯式標識                           |
| 7   | 用 regex pattern 搜尋（如 `github\.com/.*rust`） | regex 搜尋在 canonical filter 後做 post-filter；結果列表更新                                  | 資料：結果符合 regex pattern                          |
| 8   | 輸入 invalid regex（如 `[unclosed`）             | 搜尋被阻止，顯示 invalid pattern 錯誤訊息；目前可見結果保留不清空                             | 視覺：error feedback 即時、不讓使用者以為 app 卡死    |
| 9   | 點擊一條記錄                                     | 詳情面板展開：完整 URL、title、visit times、provenance（run ID）、browser / profile           | 資料：詳情內容與列表一致                              |
| 10  | 匯出當前篩選結果                                 | Export 觸發，生成檔案（HTML / JSONL），顯示匯出完成訊息與檔案路徑                             | 資料：匯出檔案包含且僅包含目前可見的篩選結果          |

### 退出條件

- Archive 中有真實備份資料。
- Explorer 搜尋（keyword + regex）、詳情面板、匯出功能均已驗證。

---

## Workflow 3: Import + Rollback（導入與回滾）

### 前置條件

- Archive 已初始化且有至少一次備份。
- 準備好一個 Google Takeout 匯出檔案（含 `BrowserHistory.json`）。

### 步驟

| #   | 動作                                    | 預期結果                                                                                                               | 驗證重點                                          |
| --- | --------------------------------------- | ---------------------------------------------------------------------------------------------------------------------- | ------------------------------------------------- |
| 1   | 導航到 Import                           | Import 頁面載入，顯示可用的導入方法（Google Takeout / 瀏覽器直接導入）                                                 | 視覺：Import skeleton → 真實內容                  |
| 2   | 選擇 Google Takeout 方法                | 進入 Takeout import wizard                                                                                             | 導航：wizard step indicator 可見                  |
| 3   | 上傳 Takeout 檔案                       | 拖入或選取 zip / 資料夾；開始 dry-run 掃描，顯示 progress                                                              | 視覺：scan progress overlay 含已掃描檔案數        |
| 4   | 預覽導入統計                            | dry-run 完成後顯示：candidate items 數量、時間範圍、重複項數量、quarantine 項目（如有）、warnings、audit artifact 路徑 | 資料：preview 數字合理、quarantine 項目有原因說明 |
| 5   | 確認導入（Execute）                     | 正式寫入 archive，顯示 progress overlay；完成後顯示導入結果摘要（新增 / 更新 / 跳過 / 失敗）                           | 資料：結果數字與 preview 一致                     |
| 6   | 導航到 Audit Ledger                     | Audit 頁面顯示新的 import run，`run_type` = `import`；可展開查看寫入記錄的摘要                                         | 資料：run ID 與剛才導入的一致                     |
| 7   | 在 Audit Ledger 中回滾此 import run     | 點擊 rollback；顯示確認對話框，預覽將隱藏的記錄數量；確認後 run 狀態變為 `reverted`                                    | 視覺：確認步驟有影響預覽                          |
| 8   | 導航到 Explorer 驗證 rollback           | 被 rollback 的 import 記錄**不再出現**在 Explorer 搜尋和列表中                                                         | 資料：搜尋 Takeout 特有的 URL 應無結果            |
| 9   | 回到 Audit Ledger → Restore（取消回滾） | 點擊 restore；確認後 run 狀態恢復；Explorer 中記錄重新可見                                                             | 資料：restore 後 Explorer 能再次搜到這些記錄      |

### 退出條件

- Takeout 導入、回滾、恢復的完整生命週期已驗證。
- Audit 記錄完整。

---

## Workflow 4: AI Configuration + Intelligence（AI 配置與智慧功能）

### 前置條件

- Archive 中有足夠的歷史紀錄（建議 ≥ 500 筆）。
- 本機有可用的 AI provider（如 Ollama 已啟動並載入模型）。

### 步驟

| #   | 動作                                            | 預期結果                                                                                                                                             | 驗證重點                                                    |
| --- | ----------------------------------------------- | ---------------------------------------------------------------------------------------------------------------------------------------------------- | ----------------------------------------------------------- |
| 1   | 導航到 Settings                                 | Settings 頁面載入，AI Provider 區塊可見                                                                                                              | 視覺：Settings skeleton → 真實內容                          |
| 2   | 新增 AI Provider（例如 Ollama）                 | 選擇 Ollama preset，填入 Base URL（預設 `http://localhost:11434`）                                                                                   | 資料：preset 預填正確的 URL                                 |
| 3   | 分別設定 LLM 和 Embedding provider / model      | LLM 和 embedding 可以選擇不同的 provider / model                                                                                                     | 資料：兩個設定獨立保存                                      |
| 4   | 測試 Provider 連線                              | 點擊 Test；回傳 latency、capability report、成功 / 失敗 / error code / action hint；成功時顯示 success status                                        | 資料：test result 包含 latency 數字和 capability 清單       |
| 5   | 導航到 Explorer → 切換到 Semantic search mode   | Explorer 顯示 recall mode switcher（keyword / semantic / hybrid）；切換到 semantic 後顯示 provider / model / index state                             | 視覺：mode switcher 有明確狀態                              |
| 6   | 用自然語言搜尋（如 "local-first architecture"） | semantic 搜尋觸發；結果列表包含 historyId、URL / title、match reason、score band；結果可 deep-link 回 Explorer 原始記錄                              | 資料：語義搜尋結果相關性合理                                |
| 7   | 導航到 AI Assistant → 提問                      | 進入 Assistant 頁面；輸入問題（如 "我什麼時候開始研究 MCP 的？"）；顯示 queued → running 狀態                                                        | 視覺：pulsing status indicator + 階段說明文字               |
| 8   | 驗證 Assistant 回答                             | 回答完成後顯示回應文字 + evidence citations；每條 citation 至少包含 historyId、URL / title、visited time、score                                      | 資料：citations 可點擊 deep-link 回 Explorer                |
| 9   | 若 evidence 不足                                | Assistant 拒答並顯示 `insufficient-evidence` 狀態，而不是編造答案                                                                                    | 資料：honest fallback 正確觸發                              |
| 10  | 導航到 Insights → 觸發 Refresh                  | Insights 頁面載入（skeleton → 真實內容）；點擊 refresh 觸發 insight 計算                                                                             | 視覺：refresh 期間有 pulsing indicator + 說明文字           |
| 11  | 驗證 Insight cards                              | 生成的 insight cards 顯示：生成時間、資料視窗、evidence 數量；至少有 Browsing Rhythm / Site Analytics / Topic Timeline / storage health 等可用 cards | 資料：cards 有真實內容；zero-data card 顯示 honest fallback |

### 退出條件

- AI provider 配置、語義搜尋、Assistant 問答、Insights 生成全流程已驗證。
- Evidence / citation contract 正確。

---

## Workflow 5: Schedule + Security + Settings（排程、安全與設定）

### 前置條件

- Archive 已初始化且有備份資料。
- 測試環境有 OS 原生排程能力（macOS LaunchAgent / Windows Task Scheduler / Linux systemd）。

### 步驟

| #   | 動作                                        | 預期結果                                                                                                                         | 驗證重點                                     |
| --- | ------------------------------------------- | -------------------------------------------------------------------------------------------------------------------------------- | -------------------------------------------- |
| 1   | 導航到 Schedule                             | Schedule 頁面載入；顯示排程 Preview（plist / XML / service 文件內容預覽）                                                        | 視覺：Preview tab 內容可讀；平台特定格式正確 |
| 2   | 安裝排程（Execute）或檢視手動步驟（Manual） | 自動安裝：顯示安裝結果、audit artifact；手動模式：逐步指引、可複製命令、完成確認                                                 | 資料：安裝結果記入 audit log                 |
| 3   | 驗證排程狀態                                | 已安裝排程的狀態監控可見（last run、next expected run）；如有 mismatch 或 legacy install，顯示 needs-attention warning           | 視覺：status chip 正確反映當前狀態           |
| 4   | 導航到 Security                             | Security 頁面載入；顯示當前加密狀態（明文 / 加密）                                                                               | 視覺：明文模式有明確的「資料庫為明文」標示   |
| 5   | 啟用加密（或驗證已啟用狀態）                | 設定主密碼；密碼遺失警告醒目可見；keyring integration 狀態顯示（可用 / 不可用 / 需手動輸入）                                     | 視覺：密碼遺失警告 severity 正確             |
| 6   | 驗證 keyring 整合                           | 若系統 keyring 可用，便利解鎖已啟用；若不可用（Linux 無 keyring），每次啟動需手動輸入密碼，UI 有說明                             | 資料：keyring 狀態與平台能力一致             |
| 7   | 導航到 Settings → 切換語言                  | 語言切換（如 en → zh-TW）即時生效                                                                                                | 視覺：所有 UI 文案更新為目標語言             |
| 8   | 逐頁驗證語言切換                            | 依次訪問 Dashboard、Explorer、Insights、Import、Audit、Schedule、Security、Settings；所有頁面文案為目標語言，不出現 raw i18n key | 視覺：無遺漏的未翻譯文案                     |
| 9   | 切回原語言                                  | 語言恢復；確認偏好已持久保存                                                                                                     | 資料：重啟後語言偏好保留                     |

### 退出條件

- 排程安裝 / 檢視、加密啟用、keyring 整合、語言切換全流程已驗證。

---

## Workflow 6: Profile Switching + Cross-Page Consistency（Profile 切換與跨頁一致性）

### 前置條件

- Archive 中有來自**至少兩個不同 profile**的歷史紀錄。
- AI provider 已配置（用於驗證 Insights 和 Assistant scope）。

### 步驟

| #   | 動作                                             | 預期結果                                                                                                                                         | 驗證重點                                    |
| --- | ------------------------------------------------ | ------------------------------------------------------------------------------------------------------------------------------------------------ | ------------------------------------------- |
| 1   | 在 topbar 使用 profile switcher 選擇特定 profile | topbar 顯示當前 scope（profile name / badge）；shared scope 狀態更新                                                                             | 視覺：topbar badge 明確顯示 scoped profile  |
| 2   | 導航到 Explorer                                  | Explorer 自動繼承 shared scope；列表只顯示該 profile 的歷史紀錄；若沿用 shared scope，頁面有明確文案（如 "Showing: Profile X (inherited)"）      | 資料：列表中所有記錄屬於選中 profile        |
| 3   | 在 Explorer 手動設定不同 profileId filter        | page-specific filter 優先於 shared scope；列表切換到新 filter 的結果                                                                             | 資料：page filter override 正確             |
| 4   | 導航到 Insights                                  | Insights 頁面沿用 shared scope；cards / topic timeline / threads 顯示該 profile 的 scoped 資料；callout 或 badge 明確標示「profile-scoped view」 | 視覺：scoped view callout 可見              |
| 5   | 驗證 Insights 中 archive-wide vs scoped 區分     | Dashboard KPIs（若 Insights 頁有 KPI 摘要）仍為 archive-wide；insight cards 為 scoped；差異有視覺區分                                            | 視覺：scoped 與 archive-wide 區塊有明確邊界 |
| 6   | 導航到 AI Assistant                              | Assistant 尊重 shared profile scope；提問時 retrieval 限制在該 profile 的記錄                                                                    | 資料：回答的 citations 都來自選中 profile   |
| 7   | 導航到 Dashboard                                 | Dashboard 的 aggregate KPIs 仍為 archive-wide；有 callout 說明哪些區塊是 scoped、哪些是 archive-wide                                             | 視覺：callout 可見且文案清晰                |
| 8   | 切回 All Profiles（清除 scope）                  | topbar 回到 all-profiles 狀態；Explorer、Insights、Assistant 恢復顯示所有 profile 資料                                                           | 資料：所有頁面資料回到 unscoped 狀態        |
| 9   | 透過 deep-link 帶入 profileId                    | 訪問 `/explorer?profileId=xxx` 或 `/assistant?question=...&profileId=xxx`；頁面以 deep-link 的 profileId 為準，page-level scope 優先             | 資料：deep-link scope 正確覆蓋 shared scope |

### 退出條件

- Profile scope 在 Explorer、Insights、Assistant、Dashboard 四個頁面的行為一致性已驗證。
- shared scope vs page-level filter vs deep-link 三者的優先級正確。

---

## 補充驗證矩陣

### Loading State Coverage

每個 workflow 的**每一次頁面切換**都必須觀察並記錄 loading state 表現：

| 頁面         | 期望 loading 表現                                                 | 對應 workflow          |
| ------------ | ----------------------------------------------------------------- | ---------------------- |
| Dashboard    | Skeleton stat cards + recent runs table + On This Day placeholder | WF1 #6, WF2 #3, WF6 #7 |
| Explorer     | Skeleton timeline rail + skeleton list items (5–8 行)             | WF2 #4, WF3 #8, WF6 #2 |
| Insights     | Skeleton KPI cards + insight panel placeholders                   | WF4 #10, WF6 #4        |
| Import       | Progress overlay (已處理筆數 + 預估剩餘 + 可取消)                 | WF3 #3, WF3 #5         |
| AI 操作      | Pulsing indicator + 階段說明文字                                  | WF4 #7, WF4 #10        |
| Settings     | Skeleton sections → 真實內容                                      | WF4 #1, WF5 #7         |
| Audit Ledger | Skeleton run list → 真實 runs                                     | WF3 #6                 |
| Schedule     | Skeleton preview → 真實 preview content                           | WF5 #1                 |
| Security     | Skeleton status → 真實加密狀態                                    | WF5 #4                 |

### Reduced Motion Validation

在 `prefers-reduced-motion: reduce` 環境下重跑 Workflow 1 和 Workflow 2：

- Skeleton pulse 應為靜態或極微妙的透明度變化
- Progress overlay 動畫降級
- Onboarding step 切換動畫簡化
- 不影響功能正確性

### Keyboard-Only Walkthrough

每個 workflow 至少完成一遍 keyboard-only 操作：

- Tab 順序合理
- Focus ring 可見
- 所有互動元素（按鈕、checkbox、toggle、dropdown）可透過 Enter / Space 操作
- Modal / dialog 可透過 Esc 關閉
- Screen reader label 與可見文案一致

---

## 與現有測試的關係

| 層級                | 對應檔案 / 工具                                               | 備註                                                            |
| ------------------- | ------------------------------------------------------------- | --------------------------------------------------------------- |
| Browser-preview e2e | `tests/e2e/shell.spec.ts`                                     | 路由與核心交互覆蓋；參見 `e2e-workflow-rehearsal.md`            |
| Unit / product-flow | `src/app/index.test.tsx`、`src/pages/trust-flows.test.tsx` 等 | State coverage 補充                                             |
| 本文檔              | `docs/plan/e2e-workflow-tests.md`                             | 完整使用者旅程驗收；桌面真機 + browser-preview 均適用           |
| Platform runbook    | `m4-full-polish/release-readiness-runbook.md`                 | 真機 scheduler / keyring / biometric / installer / signing 驗證 |

---

## 缺陷紀錄模板

測試執行時，對每個 fail / observation 使用以下格式：

```
### [WF#-步驟#] 簡述
- **Workflow**: Workflow N — 名稱
- **步驟**: #X — 動作描述
- **環境**: macOS / Windows / Linux、語言、加密 / 明文
- **預期**: ...
- **實際**: ...
- **嚴重度**: Critical / Major / Minor / Cosmetic
- **截圖 / 錄影**: （附件路徑）
- **相關設計文檔**: （引用對應 feature / design doc 段落）
```
