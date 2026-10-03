# UI Review / Implementation Guardrails

> 補充 [screens-and-nav.md](screens-and-nav.md) 與 [ux-principles.md](ux-principles.md) 的長期審查規則。  
> 這份文檔回答的是「什麼 UI 變更一眼就該被擋下」，不是逐頁替代規格。
> 2026-10-02 依 M18 redesign 更新：舊的 Explorer / Audit / Jobs / Maintenance 條目換成現在的畫面；§8 的分頁紅線被新設計取代，理由寫在該節。

---

## 1. 目的

- 防止 UI 在重構、補 feature、換資料源時慢慢滑回「好看但不誠實」。
- 給 reviewer 一份可直接對照的紅線清單，避免每次都重新辯論同一批問題。
- 補上 prototype 沒有明畫、但使用者已多次明確要求的桌面真機與 History 工作流邊界。

---

## 2. 先判斷：這是 summary card，還是 workbench surface

- **summary card**：KPI、摘要、單一圖表、單一 insight、單一 callout、單一小表格。
- **workbench surface**：列表 + 篩選 + 詳情、PME 對話框、需要長時間停留與操作的面板。
- 預設把新 UI 當成 **summary card**。只有真的承載 review / compare / inspect workflow，才可升格成 **workbench surface**。

這個分類會直接決定能不能全寬、能不能長高，以及應不應該拆出獨立畫面。

---

## 3. 全寬卡片白名單

預設 **不允許** 任意把卡片做成 full-width。  
目前可以佔滿內容欄寬的只有：

1. **History 整個畫面**：工具列、虛擬列表、詳情面板是一個 workbench。
2. **Home → 你的一年**：年度日曆熱力圖需要 53 欄。
3. **Backup → 來源、進度卡、最近執行**：一列一個 profile / run，屬 review 表格。
4. **Settings 的設定列**：內容欄本身只有 680 px，列是全寬的。

除此之外（Home 的統計卡、30 天趨勢、On this day、常回來的主題、來源；Insights 的所有卡片；Backup 的自動備份與匯入卡）都在兩欄或自動欄寬的 grid 裡，窄於 900 px 才變單欄。

想把新卡片加進白名單，先在本節寫下理由：**它為什麼不是 summary card，而是必須橫向展開的 workbench surface。**

---

## 4. 卡片不能無限長高

- 卡片不可隨內容無限長高。現在的做法是限制筆數（On this day 4 筆、常回來的主題 4 筆、熱門網站 6 筆、常搜尋 10 筆、常重開 5 筆），或在卡片 / 面板內捲動（History 列表、詳情面板、⌘K 清單最高 420 px、對話框）。
- 要顯示「全部」時給明確入口（例如「全部 →」連到 Insights 或 History），不要把卡片撐長。
- 內捲動不是偷藏內容：必須保留可見的溢出暗示。
- raw 內容（排程檔、MCP 指令、錯誤全文）預設收合或放在有最大高度的 code block 內，路徑與錯誤允許任意斷行，不能把對話框撐寬。

審查時直接擋下：

- 為了塞更多內容，把卡片高度一路撐到整頁底部。
- 左右兩欄因其中一張過長，被強迫對齊成超高 row。
- 用 `overflow: hidden` 硬裁掉內容，卻不給任何展開或滾動入口。

---

## 5. 文案講人話，不准用假專業語氣掩蓋資訊不足

- 先回答「現在發生什麼」「這對我有什麼影響」「我下一步去哪裡」。
- 避免把內部實作詞直接扔給使用者，除非那是需要 review 的證據值。
- 不要寫成模糊的產品腔、AI 腔、簡報腔。
- disabled / empty / degraded state 不能只剩一句抽象宣告；要帶明確邊界與下一步。

### 直接擋下的文案味道

- 空泛：`Leverage your archive`、`Unlock deeper insights`、`Experience seamless intelligence`
- 假動作：`Processing...`、`Optimizing...`、`Working magic...`
- 假成功：只寫 `Ready` / `Healthy`，但沒說 ready 的是什麼
- 假錯誤：只寫 `Something went wrong`

### PathKeep 應該長這樣

- 不說「系統正在處理您的請求」，要說「正在備份第 2 / 3 個來源 · Chrome」或「正在重建語意索引」
- 不說「發現異常」，要說「Safari 被略過了，因為 PathKeep 沒有完整磁碟取用權限」
- 不說「內容不可用」，要說「備份後更新中…」或「這段時間沒有瀏覽紀錄」

所有新文案仍要遵守 i18n shipping contract：`en` / `zh-CN` / `zh-TW` 一起交付，並通過 `bun run check:i18n`。

---

## 6. 數據可視化必須真實、可解釋、可回到證據

PathKeep 的圖表不是裝飾。任何 chart / heatmap / score / badge 都必須回答三件事：

1. **它在量什麼**
2. **它的時間窗 / scope 是什麼**
3. **我怎麼回到原始 evidence**

### 必守規則

- 不可用視覺誇張替代真實含義。
- 不可把不同口徑的資料混成一張圖而不說明。
- 不可把估算值、抽樣值、AI 改寫結果偽裝成 deterministic fact。
- 卡片標題或副標要說時間窗（「最近 30 天」「每天的瀏覽數，所有瀏覽器」），Insights 的範圍由頁首切換器統一決定。
- 圖表或列表若可 drill down，必須回到 History（`?date=`、`?domain=`、`?q=`）。

### 明確紅線

- 「你的一年」熱力圖的每一格必須對應**真實日期**，點擊進 `/history?date=YYYY-MM-DD`。
- 熱力圖與圖表的顏色只用 `--heat-*` / `--chart-*`，兩種主題下都不能接近背景色。
- `On This Day` 只回看過去幾年的同一天；不得混入今年今天的資料。
- 統計若跟所選範圍口徑不同，要明講（目前已知：Insights「常搜尋」的次數是整個 query family 的歷史總數，不受範圍限制，見 STATUS 待辦）。
- 背景重算中的區段要說「更新中」，不能顯示成「沒有資料」。

如果 reviewer 看不出圖在說什麼、從哪來、能否驗證，這張圖就不算過。

---

## 7. Desktop 真機才是 truth gate

PathKeep 是 desktop app，最終 truth gate 是實際的 Tauri 視窗。

### 審查結論的優先序

1. **真實 Tauri 桌面行為**（`bun run desktop:dev` 或打包版）
2. 瀏覽器 + 真後端：`bun run dev:demo`（合成瀏覽器資料）、E2E（`bun run test:e2e`，dev IPC bridge）
3. prototype 截圖（`docs/design/prototype-2026-10/screenshots/`）

沒有 browser-preview 假後端：前端在沒有 Tauri 也沒有 dev bridge 的瀏覽器裡呼叫任何命令都會直接報錯（`src/lib/backend-client/shared.ts`）。

### 因此必須記住

- 瀏覽器裡看起來對，不代表 overlay titlebar、拖動區、選單列圖示、`pathkeep://` 事件、鑰匙圈、排程、Finder 顯示、更新、視窗尺寸與捲動在真機上對。dev bridge 下沒有選單列，也收不到桌面事件。
- 截圖若來自瀏覽器，必須誠實標示，不得冒充 desktop 已驗收。
- Windows / Linux 的系統匣、登入啟動、語言偵測程式碼從未編譯過（見 STATUS），在那兩個平台上點過之前不算完成。

---

## 8. History 紅線

History 是找回原始紀錄的主要工作台，不是內容 feed。

> 2026-10 前的 Explorer 紅線要求「上下都有『第 N / 共 M 頁』、跳頁、每頁筆數，不准改成 infinite scroll」。M18 照用戶指定的 prototype 改成虛擬化的連續時間線（cursor 分頁、捲到底自動載入），那條紅線不再適用。下面是現在要守的。

- **狀態在 URL 裡**：搜尋字串、方式、視圖、日期、瀏覽器、網站、選取的 visit 都要能從 URL 還原；外部連結（Home、Insights、Ask、⌘K）一律走 URL，不靠元件狀態傳遞。
- **結果數誠實**：先顯示「100+」，精確總數算完再換；regex 不合法時說「正規表示式無效」並且不送查詢。
- **列表永遠虛擬化**，固定列高；載下一頁不得改變目前的捲動位置；改篩選時保留舊結果淡化顯示，不閃回空白。
- **大量資料不得卡主線程**：14.4M 條時第一頁要維持毫秒級（見 [ipc-performance.md](../architecture/ipc-performance.md)）；favicon 在列出現之後才分批補上，不阻塞列表。
- **詳情面板**是 `<aside>`，在列表旁邊，不隨列表捲動；關閉時 `inert`，不能被 Tab 走進去；`?visit=` 連結在該筆載入後自動選取。
- **鍵盤**：↑↓ 移動選取、Enter 在瀏覽器打開、Esc 關閉面板；輸入框或選單打開時不攔截。
- semantic 搜尋沒開時不能假裝在跑：自動退回全文，並說明去 Settings → AI 開啟。

已知未守住（2026-10-02 試跑記下，見 STATUS）：同一頁面被重訪很多次時，搜尋結果會被同一網址洗版，需要後端提供「同一網址收成一行」。

---

## 9. Reviewer 快速清單

看一個新 UI 或改版時，至少問完這 9 題：

1. 這是 summary card，還是其實該獨立成 workbench surface？
2. 它如果做成 full-width，有沒有落在白名單內？
3. 它有沒有限制筆數或內捲動？
4. 文案是不是講人話，三種語言都交付了嗎？
5. 圖表是否說清楚量什麼、哪段時間？
6. 這個結論能不能回到 History？
7. 這個行為是不是只在瀏覽器成立，桌面真機還沒驗？
8. 如果是 History 相關，URL 狀態、虛擬化、詳情面板與鍵盤有沒有被破壞？
9. 進度 / 載入動畫誠實嗎？

任何一題答不清楚，就不要把 UI 當成完成。

### 進度條與不確定動畫（第 9 題展開）

- **誠實**：有真實進度（筆數、位元組、百分比）就顯示 determinate 進度與數字；沒有才用不確定動畫。不確定動畫不准長得像「停在 X%」的 determinate 進度。
- **transform / opacity only**：動畫只動 `transform` 或 `opacity`，不動 `width` / `left` 等觸發 layout 的屬性；determinate 的寬度變化用 transition。
- **reduced-motion**：循環動畫要在 `prefers-reduced-motion` 下停止，退成靜態的部分填充，不能是空軌（看起來壞了）或滿軌（看起來做完了）。
- 2026-10 現況，未全部達標：Backup 進度卡沒有百分比時用 1/3 寬的 `animate-pulse` 短條，reduced motion 下仍在脈動；shadcn `Progress` 在沒有 `value` 時畫空軌（Archive 升級畫面的不確定階段就是這樣）。舊的正典 `pk-indeterminate-bar` 已隨舊前端刪除，目前沒有共用的不確定進度元件。
