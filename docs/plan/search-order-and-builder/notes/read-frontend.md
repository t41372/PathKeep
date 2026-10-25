# History 前端與設計系統盤點：搜尋排序控制與進階搜尋 builder

我只讀了程式碼和文件，沒有改任何檔案。數字、行號以 `feat/search-order-and-builder` 分支為準（這個分支與 `main` 目前相同）。寬度數字是照 class 推算的，沒有在真機上量過。

## 0. 重點結論

1. **排序寫死在程式裡，URL 也沒有 `sort` 參數。**
   - 全文搜尋固定 `relevance`，regex 固定 `newest`（`src/features/history/queries.ts:137`）；時間線固定 `newest`（`queries.ts:119`）。
   - URL 參數清單見 `history-params.ts:17-27` 和 `docs/design/screens-and-nav.md:42`，裡面沒有排序。
   - `docs/plan/STATUS.md:315` 明寫：「History 畫面沒有「最舊」排序（只有相關度與最新），所以不加 History 的；用最舊排序的是 Ask 與 MCP。」
2. **後端契約只支援三種排序，語意搜尋完全沒有排序參數。**
   - 串流搜尋只收 `'relevance' | 'newest' | 'oldest' | null`（`src/lib/backend-client/search.ts:25`、`docs/architecture/search.md:38`）。
   - 語意搜尋的 `AiSearchRequest` 沒有 sort（`src/lib/types/intelligence.ts:753-769`）。
   - 「最多造訪」不在任何搜尋契約裡。
   - `SearchRequest` 還有 UI 沒用到的 `fuzzy`、`starred`（`search.ts:32-33`）。
3. **已定案的需求其實要求能選 newest / oldest。** `docs/features/recall.md:99`：「Keyword mode 預設以 relevance 排序，明確選擇 newest / oldest 時才回到時間排序」。但 `recall.md:51-61` 的「2026-10 介面沒有做到」清單沒列這一條。
4. **最大的陷阱：History 裡不要用 Radix `Select`。**（讀碼推論，沒實跑）
   - `use-list-keys.ts:40` 只在 `[role="dialog"], [role="menu"]` 存在時讓路。
   - Radix Select 的內容是 `role="listbox"`（`node_modules/@radix-ui/react-select/dist/index.mjs:462`）。方向鍵只 `preventDefault`、不 `stopPropagation`（`:481-497`），選項上按 Enter 也照樣往上冒（`:883-887`）。
   - 結果：↑↓ 會同時移動 History 列表的選取。Enter 選排序時，`target.closest('[role="listbox"]')` 成立，會觸發 `onOpen()`，在瀏覽器打開目前選取的頁（`use-list-keys.ts:51-59`）。
   - `DropdownMenu`（role=menu）和 `Popover`（role=dialog）沒有這個問題。
5. **builder 面板一定要放在 Portal 裡（Popover）。**
   - 工具列根節點在 macOS 上是 `data-tauri-drag-region="deep"`（`history-toolbar.tsx:177`）。
   - Tauri 2.11.1 的 `drag.js` 會把工具列子樹裡的非互動元素（div、span、文字）的 mousedown 當成拖視窗，並呼叫 `preventDefault`（`~/.cargo/registry/.../tauri-2.11.1/src/window/scripts/drag.js:12-30,51-70,78-105`）。
   - 算「可點」的只有 A、BUTTON、INPUT、SELECT、TEXTAREA、LABEL、SUMMARY、`tabindex≠-1`，以及 role 為 button / link / menuitem / tab / checkbox / radio / switch / option 的元素。
   - 所以內嵌在工具列裡的面板，空白處和說明文字會拖動視窗、也選不了字。
6. **文字輸入框不能放進 `DropdownMenu`。**
   - Radix Menu 內容會把字元鍵拿去做 typeahead、把焦點移走（`node_modules/@radix-ui/react-menu/dist/index.mjs:300-306`）。
   - `DropdownMenuSub*` 有包好，但整個 app 沒有人用。
7. **最好的 builder 模板是日期篩選 popover（`date-filter.tsx`）。** 它是兩步驟的狀態機，有草稿、有「返回 / 套用」、用 Portal、role 是 dialog。
8. **結果數那一行是 E2E 契約。** 至少 6 個 spec 用 `/… · Full text$/` 錨定（例：`tests/e2e/history-tools.spec.ts:90`）。不要把排序資訊接在 `history.results.*` 尾端。
9. **「正在搜尋」有兩種判斷，出現時間差 250 ms。**
   - 模式切換鈕在第一個按鍵就出現：`searching={draft.trim() !== ''}`（`history-page.tsx:369`）。
   - 結果數那一行等 250 ms debounce 寫進 URL 後才出現（`history-page.tsx:53,94,383`）。
10. **狀態必須在 URL 裡**（`docs/design/ui-review-guardrails.md:136`）。
    - 省略預設值靠的是一個靜態的 `defaults` 物件（`history-params.ts:47-55,64`），表達不了「預設排序跟著 mode 走」。
    - v0.3 的做法是：沒有 `sort` 參數時依 mode 推導（`git show v0.3.0:src/pages/explorer/url-state-derivations.ts`，約 103-111 行）。
11. **`resetKey` 不含排序**（`history-page.tsx:111`）。換排序時列表不會捲回頂端（`virtual-rows.tsx:75-77`）。
12. **透明原則直接適用 builder。** PME 的 Preview 要先顯示 visible query（`ux-principles.md:40`）。builder 產出的應該是寫進搜尋框、看得見的查詢字串，而不是藏起來的狀態。

---

## 1. 工具列版面與空間

### 1.1 結構（`src/features/history/history-toolbar.tsx`）

| 位置       | 內容                                                                | class / 尺寸                                                                                                                           | 行號             |
| ---------- | ------------------------------------------------------------------- | -------------------------------------------------------------------------------------------------------------------------------------- | ---------------- |
| 根節點     | 兩列 + 底線；macOS 上是拖曳區                                       | `flex flex-col gap-3 border-b px-7 pt-5 pb-3.5`，`{...dragRegion()}`                                                                   | 176-179          |
| 第 1 列    | 搜尋框 + 視圖切換                                                   | `flex flex-wrap items-center gap-2.5`                                                                                                  | 180              |
| 搜尋框外框 |                                                                     | `relative flex min-w-[260px] flex-1 items-center`                                                                                      | 181              |
| 搜尋框     | `type="search"`，`aria-invalid` 對應 regex 錯誤                     | `h-10 rounded-[10px] bg-card pr-[140px] pl-[38px] shadow-card [&::-webkit-search-cancel-button]:hidden`                                | 183-191          |
| 框內右側   | 清除鈕（有字時才出現）、`SearchHelp`「?」、`<kbd>⌘K / Ctrl K</kbd>` | `absolute right-2.5 flex items-center gap-1.5`；兩個按鈕 `size-[22px]`；kbd `rounded-[5px] border px-1.5 py-0.5 font-mono text-[11px]` | 192-207          |
| 視圖 Tabs  | 時間線 / 網站 / 已加星（附數量）                                    | `TabsList h-[38px] rounded-[9px] gap-0.5`；`TabsTrigger h-8 rounded-[7px] px-3 text-[13px]`；數量 `font-mono text-[11px]`              | 209-234          |
| 第 2 列    | 篩選                                                                | `flex flex-wrap items-center gap-2`                                                                                                    | 237              |
| 日期       | `DateFilterButton`                                                  | `h-[30px]`（見 §2）                                                                                                                    | 238              |
| 瀏覽器     | `BrowserFilter`（DropdownMenu）                                     | `h-[30px]`                                                                                                                             | 239-243          |
| 網站 chip  | 有 `domain` 時才出現                                                | `flex h-[30px] animate-rise items-center gap-1.5 rounded-lg bg-primary pr-1.5 pl-2.5 text-[13px] text-primary-foreground`              | 244-258          |
| 清除篩選   | 有篩選時才出現                                                      | `Button ghost sm h-[30px] px-2 font-normal text-muted-foreground`                                                                      | 259-268          |
| 搜尋方式   | 只在 `props.searching` 時出現                                       | `ToggleGroup … className="ml-auto animate-rise"`                                                                                       | 269-276、133-140 |

搜尋時列表上方（在 `<section>` 裡，不在工具列裡，`history-page.tsx:382-399`）：

- 結果數那一行：外層 `flex min-h-[38px] items-center gap-2 px-7 pt-3.5 pb-1.5 text-[13px] text-muted-foreground`，裡面是 `<p aria-live="polite"><ResultsLine/></p>`。這個 flex 列目前只有一個子元素（`history-page.tsx:383-389`）。
- `SemanticNote`：`px-7 pb-1.5 text-xs`（`semantic-status.tsx:28-30,55`）。
- `SearchCoverageNotice`：`px-7 pb-2`（`history-page.tsx:397-399`）。

工具列總高約 117 px（20 + 40 + 12 + 30 + 14 + 1），是從 class 算的。

### 1.2 只在搜尋時出現的東西

- **搜尋方式切換鈕**：依 draft 判斷（`history-page.tsx:369`），一打字就出現，進場用 `animate-rise`，離場沒有動畫、直接卸載。
- **視圖 Tabs**：依 URL 的 `q`，搜尋時三個都不選中（`history-page.tsx:357`，傳 `'' as HistoryView`）。點任何一個會清掉 `q`（`setView`，`:217-220`）。
- **結果數那一行、語意狀態、索引覆蓋提示**：都依 URL 的 `q`（`:383-399`）。
- **列表**：換成 `ResultRow`，列高 54 px，label 是「History results」（`timeline-model.ts:160-166`、`history-page.tsx:331-348`）。
- **清除鈕**：draft 非空才出現。它在右側叢集的最左邊，所以出現時「?」和 kbd 不會移位（`history-toolbar.tsx:192-206`）。
- **寫進 URL 的時機有兩種：**
  - 打字經過 250 ms debounce（`useSearchDraft`，`history-page.tsx:53,65-76`）。
  - 點標籤時立刻寫：先 `setDraft(null)` 再 `update({ q, mode: 'full' })`（`:228-234`）。builder 的「套用」適合照第二種做。
- **搜尋方式在清掉搜尋後會留在 URL**（`update({q:''})` 只刪 `q`，`history-params.ts:60-72`）。排序參數照理也會留著。

### 1.3 寬度預算

以下是照 class 推算的（Geist 13 px 約每字 6.5–7 px），要在 1080 px 的真機上驗證。

- **視窗限制**：預設 1480×960，最小 `minWidth: 1080`、`minHeight: 760`（`src-tauri/tauri.conf.json:17-20`）。
- **History 可用寬度**：1080 − 導航列 76（`nav-rail.tsx:166`）− 右邊距 10 − 邊框 2 = 992 px（`app-frame.tsx:112-118`）。扣掉工具列 `px-7`，內容寬 **936 px**。Windows / Linux 上面板頂端是 `mt-8`，寬度不變。
- **第 1 列**：Tabs 約 230 px，搜尋框約 690 px（下限 260 px）。
- **框內右側**：清除 22 + 說明 22 + kbd（⌘K 約 30、「Ctrl K」約 44）+ 間距約 22，共 96–110 px。目前保留 `pr-[140px]`，只剩約 30–44 px。框內再放一顆 22 px 的鈕會很緊。
- **第 2 列（英文，所有 chip 都在）**：
  - 日期約 105、瀏覽器約 135、網站 chip 約 110（github.com）、清除篩選約 88、搜尋方式約 210（三顆 `h-7 px-2.5 text-xs`，間距 4 px）、間距 32。
  - 合計約 680 px，剩約 250 px。
  - 最壞情況（自訂日期範圍、長網域如 developer.mozilla.org）只剩約 145 px。一個「Newest first」觸發鈕約 130 px，已經擦邊。
  - 一旦換行，第 2 列高 38 px，列表會被往下推，造成 layout shift。
- **詳情面板打開時**（`aside w-80`，`history-page.tsx:418-425`）：工具列跨在列表和面板上方，寬度不變（`:353-381`）；結果數那一行在 `<section>` 裡，最小視窗時剩約 992 − 320 − 56 = **616 px**。

### 1.4 拖曳區

- 工具列根節點是 deep 拖曳區（`history-toolbar.tsx:177`、`src/components/app/drag-region.ts:15-18`）。
- 文件說法：「頁首與工具列（History 的搜尋列…）的空白處」可拖動；「按鈕、輸入框、連結照常可點」（`screens-and-nav.md:32`）。
- Tauri 判斷可點的規則見 §0 第 5 點。注意 `role="menuitemradio"` 和 `combobox` **不在**可點清單裡，除非元素本身是 BUTTON 或 `tabindex≥0`。
- Portal 出去的 Popover / DropdownMenu 不在工具列子樹裡，不受影響。

---

## 2. 樣式慣例

### 2.1 History 現有控制項的 class

| 控制項                     | 寫法                                                                                                                                                                                                                                                                    | 行號                                           |
| -------------------------- | ----------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | ---------------------------------------------- |
| 篩選觸發鈕（日期、瀏覽器） | `<Button variant="outline" size="sm" className="h-[30px] bg-card px-2.5 font-normal">`；非預設值時加 `border-foreground`；左圖示 `size-3.5`、右 `ChevronDown size-3.5 text-muted-foreground`；`aria-label="${label}: ${value}"`                                         | toolbar 77-97、date-filter 94-106              |
| 搜尋方式（ToggleGroup）    | `type="single" spacing={1}`，`onValueChange={(n) => n && onMode(n)}`（擋掉取消選取）；項目 `h-7 rounded-full border px-2.5 text-xs font-medium text-muted-foreground data-[state=on]:border-primary data-[state=on]:bg-primary data-[state=on]:text-primary-foreground` | toolbar 131-140                                |
| 停用項目加 tooltip         | 包一個 `<span tabIndex={available ? -1 : 0}>`，因為停用的 button 收不到 pointer 事件；`TooltipContent className="max-w-[260px]"`；全域 `TooltipProvider delayDuration={400}`                                                                                            | toolbar 147-165、`src/app/index.tsx:63`        |
| DropdownMenu 單選          | `DropdownMenuContent align="start" className="min-w-[200px]"`，裡面 `DropdownMenuRadioGroup` / `RadioItem`（左側圓點、`pl-8`）                                                                                                                                          | toolbar 99-118、`ui/dropdown-menu.tsx:119-141` |
| 框內小圓鈕                 | `flex size-[22px] items-center justify-center rounded-full … outline-none hover:text-foreground focus-visible:ring-2 focus-visible:ring-ring/50`；打開時 `data-[state=open]:text-foreground`                                                                            | toolbar 198、search-help 77                    |
| 篩選 chip 的移除鈕         | `flex size-5 … rounded outline-none hover:bg-primary-foreground/15 focus-visible:ring-2 focus-visible:ring-ring/50`                                                                                                                                                     | toolbar 253                                    |

焦點環有兩套：自己寫的小按鈕用 `focus-visible:ring-2 focus-visible:ring-ring/50`；shadcn 原件用 `focus-visible:border-ring focus-visible:ring-[3px] focus-visible:ring-ring/50`（`ui/button.tsx:7`、`ui/toggle.tsx:7`）。

### 2.2 日期篩選 popover：builder 的模板（`date-filter.tsx`）

- **受控開關**：`open` / `picking` / `draft` 三個 state（67-69）。每次打開都回到預設步驟：`onOpenChange={(next)=>{setOpen(next); if(next) setPicking(false)}}`（86-92）。
- **內容寬度跟著步驟變**：`PopoverContent align="start" className={cn('p-1', picking ? 'w-auto' : 'w-[200px]')}`（108-111）。這只是浮層內縮放，不推擠頁面。
- **第一步：預設清單**（144-178）。外層 `role="menu"`，每項是 `<button role="menuitemradio" aria-checked>`，class 為 `flex h-8 items-center gap-2 rounded-md px-2 text-left text-[13px] outline-none hover:bg-muted focus-visible:bg-muted`；勾號 `size-3.5`，未選時 `opacity-0`，所以不跳位。`<Separator className="my-1"/>` 之後是「自訂範圍…」。
- **第二步：日曆**（112-142）。`Calendar mode="range"`，locale 依語言（`calendarLocales`，31），`disabled={{after: new Date()}}`。底列 `flex items-center justify-between gap-2 p-2`，左邊 ghost「返回預設」，右邊主按鈕「套用」，草稿沒選好時停用（123-141）。
- **選擇即關閉**：`choose()` 呼叫 `onChange` 後關閉（71-74）。Radix 非 modal popover 關閉時會把焦點還給觸發鈕（`react-popover/dist/index.mjs:35,174-177`，`modal = false`、`trapFocus: false`）。
- **注意**：這裡的 `role="menu"` 是一般 button，**沒有方向鍵 roving focus**，只能用 Tab，ARIA 的 menu 模式並不完整。E2E 依賴這些 role 名稱（`history-tools.spec.ts:569,662-663` 用 `getByRole('menuitemradio', { name: 'Last 7 days' })`）。

### 2.3 速查表 popover（`search-help.tsx`）

- 滑鼠停 350 ms 打開、離開 200 ms 關閉（20-21）。用 `pinned` ref 記住是否由點擊或鍵盤打開（48, 52-69）。
- 由 hover 打開時，用 `onOpenAutoFocus` 的 `preventDefault` 讓焦點留在搜尋框（87-90）。
- 尺寸與 class：
  - `PopoverContent align="end" sideOffset={10} className="w-[380px] p-0"`（82-92）
  - 標頭 `border-b px-4 pt-3.5 pb-3`，標題 `text-sm font-semibold`，說明 `text-xs text-muted-foreground`（93-100）
  - 清單 `max-h-[min(420px,60vh)] overflow-y-auto p-1.5`（101）
  - 每項 `flex w-full flex-col gap-0.5 rounded-md px-2.5 py-1.5 text-left outline-none hover:bg-muted focus-visible:bg-muted focus-visible:ring-2 focus-visible:ring-ring/50`；`<code className="font-mono text-[12px]">`（106-121）
  - 頁尾 `border-t px-4 py-2.5 text-[11px] leading-relaxed`（126-128）
- **陷阱**：hover 打開後，在裡面點擊**不會**把它釘住（`pinned` 只在 `onOpenChange(true)` 時設）。如果在這個浮層裡放輸入框，指標一離開 200 ms 就關掉。
- **選範例的行為**：`onExample` 把字填進 draft；不是 `/…/` 的範例會順便切回全文（`history-page.tsx:375-379`，註解寫「Operators are full-text syntax」）。`/…/` 不管目前選哪種方式都會被當成 regex（`history-params.ts:168-181`）。

### 2.4 其他可以照抄的先例

- **低調的排序 Select**（Insights 熱門網站）：`SelectTrigger size="sm" className="h-7 gap-1 border-none bg-transparent px-2 text-xs text-muted-foreground shadow-none"` 加 `aria-label`，`SelectContent align="end"`（`src/features/insights/insights-cards.tsx:240-257`）。文案 key 是 `insights.topSites.orderLabel` / `insights.topSites.order.*`（`messages/insights.ts:151-156`、`639-644`、`1121-1126`）。**它存在元件 state 裡，不在 URL；放到 History 會踩到 §0 第 4 點。**
- **分段控制**（Insights 範圍）：群組 `rounded-[9px] bg-muted p-[3px]`；項目 `h-[30px] rounded-[7px] px-3 text-[13px] data-[state=on]:bg-card data-[state=on]:shadow-card`；用 `PopoverAnchor` 讓自訂範圍的 popover 對齊整個群組（`src/features/insights/scope-controls.tsx:71-113`）。外觀和 History 的 Tabs 一致。
- **分組下拉選單**（Ask 選模型）：`DropdownMenuGroup` + `DropdownMenuSeparator` + `DropdownMenuLabel className="text-xs text-muted-foreground"`，勾號用 `opacity-0` 佔位（`src/features/ask/model-picker.tsx:62-100`）。
- **可搜尋的挑選器（Popover + cmdk）**：`PopoverAnchor` 包住輸入框，`PopoverContent className="w-(--radix-popper-anchor-width) min-w-[320px] p-0"`，裡面 `Command / CommandInput / CommandList / CommandEmpty / CommandItem className="text-[13px]"`（`src/features/ai-setup/model-field.tsx:100-163`）。適合拿來挑網站、標籤、運算子。
- **現成的查詢序列化**：`tagQuery()`（`tags.ts:44-48`）。值含空白、`"` 或 `\` 時加引號並跳脫 `"`、`\`。**它沒有跳脫彎引號 “ ”**，但後端分詞器把 “ ” 也當引號（`search_query.rs` 的 `tokenize_query`，約 154-172 行）。

### 2.5 `src/components/ui/` 清單

| 元件                            | 用到它的檔案數 | History 有沒有用   | 備註                                                                                                                                                                                                 |
| ------------------------------- | -------------- | ------------------ | ---------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| popover                         | 7              | 有（日期、速查表） | 預設 `w-72 p-4 rounded-md border bg-popover shadow-md` 加 tw-animate-css 進出場（`popover.tsx:30`）；也匯出 `PopoverAnchor`、`PopoverHeader/Title/Description`                                       |
| dropdown-menu                   | 5              | 有（瀏覽器）       | 包了 `Sub/SubTrigger/SubContent`、`CheckboxItem`、`Label`、`Shortcut`；**Sub 系列全 app 沒人用**                                                                                                     |
| select                          | 7              | 沒有               | 預設 `position='item-aligned'`；觸發鈕 h-9 / sm 是 h-8（`select.tsx:37,53`）；內容是 role=listbox                                                                                                    |
| tabs                            | 2              | 有                 |                                                                                                                                                                                                      |
| toggle-group / toggle           | 3 / 1          | 有                 | 沒設 `orientation` 時 ↑↓ 也會移動群組焦點（`react-roving-focus/dist/index.mjs:205-210`）                                                                                                             |
| command（cmdk 1.1.1）           | 3              | 沒有               | `CommandDialog` 預設 title / description 是英文寫死的（`command.tsx:31-32`，sr-only）；有一個本地加的 `shouldFilter` prop（`:36,43`），看起來是在 registry 原檔上改的                                |
| dialog / alert-dialog / sheet   | 13 / 10 / 2    | 沒有               |                                                                                                                                                                                                      |
| calendar（react-day-picker 10） | 2              | 有                 |                                                                                                                                                                                                      |
| collapsible                     | 6              | 沒有               | 如果做成工具列內展開，會踩到 §1.4 的拖曳區                                                                                                                                                           |
| 其他                            |                |                    | badge、button、checkbox、empty、input、kbd（只有 `security-section.tsx` 用；History 的 kbd 是手寫的）、label、progress、radio-group、separator、skeleton、sonner、spinner、switch、textarea、tooltip |

另外：`node_modules/@radix-ui/` 已經有 react-hover-card、react-toolbar、react-scroll-area、react-slider、react-accordion、react-context-menu、react-menubar、react-navigation-menu（由 `radix-ui` 1.4.3 meta 套件帶進來，`package.json:69`），但 `ui/` 沒有對應的 wrapper。

### 2.6 動畫

- `animate-rise`：220 ms，`cubic-bezier(0.2,0.7,0.2,1)`，上浮 6 px 加淡入（`src/index.css:198-208`）。History 用在搜尋方式切換鈕、網站 chip、標籤 chip、空狀態（`history-toolbar.tsx:139,245`、`tag-editor.tsx:144`、`state-message.tsx:75`）。
- Popover / DropdownMenu / Select 用 tw-animate-css 的 `data-[state=open]:animate-in fade-in-0 zoom-in-95 slide-in-from-*-2`，關閉時 `animate-out`。
- 詳情面板：`transition-[width] duration-200 ease-out motion-reduce:transition-none`（`history-page.tsx:421`）。
- 換篩選時列表淡化：`opacity-60 transition-opacity`（`visit-rows.tsx:91-93`）。
- reduced motion：全域把 animation 和 transition 縮到 0.01 ms（`src/index.css:224-232`）。

---

## 3. 鍵盤、焦點、ARIA：現況與陷阱

- **`useListKeys`**（`use-list-keys.ts`）掛在 window 的 keydown：
  - ↑↓ 移動選取，Enter 打開，Esc 關閉面板。
  - 有修飾鍵、焦點在 INPUT / TEXTAREA / SELECT / contentEditable 時不處理（17-23, 33-39）。
  - 有 `[role="dialog"], [role="menu"]` 時不處理（40）。
  - Enter 只在 `target === body` 或焦點在 `[role="listbox"]` 裡面時打開頁面（51-59）。
  - 它不看 `event.defaultPrevented`。所以 Radix Select 會出 §0 第 4 點的問題。
- **ToggleGroup 沒設 orientation**：↑↓ 會同時移動群組焦點和列表選取。這是現有行為。
- **全域快捷鍵**：⌘K / Ctrl+K 開關命令面板，⌘L 鎖定（`app-frame.tsx:80-93`）。⌘[ / ⌘]（Alt+← / Alt+→）上一頁 / 下一頁，在文字框裡不攔（`:48-78`）。**沒有任何快捷鍵能把焦點移到 History 自己的搜尋框**；框內的 ⌘K 提示其實指向全域面板（`history-toolbar.tsx:31-34,204-206`）。
- **ARIA 現況**：
  - 篩選觸發鈕的 aria-label 是「標籤: 目前值」，E2E 用 `/^Date filter: /` 抓（`history-tools.spec.ts:569,584`）。
  - 結果數那一行是 `aria-live="polite"`。
  - 列表是 `role="listbox"` 加 label，列是 `role="option"` 加 `aria-selected`（`virtual-rows.tsx:84-89`、`timeline-rows.tsx:123-127`）。
  - 星號改成 `role="img"` 加三語 label，因為原本 `aria-hidden`，讀螢幕的人不知道哪一列加了星（`timeline-rows.tsx:84-94`、`STATUS.md:112`）。
  - 搜尋方式的 E2E 用 `getByRole('radio', { name: 'Regex' })`（`history.spec.ts:106`、`search-limits.spec.ts:54`），新的分段控制項目名稱不要重複。
  - 速查表在 E2E 裡用 `getByRole('dialog')` 抓（`history-tools.spec.ts:923-926`）；同時開兩個 popover，這個選擇器會變得有歧義。
- **E2E 缺口**：沒有任何 E2E 按過 ArrowDown（`grep ArrowDown tests/e2e` 沒有結果）。History 列表的鍵盤操作契約目前沒有回歸測試。

---

## 4. 排序的資料流與既有契約

- **請求怎麼組**：`useVisitList` 的 `typed` useMemo 組出 `SearchRequest`，`sort: search.mode === 'full' ? 'relevance' : 'newest'`，deps 裡沒有排序（`queries.ts:131-156`）。`useSearchStream` 用 `JSON.stringify(request)` 當 session key，request 一變就重開 session，新結果還沒到之前保留舊的列並淡化（`use-search-stream.ts:83,93-94,111-117`、`queries.ts:220-221`）。
- **換請求時結果數那一行會短暫消失**：placeholder 期間 `ResultsLine` 回傳 null（`results-line.tsx:42`）；外層 `min-h-[38px]` 撐住高度，不會跳版。
- **語意搜尋**：`useInfiniteQuery` 的 key 是 `('semantic', text, filters)`，沒有排序（`queries.ts:161-179`）。
- **時間線**：`explorerClient.queryHistory({ sort: 'newest', … })`（`queries.ts:111-128`）。`HistoryQuery.sort` 也支援 `'oldest'`（`src/lib/types/archive.ts:332`），但 `buildTimeline` 假設資料是新到舊：
  - 檔頭寫明「newest first」（`timeline-model.ts:2`）。
  - 分 session 的判斷是 `previous.visitTime - item.visitTime > SESSION_GAP_MS`（78-82），倒過來排時差值是負的，一天內永遠不會切 session。
  - session 標頭的 start / end 也會顛倒（109-110）。
- **排序語意**（後端文件，前端看得到的事實）：
  - relevance：分數，再比最新可見造訪，再比 page id。
  - newest：照每頁最新的造訪排，列上顯示那次。
  - oldest：照每頁在範圍內的**第一次**造訪排，列上也顯示那次（`docs/architecture/search.md:11`、`recall.md:14-16`、`STATUS.md:312-314`）。
  - 所以換排序時，`ResultRow` 上的日期意思會變（`timeline-rows.tsx:205-209`），但列上沒寫是哪一次造訪。
  - regex 只寫了 newest / oldest 兩個方向（`search.md:11`）。
- **URL**：`parse()` 用白名單加後備值（`history-params.ts:32-45`）；`update()` 遇到等於 `defaults[name]` 或空字串的值就刪掉（`:60-72`）。
- **前例：sort 以前有過**
  - v0.2 的 `ExplorerQueryFiltersPanel` 有排序 `<select>`，2026-05-20 在 `afb7e943` 被刪。
  - v0.3 保留 `?sort=`，預設值依 mode 推導，但沒有 UI。
  - 三語文案在 `git show v0.3.0:src/lib/i18n/catalog/explorer.ts`：
    - 192-195：`filterSort: 'SORT'`、`sortRelevance: 'Relevance'`、`sortNewest: 'Newest first'`、`sortOldest: 'Oldest first'`
    - 689-692：`排序`、`相关性优先`、`最新优先`、`最早优先`
    - 1172-1175：`排序`、`相關性優先`、`最新優先`、`最早優先`
- **History 其他視圖的後端已經能排序，但 UI 寫死**：
  - 網站視圖：`insightsClient.topSites` 的 `sortBy` 支援 `'visits' | 'unique_days' | 'average_daily_visits'`（`src/lib/backend-client/insights.ts:100`），History 用的是預設值（`queries.ts:330`）。
  - 已加星：`StarSort` 支援 `'recently_starred' | 'most_revisited'`（`src/lib/backend-client/stars.ts:27`），History 寫死 `recently_starred`（`queries.ts:374`）。BACKLOG `WORK-STARS-PAGINATE-A`（`[ ]`）提過 most_revisited 以前只在視窗內成立，要露出前先驗證。

---

## 5. i18n

### 5.1 檔案與機制

- **檔案位置**：每個功能一個檔，三種語言並排寫在同一個檔裡（`src/lib/i18n/messages/index.ts:1-4`）。
  - `history.ts`：工具列、列表、詳情。
  - `history-search.ts`：檔頭寫「History's search aids: the syntax cheat sheet, regex dialect errors and semantic status」。
  - `history-page.ts`：詳情面板的預覽與標籤。
  - 共用詞在 `common.ts`。
  - 新檔案要在 `index.ts:26-46` 的 `namespaces` 註冊。
- **英文定形狀**：`defineMessages` 要求 zh-CN / zh-TW 和 `en` 的 key 一模一樣，少一個或多一個都會讓 `tsc` 失敗（`define.ts:26-32`）。`MessageKey` 是所有葉節點的點號路徑，複數後綴會被摺掉（`define.ts:34-42`、`index.ts:63`）。用 template literal 組的動態 key 一樣有型別檢查，例如 ``t(`history.modes.${mode}`)``（`results-line.tsx:47`）。
- **插值**：`{name}`。數字參數會依語系自動 `toLocaleString`。找不到的參數保留原字面（`runtime.ts:55-61`）。
- **複數**：`_one` / `_other`，在參數裡有數字型的 `count` 時，用 `Intl.PluralRules` 選（`runtime.ts:42-48`）。
  - 中文永遠選到 `other`，但因為形狀以英文為準，**中文也必須寫 `_one` 和 `_other` 兩個 key**（例：`history.ts:177-178`）。
  - 呼叫時不帶後綴：`t('history.repeats', { count })`。
- **後備**：找不到 key 時退回英文，dev 模式在 console 警告（`runtime.ts:49-53`）。
- **常用的組合**：`common.pages` / `common.visits` 先組好，再塞進 `history.results.*`（`results-line.tsx:52-60`）。
- **格式化**：`useFormat()` 提供 `number / compact / percent / date / relative / dayAndTime / timeRange…`（`format.ts:130-224`）。

### 5.2 新增 key 的步驟

1. 在對應檔的 `en` 加 key，再在同一檔的 `'zh-CN'`、`'zh-TW'` 加同一個路徑。`bun run typecheck` 會擋形狀不一致。
2. 三種語言的 `{placeholder}` 集合要一樣；只有 `_one` / `_other` 可以省掉 `{count}`。
3. 跑 `bun run check:i18n`（`scripts/i18n-progress.ts`，包含在 `check:js` 裡，也就是 `bun run check`，`package.json:22-23,43`）。現在的基線：`2181 keys × 3 languages, 0 problem(s)`（剛跑過）。

### 5.3 gate 擋什麼（`scripts/i18n-progress.ts`），以及幾個坑

- **中文不得夾未翻譯的英文詞**（26-42）：Full Disk Access、profile、adapter、append-only、rollup、digest、worker、payload、schema、migration、enrichment、trace、**visit**、**archive**、checkpoint。
- **繁中不得用大陸用語**：`/會話|視圖|訪問|信號|智能|卸載|刷新|決定性|語義/`（44-50）。所以繁中要寫「檢視、造訪、語意、重新整理」。「最多造訪」可以，「最多訪問」不行。
- **英文不得用內部術語**（52-59）：shell state、append-only、app data、canonical place、profile scope、all profiles。
- **所有語言**（61-64）：不得有 `TODO`；不得有看起來像 key 的值，規則是 `^[a-z]+(\.[a-z][a-zA-Z0-9]*)+$`。**整句只有 `github.com`、`docs.rs` 這種網域的 message 會被擋**，placeholder 範例要寫在程式裡（像 `search-help.tsx:30-42`）或放進句子裡。
- **任何 `{單字}` 都會被當成 placeholder**，runtime 和 gate 都是。例如 `historySearch.regex.brace` 裡的 `{2}`（`history-search.ts:36`）能過，只因為三種語言都有它，而且沒人傳名為 `2` 的參數。
- **gate 不檢查沒人用的 key**。例：`history.date.rangeHint`、`history.state.loadingMore`、`history.state.untitled` 在程式裡都找不到。

### 5.4 現有 key 清單（en，共 168 個）

- `history.search`：placeholder、label、clear
- `history.views`：label、timeline、sites、starred
- `history.modes`：label、full、regex、semantic、invalidRegex
- `history.date`：label、all、today、yesterday、last7、last30、custom、rangeHint、apply、back
- `history.browser`：label、all
- `history.filters`：clear、removeSite
- `history.day`：today、yesterday
- `history.repeats_one/_other`、`history.sameVisits`
- `history.session`：label、labelOpen
- `history.results`：pagesAndVisits、pages、pagesMore、searching、sorting、relevance、listLabel、timelineLabel
- `history.state`：loading、loadingMore、noHistoryTitle/Body、goBackup、noMatchTitle/Body、noResultsTitle/Body、noSitesTitle、noStarredTitle/Body、errorTitle、searchErrorTitle、noResultsPartialTitle/Body、searchConnectionLost、untitled
- `history.coverage`：building、updating、failed、progressLabel
- `history.sites`、`history.starred`、`history.detail.*`（30 個）
- `historySearch.help`：button、title、intro、site、exclude、phrase、or、intitle、inurl、filetype、dates、tag、note、regex、footer
- `historySearch.regex`：syntax、lookAround、backreference、group、escape、brace、backendTitle、backendBody
- `historySearch.semantic`：builtIn、builtInModel、using、indexed_one/\_other、updated、stale.{watermark,enrichment,textFormat}、rebuild、fellBack、openSettings、whereToFix、reason.{off,building,queued,paused,empty,failed,degraded,blocked}
- `historyPage.related.*`、`historyPage.preview.*`、`historyPage.tags.*`（標籤編輯器用的是 `historyPage`，不是 `history`，`tag-editor.tsx:122`）

各語言位置：`history.ts` en 4-133、zh-CN 134-261、zh-TW 262-389；`history-search.ts` en 5-69、zh-CN 70-129、zh-TW 130-190。

### 5.5 文案語氣

- **規則**：
  - `ui-review-guardrails.md` §5（60-78）。
  - AGENTS.md：「says things plainly, the way a person would. No AI slop.」
  - `ux-principles.md:97-103`，含「設計要預留中文與長錯誤訊息的換行」。
- **觀察到的慣例**：
  - 英文是 sentence case（「Search tips」「Clear filters」「Custom range…」）。
  - 打開更多 UI 的項目、進行中的狀態都用單一字元 `…`（216 處，沒有 `...`）。
  - 英文撇號、引號用彎的 `’ “ ”`（75 處對 7 處直的，直的都在 insights）。
  - 分隔用 `·`。
  - aria-label 用名詞片語（「Search mode」「Date filter」），按鈕用動詞（「Apply」「Back to presets」）。
  - 錯誤說原因再給下一步（例：`historySearch.regex.lookAround`）。
  - 中文引號：zh-CN 用 `“”`，zh-TW 用 `「」`（`history.ts:204,332`）。
  - 模式名稱：全文 / 正则 / 语义（zh-CN），全文 / 正規 / 語意（zh-TW）。

### 5.6 文案撞名，以及 E2E 依賴的字串

- **中文「排序」已經被用掉兩處**：
  - 結果數那一行的「仍在排序 / Still ranking」（`history.results.sorting`），指相關度排名還沒定。
  - Settings → AI 的「搜尋排序 / 搜索排序」，指語意融合排名的權重（`settings-ai.ts:283,574,862`）。
- **E2E 錨定結果數那一行**：
  - `history-tools.spec.ts:90-91`
  - `backup.spec.ts:88-94`
  - `browser-import.spec.ts:63-70,201-208`
  - `takeout-import.spec.ts:52-59`
  - `search-limits.spec.ts:36`
  - `history-tools.spec.ts:945`

---

## 6. 設計文檔裡適用的規則（原文引用）

**鍵盤、焦點、ARIA**

- `ui-review-guardrails.md:142`：「**鍵盤**：↑↓ 移動選取、Enter 在瀏覽器打開、Esc 關閉面板；輸入框或選單打開時不攔截。」
- `ui-review-guardrails.md:141`：「**詳情面板**是 `<aside>`…關閉時 `inert`，不能被 Tab 走進去」
- `ux-principles.md:46`：「current step / selected filter / status chip 要有可朗讀的 label，而不是只靠顏色或位置辨識。」
- `screens-and-nav.md:31`：「篩選與選取用 `replace`，不進歷史…在文字框裡不攔。」
- `recall.md:181`：「支援 keyboard shortcut 和快速導航（上下鍵切換記錄、Enter 展開詳情、Esc 關閉）。」

**動畫與 reduced motion**

- `ux-principles.md:20`：「**動畫**：克制。畫面與 Settings 分頁進場用 220 ms 的 `animate-rise`（6 px 上浮加淡入），History 詳情面板 200 ms 滑入；hover / 顏色變化 100–200 ms。沒有彈簧、沒有裝飾性循環動畫。」
- `ux-principles.md:128`：「`prefers-reduced-motion`：全域關掉動畫與轉場…狀態一律另有文字或顏色。」
- `design-tokens.md:83`：「shadcn components bring their own enter/exit animations from `tw-animate-css`. Under `prefers-reduced-motion` a global rule in `src/index.css` cuts every animation and transition to 0.01 ms.」
- `ui-review-guardrails.md:166`：「**transform / opacity only**：動畫只動 `transform` 或 `opacity`，不動 `width` / `left` 等觸發 layout 的屬性」
- AGENTS.md：「互動與轉場要有動畫、要順（目標 60fps）；生硬跳變、缺動畫視為缺陷而非預設。」

**100 ms 回饋、載入、不跳版**

- `ux-principles.md:109`：「所有畫面必須提供有意義的載入狀態，~100 ms 內要有視覺回饋，不允許空白或凍結的 spinner。」
- `ux-principles.md:118`（History）：「改篩選時保留舊結果並淡化（`dimmed`），新結果到了再換。搜尋結果數跟著串流更新，數完之前是「N+ 頁」。」
- `ux-principles.md:127`：「尺寸要接近最終內容，避免載入完成後 layout shift。」
- `ui-review-guardrails.md:139`：「**列表永遠虛擬化**，固定列高；載下一頁不得改變目前的捲動位置；改篩選時保留舊結果淡化顯示，不閃回空白。」
- `ui-review-guardrails.md:140`：「**大量資料不得卡主線程**」
- `ux-principles.md:43`（Execute paint-first，`src/lib/wait-for-next-paint.ts`）
- AGENTS.md：「輸入用 debounce / throttle」「性能敏感改動要 profiling…驗證後才宣稱「夠快」」

**URL 狀態**

- `ui-review-guardrails.md:136`：「**狀態在 URL 裡**：搜尋字串、方式、視圖、日期、瀏覽器、網站、選取的 visit 都要能從 URL 還原」
- `screens-and-nav.md:42`：「預設值不寫進 URL。」
- `screens-and-nav.md:52`：「History 的狀態全部在 URL 裡」
- `ui-review-guardrails.md:158`（第 8 題）：「如果是 History 相關，URL 狀態、虛擬化、詳情面板與鍵盤有沒有被破壞？」

**文案語氣**

- `ui-review-guardrails.md:60-63`：「先回答「現在發生什麼」「這對我有什麼影響」「我下一步去哪裡」…disabled / empty / degraded state 不能只剩一句抽象宣告；要帶明確邊界與下一步。」
- `ui-review-guardrails.md:67-70`：直接擋下空泛、假動作（`Processing...`）、假成功、假錯誤（`Something went wrong`）。
- `ui-review-guardrails.md:78`：三種語言一起交付，並通過 `check:i18n`。
- `ux-principles.md:97`：「包含 aria-label、placeholder、toast、空/錯/載入/停用狀態。」

**PME 與透明**

- `ux-principles.md:40`：「**Preview**：先顯示邊界、影響範圍、generated artifact / visible query / profile scope，再出現真正的 execute CTA。」
- `ux-principles.md:42`：「**Execute**：執行按鈕文案必須直接說明會做什麼」
- `ui-review-guardrails.md:94`：「不可把估算值、抽樣值、AI 改寫結果偽裝成 deterministic fact。」只排已載入的那幾頁，就屬於這種。
- `ui-review-guardrails.md:143`：「semantic 搜尋沒開時不能假裝在跑」
- `recall.md:123`：「篩選狀態在 UI 上有清晰的標籤式展示，可逐個移除或一鍵清除。」

**尺寸、溢出、token**

- `ui-review-guardrails.md:47`：「內捲動不是偷藏內容：必須保留可見的溢出暗示。」
- `ui-review-guardrails.md:32`：History 整個畫面在全寬白名單裡（它是 workbench）。
- `design-tokens.md:13`（用 Tailwind utilities，不用 raw `var(--…)`）、`:33`（`--popover` 給 menus / popovers / dialogs）、`:34`（`--muted` 給 segmented controls）、`:35`（`--accent` 是 hover，不是品牌色）、`:95`（不寫死顏色）、`:97`（「Don't edit files under `src/components/ui/`…pass classes or props」）。
- `design-tokens.md:82` 說 `--shadow-float` 用在 popovers，但 shadcn 的 Popover / DropdownMenu 實際用 `shadow-md`，只有 heatmap tooltip 和 Ask composer 用 `shadow-float`。文件和程式不一致。

**AGENTS.md 其他相關條目**

- 組件單一職責，業務邏輯不洩漏進 UI：查詢的序列化應該放在 UI 之外。
- 新依賴需用戶授權：例如引入 query-builder 套件。
- 已定案文檔不可隨意推翻：速查表是已定案需求（`recall.md:101`）。
- 文檔跟著代碼走：要同步改 `recall.md`、`screens-and-nav.md:42,70`、`ui-review-guardrails.md:136` 的 URL 清單。
- E2E 優先；只有規格清楚、E2E 跑起來太貴的純邏輯才寫 unit test，而且先寫失敗模式（`quality-matrix.md:53-60`）。目前 `src/` 下沒有任何 `*.test.ts`。

---

## 7. 文件目前怎麼描述搜尋方式、速查表和排序

- **`screens-and-nav.md:70`（History）**：
  - 工具列列了「搜尋方式（全文 / Regex / 語意）」和「「?」打開語法速查，點例子就填進搜尋框」。
  - 結果「邊排序邊出現…排序未定時加「仍在排序」」；網站視圖「依瀏覽次數排序」。
  - **沒有提到排序控制。**
  - URL 契約（`:42`）沒有 `sort`。
- **`recall.md`**：
  - `:40`：打字 250 ms 後寫進 URL。
  - `:41`：全文「依相關度排序」；速查表「點擊、鍵盤或滑鼠停留」打開，「點一下就填進搜尋框並切回全文；日期例子固定是上個月」。
  - `:42`：regex「依時間新到舊」，`/…/` 一律當 regex，用 Rust 方言檢查，不送出並保留舊結果。
  - `:44`：語意搜尋只在開啟而且索引就緒時能選，否則退回全文並說明原因。
  - `:99`：relevance 是預設，明確選 newest / oldest 才回到時間排序。
  - `:101`：速查浮窗是 hover / focus 都看得到的**必備**項目，三語。
  - `:111`：切換方式時保留目前輸入的字。
  - `:113`：還寫著舊的 `?regex=1`，`:135` 括號註明 v0.4 已改成 `?mode=regex`。
  - `:136`：regex 只比對 URL 和標題。
  - `:14-16`：可以明確選時間排序，newest 照最新、oldest 照第一次造訪。
- **`docs/architecture/search.md:11,38`**：排序語意與型別，見 §4。
- **`docs/architecture/lexical-recall-v2.md:148-152`**（FTS5 時代的 Accepted 記錄）：沒有關鍵字預設 newest；有關鍵字而沒指定時是 relevance；明確 newest / oldest 優先。
- **`STATUS.md:315`**：History 沒有「最舊」，只有 Ask 和 MCP 在用。
- **builder**：docs 和 BACKLOG 都沒有提過進階搜尋 builder 或二級選單。prototype 也沒畫排序或 builder。

---

## 8. builder 必須遵守的語法邊界

這些是後端的事實，但會決定前端該產出什麼字串。

- **語法來源**：`search-help.tsx:6-8` 指定 `vault-core/src/archive/search_query.rs` 是唯一來源，串流搜尋透過 `vault-core/src/search/query.rs:23` 使用它。
- **運算子**（`search_query.rs:77-133`）：
  - `site:`
  - `intitle:` 或別名 `title:`
  - `inurl:` 或別名 `url:`
  - `filetype:` 或別名 `ext:`
  - `tag:`（多個要全部符合，`-tag:` 排除）
  - `note:`（子字串比對）
  - `after:` / `before:`：**不能加 `-`**。`-after:X` 會掉進一般的文字排除（`search_query.rs` 的 match 落到 `_ if negated`），`docs/plan/test-audit-2026-10/vault-core-intelligence.md:177-185` 列為待決定。
  - `is:starred`：只能正向。
  - 任何 token 前加 `-` 就是排除；`"片語"`；彎引號 “ ” 也算引號；引號內用 `\` 跳脫。
- **日期格式**：`YYYY-MM-DD`、`YYYY/MM/DD` 或 `YYYY`（整年），after 從當天開始、before 到當天結束，都按本機時區。格式不對的日期會被**靜默丟掉**（`search_query.rs:269-299`；`after_ms` 的處理在約 126-128 行）。
- **未知的 `foo:bar`** 會被當成一般關鍵字（`split_operator`、`:209-211`）。
- **`OR` 不分大小寫**：小寫 `or` 也會切成另一組（`search/query.rs:34`）。
- **速查表沒列的**：`is:starred`、別名 `title:`、`url:`、`ext:`。
- **日期和網站各有兩條路**：可以用運算子 `after:/before:`、`site:`，也可以用工具列篩選（URL 的 `date`、`domain`），兩者會取交集（`recall.md:108`）。builder 要選一條。
- **regex**：Rust 方言，不分大小寫（`vault-core/src/search/regex.rs:357-366`）。前端在送出前先檢查（`regex-dialect.ts`），只比對 URL 和標題。

---

## 9. 其他意外發現

- **快捷鍵**：History 搜尋框裡的 ⌘K 提示指向全域命令面板，不是這個搜尋框；沒有任何快捷鍵能聚焦它。
- **文件缺漏**：`recall.md:51-61` 的缺口清單沒列「可選 newest / oldest」，但 `:99` 要求有。
- **registry 原檔有改動**：`ui/command.tsx` 有本地加的 `shouldFilter` prop，`design-tokens.md:97` 卻說 `ui/` 不改。
- **E2E 隱性依賴文案**：瀏覽器篩選鈕在 E2E 裡也用 `getByRole('button', { name: /All browsers/ })` 抓（`history.spec.ts:110`）。新按鈕的 aria-label 不要含這段字。
- **排序參數的位置**：`useHistoryParams` 沒有 `sort` 這個欄位。新增時 `parse`、`defaults`、`resetKey`、`typed` 的 deps，以及語意模式下的停用說明都要一起處理。
