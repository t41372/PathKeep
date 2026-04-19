# UI Review / Implementation Guardrails

> 補充 [screens-and-nav.md](screens-and-nav.md) 與 [ux-principles.md](ux-principles.md) 的長期審查規則。  
> 這份文檔回答的是「什麼 UI 變更一眼就該被擋下」，不是逐頁替代規格。

---

## 1. 目的

- 防止 UI 在重構、補 feature、換資料源時慢慢滑回「好看但不誠實」。
- 給 reviewer 一份可直接對照的紅線清單，避免每次都重新辯論同一批問題。
- 補上 prototype 沒有明畫、但使用者已多次明確要求的桌面真機與 Explorer 工作流邊界。

---

## 2. 先判斷：這是 summary card，還是 workbench surface

- **summary card**：KPI、摘要、單一圖表、單一 insight、單一 callout、單一小表格。
- **workbench surface**：列表 + 篩選 + 詳情、PME artifact review、大型 run review、需要長時間停留與操作的面板。
- 預設把新 UI 當成 **summary card**。只有真的承載 review / compare / inspect workflow，才可升格成 **workbench surface**。

這個分類會直接決定能不能全寬、能不能長高，以及應不應該拆出獨立頁面。

---

## 3. 全寬卡片白名單

預設 **不允許** 任意把卡片做成 full-width。  
只有下面這些資訊結構可以佔滿主內容欄寬：

1. **Dashboard / Recent Runs**
   - 理由：本質是 run review table，不是裝飾性摘要卡。
2. **Explorer 主工作區**
   - 包含 timeline / summary、結果列表、grouped result surface、sticky detail rail 這整套 canonical recall workbench。
3. **Audit run detail**
   - 理由：要承接 `Summary / Artifacts / Warnings` 與 open / copy path，屬 review surface。
4. **Jobs 的主要 queue / recovery review surface**
   - 例如 hero card、queue family overview、需要看 phase / heartbeat / recovery note 的大面板。
5. **Settings 裡的 external-output / trusted local host / remote backup verify 類 review surface**
   - 前提：它承載的是 artifact review / payload review / verify 結果，不是單純宣傳或摘要。

除此之外：

- insight cards
- dashboard stats
- On This Day
- spotlight / research signals / evidence-health 子卡
- 一般 settings / security / onboarding / import 的說明卡
- 任意新長出的 chart card

都應維持在正常卡片寬度，不得因為「版面看起來空」就直接拉滿。

對 `/intelligence` 目前再額外固定一條更窄的規則：

- 只有 **執行摘要 / 時段概覽 / 瀏覽節奏** 可以 full-width
- 其他 intelligence 卡片一律留在 half-width row 或 secondary grid

如果某張新卡想進白名單，先在相關 design / feature doc 補上理由：  
**它為什麼不是 summary card，而是必須橫向展開的 workbench surface。**

---

## 4. 所有卡片都有限高；超出就內滾動

這條規則適用於一般卡片，也適用於全寬白名單。

- 卡片不可隨內容無限長高，避免單張卡把整頁節奏拖爛。
- 卡片需要明確的 **max-height**；超出內容走 **internal scroll**。
- header / title / key actions 應盡量固定在卡片內可見區，不要一滾就消失。
- 滾動應發生在「內容區」，不是把整個頁面拉成超長瀑布。
- 內滾動不是偷藏內容：必須保留可見的溢出暗示，讓人知道下面還有東西。

審查時直接擋下這些做法：

- 為了塞更多內容，把卡片高度一路撐到整頁底部。
- 左右兩列卡片因其中一張過長，被強迫對齊成超高 row。
- detail / evidence / list 內容跟著頁面一起長到失控，沒有自己的 scroll container。
- 用 `overflow: hidden` 硬裁掉內容，卻不給任何展開或滾動入口。

唯一例外是本來就不是 card 的整頁流程容器；但一旦它被設計成 panel / card / rail，就回到這條規則。

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

- 不說「系統正在處理您的請求」，要說「正在掃描 Chrome profile」或「正在重建 semantic index」
- 不說「發現異常」，要說「這次 backup 缺少 Safari history DB，請先授予 Full Disk Access」
- 不說「內容不可用」，要說「還沒有可顯示的 periodic summary；你仍可去 Explorer 查看原始記錄」

所有新文案仍要遵守 i18n shipping contract：`en` / `zh-CN` / `zh-TW` 一起想，不得拿英文占位頂著。

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
- 不可只顯示結論，不顯示 window / scope / evidence count / freshness。
- 圖表若可 drill down，必須能回到 Explorer、Audit 或對應 evidence drawer。

### 明確紅線

- `Browsing Rhythm` 主圖必須對應**真實日期**；目前 accepted contract 是 GitHub 式日曆熱力圖，點某一天後再顯示當天 digest / top sites / 小時分布。
- `On This Day` 只回看過去幾年的同一天；不得混入當前年份今天的資料。
- archive-wide 與 profile-scoped 不能混講；如果指標不受 shared scope 影響，要明講。
- queue / runtime 類資訊不能冒充 insight 主內容；Intelligence 的 runtime digest 只能是 digest，不是第二個 Jobs。

如果 reviewer 看不出圖在說什麼、從哪來、能否驗證，這張圖就不算過。

---

## 7. Desktop 真機才是 truth gate

browser preview、browser-desktop-bridge、單元測試、e2e 都有價值；  
但 **PathKeep 是 desktop app**，最終 truth gate 仍是實際桌面 runtime。

### 審查結論的優先序

1. **真實 Tauri 桌面行為**
2. browser-desktop-bridge
3. browser preview fixture
4. 靜態截圖 / prototype

### 因此必須記住

- browser preview smoke 不等於 desktop acceptance。
- 只在 preview fixture 看起來對，不代表 scheduler、keyring、filesystem、window layout、sticky rail、scroll 行為真的對。
- 如果桌面真機與 preview 不一致，先以桌面真機為準，再追是 source drift、WebView cache drift、還是 preview fixture 失真。
- review 截圖若來自 browser preview，必須誠實標示，不得冒充 desktop 已驗收。

任何牽涉下列項目的 UI 變更，都不能只靠 browser 驗收：

- sticky / split-pane / detail rail 行為
- workspace-scroll 與 pagination 保持位置
- native capability / degraded state
- path / reveal / open / verify / updater / scheduler / keyring / lock screen
- 桌面視窗尺寸、縮放與長內容滾動體驗

---

## 8. Explorer 紅線：pagination 與 detail rail 不能退

Explorer 是 canonical evidence workbench，不是靜態內容頁。

### pagination 紅線

- 上方 timeline / summary 與底部分頁列都要顯示 **當前頁 / 總頁數**。
- 底部分頁列必須承接：
  - 上一頁 / 下一頁
  - 跳頁
  - 每頁筆數切換
- 不能只顯示 loaded count，讓使用者不知道自己在結果集哪裡。
- 翻頁或切換 page size 時，只能刷新結果；**不得把 `workspace-scroll` 強制拉回頁首**。

### detail rail 紅線

- time view detail rail 必須 sticky 在可視區內。
- detail rail 不能跟左側長列表一起被拉成整列高度。
- 使用者在列表底部選到某筆記錄時，不應被迫捲回頁首才能看 detail。
- detail rail 自己也要有限高與內滾動，不能因為 evidence 太長就把整頁撐爆。

### grouped views 紅線

- `session` / `trail` 仍屬 `/explorer` 的 canonical view，不可做成另一套路由或另一套不一致的 detail 行為。
- grouped view 也必須保留 window / scope honesty，不能偷偷退回 archive-wide。

只要有人提議把 Explorer 改成「更像 feed」「更像 infinite scroll」「先拿掉 detail rail 以後再補」，預設答案就是 **不行**。

---

## 9. Reviewer 快速清單

看一個新 UI 或改版時，至少問完這 8 題：

1. 這是 summary card，還是其實該獨立成 workbench surface？
2. 它如果做成 full-width，有沒有落在白名單內？
3. 它有沒有明確限高與內滾動？
4. 文案是不是講人話，而不是產品腔 / AI 腔？
5. 圖表是否說清楚 scope、window、evidence、freshness？
6. 這個結論能不能回到 Explorer / Audit / evidence drawer？
7. 這個行為是不是只在 preview 成立，桌面真機還沒驗？
8. 如果是 Explorer 相關，pagination / sticky detail rail / scroll position 有沒有被破壞？

任何一題答不清楚，就不要把 UI 當成完成。
