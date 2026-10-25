# 後端搜尋排序現況：給「排序選擇」與「進階搜尋 builder」設計用

範圍：`vault-core/src/search/*`、`vault-search/src/*`、Tauri / worker / dev bridge 的 `start_search` 入口，以及前端實際送出的請求。沒有改任何檔案。行號以 `feat/search-order-and-builder`（= `dc42cd7a`）為準。

## 0. 結論

1. **三種順序都把所有 posting 走完，沒有提前終止。** Walker 對每個 segment 一路走到 `TERMINATED`（progress.rs:290-370）。`Newest` / `Oldest` 與 `Relevance` 的差別只有一處：前兩者不收集 BM25 統計、不打分（progress.rs:227-231、275-284、339-350）。主要成本是 canonical eligibility，也就是逐頁讀可見訪問的 SQL。所有順序共用這一段，每頁只讀一次。Newest/Oldest 在規模上沒有量過。
2. **「最常造訪」要的每頁資料，今天已經對每個候選頁算好。** 這些值存在 `Membership.counts` / `Membership.newest`（eligibility.rs:159-168、438-448、470-477），但沒有用來排序，只加總進 `visits` 計數（session.rs:256）。所以 keyword 路徑加這個順序，SQL 與記憶體幾乎不增加。
3. **真正的風險在 hydrate。** 每個 step（目標 8 ms，common 詞約 710 個 step）都會把當前 top-k（≤100 頁）每頁的**全部** scope 內訪問逐列讀一遍（eligibility.rs:326-375，由 session.rs:283-299 每 step 呼叫）。「最常造訪」按定義把訪問最多的頁放進 top-k，這個成本會被推到最大。benchmark fixture 每頁只有 4–5 次訪問（benchmark/fixture.rs:35-38），測不出這個問題。
4. **Regex 沒有 relevance。** `sort` 只要不是 `"oldest"`（包括 `"relevance"`），都會默默變成新到舊（session.rs:106）。
   - 有字面片段的 candidates 路徑：「最常造訪」可以用有界 heap 做。
   - 沒有片段、要全量 walk 的路徑：必須掃完才能排。
5. **Cursor 不編碼順序。** 線上傳的 cursor 是 session 內的隨機 token；順序存在 session（session.rs:71），排序鍵存在 `Hit`（`Copy`：page_id / visit_ms / score）。新順序的鍵得放進 `Hit`。字串鍵（標題、網站）會和 `Hit: Copy` 衝突。
6. **換排序等於整個 search 重跑。** 前端 requestKey 一變就開新 session（use-search-stream.ts:83-94），eligibility 和計數全部重算。common 詞在 dev 機約 8.2 s，而計數本身與排序無關。若在 session 內 re-sort，重用 `Membership` 與 pinned reader（`requery` 已有，progress.rs:87-106），切換可降到下一頁的成本（約 267 ms）。目前沒有這種命令。
7. **`sort` 是自由字串，各處各自解析、各自退回預設，都不驗證、不 trim。**
   - keyword → Relevance（session.rs:130）
   - regex / 時間線 → newest
   - AI 路由只認 `"oldest"|"newest"`（ai/search.rs:65）。新 token 會流進 fusion 路徑，在那裡把 BM25 關掉（見 §3）。

---

## 1. 端到端流程

### 1.1 入口

| 層                                                               | 位置                                                                                                                                                                                                                                                               |
| ---------------------------------------------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------ |
| 前端                                                             | `useVisitList` 組請求（queries.ts:131-156）→ `useSearchStream`（use-search-stream.ts:82-155）→ `openSearchStream`（search-stream.ts:36-89）                                                                                                                        |
| Tauri                                                            | `start_search({request, onEvent: Channel})`（commands/search.rs:10-21）→ `worker_bridge::start_search_impl`（worker_bridge/search.rs:8-14）→ `vault_worker::start_search`（vault-worker/src/search.rs:10-16）                                                      |
| Owner                                                            | `SearchSessions::start` → `spawn`（stream.rs:50-57、77-143）：計算 fingerprint `(request, ranking_key)`（85），開 `search-session` thread，迴圈 `step(8ms)` 推 `SearchEvent::Step`；一頁完成（`!provisional && count.exact`）後阻塞，等 `Command::Next`（114-132） |
| Session                                                          | `SearchSession::start_owned`（session.rs:479-497）→ `RunningSession::start_owned`（77-195）                                                                                                                                                                        |
| 同步 adapter（`query_history`、命令面板、AI lexical、code mode） | `page_from` 等到 `!provisional && count.exact` 才回（history.rs:103-142、127），cursor 改寫成 `"{session_id}:{token}"`（137）                                                                                                                                      |

### 1.2 關鍵字搜尋

1. 解析順序：`"newest"`→Newest、`"oldest"`→Oldest、其他→Relevance（session.rs:127-131）。`first = order==Oldest`（132）。
2. Membership 有兩種：
   - 有日期界、非 fuzzy、非 regex、且索引已追上（122-126、134）：eager，用 `eligibility::load` 透過時間索引一次掃完 scope 內所有訪問（eligibility.rs:221-260）。
   - 其他情況：`Membership::empty`，之後按批 lazy 解析（396-401）。
3. Reader：`index.search(ast, Filters{order, profile_id})` 把順序釘進 snapshot（session.rs:153-160 → index.rs:312-332）。session 開兩條 `MatchHandle`：rank（scoring=true）和 count（scoring=false）（session.rs:161-162）。
4. Walker：只有 `scoring && order==Relevance` 時才收集統計並啟用 BM25；其他順序 `score=0.0`（progress.rs:230、277-283、339-350）。`visit_ms` 先取自 index 的 `newest_visit_ms` fast field（300-305、335-338），但 vault-core 會覆寫，見第 6 步。
5. Rank 迴圈（session.rs:220-244）：每批最多 1024 個 hit（`CANDIDATE_BATCH`，41）→ `resolve` → `Membership::ensure`，每批一條 SQL（eligibility.rs:404-479，SQL 在 434-437）。
6. 對 membership 內、還沒回傳過的頁，把 `hit.visit_ms` 覆寫成 `membership.first(id)`（Oldest）或 `membership.newest(id)`（其他）（session.rs:231-235），再放進 `RankCollector`（limit = page_size+1，作為 lookahead；175）。
7. Collector 比較規則（collector.rs:84-90）：
   - Relevance = score → visit_ms → page_id（`Ord for Hit`，21-28）
   - Newest = visit_ms desc → page_id desc
   - Oldest = visit_ms asc → page_id desc
8. Hydrate：`eligibility::hydrate(..., first)` 決定每列顯示哪一筆訪問（Oldest 顯示第一次，其他顯示最新），同時逐列數 `visit_count`（eligibility.rs:306-393）。

### 1.3 Regex

- 文字不經 operator 解析：`query::build("", false)`（session.rs:84-88）。只有請求層的日期、profile、瀏覽器、domain、starred 會生效。
- `Scan::new(query, scope, page_size+1, oldest = sort==Some("oldest"), …)`（session.rs:101-112），`oldest` 是一個 bool。
- 掃描來源（regex.rs:19-36、65-92）：
  - **Listing → Candidates**：pattern 有 ≥3 字元的必要 ASCII 片段時（regex_prefilter.rs:106-122），先用 Tantivy substring 查詢列出候選頁，再加上 snapshot 之後 outbox 裡的頁（25-70、89-104）。之後按 page id 升冪、每批 256 頁，`GROUP BY u.id` 取 `COUNT(*)` 與 `MAX`/`MIN(visit_time_ms)`（regex.rs:147-206、321-328）。
  - **Walk**：沒有片段或索引壞掉時，按 `(visit_time_ms, id)` 每次 512 筆訪問，新到舊或舊到新，透過 `idx_visits_visible_time_id` seek（208-262、345-350）。
- 每個 step 依 `rank()`（時間 → page id，352-355）排序並截到 limit；`winners` 只保留 top-k（136-139）。
- 計數與排名來自同一次掃描：`count_done = rank_done = complete`（session.rs:210-217）。
- Hydrate 依訪問 id 逐筆讀（`evidence_from`），對顯示的那筆訪問再比一次 pattern（regex.rs:266-309）。

### 1.4 獨立的計數 walk（keyword）

- 第二條 posting walk（scoring=false），與 rank walk 共用 `Membership`。每頁的 SQL 只做一次：`ensure` 會先濾掉已解析的頁（eligibility.rs:414-415）。
- 每個頁：`matched_pages += 1`、`matched_visits += membership.count(page)`（session.rs:246-264）。與順序無關。
- 每個 step，rank 與 count 各用一半預算（225、241、251、261）。
- Cursor 只在 `has_next && count_done` 時發出（325）。所以任何順序的第一頁，都要等計數走完才能翻頁。
- 下一頁時，keyword 不重算計數（`count_done` 保持 true）；regex 會整個重掃並重算（442-446；regex.rs:97-110）。

### 1.5 排序當下每頁有哪些資料

| 資料                               | 在哪裡算                                                                         | 存在哪                                                                       | 用在哪                                                         |
| ---------------------------------- | -------------------------------------------------------------------------------- | ---------------------------------------------------------------------------- | -------------------------------------------------------------- |
| BM25 分數                          | Walker（progress.rs:339-350），AI fusion 可由 `Filters.score` 覆寫               | `Hit.score`                                                                  | 只有 Relevance                                                 |
| scope 內最新可見訪問時間           | `ensure` 的批內 HashMap（eligibility.rs:439-449）；eager 走 `insert`（180-192）  | `Membership.newest: Vec<i64>`（dense，依 page id）                           | Newest、Relevance 的同分決勝、hydrate                          |
| scope 內第一次訪問                 | 同上                                                                             | `Membership.first: Option<Vec<i64>>`，**只有 Oldest 才配置**（164-166、176） | Oldest                                                         |
| scope 內可見訪問數                 | 同上（`entry.0 += 1`；`counts[id] += 1`）                                        | `Membership.counts: Vec<u64>`（177），**每個 session 都配置**                | 只加總進 `SearchStep.visits`（session.rs:256），**不參與排序** |
| 列上顯示的 `visitCount`            | hydrate 逐列重新數，用的是當前可見性（eligibility.rs:335、368）                  | `HistoryEntry.visit_count`                                                   | 前端                                                           |
| index `newest_visit_ms` fast field | Walker 讀取（progress.rs:300-305）                                               | `Hit.visit_ms`                                                               | **vault-core 一律覆寫**（session.rs:231-235），等於沒用        |
| Regex：每頁符合的訪問數與時間      | candidates 批內 `matched`（regex.rs:172-188）；walk 用 `seen` bitset + `winners` | `winners: HashMap<page,(Shown, visits)>`，只保留 top-k                       | 排序（時間）、`visitCount`                                     |

2.9M 頁時，Membership 各陣列大小：bits / checked 各約 363 KB，newest 23.2 MB，counts 23.2 MB，first（只有 oldest）23.2 MB。用 calloc 配置，RSS 隨實際碰到的頁成長。

---

## 2. Regex 收到 `sort=relevance` 會怎樣；前端各模式送什麼

**Regex + relevance：**

- `oldest=false`，所以結果是新到舊，所有 hit 的 `score=0.0`。
- `self.order` 會被設成 Relevance，但 regex 分支不讀它。唯一副作用是配置了一個用不到的 `Membership::empty`（session.rs:137）。
- 不報錯、也不提示。

**前端送出的請求（queries.ts）：**

| 模式                           | 命令                           | `sort`                                     | 其他                                                                                                |
| ------------------------------ | ------------------------------ | ------------------------------------------ | --------------------------------------------------------------------------------------------------- |
| 全文                           | `start_search`                 | `'relevance'`                              | `query: pattern, regex:false, pageSize:100, startTimeMs, endTimeMs, browserKind, domain`（131-156） |
| Regex（含 `/…/` 包起來的查詢） | `start_search`                 | `'newest'`（137）                          | `regex:true`，`query` 是去掉斜線的 pattern                                                          |
| 語意                           | `search_ai_history`            | 不送（167-176）                            | —                                                                                                   |
| 空查詢                         | `query_history`，走時間線      | `'newest'`（111-128）                      | 一列一筆訪問，**不是**一列一頁；走 timeline.rs，只認 `"oldest"`（77、105、146）                     |
| 命令面板                       | `query_history` + `groupByUrl` | `'relevance'`（command-palette.tsx:72-84） | —                                                                                                   |

補充：

- 前端從不送 `fuzzy`、`starred`（`is:starred` 寫在文字裡）、`profileId`。
- History UI **沒有排序控制項，也從不送 `'oldest'`**；`history-params.ts` 裡沒有 sort 參數。recall.md:99 寫「明確選擇 newest / oldest 時才回到時間排序」，但這在 History UI 裡做不到，只有 AI 工具與 API 呼叫者能用。

---

## 3. Cursor 怎麼編碼順序；新增一種順序要改哪些地方

**Cursor 機制：**

- rank 走完時，`self.cursor = hits.last()`（完整的 `Hit`），token 是 16 bytes 隨機 hex（session.rs:277-282）。
- `next_page(token)` 的步驟（session.rs:429-456）：
  1. 比對 token（430-432）。
  2. 把這頁的 page_id 寫進 `returned` bitset，防止重複（433-439）。
  3. keyword：重新走 rank walk，`RankCollector::new(page_size+1, cursor, order)` 丟掉所有「≥ cursor」的 hit（collector.rs:47-49；session.rs:448-450）。regex：`compare(hit, after) != Less` 的都跳過（regex.rs:196-200、252-256）。
- 順序本身不在 cursor 裡，存在 `RunningSession.order`。
- 同步 adapter 會用含 `sort` 的 fingerprint 拒絕跨請求使用 cursor（stream.rs:155-173）。串流路徑只靠 session id + token。

**加一種順序的條件：**

- 比較規則必須是嚴格全序，最後以 page_id 決勝。
- 排序鍵必須能從 `Hit` 重建。數字鍵直接加欄位即可；字串鍵要嘛放棄 `Copy`，要嘛改存 interned id / collation key。
- Cursor 的 wire 格式不用改。

**要改的地方：**

- **Engine**
  - `SortOrder`（filter.rs:118-125）
  - `Hit` 的欄位（collector.rs:11-19）與 `Ord for Hit`（21-28）。注意 AI 的 `collect_matches` 也用 `Ord for Hit` 排序（session.rs:375）。
  - `compare`（collector.rs:84-90）
  - Walker 建構 `Hit` 的地方（progress.rs:333-351）。非 Relevance 會自動不算 BM25。
  - 測試：contracts.rs 的 brute force、canonical.rs:61-84
- **Owner**
  - 解析：session.rs:127-131，以及 regex 的 bool（106）
  - 要配置哪些陣列：session.rs:132-138；eligibility.rs:159-212
  - 填排序鍵：session.rs:231-235
  - hydrate 顯示哪一筆訪問：292-296 → eligibility.rs:336-342
  - Regex：regex.rs:49-63、136-139、171-205、228-262、352-355
  - AI 必須一起對齊：ai/search.rs:64-70、ai/search_pages.rs:18-21、ai/search_ranking.rs:35（原樣把 `sort` 傳下去，所以新 token 會讓 fusion 前的 `collect_matches` 失去 BM25）、models/intelligence.rs:1628-1669（`AiSearchSort`，有 trim）、ai/agent_tools.rs:239（enum）、ai/code_mode.rs:277
- **Wire**
  - types.rs:15、search.ts:25、archive.ts:332
  - docs：architecture/search.md:38、features/recall.md:14-16、99
- **前端**
  - queries.ts:137、history-params.ts
- **會遍歷所有順序的測試**
  - ported_grouped.rs:25、73；ai/tests.rs:410；timeline_tests.rs:23、55
- **可參考的先例**
  - `4ad0c16e fix(search): oldest first goes by each page's first visit` 改了 13 個檔案，是「每頁多一種資料」的完整改法。

---

## 4. 「最常造訪」的可行性與成本（14.4M 訪問 / 2.9M 頁）

### 4.1 Keyword

**需要的資料：** 每個符合頁的 (scope 內可見訪問數, 最新訪問, page_id)。三者今天都已經對每個候選頁算好，原因有兩個：

- rank walk 本來就走完所有 posting，並逐批解析 membership。
- eager 日期路徑在 `insert` 逐筆累加。

**接點：**

- 新增 `SortOrder::MostVisited`。
- `Hit` 加 `visits: u32`。剛好填進現有的 padding，`Hit` 仍是 24 bytes。
- `compare` 加一條：visits desc → visit_ms(newest) desc → page_id desc。
- 在 session.rs:231-235 填 `membership.count(id)`；hydrate 用 `first=false`。

**時間：** 與 Newest 走同一條路徑，Newest 沒量過。現有的量測都是 Relevance，在 dev 機（M5 Max，不是目標的 4 核機）上（research/2026-10-vault-core-search-benchmark.md:24-35 與 .jsonl）：

| 情境                  | 第一批 provisional | 最終第一頁 | 下一頁    | 其他                             |
| --------------------- | ------------------ | ---------- | --------- | -------------------------------- |
| common（2.9M 頁全中） | 11–94 ms           | 8.2–8.3 s  | 約 267 ms | 約 710 steps，最大 step 17–30 ms |
| 日期 2%               | —                  | 336–411 ms | —         | —                                |
| profile               | —                  | 3.8–4.3 s  | —         | —                                |

MostVisited 會省掉 BM25 的統計與打分。

**記憶體：** 不增加。`counts` 今天每個 session 本來就配置；若改成 u32 還能省 11.6 MB。

### 4.2 Regex

- **Candidates 路徑：** 同一頁的所有 URL 都在同一批（`p.id IN batch … GROUP BY u.id`），所以批內的 `entry.2` 就是該頁完整的符合訪問數（regex.rs:172-188）。換成有界 heap 即可，額外記憶體 O(k)。
- **Walk 路徑：** 訪問按時間抵達，一頁的次數要掃完才確定。
  - 現有的 `winners` 只對 top-k 累加次數（regex.rs:240-245），不能沿用。
  - 需要 dense 陣列：次數 u32 11.6 MB + 時間 i64 23.2 MB + 顯示訪問的 id i64 23.2 MB，約 ≤58 MB。掃完後再用 O(max_page_id) 選出 top-k。
  - 掃完之前的 provisional 結果沒有意義。
  - 今天 regex 每翻一頁都整個重掃（regex.rs:97-110）。若保留這些陣列，下一頁只要重新選一次，反而更快。
  - 現行實作的 14.4M 全量 walk 沒有量測。ipc-performance.md:123-130 的 18.9 s 來自已刪除的 `regex_scan.rs`。
- **語意差異：** regex 的「符合訪問」只算符合 pattern 的 URL 列上的訪問（regex.rs:175-187、231-234）；keyword 算的是符合頁的全部可見訪問。

### 4.3 風險

1. **Hydrate 每 step 讀全部訪問（主要風險）。**
   - SQL 為 PAGE_VISITS（pages→urls→visits→source_profiles），對每筆訪問再 `LEFT JOIN search_documents`。沒有 LIMIT、沒有快取，provisional step 即使 top-k 沒變也會重做（eligibility.rs:326-375）。
   - 成本 ≈ Σ(top-100 頁的訪問數) × 每筆數次 B-tree seek × step 數。
   - Step 的時間預算管不到 hydrate：hydrate 在有預算的 walk 迴圈之後才執行。
   - vault-worker 的 `archive_scale_bench` 有一頁 100,005 次訪問（turso_reads.rs:224），可以拿來量偏斜分佈。
   - `visits.rs:17-60 newest_visible_by_page` 已經示範了「每個 URL 一次 seek 取最新可見訪問」的寫法。
2. **排序用的次數與顯示的次數可能不一致。** 排序用 snapshot（釘住的訪問 id 前綴 + 解析當下的可見性），顯示的 `visitCount` 用當前可見性。session 中途有訪問被撤銷時，次數較小的列可能排在較大的上面。`returned` bitset 保證不會重複。
3. **AI 路由會漏接新 token**（見 §3）。
4. **空查詢。** History 的空查詢是逐筆訪問的時間線。「不打字、看最常造訪的頁」有兩條路：
   - 用空 AST 開 session：AllQuery，每頁都要解析，成本等於 common 情境。
   - 新寫一個讀 `search_document_profiles.visible_visit_count` 的查詢：這是每頁×每 profile 的值，沒有按次數排序的索引，也沒有日期維度（store_schema/archive.rs:283-289、334）。

### 4.4 標題 A–Z 與依網站（簡述）

**標題 A–Z：**

- 列上顯示的標題是「顯示的那筆訪問」所屬 URL 的 `u.title`（eligibility.rs:327），會隨 scope 改變。`search_documents.title` 是多個標題用 `|` 串起來的（search_documents.rs:56-59）。Tantivy 的 title 是分詞欄位，沒有 fast field（schema.rs:26-31、40-47）。
- Keyword 有兩條路：
  - `ensure` 多讀 `u.title`：每個 URL 多一次 urls 表查找，common 詞約多 2.9M 次；collector 要持有字串。
  - 新增 index 欄位：要升 `SCHEMA_VERSION` 並全量重建（加密 2.9M 頁約 35.6 s），而且欄位值不隨 scope 變。
- 中日文需要 locale collation。`icu_collator 2.3.1` 已在 Cargo.lock（是 `turso_core` 的間接依賴），照 AGENTS.md 改成直接依賴不算新依賴。
- Regex 的 url/title 本來就已讀出（regex.rs:164、222）。newest-first walk 裡，每頁第一次出現時鍵就固定了，所以有界 heap 可行。

**依網站：**

- 「網站」有三種定義：
  - `pages.registrable_domain`：照公共後綴清單切，有索引，Sites view 用這個。
  - `HistoryEntry.domain`：host（utils.rs:190-192）。
  - Tantivy `domain` STRING|FAST：也是 host；fast field 的 ordinal 只在 segment 內有效，跨 segment 比較要轉回字串。
- 「網站 A–Z → 新到舊」：每頁的鍵固定，有界 heap 即可。
- 「依網站分組、網站按符合訪問數排序」：這是新的結果形狀（一列一個網站），需要新的 wire 型別。
  - 適合在 count 迴圈裡聚合，因為每頁剛好經過一次（session.rs:253-258）。
  - 另需 page→site 對照（dense u32 約 11.6 MB）。
  - 再加一個 `HashMap<site, (visits, pages, newest)>`。

---

## 5. 性能護欄與測試

**Query plan 測試（在 `cargo test` 裡，屬於 `bun run check`）：**

- `posting_batches_seek_page_urls_and_visible_visits_without_archive_scans`（eligibility.rs:492-505）：`ensure` 的 SQL，不得 SCAN、不得 temp b-tree。
- `selective_scopes_seek_canonical_time_indexes_without_sorting`（506-538）：eager load。
- `each_chunk_seeks_to_the_cursor_instead_of_scanning_from_the_end`（regex.rs:390-405）：涵蓋新到舊與舊到新兩個方向。
- `candidate_pages_seek_their_urls_and_visits_without_scanning_the_archive`（407-427）：其中 `shown_visit_sql` 只測了 newest。
- `starred_page_union_seeks_both_page_indexes`（visits.rs:153-172）。
- **缺口：** hydrate 的 SQL、`shown_visit_sql(oldest=true)` 都沒有 plan 測試。

**順序正確性：**

- ported_grouped.rs:11-53：三種順序下，一筆一筆走等於一次大頁；總數正確；newest 同分時的順序。
- ported_grouped.rs:55-90：各順序顯示哪一筆訪問（oldest 顯示 5_000，其他顯示 9_530）。
- canonical.rs:61-84：Oldest + 計數 visitor。
- contracts.rs：Relevance 對照 brute force 與分頁。
- tests.rs:133-212：寫入或撤銷期間分頁，只測預設順序。
- ported_regex.rs：只測 newest。
- ai/tests.rs:410。
- E2E：
  - search-ranking.spec.ts：標題命中 > 網址命中；同分時新的在前。
  - ask-chats.spec.ts：「第一次看到」用 oldest。
  - search-limits.spec.ts：regex 找得到最舊的訪問。

**Benchmark（`#[ignore]`，手動跑）：**

- vault-core `search::benchmark::encrypted_archive_search`（benchmark.rs:16-140）：情境只有 common / medium / rare / date / profile，**全部是預設 Relevance**。fixture 每頁 1 個 URL、4–5 次訪問，分佈均勻。
- vault-search `tests/benchmark.rs`：只測 engine，也只有 Relevance。
- **沒有** Newest、Oldest、regex、偏斜分佈的規模量測。

---

## 6. Wire 型別

**Rust `SearchRequest`（types.rs:8-43）：**

- `#[serde(rename_all="camelCase", default)]`，沒有 `deny_unknown_fields`。
- 欄位：`query`、`sort: Option<String>`、`page_size`（預設 100，0 會報錯，session.rs:83）、`profile_id`、`browser_kind`、`domain`、`start_time_ms`、`end_time_ms`、`regex`、`fuzzy`、`starred`。
- `ranking_key` 標了 `#[serde(skip)]`，是內部欄位。

**TS（search.ts:22-34）與文件（search.md:35-47）：** 欄位相同，`sort?: 'relevance'|'newest'|'oldest'|null`。

**`HistoryQuery`：** Rust 在 models/archive.rs:672-699，TS 在 archive.ts:325-345，`sort` 原樣傳給 search（history.rs:21、154）。

**事件：** `SearchEvent` 為 `{type: indexer|step|error|cancelled, data}`（stream.rs:23-30）；`SearchStep` 見 types.rs:79-96 / search.ts:87-100。

**命令：**

| 命令                   | Tauri 參數            | Dev bridge                                                                       |
| ---------------------- | --------------------- | -------------------------------------------------------------------------------- |
| `start_search`         | `{request, onEvent}`  | `POST /commands/start_search {request}`（dev_ipc_bridge/search.rs:64-67、79-90） |
| `next_search_page`     | `{sessionId, cursor}` | `POST /commands/next_search_page {sessionId, cursor}`                            |
| `cancel_search`        | `{sessionId}`         | `POST /commands/cancel_search {sessionId}`                                       |
| `search_indexer_state` | —                     | `POST /commands/search_indexer_state`                                            |

**Dev bridge 事件串流：** `GET /searches/{sessionId}/events`，SSE 每則為 `data: <SearchEvent>`（router.rs:66）。

- 新的 Step 會取代佇列裡還是 provisional、或計數未精確的上一個 Step（search.rs:40-45）。
- Error / Cancelled 會清空佇列（36-39）。
- 斷開 SSE 就取消 session（153-163）。

---

## 7. 其他值得注意的事

- **計數一定會跑完。** `include_total:false` 與 `groupByUrl` 在搜尋路徑上沒有作用：`group_by_url` 沒有任何程式碼讀它，`include_total` 只影響回應欄位。同步 adapter 一定等計數精確。命令面板（limit 6）遇到 common 詞也得等計數跑完。
- **註解過期。** models/archive.rs:691-697 對 `group_by_url` 的說明（同一頁會在之後的 chunk 再出現、要把 visit_count 加起來）描述的是已刪除的 regex chunk 行為。
- **沒用到的程式碼與配置：**
  - `Filters.newest_visit` 從來沒人設定（filter.rs:24）。
  - vault-core 也沒用 `Filters.domain` 把 domain 推進 posting；domain chip 走 SQL 的 `instr`。
  - Regex session 會配置一份用不到的 Membership dense 陣列（session.rs:137）。
- **與 builder 有關：**
  - 只有 operator、沒有關鍵字的查詢（`site:`、`note:`、`filetype:`、只有 `-詞`）會編成 AllQuery（query.rs:52、62-65），每頁都要做 SQL 解析，成本等於 common 情境。這時 relevance 全部同分，結果實際上就是新到舊。
  - `intitle:` / `inurl:` 的運算元若有短於 3 字元的片段，也會退成 AllQuery（query.rs:154-156）。
  - `site:` 與 domain 篩選是對小寫完整 URL 做子字串比對（eligibility.rs:34-41），不是 host 比對。
  - Regex 會忽略所有 operator（session.rs:84-88）。
  - Regex 的 prefilter 只認 ASCII 片段，CJK pattern 一律全量 walk。
- **重複工作：** 非 Relevance 的順序下，rank walk 與 count walk 走的是完全相同、都不打分的 posting。可以合併成一條，但能省的只有 posting walk 的時間（約數百 ms），SQL 本來就共用。
