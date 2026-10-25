# 歷史搜尋「排序」與「進階搜尋 builder」：已定案決策與前例盤點

我只讀了程式和文件，沒有改任何檔案。下文的 `file:line` 都是相對 `/home/agent/PathKeep` 的路徑。`v0.x.0:path:line` 指 `git show v0.x.0:path` 看到的行號。

---

## 0. 重點

- **排序控制在已定案文件裡本來就是要求，v0.4 只是沒做出來。** `recall.md:99` 寫明「Keyword mode 預設以 relevance 排序，**明確選擇 newest / oldest 時**才回到時間排序」。這一行在 2026-10-07 的 Turso 遷移時還重寫過（commit `54eaf10b`），晚於 M18 redesign。M14 也把「Add Explorer `Relevance` sort control」列為交付項（`docs/plan/m14-lexical-recall-v2/README.md:34,59`）。所以補上排序是落實既有要求，不是推翻什麼。
- **排序選單出現過兩次，後來被拿掉，沒寫理由。**
  - v0.1 的選項是 newest / oldest，v0.2 加了 relevance，URL 參數都是 `?sort=`。
  - 2026-05-20 v0.3 退役舊面板時，這個 `<select>` 跟著一起刪了（commit `afb7e943`）。commit 訊息講的是退役 `?layout=legacy`，對排序本身沒有理由。
  - 2026-10 redesign 照 prototype 做，prototype 裡也沒有排序。
  - `STATUS.md:315` 直接寫了「History 畫面沒有『最舊』排序（只有相關度與最新）」。
- **builder 的前例在 v0.3 已經有一半。**
  - `paper-search-filters.ts` 能把 `q` 裡的運算子 token 解析成可移除的 chip，也能把運算子 `appendOperator` 進查詢字串。
  - 搜尋框下的「+ Tag / + Note」有接線；「+ Date / + Source / + Domain / + Visit count」是停用、點不了的佔位。
  - v0.4 只剩速查表，點例子會把整個搜尋框換掉，不是附加上去。
- **後端已經支援 relevance / newest / oldest 三種排序**，只是 History 前端寫死。
  - 引擎的 `SortOrder` 就只有這三種（`src-tauri/crates/vault-search/src/filter.rs:120-125`）。
  - 「最常造訪」沒有任何定案文件，也沒有引擎支援。
  - 語意搜尋選 newest / oldest 時，後端會**直接改走全文、略過語意排名**。這點只寫在程式碼裡，feature 文件沒有。

---

## 1. 排序與 builder 必須遵守、或會推翻的已定案決策

### 1.1 排序語意（必須遵守）

| 決策                                                                                                                                      | 出處（摘錄）                                                                                                                                                                              |
| ----------------------------------------------------------------------------------------------------------------------------------------- | ----------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| relevance 的順序：分數 → 最新符合的可見造訪 → page ID。newest 依每頁範圍內最新的造訪，oldest 依每頁**第一次**造訪，該列顯示的就是那次造訪 | `docs/features/recall.md:14-16`；`docs/architecture/search.md:11`（「Explicit newest/oldest orders remain available…oldest goes by the page's first eligible visit and shows that one」） |
| 有關鍵字時預設 relevance，明確選才改用時間排序；relevance 只來自本機詞彙相關度，「不是 embedding 或 AI ranking」                          | `recall.md:99`；`docs/architecture/lexical-recall-v2.md:148-152`（沒有關鍵字時預設 newest）                                                                                               |
| oldest 用的時間要從存檔裡範圍內的造訪現算，不能用索引的 `newest_visit_ms`；第一次造訪只在 oldest 排序時才記                               | `search.md:11`；`STATUS.md:313-314`（14.4M 時多一個約 290 萬格的陣列）                                                                                                                    |
| regex 的 oldest 反方向走（`>=?` seek），所以先碰到的就是該頁第一次造訪                                                                    | `search.md:11`；`src-tauri/crates/vault-core/src/search/session.rs:101-107`（regex 只收 `oldest` 這個布林值，其他排序一律從新到舊）                                                       |
| 搜尋結果一頁一列，列上標出符合的造訪次數（用戶 2026-10-04 決定，理由是「一頁被造訪 2,000 次會塞滿前二十個畫面」）                         | `recall.md:39`；`docs/design/screens-and-nav.md:70`；commit `ff41390c`                                                                                                                    |
| 結果邊排序邊出現，排名完成前順序會變；畫面要寫「仍在排序」，計數完成前寫「N+」                                                            | `recall.md:23-25`；`screens-and-nav.md:70`；`src/features/history/results-line.tsx:64`                                                                                                    |
| 游標綁定 session，只能用一次；**改查詢、篩選或每頁筆數都要開新搜尋**（換排序同理）                                                        | `search.md:127`                                                                                                                                                                           |

**會推翻上表的做法：**

- 把全文的預設改成 newest，違反 `recall.md:99`、`lexical-recall-v2.md:151`。
- 做成「每次造訪一列」的最舊排序，違反用戶 2026-10-04「一頁一列」的決定。
- relevance 只用詞彙相關度：如果排序控制讓全文的 relevance 混入語意排名，違反 `recall.md:99`。

### 1.2 完整覆蓋與串流（必須遵守，新增排序也一樣）

- 「**搜尋必須覆蓋整個 archive**（用戶 2026-10-04 決定）。搜不出來的記錄，對用戶來說就是不存在」：`docs/architecture/decisions/012-turso-storage-engine.md:20`、`docs/architecture/large-result-search-tradeoff.md:12`。
- ADR-012 刪掉了窗口和「由新到舊的退路」（`012-turso-storage-engine.md:53`）。驗收文件寫明「relevance never changes because a query is common」（`docs/architecture/research/2026-10-search-integration-validation.md:16`）。所以**排序不能拿來當效能退路**，例如詞太常見就自動改成 newest。
- 結果仍要分段串流呈現（用戶 2026-10-04 要求）：`012-turso-storage-engine.md:54`。regex 也是分段掃描加串流：同檔 `:55`。
- 「最常造訪」之類的新排序，必須對完整的符合集合精確排序，不能只取前 N 筆近似。範圍篩選要和造訪落在同一次（「Date and profile constraints must hold for the same visit」：`recall.md:18`、`search.md:7`）。

### 1.3 URL 狀態與導航（必須遵守；加 `sort` 屬於擴充契約）

- History 的 URL 契約目前是 `q`、`mode=full|regex|semantic`、`view`、`date`、`browser`、`domain`、`visit`，「預設值不寫進 URL」：`screens-and-nav.md:42`，程式在 `src/features/history/history-params.ts:14-45`（沒有 `sort`）。加 `sort` 要同步更新這張表。
- 「History 的狀態全部在 URL 裡」：`screens-and-nav.md:52`。紅線「狀態在 URL 裡：搜尋字串、方式、視圖…都要能從 URL 還原」：`docs/design/ui-review-guardrails.md:136`。
  - 反例：Insights「熱門網站」的排序 `Select` 只存在元件 state（`src/features/insights/insights-cards.tsx:227-258`），History 不能照抄。
- 篩選與選取用 `replace`，不進瀏覽歷史（Accepted，用戶 2026-10-12 選 D）：`docs/design/back-forward-window-frame-tradeoff.md:3,30`、`screens-and-nav.md:31`。
- 工具列空白處是視窗拖動區，新加的按鈕要能照常點：`screens-and-nav.md:32`。
- 視窗最小 1080×760：`src-tauri/tauri.conf.json:19-20`。

### 1.4 查詢語法（builder 必須遵守）

- **`q` 是唯一的真相，由後端解析。**
  - 「§1 的進階語法…由後端解析」：`recall.md:41`。
  - `src/features/history/search-help.tsx:7-8` 註明 `vault-core/src/archive/search_query.rs` 是 source of truth。
  - 推論：builder 寫回 `q` 文字最貼合現行契約；另開 `?site=` 一類的結構化參數，等於把語法分叉成兩套。
- **`OR` 只接在一般詞之間；欄位、片語、排除、metadata 條件套用到每個 OR 群組**：`recall.md:18-19`、`search.md:13-15`。builder 表達不了「(A 且 site:x) OR (B 且 site:y)」。
  - 小寫的 `or` 也算 OR（`lexical-recall-v2.md:133`；`src-tauri/crates/vault-core/src/search/query.rs:34-36` 用 `eq_ignore_ascii_case`）。
- **只支援本機能證明的運算子**；language / region / usage rights / related / image 不支援：`recall.md:100,109`、`lexical-recall-v2.md:140-143`。
- **後端實際解析、但速查表沒列的寫法**（`src-tauri/crates/vault-core/src/archive/search_query.rs:75-140`、`search/query.rs:44-55`）：
  - `is:starred`
  - 別名 `title:`、`url:`、`ext:`
  - 陷阱：`after:` / `before:` 不能加負號（`search_query.rs:126-131`），`-after:…` 會被當成排除詞。
- **title / URL 條件是子字串比對**；短別名加引號就照字面：`recall.md:19-20`。
- **語法速查必須「hover / focus 可見」**，三種語言：`recall.md:101`。builder 如果取代現在的「?」，也得保留這種被動發現的入口。
- **標籤建議不在範圍內**：「標籤命名空間、tag aliasing、tag suggestion — 此版本只保留純字串，UI 不主動推薦」（`docs/features/annotations.md:33`）。**builder 若列出既有標籤讓人挑，就是推翻這條**，要寫 trade-off 並取得用戶同意。
- 點標籤搜尋 `tag:名稱`，有空白或引號時要加引號：`annotations.md:118-119`。

### 1.5 Regex 的邊界

- 只有明確的 regex 模式才吃 regex；**regex 裡的斜線和長得像運算子的文字都是 pattern 本身**：`recall.md:20-21`、`search.md:18-19`。所以在 regex 模式下，網站、日期、瀏覽器只能走 URL 篩選（`queries.ts:134-143` 對 regex 也會送 domain、時間、browserKind），不能寫成運算子。
- `/…/` 包起來的查詢不管選哪種方式都當 regex：`recall.md:42`、`history-params.ts:168-188`。
- 語法以 Rust `regex` 為準。不支援的寫法在前端擋下、用白話說明、不送出，並淡化保留上一批結果：`recall.md:42,112`、`ui-review-guardrails.md:137`。
- 後端預設不分大小寫：`src-tauri/crates/vault-core/src/search/regex.rs:363-364`。
- 只比對 URL 和標題：`recall.md:136`。
- 「關鍵詞搜尋是預設召回方式；regex 不是它的替代品」：`recall.md:128`。要讓 regex 在大資料量下也快，「必須先新增獨立 research / benchmark」：`recall.md:131`。
- 效能事實：pattern 裡有長度 ≥3 的 ASCII 必要字面時，只讀候選頁；沒有的話，以 512 筆一批走完所有符合的造訪（`search.md:11`）。
- 前端已經把使用者從 regex 導回全文：look-around 的錯誤訊息會說「想排除頁面，可以用全文搜尋加上 -關鍵字」（`src/lib/i18n/messages/history-search.ts:27-28,152-153`）。

### 1.6 UI 紅線

- History 整個畫面是全寬白名單裡的 workbench：`ui-review-guardrails.md:32`。
- 內容不能無限長高，內捲動要保留可見的溢出暗示（⌘K 清單最高 420 px）：`ui-review-guardrails.md:45-48`。速查表用 `max-h-[min(420px,60vh)]`（`search-help.tsx:101`）。
- 改篩選時保留舊結果淡化顯示，不閃回空白；列表永遠虛擬化：`ui-review-guardrails.md:139`。
- 鍵盤「↑↓、Enter、Esc；**輸入框或選單打開時不攔截**」：`ui-review-guardrails.md:142`。
- 語意搜尋沒開時不能假裝在跑：`ui-review-guardrails.md:143`。
- 動畫要克制（220 ms rise、200 ms 面板、hover 100–200 ms）：`docs/design/ux-principles.md:20`。
- 三種語言要一起交付，`check:i18n` 會擋繁中用詞：`ux-principles.md:97-103`。
- AI 是可選的（AGENTS.md 原則 4），builder 不能只靠 LLM。
- **不約束本工作**：`docs/design/search-activity-keyword-browser-tradeoff.md` 雖然標 Accepted（2026-04-20），但第 3 行明說是 2026-10 以前的介面，「留作紀錄，不再是現行規格」。所以它的「不新增 route grammar」（`:34-38,197`）不再約束。

---

## 2. 前例

### 2.1 時間軸

| 版本 / 日期                                         | 排序 UI                                                                                                                                     | URL                                                                                                                                                                  | 語法說明                                                                                                                                                         | Regex UI                                                                                                                    |
| --------------------------------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------- | -------------------------------------------------------------------------------------------------------------------------------------------------------------------- | ---------------------------------------------------------------------------------------------------------------------------------------------------------------- | --------------------------------------------------------------------------------------------------------------------------- |
| v0.1.0（2026-04-30）                                | `<select aria-label="SORT">`：Newest first / Oldest first，預設 newest（`v0.1.0:src/pages/explorer/query-filters-panel.tsx:346-354`）       | `?sort=`                                                                                                                                                             | 「基礎語法提示」（`docs/plan/m1-solid-archive/explorer-export-and-onboarding.md:54`）                                                                            | toggle                                                                                                                      |
| v0.2.0（2026-05-09）                                | 同一個 select 加上 Relevance（`v0.2.0:…/query-filters-panel.tsx:410-428`）。q 不空且不是 regex 時顯示 relevance，否則 newest                | `q`、`profileId`、`browserKind`、`domain`、`start`、`end`、`sort`、`regex`、`page`、`pageSize`（另有 `mode`、`view`）（`v0.3.0:docs/design/screens-and-nav.md:106`） | `AdvancedSearchHelp`「?」tooltip：5 個例子加一句 regex 說明，CSS 控制 hover / focus 顯示（`v0.2.0:src/pages/explorer/advanced-search-help.tsx:26-94`）           | `.*` toggle 寫入 `?regex=1`、輸入框旁 `[.*]` 標記、「Valid regex / Invalid regex」一行（`query-filters-panel.tsx:181-218`） |
| v0.3（paper，2026-05-20 起；v0.3.0 tag 2026-07-19） | **刪掉**（`afb7e943`）。`?sort=` 仍被解析（`v0.3.0:src/pages/explorer/url-state-derivations.ts:103-111`），但 UI 沒有地方寫它               | 同上加 `date`、`semanticCursor`                                                                                                                                      | 2026-05-25 依用戶回饋補回（`58ff5f1b`），8 個例子含 `tag:`、`note:`、`is:starred`（`v0.3.0:src/components/explorer-paper/paper-advanced-search-help.tsx:29-40`） | Keyword / Regex / Smart 三個分頁；hint「Rust regex — no lookaround or backreferences」；`isRustRegexCompatible`             |
| v0.4（M18，2026-10-02 起）                          | 無。`queries.ts:137` 寫死 `sort: mode==='full' ? 'relevance' : 'newest'`；時間線 `sort:'newest'`（`:119`）；語意搜尋不送 sort（`:166-176`） | `q`、`mode`、`view`、`date`、`browser`、`domain`、`visit`                                                                                                            | 速查表 11 個例子（`search-help.tsx:30-42`），**點了會換掉整個搜尋框**、非 regex 例子切回全文（`src/features/history/history-page.tsx:375-379`）                  | 模式 toggle、`/…/` 自動當 regex、Rust 語法檢查                                                                              |

v0.2 排序的三語文案（`v0.2.0:src/lib/i18n/catalog/explorer.ts:158-161,337-340,515-518`）：

|           | en           | zh-CN      | zh-TW      |
| --------- | ------------ | ---------- | ---------- |
| 標籤      | SORT         | 排序       | 排序       |
| relevance | Relevance    | 相关性优先 | 相關性優先 |
| newest    | Newest first | 最新优先   | 最新優先   |
| oldest    | Oldest first | 最早优先   | 最早優先   |

v0.2 另外有「最近搜尋」chip：localStorage、最多 4 筆、連 sort 一起存（`v0.3.0:src/pages/explorer/hooks/use-explorer-url-state.ts:149-172`）。這對應 M1 的「為常用查詢保留 recent filter slot」（`m1-solid-archive/explorer-export-and-onboarding.md:60`）；v0.4 沒有這個功能。

### 2.2 v0.3 的半套 builder

- **篩選條**（`v0.3.0:src/components/explorer-paper/paper-filter-strip.tsx`，2026-05-22 補回，commit `0a6e437a`）：
  - 已套用的篩選顯示成「LABEL value ×」chip。
  - 「+ Filter」打開一個 popover 表單：網站文字、瀏覽器、profile（少於 2 個選項時隱藏）、起訖日、regex 勾選；按 Apply 一次寫進 URL。
  - **沒有排序。**
- **搜尋框下的篩選 chip 列**（`v0.3.0:src/components/explorer-paper/paper-search-hero.tsx:69-82,405-460`）：
  - `parseActiveSearchFilters` 把 `q` 裡的已知運算子 token 投影成可移除的 chip，含負號與引號；`removeFilterToken` 依 token 索引精準刪除（`v0.3.0:src/pages/explorer/paper-search-filters.ts`）。
  - 「+ Tag / + Note」呼叫 `appendOperator`，在查詢尾端加上 `tag:` 並把游標放在後面（`v0.3.0:src/pages/explorer/paper-search-panel.tsx:167,211,230-234,283-284`）。
  - 「+ Date / + Source / + Domain / + Visit count」沒有 handler，以停用狀態顯示。當時寫明等「broader filter-chip wiring pass」（commit `d1135f5d`；`30d4d7ce` 的 STATUS 記錄「deferred」）。
- **意外**：v0.3 的全文和 regex 結果在 UI 上**依日分組、新的日子在前、同日內依時間排序**（`v0.3.0:src/pages/explorer/paper-search-helpers.ts:169-205`；`paper-search-panel.tsx:257,265`）。後端預設送 relevance，前端卻重排成時間順序。也就是說，從 v0.3 到 2026-10 redesign，跨日的相關度順序在畫面上看不出來。只有 Smart 模式保留後端順序（`:267-272`）。
- v0.3 的搜尋要按 Search 或 Enter 才送出，起因是「每打一個字送一次查詢」造成卡頓（commit `5fd2f1de`，2026-06-23）。v0.4 改回打字停 250 ms 後自動送出（`recall.md:40`、`history-page.tsx:53`）。builder 每改一次 `q` 就會開一個新 session。
- v0.3 的 Starred hub 有 `recently_starred` / `most_revisited` 排序，但只在一個有上限的視窗裡排（`BACKLOG.md:790-796`）。

### 2.3 Prototype

- **v0.3 paper prototype**（已退役：`docs/design/handoff/README.md:3`）：`pk-search.jsx:118-130` 有 Filters 列「+ Date / + Source / + Domain / + Visit count」；`:14-19` 的範例提示含「Pages I revisited 3+ times in the last 90 days」。沒有排序。
- **M18 prototype**（現行參考）：`docs/design/prototype-2026-10/PathKeep App.dc.html:810` 只有 full / regex / semantic 三種方式；placeholder 是「/regex/ works too」（`:687-688`）。沒有排序，也沒有 builder。

### 2.4 2026-10 redesign 為什麼沒有排序

**沒有任何文件寫理由**，`1ccbae40`、`4bbd8d62`、`cd866854` 的 commit 訊息都沒提。相關的事實：

- `recall.md:51-61` 的「已接受要求中 2026-10 介面沒有做到的」清單**沒有列出排序控制**（文件漂移）。
- 先例一：2026-10-04 用 commit `06373925` 補回「accepted docs 列了但 v0.4 缺」的 History 工具（速查表、Rust regex 檢查）。
- 先例二：用戶說「UI 本身就缺很多東西…你補上就行」（`STATUS.md:57`）。
- 先例三：「用戶 2026-10-10 決定：定案文檔要求、v0.4 沒做的都補做」（`STATUS.md:32`）。不過當時具體只列了第 8–10 項。

### 2.5 v0.4 裡其他相關前例

- **Insights「熱門網站」的排序**：shadcn `Select`，選項為瀏覽次數 / 去過的天數 / 每天瀏覽次數；aria-label「Order sites by / 網站排序方式」（`insights-cards.tsx:240-258`、`src/lib/i18n/messages/insights.ts:151-156,1121-1126`）。
- **網站視圖**依瀏覽次數排序（`recall.md:39`）。
- **Ask 的 agent** 支援 `sort` 加游標（commit `adefe995`）。
- **MCP 的 `search-history` 把 `sort` 寫死為 `None`**（`src-tauri/crates/vault-worker/src/mcp.rs:302-314`）。`STATUS.md:315` 說「用最舊排序的是 Ask 與 MCP」，這句不準確：MCP 只有 3 個工具。
- **匯出**：`ExportRequest` 收完整的 `HistoryQuery`（`src-tauri/crates/vault-core/src/models/archive.rs:936-940`），後端已經能照搜尋條件匯出。

---

## 3. 有記錄的用戶需求與抱怨

1. **2026-06-24，「我第一次看 mlx 是什麼時候」**：agent 只拿到最近 8 筆，就說「第一次是 6 月 18 日」，但用戶有 2025 年的紀錄。之後才給 agent 加了 oldest 排序和分頁（commit `adefe995`）。
2. **2026-10-12，「我第一次看 smol 是什麼時候」**：oldest 照每頁最近一次造訪排，回來的全是最近兩週的頁。2026-10-13 修好（`STATUS.md:310-315`、`CHANGELOG.md:2870-2873`、commit `4ad0c16e`）。E2E 只驗了 Ask（`tests/e2e/ask-chats.spec.ts:635-680`），**沒有 History 的版本**，因為畫面上沒有這個選項。
3. **2026-10-04，用戶兩項要求**：搜尋必須覆蓋整個 archive，並且分段串流呈現（`large-result-search-tradeoff.md:12`、ADR-012 `:20,54`）。同日決定搜尋「一頁一列」（commit `ff41390c`）。
4. **2026-05-07，「某個 domain 下不包含某個關鍵詞」**：要求做成關鍵字語法，「而不是要求使用者寫負向前瞻正則」。另外修了前端把 JS 才支援的 look-around 誤送後端的問題（`CHANGELOG.md:1317-1327`）。結果是 `site:` 加 `-詞`。
5. **2026-05-07，語法要讓人看得到**：「不需要先猜語法或閱讀文檔」（`CHANGELOG.md:1329-1340`）。
6. **feedback-2026-05-25**：
   - §3.3 B：paper redesign 拿掉了語法提示，「the feature is effectively invisible」（commit `58ff5f1b`）。
   - §3.3 A：搜尋要能找標籤與筆記（`38e4fced`）。用戶接著要「C. 補完 3.3A 的優化」，才有 + Tag / + Note chip（`d1135f5d`）。
   - §3.2：搜尋零結果或 regex 錯誤時整個輸入框被換掉、無法繼續修改（`65e947f8`；記錄在 `093add7d` 的 STATUS 變更）。
   - 回饋原文 README 從沒進 repo，git 歷史裡找不到。
7. **regex 方言**：JS `RegExp` 會擋下 Rust 能接受的 `a++`、`(?i)`、`(?P<name>…)`（`recall.md:42`）。2026-10-04 改成以 Rust 為準（`CHANGELOG.md:2597`）。
8. **沒有找到**任何用戶抱怨「History 沒有排序」的紀錄。查過 STATUS、BACKLOG、CHANGELOG 和 commit 訊息，中英文關鍵字都搜了。

---

## 4. `recall.md` §1 在 v0.4 未完成、這次工作可以順帶滿足的項目

| 要求（出處）                                                                      | 可行性                                                                                                                                                                                                                                                                            |
| --------------------------------------------------------------------------------- | --------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| 依**單一 profile** 篩選（`recall.md:57,116`）                                     | **高**。`SearchRequest.profileId` 已存在，並保證和日期落在同一次造訪（`search.md:7,39`）；AI 搜尋也收 `profile_id`。v0.4 只送 `browserKind`（`queries.ts:134-143`），只需前端加 URL 參數（Insights 已有 `profile=`，`screens-and-nav.md:43`）。限制：只能單選。                   |
| 依**匯入批次**篩選（`recall.md:57,121`）                                          | **中**。`visits.import_batch_id` 有局部索引 `idx_visits_visible_batch_time_id`（`store-schema.md:149,160`），但 SearchRequest 和 parser 都沒有對應欄位，要在 session 的範圍和 eligibility 加同一次造訪的條件。可以從 Backup →「最近的匯入」直接連過來。                           |
| 依**備份 run** 篩選（同上）                                                       | **中高**。`visits.created_by_run_id` 沒有索引（`store-schema.md:143`），14.4M 時要先加索引並量測。                                                                                                                                                                                |
| 依**來源途徑**篩選（`recall.md:57,120`）                                          | **中高**。`transition` 是每次造訪的原始瀏覽器代碼（`search.md:119`），`visits` 上沒有索引。Insights 的「怎麼來到這裡」用的是 `from_visit`，不是 transition，而且 Safari 和 Takeout 不記錄 opener（`deterministic-intelligence.md:314-321`）。得先定義類別，並誠實處理「沒記錄」。 |
| 依**頁面類型**篩選（`recall.md:57,119`）                                          | **高難度**。頁面種類放在 Intelligence DB 的 `visit_derived_facts`（`store-schema.md:328`），會隨規則版本和 override 重建，多數頁面是 unknown（`deterministic-intelligence.md:196-204`）。要跨 DB，或者投影進 search documents。                                                   |
| 篩選狀態以標籤展示、可逐個移除（`recall.md:123`；不在未完成清單，但只做了一部分） | **高**。v0.4 只有網站 chip；寫在 `q` 裡的運算子不會變成 chip。可以把 v0.3 的 `parseActiveSearchFilters` 和 `removeFilterToken` 移植過來。                                                                                                                                         |
| 從 History 直接匯出目前篩選結果（`recall.md:60,182`）                             | **中低**。`ExportRequest.query` 已經是完整的 `HistoryQuery`（`models/archive.rs:936-940`），搜尋匯出會走到底、可以取消（`search.md:129`）；儲存對話框也已做好（`STATUS.md:349`）。缺的只是 History 裡的入口。                                                                     |
| 可選顯示欄位，如造訪次數（`recall.md:58,153`）                                    | **低關聯**。「最常造訪」排序會用到符合的造訪次數，但現在列上已經顯示了（`screens-and-nav.md:70`）。                                                                                                                                                                               |
| 時間軸 rail、分頁列（`recall.md:55-56`）                                          | 和這次工作無關。分頁列和 `ui-review-guardrails.md:134` 衝突，該處說舊的分頁紅線已不適用。                                                                                                                                                                                         |

---

## 5. 意外、文件漂移與陷阱

1. **語意搜尋選 newest / oldest 會直接改走全文**（`src-tauri/crates/vault-core/src/ai/search.rs:64-68`；`ai/search_pages.rs:15-48`；`models/intelligence.rs:1572-1582,1654-1657`）。feature 文件沒寫這件事。History 目前也不讀語意搜尋的提示代碼（`BACKLOG.md:79-81`），如果在語意模式露出排序，使用者看不出結果已經不是語意排名。
2. **新排序沒有 14.4M 的量測。** 基準只量了 relevance（`docs/architecture/research/2026-10-vault-core-search-benchmark.md:24-35`：常見詞最終第一頁 cold 約 8.3 s）。newest / oldest 和 Turso 時代的 regex 都沒有記錄。`ipc-performance.md` §1、§6、§7 是 SQLite 時代的數字，`regex_scan.rs` 已經刪除（`2026-10-search-integration-validation.md:189`）。
3. **命名撞名**：Settings → AI 已有「Search ranking / 搜索排序 / 搜尋排序」，指 RRF 和權重這些混合排名參數（`src/lib/i18n/messages/settings-ai.ts:283,574,862`；`screens-and-nav.md:84`）。History 若加一個「排序」，中文標籤會和它撞名。
4. **`lexical-recall-v2.md` 有幾段過時**：154-156 行還寫 FTS5 BM25，162-165 行還寫 `r|score|visitTime|id` 游標。現在的游標是 opaque、綁 session 的（`search.md:127`）。排序語意本身仍然正確。
5. **`recall.md:113` 仍寫 `?regex=1`**，現行是 `?mode=regex`（`recall.md:135`）。v0.2 / v0.3 的深連結參數（`sort`、`regex`、`profileId`、`start`、`end`）在 v0.4 都不會被解析（`history-params.ts:32-45`）。
6. **BACKLOG 有過時條目**：
   - `:703`「History regex recall 無界」：已修好，相關程式已刪。
   - `:125` 提到 `useExplorerUrlState` 匯出的函式：該檔已不存在。
   - `STATUS.md:31` 指向的 `WORK-REDESIGN-SPEC-GAPS` 在 BACKLOG 裡不存在（只有 `cd866854`、`16e3688a` 引用過這個名字）。
7. **速查表沒列 `is:starred`**，`recall.md:41` 的運算子清單也沒列。後端支援（`search/query.rs:44-47`），v0.3 有列。
8. **相關 backlog**：
   - `WORK-SEARCH-NEAR-DUPLICATES`（`BACKLOG.md:68-70`）會改變「一列」代表什麼，連帶影響「最常造訪」的計數語意。
   - `WORK-HISTORY-SEARCH-NOTES`（`:79-81`）見第 1 點。
9. **現有 E2E 可以當起點**：
   - `tests/e2e/search-ranking.spec.ts`：relevance 順序。
   - `tests/e2e/search-limits.spec.ts:43`：regex 找得到最舊的造訪。
   - `tests/e2e/history-tools.spec.ts:923`：Search tips。
   - `tests/e2e/ask-chats.spec.ts:635-680`：Ask 用 oldest 找第一次。
