# History 搜尋的 E2E 測試現況：為「結果排序」與「進階搜尋 builder」做準備

範圍：`tests/e2e` 裡跟 History 搜尋有關的 spec、`support/` 下的 helper、`playwright.config.ts`、兩支啟動腳本、合成 fixture。我沒有改任何 repo 檔案，也沒有跑 E2E：`var/playwright/` 不存在，跑一次就是 Rust debug 冷建置，而磁碟只剩 9.6 GB。fixture 的形狀是用 scratch 腳本直接呼叫純函式 `generateVisits()` 量的，那個函式不寫檔。

## 0. 結論

- **新 spec 放哪：** 只讀 fixture 的放 `read`。單獨跑時只會先帶 `first-run` 一條。要自己造頁面（新增造訪）的只能放 `change` 或更後面，還要改 `wipe.spec` 的造訪總數。
- **等結果穩定的現行做法：** 輪詢結果上方那行，直到它剛好是 `N pages · M visits · <mode>`。這招在「換排序、不換查詢」時沒用，因為那行字前後一模一樣，舊的列還以 placeholder 的身分留在同一個 `History results` listbox 裡。
- **newest / oldest 能從 fixture 精確算出：** 所有造訪時間戳都是唯一的，碰不到 page id 的平手規則。
  - relevance 一般算不出來（Tantivy BM25 加十個欄位的權重）。
  - 例外：**沒有關鍵字的查詢**（只有 `site:` `filetype:` `after:` `before:` `note:` `-詞` `is:starred`）會編成 `AllQuery`，所有頁同分，所以 relevance 必須完全等於 newest。這是唯一能直接從 fixture 算出來的 relevance 檢查。
- **fixture 恆為 100 頁，剛好等於搜尋的 `PAGE_SIZE` 100：** 只用 fixture 永遠到不了第二頁，任何排序下的 cursor 續頁都沒有 E2E 覆蓋。
- **後端已支援 `relevance | newest | oldest`：** History 畫面只送 relevance（全文）或 newest（regex）。most-visited 根本不存在，而且後端遇到不認得的 sort 字串會**默默**當成 relevance 處理。

---

## 1. Suite 架構：一個 backend、一個 archive、串成一條鏈的 projects

- **一個 webServer。** 跑的是 `bun run desktop:dev:bridge`；Linux 沒有 `DISPLAY` 時會自動加 `xvfb-run -a`（`playwright.config.ts:106,136`）。
  - 它等的是 `${devIpcUrl}/health`，`reuseExistingServer: false`，逾時 900 s（`playwright.config.ts:135-143`）。
  - E2E 時有 `PATHKEEP_E2E_RESTART_FILE`，腳本不經 `tauri dev`，依序做這幾件事（`scripts/pathkeep-dev-desktop-bridge.mjs:43-48,95-157`）：
    - `prepare:worker-sidecar --profile debug`
    - 啟動 Vite
    - `cargo build --features devtools-bridge`
    - 自己顧著 app
- **全域設定（`playwright.config.ts:110-134`）：**
  - `workers: 1`、`fullyParallel: false`、`retries: 0`
  - test 逾時 120 s，expect 逾時 15 s
  - `locale: 'en-US'`、viewport 1440×900
  - `trace: 'on'`、`screenshot: 'on'`、`video: 'off'`
- **fixture 只在 runner 進程建一次（`prepareFixture`，`playwright.config.ts:27-90`）。**
  - 位置在 `os.tmpdir()/pathkeep-e2e-*`，seed 是 `FIXTURE_SEED = 7`（`:25`）。
  - `now = Number(PATHKEEP_E2E_NOW) || Date.now()`（`:35`）。
  - archive、keyring、project root、OS sandbox 都放在同一個根目錄下。
  - 寫出 `fixture.json`，並複製一份到 `artifacts/e2e/fixture.json`（`:48-54`）。
  - 透過環境變數傳給 worker 與 backend：`PATHKEEP_E2E_FIXTURE`、`CHB_*`、`PATHKEEP_TEST_BLOCK_NETWORK=1` 等（`:56-84`）。
  - worker 一看到 `PATHKEEP_E2E_FIXTURE` 就直接 return（`:32`）。
- **projects 用 `dependencies` 串成單鏈**（`playwright.config.ts:144-209`）：
  `first-run → read → zone → change → restart → stop → move → wipe → upgrade`
  - 前面 spec 留下的狀態會帶給後面的 spec。例：`lock.spec` 讓 App Lock 一直開著（`lock.spec.ts:15`）；`wipe.spec` 把各 spec 新增的造訪全部加總（`wipe.spec.ts:72-86`）。
- **同一 project 內按檔名順序跑，不照 `testMatch` 列的順序**（`playwright.config.ts:164-165`）。
  - Playwright 1.63 收集檔案時用 `entries.sort((a,b)=>a.name.localeCompare(b.name))`（`node_modules/playwright/lib/runner/index.js:2287`）。
  - 在 localeCompare 下 `-` 排在 `.` 前面，所以 `history-tools.spec.ts` 先於 `history.spec.ts`。
- **每個 project 都是寫死的檔名清單**（`testMatch: files`，`playwright.config.ts:92-101`）。新 spec 沒加進任何清單，就**完全不會跑**。

## 2. Spec 的寫法慣例

### 2.1 打開 History、輸入查詢

- **打開：** `page.goto('/#/history')`（hash router，baseURL 是 Vite dev server，`playwright.config.ts:124`）。
  - 現有 deep link：`?q=`（`history-tools.spec.ts:334,466`）、`?mode=regex`（`:775`）、`?q=tokio&mode=semantic`（`:939`）、`?date=`（`shell-and-lists.spec.ts:83`）。
  - URL 參數只有 `q/mode/view/date/browser/domain/visit`，**沒有 sort**（`history-params.ts:17-45`）。
- **輸入：** `getByRole('searchbox', { name: 'Search history' }).fill(q)`。打字停 250 ms 後才寫進 URL 並查詢（`history-page.tsx:53,65-76`）。
- **模式：** `getByRole('radio', { name: 'Regex' })`。
  - 模式切換**只在搜尋框有字時才出現**（`history-toolbar.tsx:269-276`；`search-limits.spec.ts:52-54` 的註解也這樣寫）。
  - 用 `/…/` 包住的查詢，不管選了哪個模式都當 regex（`history-params.ts:168-188`）。
- **篩選：**
  - 瀏覽器：按 `/^Browser filter: /` 按鈕，再選 `menuitemradio` 的 `Firefox` / `All browsers`（`history-tools.spec.ts:583-589`）。`history.spec.ts:112-118` 的寫法是 `/All browsers/` 加 checkbox-或-radio。
  - 日期：按 `/^Date filter: /`，選 `menuitem 'Custom range…'`，在日曆點 `[data-day="M/D/YYYY"]`、`previous month`、`Apply`（`pickDays`，`history-tools.spec.ts:567-581`）；或直接選 `menuitemradio 'Last 7 days'`（`:662-663`）。
- **語法速查：** 按 `button 'Search tips'`，在 `dialog` 裡點一個例子，然後 `toHaveValue`，再驗結果（`history-tools.spec.ts:919-936`）。
  - 這是目前唯一碰到運算子 UI 的 E2E，也就是 builder 的直接前身。
  - 例子表在 `search-help.tsx:30-42`。選了例子會 `setDraft`，非 `/` 開頭的例子還會切回全文（`history-page.tsx:375-379`）。

### 2.2 等串流結果穩定

- **傳輸：** 全文與 regex 搜尋都走 `POST /commands/start_search`；dev bridge 再用 SSE `/searches/{id}/events` 推送每一步（`src/lib/ipc/search-stream.ts:64-71`）。
- **結果上方那行（`results-line.tsx:40-64`，英文字串在 `i18n/messages/history.ts:55-59`）：**
  - 還在數：`N+ pages · mode`
  - 數完：`N pages · M visits · mode`
  - 排序還沒定：後面再加 ` · Still ranking`
  - 畫面上是舊 session 的列（placeholder）時：整行不顯示
- **慣用閘門 `expectResults`：** 用 `expect.poll` 讀那行，正規式是 `^([\d,]+) pages? · ([\d,]+) visits? · ${mode}$`，錨定在 mode 結尾。等於一次等三件事：不是 placeholder、頁數與造訪數都已精確、不再 Still ranking。
  - 出處：`history.spec.ts:45-72`（有 mode 參數）、`history-tools.spec.ts:83-104`（只有 Full text 版）。
  - `search-limits` 用的是 `getByText(exact)` 加 30 s，並斷言畫面上沒有 `N+ pages`（`search-limits.spec.ts:34-40`）。
- **search-ranking 的做法不同：** 不等那行字，直接用會自動重試的 `toHaveText(titles.map(t => new RegExp(t)), {timeout: 30_000})`，一次等到「列數與順序都對」（`search-ranking.spec.ts:26-35`）。
- **空結果的兩種說法**（`search-limits.spec.ts:87-96`、`history-page.tsx:283-304`）：
  - `'Nothing found'`（exact）
  - 索引還在追趕時是 `'Nothing found yet'`，另有 status：`Some recent changes aren’t searchable yet…`
- **傳輸層 seam：**
  - `page.route('**/commands/start_search')` 扣住請求，製造「搜尋載入中」狀態（`search-loading.spec.ts:27-31`）。
  - `load_history_og_images` 也用同樣做法（`history-tools.spec.ts:834-837,884-895`）。

### 2.3 依序讀出結果列

- **結果 listbox：** `getByRole('listbox', { name: 'History results' }).getByRole('option')`；沒在搜尋時名稱是 `History timeline`（`history.spec.ts:74-75`；`i18n/messages/history.ts:61-62`）。
- **一列的內容（`timeline-rows.tsx:165-232`），依序是：**
  1. favicon：`<img alt="">`，或 aria-hidden 的字母徽章（`favicon.tsx:21,46-47`）
  2. 標題（沒有標題就用 URL）
  3. `{domain} · {dayAndTime}`
  4. `N visits`
  5. 有星時一個 `img 'Starred'`
  6. 裝飾用的瀏覽器圖示

  textContent 之間沒有分隔，字母徽章會直接黏在標題前面，所以 `^標題` 這種錨定不可靠。現行做法都是子字串比對：`filter({ hasText })`，或 `new RegExp(title)`。

- **列上的時間 `dayAndTime`**（`format.ts:214-221`）：今天、昨天寫成 `Today` / `Yesterday`，其他寫 `MMM D` 加時間，**沒有年份**。
  - 時鐘格式跟著 Settings 走。`clock.spec` 最後設回 Match system，在 Linux 英文介面是 12 小時制（`clock.spec.ts:46-49`）。
  - 所以現有斷言只比 `:MM`：`expectRow`（`history-tools.spec.ts:591-614`）；`rowOf` 也是靠分鐘區分三個同標題的網址（`:679-687`）。
- **虛擬化：**
  - 固定列高，搜尋結果每列 54 px（`timeline-model.ts:160-166`）；overscan 12（`virtual-rows.tsx:50-56`）。1440×900 下大約只 render 前 20 多列。
  - 離底 10 列以內會觸發下一頁（`virtual-rows.tsx:60-63`），每頁 100（`queries.ts:52`）。
- **目前只有 search-ranking 驗 UI 上的順序（2 列）。**
  - `history.spec` 和 `history-tools` 只驗「每個標題剛好一列」與總數，跟順序無關。
  - ask-chats 的 oldest 測試讀的是送給假模型的工具輸出，不是 UI（`ask-chats.spec.ts:639-672`）。

### 2.4 在報告裡記錄 `{ expected, actual }`

- `expectCount(testInfo, name, actual, expected)`：先 attach `{ expected, actual }` 的 JSON，再 `toBe`（`fixture.ts:128-143`）。只收數字。慣例是 poll 通過後才 attach（`history.spec.ts:70-71`）。
- 非數字的直接用 `testInfo.attach` 放 JSON 或文字（`history-tools.spec.ts:857-860,867-870`；`ask-chats.spec.ts:420-423,643-646`）。
- **search-ranking 完全沒有 attach**，所以順序檢查在報告裡沒有 `{expected, actual}`。目前沒有針對有序清單的 helper。

### 2.5 其他慣例

- **檔頭：** 每個 spec 都有 docblock，寫情境再加 `Proves:` 條列（例：`history.spec.ts:1-10`）。test 標題是一句行為描述。
- **先確認 fixture 真有所需的形狀：** 例如 `expect(everywhere.visits, 'the fixture must revisit the page').toBeGreaterThan(1)`（`history.spec.ts:90-94`）。
- **守衛：** 每個運算子必須真的改變答案，否則忽略它的實作也會過（`history-tools.spec.ts:532-552`）。
- **先負後正：** `TESTING.md:37`；`quality-matrix.md:39`；例子在 `history-tools.spec.ts:333-335`。
- **`backend()` 只用來檢查 UI 看不到的副作用**（`fixture.ts:6-7`）。
  - 例：輪詢 `get_url_annotation` 等筆記存好（`history-tools.spec.ts:119-128`）。
  - 用 backend 改的狀態要放在 try/finally 裡還原（`search-limits.spec.ts:80-121`）。
- **沒有 page object：**
  - `expected` / `expectResults` / `results` / `searchBox` / `detail` 在 `history.spec.ts:24-84` 和 `history-tools.spec.ts:57-104` 各寫了一份。
  - `escapeRegExp` 只存在於 `ask-chats.spec.ts:432-433`。

## 3. 新 History 搜尋 spec 該放哪個 project

### 首選 `read`（前提：只讀）

**理由：**

1. 這時 archive 裡的造訪剛好等於 `archivedVisits()`，`read` 裡沒有任何 spec 新增造訪。緊接在後的 `zone` 會數 fixture 的造訪（`playwright.config.ts:166-167`），所以 `read` 的 spec **也不能**新增造訪。
2. 單獨跑時只會先帶 `first-run` 一條。
3. 這時語意搜尋是關的（`history-tools.spec.ts:938-945`），只有全文與 regex。

**要做的事：** 把檔名加進 `playwright.config.ts:146-163` 的清單。

**`read` 的執行順序與候選檔名的位置：**

- 現有順序：`clock → export → history-tools → history → home → insights-counts → insights-drill → insights → links → search-limits → search-loading → shell-and-lists`
- 候選檔名排序後的位置：
  - `history-builder` < `history-order` < `history-search` < `history-tools` < `history.spec`
  - `search-builder` < `search-limits` < `search-loading` < `search-order` < `search-ranking` < `search-sort`

**前面的 spec 留下、從不清除的使用者資料（會影響全文搜尋與 `is:starred`）：**

- **筆記（全文也會搜筆記）：**
  - `history.spec.ts:179`：HN 那頁「Why I left tokio」的 `read again before the runtime rewrite`
  - `history-tools.spec.ts:171`：Strava run 的 `wombat sighting`
  - `history-tools.spec.ts:149`：Strava log 的 `设计系统`
  - `history-tools.spec.ts:248,250`：兩個用 `pageOf` 挑出、在 Personal profile 的頁，`wallaby first` / `wallaby second`
  - `history-tools.spec.ts:426`：jlptsensei 文法頁的 `pangolin ledger`
  - `history-tools.spec.ts:720`：VACUUM 那個帶 fbclid 的網址，`run it after the big import`
- **星號：**
  - HN 那頁（`history.spec.ts:174`）
  - `pageOf` 挑出的 work / personal 兩頁（`history-tools.spec.ts:259-286`）
  - VACUUM 的三種網址寫法（`:697`）
- 只有 `search-limits` 會清掉自己寫的筆記（`search-limits.spec.ts:120`）；標籤全部都有移除。
- 結論：全文查詢要避開 `runtime`、`first`、`run`、`after`、`before`、`import` 這些詞，或者把上面那幾頁算進期望值。

### 要自己設計頁面時：`change`

- **做法照 search-ranking（`search-ranking.spec.ts:40-61`）：**
  1. `appendChromeVisits(fixture().files['chrome:Default'], visits)`
  2. 打開 `/#/backup`，按 `Back up now`
  3. 等 `Recent runs` 第一列出現 `N new visits`
- 在 `support/` 匯出一個 `xxxVisitCount()`，並加進 `wipe.spec.ts:72-86`（照 `ranking-pages.ts:43-45` 的寫法）。
- `lock.spec` 已讓 App Lock 開著（`lock.spec.ts:15`）。導航後呼叫一次 `unlockIfLocked(page)`（`app.ts:17-27`），成本很低。
- 想重用 ranking pages 的話，檔名要排在 `search-ranking.spec.ts` 之後：`search-sort` 可以，`search-order` 不行。或者直接擴充 `search-ranking.spec.ts`。
- 單獨跑會連帶跑 `first-run`、整個 `read`、`zone`，很慢。

### 語意模式的排序

- 只有 `restart` project 的 `semantic-index.spec` 有可用的語意索引（接的是假 embeddings 服務）。
- History 的語意請求目前根本沒帶 sort（`queries.ts:161-179`）。

## 4. 期望值怎麼從 fixture 算

### 4.1 可用的 helper

- **`fixture.ts`：**
  - `fixture()`：讀 `fixture.json`
  - `archivedVisits()`：三個已歸檔 profile 的全部造訪（`:55-63`）
  - `fixture().visitsByProfile[id]`
  - `localDay(at)`：本機時區的 `YYYY-MM-DD`；E2E 跑在 UTC（`run-playwright.mjs:23`）
  - `expectCount`、`parseCount`、`backend()`、`openArchiveVisitTotal()`
- **各 spec 自己的：**
  - `expected(visits, predicate)`：回傳 `{pages, visits, titles}`，titles 已排序並以 URL 去重（`history.spec.ts:32-43`）
  - `pageOf()`：挑一個標題唯一的頁（`history-tools.spec.ts:181-215`）
  - `visitTo()`：`history-tools.spec.ts:403-408`
- **`FIXTURE_SCENES`：** 時間精確、可逐格比對的場景（`synthetic-browsers.mjs:237-319`）。
- **`siteOf()`：** `public-suffix.ts:61`。
- **`FIXTURE_CJK_WORDS`：** 只給 Insights 切詞用，History 的 spec 沒用到它（`synthetic-browsers.mjs:145-154`）。

### 4.2 比對方式的近似

- **一般關鍵字：** `${title} ${url}`.toLowerCase().includes(word)（`history-tools.spec.ts:501-502`）。
  - 後端實際上是「token 前綴」或「≥3 個字母數字時對壓緊文字做子字串比對」（`vault-core/src/search/query.rs:127-143`），所以 ≥3 字元的英數詞用 `includes` 近似是成立的。
- **`intitle:` / `inurl:` / `site:`：** 子字串。
- **`filetype:`：** 看 `new URL(url).pathname`。
- **`after:` / `before:`：** 以 `localDay` 比較，兩端都包含（`history-tools.spec.ts:503-524`；`history-params.ts:147-156`）。
- **筆記與標籤也在全文搜尋範圍內**（`history-tools.spec.ts:152-159`）。
- **瀏覽器篩選：** 必須用 `visitsByProfile`。fixture 裡只有場景造訪帶 `visit.profile`，其他造訪是用 hash 分配到 profile 的（`synthetic-browsers.mjs:855-864`；`.d.mts:11-12`）。篩選是以瀏覽器種類為單位，`chrome` 包含兩個 Chrome profile。
- **一個「頁」= 跨 profile 的一個不同 URL。** `search-limits` 的 `https` 測試已證實 archive 頁數等於 fixture 的不同 URL 數（`search-limits.spec.ts:21-41`）。

### 4.3 後端的排序規則

| 排序      | 排序依據                           | 列上顯示哪次造訪 | 平手時                                    |
| --------- | ---------------------------------- | ---------------- | ----------------------------------------- |
| relevance | score 由高到低                     | 範圍內最新一次   | 範圍內最新造訪由新到舊 → page_id 由大到小 |
| newest    | 範圍內最新造訪，由新到舊           | 那一次           | page_id 由大到小                          |
| oldest    | 範圍內第一次造訪，由舊到新         | 那一次           | page_id 由大到小                          |
| regex     | 只有 newest / oldest，score 固定 0 | 同上             | page_id 由大到小                          |

- **出處：** `collector.rs:11-28,84-90`；`session.rs:127-132,227-236`；`eligibility.rs:164-211,306-342`；`regex.rs:24-38,171-195,352-355`；`docs/architecture/search.md:11`；`docs/features/recall.md:14-16`。
- **History UI 目前送的值：** 全文送 `relevance`、regex 送 `newest`（`queries.ts:137`）；時間線永遠是 `newest`（`queries.ts:119`）；command palette 固定送 relevance（`command-palette.tsx:79`）。
- **oldest 目前只有 Ask / MCP 在用。** `STATUS.md:315` 寫明 History 沒有「最舊」排序，所以 E2E 只放在 `ask-chats.spec.ts:607-691`。那條測試的算法正好可以拿來重用：

  ```
  matches.sort((a, b) => a.at - b.at)
  → 每個 URL 只留第一次出現（ask-chats.spec.ts:611-628）
  ```

- **沒有 most-visited：** `SortOrder` 只有三種（`vault-search/src/filter.rs:118-125`）。每頁在範圍內的造訪數已經有了（`eligibility.rs:199-201`），列上的 `N visits` 顯示的就是它。

### 4.4 計算草稿

```ts
type Agg = {
  url: string
  title: string
  first: FixtureVisit
  last: FixtureVisit
  visits: number
}
function pagesMatching(
  scope: FixtureVisit[],
  match: (v: FixtureVisit) => boolean,
): Agg[] {
  const byUrl = new Map<string, Agg>()
  for (const v of scope) {
    if (!match(v)) continue
    const p = byUrl.get(v.url)
    if (!p)
      byUrl.set(v.url, {
        url: v.url,
        title: v.title,
        first: v,
        last: v,
        visits: 1,
      })
    else {
      p.visits += 1
      if (v.at < p.first.at) p.first = v
      if (v.at > p.last.at) p.last = v
    }
  }
  return [...byUrl.values()]
}
// scope：瀏覽器用 visitsByProfile 合併；日期 / after / before 用 localDay，兩端都包含
const newest = (ps: Agg[]) => [...ps].sort((a, b) => b.last.at - a.last.at) // 列顯示 p.last
const oldest = (ps: Agg[]) => [...ps].sort((a, b) => a.first.at - b.first.at) // 列顯示 p.first
// 無關鍵字的查詢：relevance 必須等於 newest(ps)
// most-visited（後端未實作）：b.visits - a.visits，平手規則要先訂（見 §5.3）
```

- 每一列的期望可以是：escape 過的標題，加 `:MM`，加 `N visit(s)`。
- 標題要先 escape。fixture 裡有 `|`（`Nikon Z f Mirrorless Camera Body | B&H`）和 `?`（`Async Rust: what is a runtime?`）；search-ranking 那種沒 escape 的 `new RegExp(title)` 用在這些標題上會錯。

### 4.5 relevance 怎麼驗

- **無關鍵字的查詢 → `Ast::And([])` → `AllQuery`，全部同分：**
  - 出處：`vault-core/src/search/query.rs:49-55` 會跳過 `site/filetype/ext/tag/note/after/before`；`:85`；`vault-search/src/query.rs:90`。
  - 所以 relevance 必須**完全等於** newest。
  - 例外：`tag:` 會加一個 Tags 欄位的子句，有分數（`query.rs:88-96`）。
- **有關鍵字的查詢：** BM25，欄位權重是 url 6 / title 12 / search_terms 4 / normalized_url 5 / normalized_title 10 / normalized_search_terms 4 / cjk 2 / enrichment 7 / notes 9 / tags 6（`vault-search/src/model.rs:47`）。造訪次數不進分數。在 spec 裡算不出確切順序。
  - 現行做法是自己設計頁面：`support/ranking-pages.ts:1-45`。
  - 它在各種排序下的鑑別力：

| 查詢              | relevance            | newest                      | oldest                              | 造訪次數                               |
| ----------------- | -------------------- | --------------------------- | ----------------------------------- | -------------------------------------- |
| `lighthouse`      | IN_TITLE, IN_ADDRESS | IN_ADDRESS, IN_TITLE        | IN_TITLE, IN_ADDRESS（= relevance） | IN_ADDRESS(4), IN_TITLE(1)（= newest） |
| `ferry timetable` | NEWER, OLDER         | NEWER, OLDER（= relevance） | OLDER, NEWER                        | 1 比 1 平手                            |

單看任何一個查詢，都分不出某兩種排序，所以一定要加 §2.5 那種守衛。

### 4.6 regex 怎麼算

- **比對：** `const re = new RegExp(pattern, 'i')`，`match = v => re.test(v.url) || re.test(v.title)`。
  - 只比網址與標題，不比搜尋詞、筆記、標籤（`regex.rs:175,231`）。
  - 不分大小寫（`regex.rs:357-367`）。
  - pattern 要挑 JS 與 Rust 語意相同的寫法：避開 `(?i)`，也注意 `\d` / `\b` 在 Unicode 下的差異。
- **兩條程式路徑，對排序都敏感，oldest 兩條都該測**（`regex.rs:18-27`；`search.md:11`）：
  - pattern 裡有 ≥3 個 ASCII 的必要字面字串 → 讀候選頁，每個 URL 取 `MIN` / `MAX`
  - 沒有 → 依排序方向每次走 512 筆造訪
- **以 now=2026-10-25T07:13Z 這份樣本為例：**
  - `\d{4}$`（無字面字串）：3 頁，newest 是 `HN | Strava run | tokio issue`，oldest 是 `Strava run | tokio issue | HN`。
  - `v2 eg`（有字面字串）：20 頁，兩種順序完全不同。
  - `^https://docs\.rs/`：兩種順序**一樣**，沒有鑑別力。

## 5. Fixture 的形狀（實測）

### 5.1 產生器

- **時間跨度：** 420 天每天都有，之後 730 天每 9 天一段、量是 0.15 倍（`synthetic-browsers.mjs:407-412`），總共約 3.15 年。
  - `TESTING.md:29` 說「約 21,000 筆、14 個月」，不準。
- **改寫搜尋：** 每 7 次搜尋就改寫兩次：先 `x v2`，30 s 後再 `x v2 eg`（`:454-475`）。
- **時間戳全部唯一**（`:497-505`）。
- **資料會隨 `now` 變動：** 星期幾會改變當天的量，也就改變了亂數流後面怎麼走。因此任何順序或次數都不能寫死（`quality-matrix.md:36`）。

### 5.2 在 2026-10-19 到 25 之間取 28 個 `now`（每天 01/07/13/23 UTC）

- 恆為 **100 頁**；造訪 22,140–22,969 筆；時間戳全部唯一。
- 頁面結構：

| 類別                                                 | 頁數 | 造訪數        | 第一次造訪        | 最近一次    |
| ---------------------------------------------------- | ---- | ------------- | ----------------- | ----------- |
| 主題頁（例：`smol - Rust`、`SQLite FTS5 Extension`） | 24   | 575–1,285     | 約 760–1,150 天前 | ≤ 3–8 天前  |
| 基本搜尋頁                                           | 20   | 36–76（樣本） | 840–1,150 天前    | 0.4–29 天前 |
| 改寫搜尋（20 對 `v2` / `v2 eg`）                     | 40   | 2–15          | 238–1,148 天前    | 2–163 天前  |
| 場景頁                                               | 16   | 1–12          | 見下              | 見下        |

- **「很久以前第一次、最近又回來」是主題頁的常態**，24 頁全都是。
- **時間精確的場景**（今天午夜往回算的天數，加當地時間）：
  - `Write-Ahead Logging`：5 次，跨三個 profile —— 14d 03:05 FF、12d 03:17 FF、11d 04:41 Personal、9d 03:29 FF、8d 04:53 Work
  - `First seen here`：20d 03:40 Personal；7d 05:10 與 05:40 Work（乾淨的「首訪久遠、近期回訪」）
  - `VACUUM · SQLite` 三個網址：都在 6d，03:10 / 03:50 / 04:30，同一個標題
  - `B-tree page format`：5d 03:20，全 fixture 唯一的 PDF
  - `简体中文搜索说明`：5d 04:10
  - FlexiSpot 兩頁：421d 05:00 起，分別 3 次與 2 次，之後再沒去過
  - Mio 兩頁：2 天前 21:00 起
  - 三種大小寫的 `SQLite WAL mode` 搜尋，以及 `news.ycombinator.com` 的地址搜尋：都在 10d

### 5.3 most-visited 需要的形狀

- **20 對 `v2` / `v2 eg`：** 在 28 個取樣裡**每一對**造訪數都相同，而且 `v2 eg` 的第一次與最近一次都晚約 30 s。可以拿來釘住平手規則。實際配對要從 fixture 找，例如樣本裡的 `marathon taper v2` / `v2 eg` 都是 15 次。
- **主題頁之間的造訪數通常不同，但不保證。** 也有完全平手的，例如樣本裡 VACUUM ×3 與 B-tree 都是 1 次。
- **瀏覽器篩選會改變次數：** WAL 在 Firefox 是 3 次，在 Chrome 是 2 次。

### 5.4 可用的鑑別查詢（樣本 now=2026-10-25T07:13Z，正式使用時請重算）

- **`site:www.sqlite.org`（無關鍵字，所以 relevance 應等於 newest），6 頁：**
  - newest：`FTS5 | B-tree | VACUUM 04:30 | 03:50 | 03:10 | WAL`
  - oldest：`FTS5（約 3 年前）| WAL 14d 03:05 | VACUUM 03:10 | 03:50 | 04:30 | B-tree`
  - 選 Firefox 後只剩 FTS5 與 WAL；oldest 時 WAL 那列顯示 14d 03:05、3 次。
  - 再加一個切掉 FTS5 歷史的日期範圍，FTS5 那列在 oldest 下就會移到範圍內的第一次。這正是 `WORK-SEARCH-FIRST-VISIT` 那類 bug。
- **`tokio`：** 10 頁，newest 與 oldest 不同，也不是彼此的反序。

## 6. 執行方式、時間、產物

- **指令：**
  - `bun run test:e2e tests/e2e/history.spec.ts`
    - 等同 `node scripts/run-playwright.mjs …`（`CHANGELOG.md:451`）；`BACKLOG.md:438` 用的是 `test:e2e -- <file>` 的寫法。
    - 腳本會強制 `TZ=UTC`（`run-playwright.mjs:23`）。不要直接跑 `bunx playwright test`。
  - `-g "<標題>"`：只篩選目標 project 的 test。
  - `--project=read`：跑 first-run 加整個 read。
  - `bun run test:e2e:headed`：看著它跑。
- **依賴 project 一律不受篩選、整個跑完**（`node_modules/playwright/lib/runner/index.js:2517-2523,2566-2604`）。`--repeat-each` 只作用在目標 project（`:2586`）。`--no-deps` 沒有用：每次都是新的 fixture，沒有 onboarding。
- **環境變數：**
  - `PATHKEEP_E2E_NOW=<fixture.json 的 now>`：重現同一份資料
  - `CARGO_TARGET_DIR`：預設 `var/playwright/desktop-bridge/cargo-target`，留著可以免去冷建置（`playwright.config.ts:81-83`）
  - `PLAYWRIGHT_CHROMIUM_EXECUTABLE_PATH`：`:131-132`
  - `PATHKEEP_DEV_SERVER_PORT` / `PATHKEEP_DEV_IPC_PORT`：固定連接埠；不設時隨機偏移 0–1999（`:85-89`）
  - `CI`：改用 github reporter，並開 `forbidOnly`
  - **不要手動設 `PATHKEEP_E2E_FIXTURE`：** `prepareFixture` 會直接 return（`:32`），`CHB_*`、keyring、sandbox、斷網、重啟檔全部不會設，debug backend 就會指向真實的瀏覽器資料夾與 project root。
- **時間：**
  - 唯一的紀錄在 `STATUS.md:308`（2026-10-12）：E2E 93 條 10.3 分鐘；整個 `bun run check` 19.2 分鐘（沿用編譯快取，乾淨建置另計）。
  - 現在大約有 113 個 `test(`（我數的）。
  - 冷建置要「several minutes」（`playwright.config.ts:139-141`）。
  - `first-run` 最多會等首次 Insights 建置 120 s（`first-run.spec.ts:141-142`）。
  - 沒有逐 spec 的時間紀錄。跑一次之後可以看 `results.json` 裡每條的 duration。
- **產物，在 `artifacts/e2e/`：**
  - `fixture.json`（seed、`now`、每個 profile 的每次造訪）
  - `report/`：HTML，附各 attachment
  - `results.json`
  - `test-results/`：每條都有 trace 與截圖
  - `thread-stacks/`：只在失敗時產生，Linux 用 gdb（`thread-stacks-reporter.ts:17,61-76`）
  - 查看方式：`bunx playwright show-report artifacts/e2e/report`、`show-trace …/trace.zip`
  - 下一次跑會清掉 `test-results/` 與 `report/`（`artifacts/e2e/README.md:6-29`）

## 7. 意外與風險

1. **regex「從不送出」的斷言其實什麼都沒驗到。** `history-tools.spec.ts:769-773` 監聽的是 `/commands/query_history`，但搜尋走的是 `/commands/start_search`（`queries.ts:111-159`、`search-stream.ts:64`）。`:789,796` 那兩個斷言恆真，真正有作用的只有「舊的列還在」。builder 如果會產生 regex，應該檢查 `start_search` 的 body（`{ request: { query, regex, sort } }`）。
2. **換排序、不換查詢時，現有的等待閘門失效。**
   - 舊 session 的列以 placeholder 的身分、在同一個 `History results` label 下調暗顯示（`use-search-stream.ts:7-8,110-117`；`history-page.tsx:334-338,347`）。
   - 結果那行在 placeholder 期間隱藏（`results-line.tsx:40-42`）。新 session 數完後，那行字跟換排序之前完全相同。
   - 畫面上唯一的區別是 CSS 的 `opacity-60`，沒有 `aria-busy`。
   - 所以順序要用會自動重試的斷言去等，例如 `expect.poll` 或 `toHaveText([...])`，並加守衛「每種排序的期望順序互不相同」。
3. **fixture 恆為 100 頁，剛好等於 `PAGE_SIZE`**（`queries.ts:52`）。後端是在 `hits > page_size` 才給 cursor（`session.rs:275`），所以只靠 fixture，任何排序下的續頁都測不到。要測得自己造 100 頁以上，而且只能在 `change`。
4. **後端遇到不認得的 sort 字串會默默當成 relevance**（`session.rs:127-131`）；regex 路徑只認 `"oldest"`（`:106`）。前端如果先送 `"visits"`，結果只是看起來沒壞。E2E 必須斷言實際順序。
5. **`resetKey` 沒有包含 sort**（`history-page.tsx:111`）：換排序不會捲回頂端。
6. **列上的日期沒有年份**（`format.ts:214-221`）：oldest 時，3 年前的那次只會顯示成 `Oct 15 2:05 PM`。E2E 只能比月、日、分鐘，跨年份是模糊的；這同時也是個 UX 問題。
7. **`ResultRow` 的註解說列上是「most recent matching visit」**（`timeline-rows.tsx:165-168`），在 oldest 下不成立。
8. **語意模式不帶 sort**（`queries.ts:161-179`），要驗只能在 `restart` 的 `semantic-index.spec`。
9. **文件與設定不一致：**
   - `artifacts/e2e/README.md:11`、`TESTING.md:47`、`quality-matrix.md:48` 都寫有影片，但設定是 `video: 'off'`（`playwright.config.ts:129`）。
   - `TESTING.md:31` 列的 project 鏈是舊的。
10. **`history-tools.spec.ts` 已有 952 行**；`repo-baseline.md:21` 只量 `src/` 與 `src-tauri/`，不量測試檔。

## 8. 可以直接用的情境種子（建議）

- **排序（`read`）：**
  - 用 `site:www.sqlite.org` 依序切 relevance → newest → oldest。
  - 斷言：
    - 每種排序下頁數與造訪數相同、列集合相同
    - 順序各自等於 fixture 算出的結果
    - relevance 等於 newest
    - oldest 下每列顯示範圍內第一次造訪的分鐘
  - 接著加上 Firefox 與日期範圍，驗「範圍內第一次」。
  - 再用一個有關鍵字的查詢（例如 `tokio`）驗 newest ≠ oldest。
- **regex + oldest：** 用 `v2 eg`（候選頁路徑，20 列）和 `\d{4}$`（逐筆造訪路徑）各跑一次。
- **排序切換時的載入狀態：** 照 `search-loading` 的做法扣住新排序的 `start_search`，斷言扣住期間不會把舊順序當成最終結果呈現；放行後出現新順序。
- **most-visited（等後端支援）：** 用一對 `v2` / `v2 eg` 釘住平手規則，再驗列上的 `N visits` 由大到小。
- **builder：** 在 UI 上組出 `site:docs.rs -smol`、`"async runtime"`、`intitle:`、`inurl:`、`filetype:pdf`、`vibrancy OR strava`、`site:strava.com after:… before:…`。
  - 搜尋框的值必須等於組好的字串（`toHaveValue`）。
  - 結果直接重用 `history-tools.spec.ts:497-565` 的 predicate 與守衛。
  - 含空格的標籤要加引號：`tag:"e2e fts"`（`:46-47,355-356`）。

我量 fixture 用了三支腳本，執行方式：`TZ=UTC node <腳本> [nowMs]`。前兩支會用 repo 的 `generateVisits()` 重做 profile 分配：

- `/tmp/claude-1000/-home-agent-PathKeep/862dae19-3719-411a-9608-44a1ac8899da/scratchpad/fixture-shape.mjs`（每頁的造訪數、第一次與最近一次、各種排序）
- `/tmp/claude-1000/-home-agent-PathKeep/862dae19-3719-411a-9608-44a1ac8899da/scratchpad/fixture-variance.mjs`（`now` 變動時形狀的變化）
- `/tmp/claude-1000/-home-agent-PathKeep/862dae19-3719-411a-9608-44a1ac8899da/scratchpad/regex-orders.mjs`（regex 的 newest 與 oldest 順序）
