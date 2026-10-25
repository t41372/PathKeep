# 語意（AI）搜尋路徑與所有排序消費者盤點

範圍：History 語意模式、`search_ai_history` 的後端路由、note codes、其他傳 sort 的呼叫者、進入 History 的 deep link。全程只讀，沒有改任何檔案。分支 `feat/search-order-and-builder`（HEAD `dc42cd7a`）。

## 0. 先看這幾條：意外的發現，以及會直接影響設計的事實

1. **後端收 sort，前端型別沒有這個欄位。**
   - Rust 的 `AiSearchRequest.sort: Option<String>`，帶 `#[serde(default)]`（/home/agent/PathKeep/src-tauri/crates/vault-core/src/models/intelligence.rs:1572-1582）。
   - TS 的 `AiSearchRequest` 沒有 `sort`（/home/agent/PathKeep/src/lib/types/intelligence.ts:753-769），History 語意分支也不送（/home/agent/PathKeep/src/features/history/queries.ts:166-176）。所以 History 語意今天一律是 relevance（hybrid RRF）。
   - TS 的 `AiSearchResponse` 也少了 Rust 有的 `appliedLimit`、`hasMore`（intelligence.ts:836-854 對照 models/intelligence.rs:1836-1843）。
   - repo 裡沒有 Rust→TS 型別生成，兩邊全靠手動同步。

2. **語意模式的 `newest` / `oldest` 其實是純關鍵字的日期列舉。**
   - `search_history_internal` 一看到 sort 是精確字串 `"oldest"` 或 `"newest"`，就在任何 embedding 和向量計算之前改走 `lexical_page`（/home/agent/PathKeep/src-tauri/crates/vault-core/src/ai/search.rs:64-69）。
   - 只有語意才找得到的頁全部消失，而且沒有任何 note 說明。
   - 回應標記：`providerId: "date-ordered"`、`model: "none"`、每列 `matchReason: "lexical-date-ordered"`（/home/agent/PathKeep/src-tauri/crates/vault-core/src/ai/search_pages.rs:34-47,70-71）。
   - 這是刻意的。`54eaf10b` 之前的註解寫明：用意思重排會毀掉使用者要的時間順序，所以完全繞過語意平面。

3. **語意 relevance 沒有相似度門檻（cutoff）。**
   - 每個向量都會計分（/home/agent/PathKeep/src-tauri/crates/vault-core/src/ai/vector_index.rs:67-104），範圍內每個有向量的頁都進結果。
   - 測試甚至要求反向相似的頁也要回來：/home/agent/PathKeep/src-tauri/crates/vault-core/src/ai/tests.rs:434-478 把一頁的向量設成查詢向量取負號，斷言它回來了，而且 `total == 1`。
   - 所以 `total` ≈ 範圍內已索引的頁 ∪ 關鍵字命中。History 顯示的「N 個頁面 · 語意」（queries.ts:237）不是「符合的數量」。
   - 對排序設計的意思：「語意結果按時間排」如果不先定義門檻，就等於「範圍內全部已索引頁按時間排」。
   - 門檻的先例：相關頁面用「隨機兩頁相似度的第 98 百分位」（/home/agent/PathKeep/docs/features/intelligence.md:102）。
   - 但已接受文件要求「搜尋必須覆蓋全部可用的向量與詞彙匹配頁面」（intelligence.md:93）。加門檻等於推翻已接受文件，需要 trade-off 文檔加用戶同意。

4. **`AiSearchSort::is_date_ordered` 沒有任何呼叫者。**
   - enum 和 `parse` / `is_date_ordered` / `list_history_sort` 三個方法只剩定義（models/intelligence.rs:1626-1670）。
   - `54eaf10b`（2026-10-07，Turso/Tantivy 遷移）把路由改成三處字串比對：
     - search.rs:65
     - search_pages.rs:18
     - /home/agent/PathKeep/src-tauri/crates/vault-core/src/search/session.rs:127-131
   - `1e0fadce`（2026-10-12）刪掉了最後用到它的測試。
   - 行為差異：`parse` 會 trim，現行比對不會，所以 `" newest "` 現在是 relevance。
   - 它的 doc 還寫著 sort 會「thread into `list_history`」，這條路已經不存在。
   - 未知值（例如 `"most_visited"`）在後端一律靜默變成 relevance，不報錯。

5. **Bug：`lexical_page` 不帶 `browser_kind`。**
   - search_pages.rs:19-29 沒有帶；hybrid 路徑的 /home/agent/PathKeep/src-tauri/crates/vault-core/src/ai/search_ranking.rs:38 有帶。
   - `22c6aafc` 加瀏覽器篩選時只改了 hybrid 路徑。
   - 後果：History 語意加上瀏覽器篩選時，只要落到 `lexical_page`，瀏覽器篩選就被默默忽略。落到 `lexical_page` 的情況是：用 newest/oldest，或 provider 解析失敗而且沒有任何星號。
   - 如果 History 語意加 newest/oldest，一定會踩到這條。

6. **History 一個搜尋提示都不讀。**
   - `noteCodes`、`notes`、`matchReason`、`providerId`、`model`、`enrichmentExcerpt` 全被丟掉（queries.ts:193-203）。
   - `SearchStep.searchNotes` 有型別，但沒人讀（/home/agent/PathKeep/src/lib/backend-client/search.ts:99）。
   - 語意說明行（/home/agent/PathKeep/src/features/history/semantic-status.tsx:44-66）讀的是快照裡的 `aiStatus`。它的 provider、已索引數、更新時間都是「所選」模型的，不是正在服務的那一份（/home/agent/PathKeep/src-tauri/crates/vault-core/src/ai/read_model.rs:93-111,136-151,178）。在 previousModel 的情況下，這行會寫錯模型和數字（見 §3）。

7. **沒有任何 deep link 帶 `mode` 或排序**，只帶 `q` / `date` / `domain` / `visit`（§5）。

8. **cursor 綁死排序；AI 搜尋和 `query_history` 的 session 永遠不釋放。**
   - cursor 的 fingerprint 是整個 request，包含 sort、limit、`ranking_key`（/home/agent/PathKeep/src-tauri/crates/vault-core/src/search/stream.rs:85,162-168）。換排序必定開新 session。
   - `page_from` 只在沒有 cursor 時才 cancel（/home/agent/PathKeep/src-tauri/crates/vault-core/src/search/history.rs:137-140）。
   - 前端只有串流路徑會呼叫 `cancel_search`（/home/agent/PathKeep/src/lib/ipc/search-stream.ts:54-60）。Ask、code mode、MCP 也都不釋放。全 repo 只有 bench 有 `release_cursor`（/home/agent/PathKeep/src-tauri/crates/vault-core/src/ai/semantic_stage_bench.rs:136-142）。
   - 因為第 3 點，語意 relevance 幾乎永遠有下一頁。所以每次語意搜尋都會留下：
     - 一條卡在 `rx.recv()` 的執行緒（stream.rs:102-131）
     - 一份寫滿的 Membership：newest 和 counts 每頁各 8 B（/home/agent/PathKeep/src-tauri/crates/vault-core/src/search/eligibility.rs:159-179,239-240）
     - 一份 Scores：每頁 5 B
   - 這些會一直留到 Vault 關閉；App Lock 不會關 Vault。2.88M 頁時（/home/agent/PathKeep/docs/plan/BACKLOG.md:113），讀碼估算每個 session 約 60 MB。**未量測。**排序切換會放大這個量。

## 1. History 語意分支（前端）

`useVisitList`（/home/agent/PathKeep/src/features/history/queries.ts:102-252）：

**進入條件**

- 條件是 `search.mode === 'semantic' && text !== ''`（107）。
- `resolveSearch` 在語意不可用時退回 full；用 `/…/` 包起來的查詢一律當 regex（/home/agent/PathKeep/src/features/history/history-params.ts:169-188）。
- 「可用」的定義：`ai.enabled && semanticIndexEnabled && aiStatus.ready`（/home/agent/PathKeep/src/features/history/semantic-info.ts:60-61）。後端的 `ready` 是「serving 有向量，而且所選服務可用」（read_model.rs:156）。

**送出的請求**（166-176）

- `query: search.text`：整串原文，運算子也在裡面。
- `domain`、`browserKind`。
- `startDate` / `endDate`：把毫秒邊界轉回本地日期。
- `limit: 50`：等於後端上限（search.rs:11,45-48）。
- `cursor`。
- 沒送的：`sort`、`profileId`、`starredOnly`。

**queryKey**

- `key('semantic', text, filters)`（163），掛在 `archiveData` 底下，所以備份會讓它失效。
- 但它不含 `searchRevisionKey`，所以筆記或標籤編輯後的 `invalidateSearches`（43-50）刷不到語意結果。
- 而 `tag:` / `note:` 在語意路徑也生效（§2.5）。查詢的 staleTime 是 5 分鐘（/home/agent/PathKeep/src/lib/query.ts:17）。

**每列的資料**（193-203）

- 只取 `historyId`→`id`、`visitedAt`→`visitTime`、`score`。
- 沒有 `visitCount`，因為 AiSearchEntry 沒這一欄（§2.4）。

**結果狀態**（230-243）

- `pages = {value: first.total, exact: true}`、`visits: null`、`sorting: false`、coverage 取自 `searchState`。
- ResultsLine 顯示「{N 個頁面} · 語意」（/home/agent/PathKeep/src/features/history/results-line.tsx:47-60）。

**列上的相關度**

- 只有 `0 ≤ score ≤ 1` 時才顯示「相關度 N%」（/home/agent/PathKeep/src/features/history/timeline-rows.tsx:187-190,216-223）。
- hybrid 分數先正規化到最大值 1，再給星號頁加 `starred_boost`（預設 0.15、上限 0.5）（search_ranking.rs:203-217；models/intelligence.rs:542-551）。
- 所以加星的頁分數會超過 1，相關度欄位直接消失。

**今天各模式實際用的排序**

| 位置             | 排序                 | 出處               |
| ---------------- | -------------------- | ------------------ |
| 時間線           | `newest`             | queries.ts:119     |
| 全文             | `relevance`          | queries.ts:137     |
| regex            | `newest`             | queries.ts:137     |
| 語意             | 不送，等於 relevance | queries.ts:166-176 |
| Sites 的最新一頁 | `newest`             | queries.ts:359     |
| 已加星           | `recently_starred`   | queries.ts:374     |

/home/agent/PathKeep/docs/plan/STATUS.md:315 寫著「History 畫面沒有『最舊』排序（只有相關度與最新）；用最舊排序的是 Ask 與 MCP」。其中 MCP 那一半不對：MCP 沒有 sort（§4）。

**其他接點**

- `resetKey = text|mode|filters`（/home/agent/PathKeep/src/features/history/history-page.tsx:111）決定虛擬列表何時重置捲動位置。
- 語意說明行在 `searching && (search.mode==='semantic' || params.mode==='semantic')` 時出現（390-396）。
- 挑速查例子（375-379）或點標籤（228-234）都會把 mode 改回 full，理由是「運算子是全文語法」。但後端大多數運算子在語意路徑也是硬篩選（§2.5）。

## 2. 後端：`search_ai_history`

### 2.1 呼叫鏈

1. `intelligenceClient.searchHistory`（/home/agent/PathKeep/src/lib/backend-client/intelligence.ts:99-100）
2. Tauri 命令（/home/agent/PathKeep/src-tauri/src/commands/intelligence/ai.rs:218-226，spawn_blocking）；dev bridge 走 /home/agent/PathKeep/src-tauri/src/dev_ipc_bridge/dispatch.rs:610-614
3. `worker_bridge::search_ai_history_impl`（/home/agent/PathKeep/src-tauri/src/worker_bridge/intelligence/ai.rs:172-179）
4. `vault_worker::search_ai_history`（/home/agent/PathKeep/src-tauri/crates/vault-worker/src/intelligence/ai_queue.rs:708-731）：解析 serving provider；解析失敗就用 provider=None 重跑，並附上 `providerResolutionFailed`（715-721）
5. `semantic_search_history`：把 limit 夾到 50（search.rs:37-50）
6. `search_history_internal`（search.rs:56-72）

serving provider 怎麼選（/home/agent/PathKeep/src-tauri/crates/vault-core/src/semantic_index.rs:31-52；/home/agent/PathKeep/src-tauri/crates/vault-worker/src/context.rs:219-234）：

- 所選的已完成，就用所選。
- 否則用最近完成、而且服務還在設定裡的那一份。
- 都沒有，就用所選的部分索引或空索引。

所以 provider=None 只會在兩種情況發生：沒選任何嵌入服務，或解析失敗。

### 2.2 路由（search.rs:64-71，由上往下判斷）

| 條件                                    | 走哪條路               | 排序           | 讀語意嗎         | providerId / model                                                          | 每列的 matchReason                                                                     | 附的 note                   |
| --------------------------------------- | ---------------------- | -------------- | ---------------- | --------------------------------------------------------------------------- | -------------------------------------------------------------------------------------- | --------------------------- |
| query 空白                              | `lexical_page`         | 強制 newest    | 否               | `recent-visits` / `none`                                                    | `recent-visit`                                                                         | 無                          |
| sort 是精確字串 `"oldest"` / `"newest"` | `lexical_page`         | 該日期順序     | 否               | `date-ordered` / `none`                                                     | `lexical-date-ordered`                                                                 | 無                          |
| provider=None，而且一個星號都沒有       | `lexical_page`         | BM25 relevance | 否               | `lexical-fallback` / `none`                                                 | `lexical`                                                                              | `lexicalFallbackNoProvider` |
| 其他                                    | `search_ranking::page` | RRF relevance  | 有 provider 時讀 | provider id / serving model；沒有 provider 時是 `lexical-fallback` / `none` | `lexical` / `semantic` / `lexical+semantic`，加星時再加 `+starred`（session.rs:31-40） | 見 §3                       |

- `starred_only` 時，`lexical_page` 的 reason 會加上 `+starred`（search_pages.rs:62-66）。
- `providerResolutionFailed` 是 worker 事後附加的，任何一條路徑都可能帶（context.rs:242-252）。

### 2.3 relevance（hybrid）怎麼排（search_ranking.rs:233-278）

1. **詞彙名次**：用 `ranked_query` 開 session（46-60）。sort 照抄；`ranking_key` = provider/model 加上 `hybrid_rrf_k`、`lexical_weight`、`semantic_weight`、`starred_boost`。`collect_matches` 收齊全部詞彙命中並排名（62-70）。
2. **開 serving 索引並計分**：
   - 索引過期就加 `stale`；索引是空的就加 `emptySemanticIndex` 並放棄語意（73-91）。
   - 用原始 query 字串做 embedding。
   - 維度或 fingerprint 漂移就加 note 並放棄語意（93-108；search.rs:170-200）。
   - 否則用 `score_all` 給每個向量一個分數。
3. **`scope_pass`**：一次串流讀完範圍內的每次造訪，每頁取範圍內造訪的最高語意分（144-167）。這是最貴的一段：100 萬次造訪 385 ms，14.4M 次估 5.5 s（M5 Max 上量的，BACKLOG.md:56-59）。**在語意模式換排序，等於整段重跑。**
4. **`rank_and_fuse`**（169-219）：
   - 詞彙分 `w_l/(k+rank-1)`。
   - 語意先按「分數 → 該頁最新造訪 → page id」排名，再給 `w_s/(k+rank)`。
   - 除以最大值正規化，之後才給星號頁加 boost。
   - 預設值：k=60、兩個權重都是 1、boost 0.15（models/intelligence.rs:532-551）。使用者可在 Settings → AI「搜尋排序」調整。
5. **`install_scores`** 把 collector 換成 `SortOrder::Relevance`，不管 request 的 order 是什麼（session.rs:397-428，第 416 行）。同分時依 visit_ms、再依 page_id 遞減（/home/agent/PathKeep/src-tauri/crates/vault-search/src/collector.rs:21-27,84-90）。
6. 每列顯示該頁「範圍內最新」的那次造訪（eligibility.rs:306-342）。

### 2.4 newest / oldest，以及引擎能力

**走法**

- `lexical_page`（search_pages.rs:15-49）→ `crate::search::page` → `RunningSession`。
- order 的判定：`"newest"`→Newest，`"oldest"`→Oldest，其他全部 Relevance（session.rs:127-131）。

**語意**

- newest 按每頁範圍內最新的那次造訪排。
- oldest 按每頁範圍內第一次造訪排，那一列也顯示那次造訪（來自 WORK-SEARCH-FIRST-VISIT，STATUS.md:310-315；session.rs:231-235；eligibility.rs:306-342）。

**回應內容**

- `total` = 詞彙命中頁數，精確值。
- `score` = 該頁 BM25 ÷ 這一頁第一列的 BM25（search_pages.rs:56,75）。按日期排時第一列不一定是最高分，所以分數跨頁不可比，也可能大於 1。
- 不做 embedding，不讀向量，所以也沒有「語意門檻」這回事。
- 文件明定日期排序不帶 previousModel（/home/agent/PathKeep/docs/architecture/semantic-index-control.md:14-15；intelligence.md:86）。
- 會丟掉 `browser_kind`（§0 第 5 點）。

**引擎能力**

- 只有三種順序：`SortOrder { Relevance, Newest, Oldest }`（/home/agent/PathKeep/src-tauri/crates/vault-search/src/filter.rs:118-125）。
- **沒有「最常造訪」。**每頁範圍內的造訪數其實已經算了（`Membership.counts`，eligibility.rs:159-201），`hydrate` 也寫進了 `visit_count`（eligibility.rs:368）。但 collector 不能依它排序，而且 `history_entry_to_search_entry` 把它丟了（search.rs:219-239），所以語意列沒有造訪數。
- 先例可參考：星號有 `most_revisited`（/home/agent/PathKeep/src-tauri/crates/vault-core/src/ai/agent_tools/user_records.rs:48-52,65-68）；Insights 的 topSites 有 `sortBy: visits | unique_days | average_daily_visits`。

**其他模式**

- 時間線（不搜尋）已經支援 oldest，用 ASC（/home/agent/PathKeep/src-tauri/crates/vault-core/src/archive/history/timeline.rs:77,105,146）。
- regex 的 `Hit.score` 一律是 0（/home/agent/PathKeep/src-tauri/crates/vault-core/src/search/regex.rs:195,251），所以 relevance 實際等於 newest；oldest 走反向掃描（session.rs:101-110）。

### 2.5 運算子在語意模式下的行為

- `RunningSession` 會先用 `query::build` 解析運算子（session.rs:84-89）。
- `Scope::new` 把下列運算子都變成造訪範圍的 SQL（eligibility.rs:18-100）：`after:` / `before:`、`site:` / `-site:`、`tag:` / `-tag:`、`note:` / `-note:`、`filetype:`、`intitle:` / `inurl:`（含否定）、`-詞`、`is:starred`。
- 語意分數就是在同一個範圍上算的（search_ranking.rs:144-167），所以**這些在語意模式也是硬篩選**。
- 片語 `"…"`、`OR` 和一般關鍵字只影響詞彙名次。
- 拿去做 embedding 的是原始整串，`site:docs.rs` 這類字樣也在裡面（search_ranking.rs:139,250）。
- 後端還認得、但速查表沒列的：`is:starred`、別名 `title:` / `url:` / `ext:`（/home/agent/PathKeep/src-tauri/crates/vault-core/src/archive/search_query.rs:77,84,94,104；對照 /home/agent/PathKeep/src/features/history/search-help.tsx:30-42）。

### 2.6 回應欄位對照

| 欄位                        | Rust（models/intelligence.rs:1819-1844） | TS（types/intelligence.ts:836-854） | History 有用嗎                                                                                                                                 |
| --------------------------- | ---------------------------------------- | ----------------------------------- | ---------------------------------------------------------------------------------------------------------------------------------------------- |
| `total`                     | usize                                    | number                              | 有，當頁數，標成 exact                                                                                                                         |
| `providerId` / `model`      | String                                   | string                              | 沒有                                                                                                                                           |
| `items[]`                   | `AiSearchEntry`（1675-1703）             | `AiSearchResultItem`（776-802）     | 部分：id、profileId、url、title、domain、visitedAt、score                                                                                      |
| `items[].matchReason`       | 穩定代碼（§2.2）                         | string                              | 沒有。兩邊註解都說前端用 `localizeAiMatchReason` / `aiMatchReason*` 解析（intelligence.ts:784-792；search.rs:107-113），但 src/ 裡兩者都不存在 |
| `items[].enrichmentExcerpt` | Option，沒有就不序列化                   | `?string`                           | 沒有                                                                                                                                           |
| `notes`                     | 英文句子，給模型看                       | `string[]`                          | 沒有（本來就不該直接顯示）                                                                                                                     |
| `noteCodes`                 | `Vec<AiSearchNote>`，serde default       | `AiSearchNote[]?`                   | 沒有                                                                                                                                           |
| `nextCursor`                | `sessionId:token`                        | `string?`                           | 有                                                                                                                                             |
| `searchState`               | `SearchCoverage`                         | `SearchCoverage?`                   | 有                                                                                                                                             |
| `appliedLimit`              | `Option<u32>`                            | **缺**                              | —                                                                                                                                              |
| `hasMore`                   | bool                                     | **缺**                              | —                                                                                                                                              |

請求欄位（Rust，1566-1606）：`query`、`profileId`、`domain`、`limit`、`cursor`、`sort`、`starredOnly`、`startDate`、`endDate`、`browserKind`。TS 只少 `sort`。

**cursor 規則**

- cursor 只能用一次，綁定 session 和完整參數。參數不同時會報錯："this cursor belongs to a search with other arguments…"（stream.rs:162-168；/home/agent/PathKeep/docs/architecture/search.md:127）。
- React Query 快取裡舊排序的頁，它的 cursor 之所以今天還能用，正是因為 session 沒被釋放（§0 第 8 點）。
- 所以如果之後改成切換排序時 cancel，從快取回來的舊排序頁會拿到失效的 cursor，兩件事要一起設計。

## 3. Note codes

### 3.1 BACKLOG `WORK-HISTORY-SEARCH-NOTES`（/home/agent/PathKeep/docs/plan/BACKLOG.md:79-81）

- **現況**：後端每次語意搜尋都回 `noteCodes`，涵蓋索引是空的、過期、設定變了、服務解析失敗，以及 2026-10-15 起的「用的是上一個模型的索引」。但 `src/features/history/` 一個都沒讀。使用者看不出語意搜尋退回了關鍵字、索引還沒建完、或用的是舊模型。
- **要做**：History 在結果上方照 code 用三種語言說一句，附下一步（去 Settings → AI）。
- **E2E 至少要驗**：「索引還沒建完」和「用上一個模型」。

### 3.2 全部 code

出處：types/intelligence.ts:825-834；models/intelligence.rs:1757-1778。wire 形狀是 `{code, …}`，camelCase。

**`lexicalFallbackNoProvider`**

- 產生處：search_pages.rs:57-61；search_ranking.rs:253-256。
- 何時出現：provider=None（沒選嵌入服務，或解析失敗）。
- 實際結果：只有關鍵字結果。
- History 今天顯示：沒選服務時 History 根本進不了語意模式；解析失敗時照樣寫「語意搜尋使用 X · 已索引 N 個頁面」，但列全是關鍵字結果。

**`providerResolutionFailed {reason}`**

- 產生處：context.rs:242-252。
- 何時出現：serving provider 解析失敗，例如金鑰或設定被移除。
- 實際結果：只有關鍵字結果，通常和上一個 code 一起出現。沒有星號時會走 `lexical_page`，瀏覽器篩選也被丟掉。
- History 今天顯示：同上，什麼都不說。

**`stale {reason: watermark|enrichment|textFormat}`**

- 產生處：search_ranking.rs:80-84（判斷在 /home/agent/PathKeep/src-tauri/crates/vault-core/src/ai/search_store.rs:7-41）。
- 何時出現：serving 索引落後，或用的是舊的文字格式。
- 實際結果：照常做語意搜尋，不放棄。
- History 今天顯示：只顯示「所選」模型的過期原因（semantic-info.ts:83-86），不是 serving 的。換模型期間兩者可能不同。

**`emptySemanticIndex`**

- 產生處：search_ranking.rs:86-88。
- 何時出現：serving 的 `.pkvec` 是空的。
- 實際結果：只有關鍵字結果。
- History 今天顯示：`ready` 要求 serving 有向量，平常進不來；發生競態時什麼都不說。

**`configDriftDimension`**

- 產生處：search.rs:179-181（經 search_ranking.rs:103-108 呼叫）。
- 何時出現：查詢向量的維度和索引不同。
- 實際結果：只有關鍵字結果。
- History 今天顯示：什麼都不說，照樣寫「語意搜尋使用…」。

**`configDriftFingerprint`**

- 產生處：search.rs:196-198。
- 何時出現：維度相同，但 pooling、正規化等設定不同。
- 實際結果：只有關鍵字結果。
- History 今天顯示：同上。

**`semanticMatchesFilteredOut`**

- 產生處：search_ranking.rs:262-264。
- 何時出現：有語意分數，但篩選範圍內沒有任何一頁有向量。例如範圍內的造訪都還沒進索引。
- 實際結果：只有關鍵字結果。
- History 今天顯示：什麼都不說；結果是空的時候顯示一般的「沒有找到」。

**`previousModel {model}`**

- 產生處：search_ranking.rs:266-274。
- 何時出現：讀了 serving ≠ 所選的那份語意平面。
- 實際結果：用舊模型做語意搜尋。
- History 今天顯示：說明行反而寫「所選」的服務與模型、所選模型的已索引數和更新時間（semantic-info.ts:63-86；read_model.rs:93-111,136-151,178）。讀碼推得的例子：在 E2E 換到 `e2e-embed-next` 之後，會寫「Semantic search with E2E embeddings · e2e-embed-next · 0 pages indexed」，結果卻來自 `e2e-embed`。/home/agent/PathKeep/tests/e2e/semantic-index.spec.ts:485-530 正好走過這個情況，但只驗結果，沒驗任何提示。

**補充**

- **換頁也帶同一組 note**：hybrid 路徑把 note 存在 session 裡（search_ranking.rs:229；session.rs:557-560），每個 step 都帶出來（session.rs:329）；`lexical_page` 每頁重算。
- **沒有「索引還沒建完」的 code**：
  - 沒有完整索引時，serving 就是所選的部分索引（semantic_index.rs:51）；只要有向量 `ready` 就成立，搜尋不加任何 note。
  - 沒完成的 build 不會推進 ledger 的 watermark，所以也不會出 `stale`。
  - 所以 BACKLOG 要的「索引還沒建完」E2E，得靠 `AiQueueStatus.semanticIndex`（`chosen` / `serving` / `chosenComplete` / `build`，types/intelligence.ts:474-487,513），或新增一個 code。
  - History 目前沒讀這份資料；只有 Settings 每秒輪詢它（/home/agent/PathKeep/src/features/ai-setup/use-semantic-index.ts:83-90）。
- **文件漂移**：
  - search.md:77-87 的 `AiSearchNote` 少了 `previousModel`，`stale.reason` 少了 `textFormat`。
  - search.md:129 說 MCP 回傳 `notes`，沒提 `noteCodes`；實際上 /home/agent/PathKeep/src-tauri/crates/vault-worker/src/mcp.rs:62 有。
- **i18n**：/home/agent/PathKeep/src/lib/i18n/messages/history-search.ts:41-67 只有狀態原因 `semantic.reason.*` 和 `semantic.stale.*`，沒有任何 note code 的文案。

## 4. 其他傳 sort 的呼叫者

**History 時間線**

- 指令：`query_history`，不帶 q，走時間線 SQL。
- 排序：`newest`（queries.ts:119），每頁 100 筆，用 cursor。
- 加 History 層級的 sort 參數會怎樣：只是前端參數；時間線 SQL 已經支援 oldest。

**History 全文 / regex**

- 指令：`start_search` 串流。
- 排序：全文 `relevance`、regex `newest`（queries.ts:137）。
- 換 request 時會自動 cancel 舊 session。
- 加參數會怎樣：request 的 JSON 本身就是 session key（/home/agent/PathKeep/src/features/history/use-search-stream.ts:83-94），sort 一加進去就會自動重開。

**History 語意**

- 指令：`search_ai_history`，今天不送 sort（§1）。
- 加參數要改的地方：TS 型別、請求、queryKey、`resetKey`，還有 §0 第 5、8 點。

**Sites 的最新一頁**

- 指令：`query_history`，不帶 q。排序 `newest`，limit 1（queries.ts:352-367）。不受影響。

**已加星**

- 指令：`listStars`，排序 `recently_starred`（queries.ts:374）。不受影響。

**命令面板**

- 指令：`query_history`，帶 `groupByUrl: true`。Rust 端其實沒讀這個欄位，因為搜尋本來就是一頁一列。
- 排序：`relevance`（/home/agent/PathKeep/src/app/shell/command-palette.tsx:72-84）。limit 6，debounce 180 ms，至少 2 個字才查。
- 加參數會怎樣：面板自己的排序是寫死的；連到 History 只帶 `q`（有選結果時加 `visit`），見 §5。另外，只要結果超過 6 筆，每次查詢都會留下一個 session（§0 第 8 點）。

**Ask 的 `search_history` 工具**

- 直接呼叫 `search_history_internal`（/home/agent/PathKeep/src-tauri/crates/vault-core/src/ai/agent_tools.rs:291-299），自己把 limit 夾到 50。
- 排序由模型選，enum 是 `relevance` / `newest` / `oldest`（agent_tools.rs:239）；工具描述教它用 oldest 找「第一次」（225）。`browser_kind` 固定 None（288）。
- 加參數會怎樣：它不讀前端狀態。但後端 sort 語意一改就會直接影響它：/home/agent/PathKeep/tests/e2e/ask-chats.spec.ts:607-688 釘死了 oldest 的第一列必須是最早的造訪，而且 reason 是 `lexical-date-ordered`（663）。

**Ask `run_code` 裡的 `query_history`**

- 同一條路（/home/agent/PathKeep/src-tauri/crates/vault-core/src/ai/code_mode.rs:569-612），limit 最多 1000。
- `sort` 原樣傳下去（596）；用 oldest/newest 時 `plane` 參數沒有作用，一律走 `lexical_page`。`browser_kind` 固定 None（600）。
- 加參數會怎樣：同上。
- **另一個 bug**：`QueryHistoryArgs` 沒有 `rename_all`，所以 `profile_id` / `starred_only` 只認 snake_case（259-293，第 270、286 行；只有 `startDate` / `endDate` 有 rename）。但給模型看的說明寫的是 `profileId?`、`starredOnly?`（agent_tools.rs:478）。模型照說明寫，參數會被默默忽略。

**MCP `search-history`**

- 走 worker 的 `search_ai_history`。
- 請求型別根本沒有 sort（mcp.rs:30-44），內部寫死 `sort: None`（mcp.rs:302-314）。日期和 `browser_kind` 也是 None。
- 加參數會怎樣：只有改 relevance 語意時才受影響。使用指南裡描述了 `lexical-date-ordered`（mcp.rs:186）。

**Settings 匯出**

- `export_history`，`query: {}`（/home/agent/PathKeep/src/features/settings/export-row.tsx:49），走時間線。不受影響；History 自己沒有匯出功能（/home/agent/PathKeep/docs/features/recall.md:60）。

**Insights / Home**

- 只是連結，見 §5。

**共用點**

- History 語意和 MCP 經過 `semantic_search_history`；Ask 的兩個工具直接打 `search_history_internal`；最後全部進同一個 `RunningSession` / `SearchSessions`。
- 所以改 `search_history_internal`、`lexical_page` 或 `RunningSession` 的排序，這幾處會同時生效。只改前端參數的話，只影響 History。

## 5. 進入 History 的 deep link

**History 怎麼解析 URL**

- 讀 `q`、`mode`（預設 full）、`view`、`date`、`browser`、`domain`、`visit`（history-params.ts:17-45）。
- `update` 用 replace 寫回 URL（/home/agent/PathKeep/src/lib/search-params.ts:34-42）。
- 值等於預設的鍵會被刪掉；其他未知的鍵保留（history-params.ts:57-74）。

**每個來源設了哪些參數**

| 來源                                                    | 檔案:行                                                                                                      | 設的參數                                              |
| ------------------------------------------------------- | ------------------------------------------------------------------------------------------------------------ | ----------------------------------------------------- |
| 命令面板「搜尋歷史」（選單列的 Search history… 也開它） | command-palette.tsx:143,201-209                                                                              | `q`                                                   |
| 命令面板的一筆結果                                      | command-palette.tsx:186-188                                                                                  | `q`、`visit`                                          |
| Ask 引用（sources.tsx:30、assistant-turn.tsx:96）       | /home/agent/PathKeep/src/features/ask/citations.ts:12-15                                                     | `q`=標題或主機名、`visit`=引用的 historyId            |
| Insights 搜尋 drill-in 的「在 History 看」              | /home/agent/PathKeep/src/features/insights/search-page.tsx:104                                               | `q`（不帶 scope 日期）                                |
| Insights 搜尋 drill-in 的每次造訪                       | search-page.tsx:354                                                                                          | `date`、`visit`                                       |
| Insights「搜尋什麼」裡的詞                              | /home/agent/PathKeep/src/features/insights/search-cards.tsx:123                                              | `q`                                                   |
| Insights day                                            | /home/agent/PathKeep/src/features/insights/day-page.tsx:127、387、542                                        | 依序是 `date`；`date`+`domain`；`date`+`visit`        |
| Insights page                                           | /home/agent/PathKeep/src/features/insights/page-page.tsx:102-105                                             | `q`=標題或網址、`domain`                              |
| Insights site                                           | /home/agent/PathKeep/src/features/insights/site-page.tsx:76、132                                             | `domain`+`date`（scope）；`q`=頁標題或路徑 + `domain` |
| Home「歷史上的今天」                                    | /home/agent/PathKeep/src/features/home/home-cards.tsx:165                                                    | `date`                                                |
| Home 統計卡、導覽列                                     | /home/agent/PathKeep/src/features/home/home-page.tsx:123；/home/agent/PathKeep/src/app/shell/nav-rail.tsx:50 | 無                                                    |

- Insights 的連結全部經過 `historyPath`，它只會寫 `q` / `date` / `domain` / `visit`（/home/agent/PathKeep/src/features/insights/links.ts:55-69）。
- 沒有任何地方寫 `mode`、`browser`、`view` 或排序。
- router 裡沒有舊路由的轉址。

**`?visit=` 和排序的互動**

- `?visit=` 只在那個 id 出現在已載入的列裡時才選中，不會自動翻頁（history-page.tsx:157-165）。
- 搜尋結果一頁一列，列的 id 是該頁的代表造訪：relevance / newest 用範圍內最新的那次，oldest 用第一次。
- 因此：
  - **命令面板**：面板用 relevance、無篩選，History 全文也是，所以對得上。
  - **Ask 用 oldest 搜尋**：Ask 用 `sort: "oldest"` 時，引用的是第一次造訪。History 開在 relevance，列上是最新造訪，所以造訪過多次的頁選不到。
  - **Ask 帶日期範圍搜尋**：引用的是範圍內最新的造訪，但連結不帶日期，同樣可能對不上。
  - **E2E 覆蓋**：只驗了引用連結存在（ask-chats.spec.ts:683-687），沒有點進 History；Insights 的 day→History 只驗 URL（/home/agent/PathKeep/tests/e2e/insights-drill.spec.ts:245）。
- **排序偏好存放位置的影響**：
  - 如果存在 URL 以外（例如 localStorage），所有不帶排序的 deep link 都會落在那個偏好上，`visit` 可能因此選不到。
  - 如果只存在 URL，deep link 一律落在預設排序。
  - 另外 `filterByTag` 只把 mode 改回 full，不會清掉 URL 裡的 sort。

## 6. 設計時直接相關的既有約定

- **已接受的需求**：recall.md:99 已經寫明「Keyword mode 預設 relevance，明確選擇 newest / oldest 時才回到時間排序」。recall.md:14-16 定義了 newest / oldest 的語意：每頁的最新或第一次造訪，列上也顯示那一次。
- **要改的設計文檔**：/home/agent/PathKeep/docs/design/screens-and-nav.md:70 的 History 工具列沒有排序控制項，加排序就要改這份文檔。
- **語意的覆蓋要求**：intelligence.md:93 要求全量覆蓋（見 §0 第 3 點）。在語意模式加相似度門檻屬於推翻已接受文件。
- **成本差異**：
  - 語意 relevance 換排序 = 重跑完整的 scope pass，14.4M 時估秒級。
  - 換到 newest / oldest 走的是較便宜的 lexical 路徑；沒有日期篩選時 membership 是懶載入的。
