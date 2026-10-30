# BACKLOG — 後續工作塊佇列

> 這裡只放 **half-milestone 粒度**的 work blocks。  
> `STATUS.md` 清空或完成當前 block 後，才從這裡把下一個未被阻塞的 block 搬上去。

---

## 工作塊佇列（按順序）

> 2026-10-25：清掉已完成或前提已不存在的 block（v0.3 Paper 前端、已移除的 coverage / mutation gate、已修好的 flaky 測試等，清單見 CHANGELOG 同日）。2026-04 到 2026-06 的收尾註記也一併移除，要看去 git 歷史或 CHANGELOG。

- [ ] **WORK-INSIGHTS-COLD-READS** — 目標機大小的機器上，Insights 有卡片第一次讀要等一分鐘以上（2026-10-25 14.4M 量測找到；和「目標機上流暢」的硬指標衝突，下一輪先做）
  - 讀先：`docs/architecture/ipc-performance.md` §11（各讀取的量法與 2026-10-09 的表）與 §16 最後一段、`vault-worker/tests/archive_scale_bench/turso_intelligence.rs`。
  - 現象（Modal microVM 4 vCPU / 8 GB，release、加密，14.4M 造訪，預設形狀；資料最大 22.5 GB）：app 剛起來的第一次讀，interrupted habits 30 天 66 s、一年 68 s，全部時間的 digest 單獨讀 67 s，一年的第一批卡片 29.5 s；同一個讀取第二次 1 s 以內到 0.5 s。
  - 對照：2026-10-09 在 Modal EPYC VM、`habits` 形狀上，interrupted habits 第一次 2.7 s、全部時間 digest 2.9 s；形狀和機器都不同，不是同條件比較。一年的第一批在預設形狀上當時是 39.1 s，所以那一條不是新退步，而是一直都慢。
  - 猜測（沒驗證）：資料進不了 8 GB 的頁快取，第一次讀要從磁碟拿；Modal 的磁碟和真機不同，實際可能沒這麼慢，也可能更慢。
  - 「最常回來搜的」（`get_search_effectiveness` 的 hardest topics，`2351ba22` 起跨瀏覽器與引擎合併同一句搜尋）每一頁仍讀完範圍內所有 query family，跟合併之前一樣（舊的 SQL 排序也讀完範圍）；2026-10-29 本機 debug build 同一份資料比較：10 萬個 family 新讀法每頁 0.3 s、舊 SQL 0.16 s，30 萬個新讀法 1.4–1.6 s、舊 SQL 2.3–3.0 s。這次 14.4M 也量它的第一頁與下一頁；太慢就在建置時存好合併用的 key，讓 SQL 分組、只排前 k 個。
  - 要做：(1) 先量每條讀了多少頁、多少列（同一台小機器，預設與 `habits` 兩種形狀各一次），分清是讀太多還是磁碟慢；(2) 讀太多就改讀法（例如 interrupted habits 在建置時算好存起來、all-time digest 走快照）；(3) 不管多快，卡片等的時候都要有 skeleton 與漸進載入，不能讓整頁空著；(4) 結果寫回 `ipc-performance.md`。

- [ ] **WORK-FLAKY-BLOCK-CACHE-RSS** — `vault-crypto` 的 `the_block_cache_bounds_memory` 量常駐記憶體（RSS），記憶體緊時不準（2026-10-25 gate 連兩次失敗，同一晚 03:31 的整套 Rust 測試過了；vault-crypto 當晚沒改）
  - 現象：上限 2 MiB 的快取 RSS 漲 8.3 / 6.9 MiB，沒有上限的漲 11.1 / 4.9 MiB（它的快取最多 9.9 MiB），斷言要前者比後者少 4 MiB 以上。
  - 根因：測試的索引只有 16.6 MiB，兩種快取的差距本來就只比門檻大一點；macOS 記憶體緊時會壓縮頁面，RSS 不跟著快取長（那時另一個 session 在跑 benchmark，壓縮器裡有約 126 萬頁）。
  - 方向：斷言快取自己回報的用量（child 已經印出 `max_cache_mib`），RSS 只當上限的佐證；或把索引做大到差距遠超雜訊。不要只是放寬門檻。
  - 2026-10-25 16:13 Modal 上閒著的 Linux 跑過了，和「記憶體緊時才失敗」一致；測試本身的弱點還在。

- [ ] **WORK-BACKGROUND-DASHBOARD** — 後台任務做成 dashboard，不是設置頁（用戶 2026-10-15，說之後再做）
  - 現況：Settings → 後台任務是一列列設置卡片加工作清單。它其實是給人看「現在在跑什麼、跑多快、哪裡卡住」的地方：正在跑的工作、進度與速度、排隊的、失敗的與原因、可以暫停 / 繼續 / 取消 / 重試。
  - `WORK-AI-INDEX-CONTROL` 已做完（2026-10-15）：暫停 / 繼續 / 取消、會動的進度與速度、剩餘時間、工作結束的事件都有了，這頁可以直接用（`use-index-pace.ts`、`use-background-work.ts` 的工作動作、分頁的 `JobList`）。先讀 `docs/design/` 核心四份再設計。

- [ ] **WORK-INDEX-SEARCH-WHILE-BUILDING** — 換模型時，新索引一邊建一邊就能搜；建置由新到舊（2026-10-16 寫內建 transformer 的 trade-off 時提出，用戶同日同意）
  - 現況：換模型（例如從內建靜態模型換成 LM Studio 跑的 Qwen3-Embedding）後，搜尋一直用舊的完整索引，直到新索引整份建完；14.4M 造訪、外部服務時可能好幾天都用不到新模型。建置照 history ID 由小到大走，最常被搜的近期頁面最後才進新索引。
  - 提案（[inapp-transformer-embedding-tradeoff.md](../architecture/inapp-transformer-embedding-tradeoff.md)「Searching while it builds」「Build order」）：建置中的索引與完整的舊索引一起搜，兩份語意結果和全文結果以 RRF 融合，不同模型的分數不互相比較；建置改成由新到舊，cursor 仍是一條水位線、往下走。代價：建置期間每次搜尋要用兩個模型各算一次查詢向量，外部服務多一次請求。
  - 推翻 `semantic-index-control.md` 與 `intelligence.md` §1（未完成的索引只在沒有完整索引時才搜）：用戶 2026-10-16 同意。做的時候改這兩份文檔。和 `WORK-INDEX-REBUILD-SWAP` 一起設計：同一個模型重建時也是「兩份索引、一邊建一邊搜」，查詢向量只要算一次；等 REBUILD-SWAP 合進 main 再開工（2026-10-25 在做）。
  - 不做：外部服務不在時退回靜態模型、兩份索引永久並存（用戶 2026-10-16：不要）。

- [ ] **WORK-INDEX-ROW-CLEANUP-OFF-JOB** — 完整重建成功後，刪舊 lineage 的 provenance 列在工作執行緒上同步跑，14.4M 時約 28k 個寫入交易（2026-10-30 PR #16 第三輪審查找到，讀程式碼確認，沒量）
  - 現況：`indexing/backfill.rs` 結尾的 `generations::collect(vault, true)` 在 `INDEX_EXECUTION` 裡刪完舊列才返回，所以 `pathkeep://ai-index-job-ended` 要等刪完才發，排隊的下一個工作也跟著等。搜尋分組的暖機已經移到刪除之前（同日）。如果刪除改由背景 reaper 做，下一個工作開頭的 `collect` 會在 `admit()` 等它，停在 0%。
  - 方向（先量 14.4M 刪一個 lineage 要多久）：工作結束只做決定與改名，列的刪除交給 reaper；工作開頭的清理在已有 collector 時不等它（它本來就在做同一件事），但要先確認 `recover_pending` 等開頭清理不是建置的前提。

- [ ] **WORK-INTELLIGENCE-MIGRATION-OPEN** — Intelligence 資料庫的 schema 遷移失敗（例如磁碟滿）會讓整個存檔打不開，雖然語意索引是可選的衍生資料（2026-10-30 PR #16 第三輪審查找到）
  - 現況：vault-store 開檔時依序跑每個資料庫的遷移，任何一個失敗 `Store::open` 就失敗。v2 遷移已改成每批搬移，峰值只多一批，但磁碟幾乎滿時仍可能失敗，而且每次啟動都在同一點失敗，直到用戶騰出空間。
  - 要決定（寫 trade-off 再改）：Intelligence 遷移失敗時是否先開 archive、把 AI 功能標成不可用並說明原因；或啟動前先檢查空間並給明確的錯誤。牽涉 vault-store 的開檔契約，不是這個 PR 的範圍。

- [ ] **WORK-TURSO-MINMAX-WRAPPER-AUDIT** — `COALESCE(MAX(id),0)` 這種包了一層的 MAX 在 Turso 是全表掃描（2026-10-30 PR #16 第三輪審查找到）
  - 根因：Turso 0.8.1 `translate/optimizer/mod.rs` 的 `detect_simple_aggregate` 只在結果欄「正好是」聚合式時走 MIN/MAX 捷徑（沿索引或 rowid 直接走到極值）；外面包 `COALESCE`、`+1` 都不算，就退回掃描。Intelligence v2 遷移因此每批掃一次越長越大的新表，1M / 4M 量到 26.5 s / 129 s；改成不包、在 Rust 裡處理 NULL 之後 16.4 s / 66.9 s（`semantic-index-control.md`）。
  - 要查（14.4M 時每次都掃一遍的優先）：`intelligence/build_pipeline.rs:24`（每次 intelligence 建置掃 `visits`）、`ai/indexing/backfill.rs:413` 與 `:539`、`ai/generations/build.rs:145`、`ai/similar_searches.rs:198`、`ai/indexing/store_rows.rs:91`、`archive/doctor_derived.rs:76`，以及 `intelligence_runtime/enqueue.rs`、`job_control.rs` 的 `MAX(list_order)+1`（小表，可能不用改）。
  - 做法：先用 `EXPLAIN` 確認每一條真的掃表，再改成不包的 `MAX(...)` 並在 Rust 裡處理 NULL；有 WHERE 的要確認存取路徑能直接走到極值。結果記進 `turso-review-ledger.md`。

- [ ] **WORK-SEMANTIC-VISIT-SIDECAR** — 語意搜尋每次都讀一遍範圍內的所有造訪：100 萬次 385 ms，14.4M 估 5.5 秒（M5 Max），目標機更慢（2026-10-17 做完 `WORK-SEMANTIC-PIPELINE` 量到）
  - 現況：一次語意搜尋 100 萬次造訪 500 ms，其中 385 ms 是一條 SQL 串流範圍內每次造訪（算頁面成員、最新時間與每頁最高分）。向量掃描只要 6 ms。
  - 串流是一次讀取，整段佔住 archive 兩條讀取連線中的一條；writer 做 WAL truncate checkpoint 前要等進行中的讀取結束（`vault-store/src/checkpoint.rs` 的 `wait_for_readers`，`worker.rs` 呼叫），這段時間進來的寫入也跟著等。14.4M 時一次語意搜尋可能讓匯入、備份的寫入延後數秒。以前的逐批回查是很多次短讀取，不會這樣；日期篩選的全文搜尋（`eligibility::load`）本來就是同一種長讀取。
  - 方向（先設計、先量）：建索引時把每次造訪的頁面與時間投影到索引旁（跟 `.pkpos` 同一份或並排），加上撤銷造訪的集合；沒有篩選或只有日期篩選時，頁面成員與每頁分數在記憶體裡算完，用到網站、標籤、筆記、星號、profile 等篩選才走 SQL 串流。要處理：索引之後才進來的造訪（成員與最新時間仍要算到它們）、撤銷與還原、App Lock 時放掉記憶體、`semantic_stage_bench` 量 1M / 4M。

- [!] **WORK-VECTOR-MEMORY-AT-SCALE** — 大語料時完整 int8 排序放不進 8 GB（等用戶在研究文件「What this means」的 A / B / C 中選）
  - 大語料時完整 int8 排序放不進 8 GB，要嘛從磁碟讀、要嘛用緊湊碼排全部再重算前段（後段順序是近似的），要嘛回到候選池（推翻 2026-10-07 的完整計分）。Turbovec 2-bit 當第一段（挑 2,000 筆再用 int8 重算）在 256 維與 1,024 維都不掉召回；sign bits 只在高維夠用。這只在 13 萬頁上量過（2,000 筆佔 1.5%）；到幾百萬頁時候選數多半要跟著放大，選 B 或 C 之前要先在幾百萬頁的真實語料上量。

- [!] **WORK-DECISION-MODELS** — 量決策模型（Jev、OpenAI Decisions、開源 pplx-decider 等）在 PathKeep 的用處（用戶 2026-10-17：磁碟沒空間，之後再說）
  - 讀先：[2026-10-decision-models.md](../architecture/research/2026-10-decision-models.md)。
  - 先量：在 LM Studio 下載一個決策模型（3–25 GB，先問用戶），量這台 Mac 與目標機的速度、在標題＋網址（中英混雜）上的準確度。候選用途：每月固定類別、Ask 的篩選工具與語意結果重排、送雲端前的本機敏感頁檢查。

- [ ] **WORK-QUIT-FOLLOWUPS** — 結束前詢問的收尾（2026-10-25 `WORK-QUIT-DURING-WORK` 留下）
  - 更新後重開（`relaunch_after_update` → `RESTART_EXIT_CODE`）不攔：匯出中按了更新重開，工作照樣中斷。方向：有工作在跑時，About 頁的更新先說正在做什麼，等做完或確認再重開。
  - E2E 只驗了匯出；匯入、備份、維護、復原匯入、兩種工作同時、問題開著時工作先做完，都還沒有 E2E。「現在結束」不先停下工作，匯出的暫存檔會留在目標旁。

- [ ] **WORK-SEARCH-OR-OPERATORS** — `OR` 也能連運算子：`site:a.com OR site:b.com`、`intitle:x OR inurl:y`（2026-10-25 把 `OR` 改成 Google 的讀法時留下）
  - 現況：`OR` 只連兩邊的字或引號片語；旁邊是運算子或減號時什麼都不連（`search.md`），所以 `site:a OR site:b` 還是兩個都要，什麼都找不到。Google 允許這樣寫。
  - 為什麼沒一起做：`site:`、`intitle:`、`inurl:`、`filetype:`、`tag:`、`note:` 是 canonical SQL 上的 AND 條件（`eligibility::Scope`），索引那層的候選也照 AND 算；要讓它們能 OR，條件要變成一棵 AND / OR 樹，SQL 與索引候選兩層都得照這棵樹算，還要保持完整覆蓋與現在的查詢計畫。
  - 要做：先設計（解析成樹、兩層怎麼各自求值、14.4M 的查詢計畫），再做；E2E 用 fixture 驗 `site:a OR site:b` 找到兩個網站的頁、`-site:` 與 OR 一起用的情形。

- [ ] **WORK-SEARCH-QUOTED-LITERALS** — 引號裡的字照字面找：`"site:x"`、`"-beta"` 現在還是被讀成運算子或排除（2026-10-29 審查 PR 16 時找到，進階搜尋的表單因此帶不出這種字）
  - 現況：`archive/search_query.rs` 先看運算子與開頭的 `-`，再看是否在引號裡，所以整段加了引號也還是運算子或排除；`intitle:"release notes"` 這種運算子後面接引號值的寫法要照舊。Google 把整段引號當字面。
  - 要做：tokenizer 記下一個 token 是不是從引號開始，從引號開始的不當運算子、不當排除；`search/query.rs` 的 `OR` 分組讀同一個判斷。進階搜尋的 `words-query.ts` 就能把字面的 `site:x`、`-beta` 加引號送出，拿掉它的註解裡「quoted, still an operator」那條例外。E2E：搜一個標題含 `site:` 字樣的頁、一個以 `-` 開頭的字。

- [ ] **WORK-REGEX-NEXT-PAGE-REWALK** — 沒有字面片段的 regex，每一頁都從頭讀 archive（2026-10-25 加「最常瀏覽」時找到）
  - 現況：`regex.rs` 的 `next_page` 把走訪重設到起點（`reset_scan`）。時間排序要先讀過 cursor 之前所有比它新（或舊）的造訪才到下一頁；「最常瀏覽」要整個 archive 重讀一次、重建每頁的計數。14.4M 時，一個沒有三個字元以上字面片段的 regex，每多一頁就是一次完整走訪。
  - 方向（先量）：時間排序從 cursor 的時間接著讀（seek，跟第一頁一樣）；「最常瀏覽」在 session 留著走完的計數（記憶體跟符合的頁數成正比，最多約 170 MB，見 `search.md`），之後各頁直接從裡面排；在 App Lock、session 結束時放掉。量 1M / 14.4M 的第二頁時間與記憶體。

- [ ] **WORK-SEARCH-NEAR-DUPLICATES** — 搜尋結果合併近似重複：同一篇文章的不同網址、AMP、鏡像（`WORK-EMBED-USES` 剩下的候選，2026-10-17）
  - 2026-10-25 審查：先做研究，不先寫碼。在真實資料的測試副本上（只看數字）分類一批查詢前段結果裡的近似重複（同網站同標題、AMP 與行動版主機、跨站鏡像、只差追蹤參數；先查 `canonical_url` 已經合併了哪些），抽樣標錯合的比例；再寫 trade-off（合成一列「另有 N 個網址」並能展開，讓每個網址都找得到；計數算頁還是算文章；在排名時還是顯示時合併）。它改的是「一頁一列」的結果契約，要用戶決定哪幾類能合。
  - 相關頁面在真實歷史上看到約三成的近鄰是同一頁換了網址；`ai/related_pages.rs` 的折疊規則（可註冊網域＋標題主體，去掉計數括號）可以沿用。搜尋結果要合併的範圍更大（跨網站的鏡像、AMP），先在真實資料上量哪些情況值得合、合錯的代價。
  - 讀先：[2026-10-related-pages.md](../architecture/research/2026-10-related-pages.md)、`docs/architecture/search.md`。

- [ ] **WORK-HISTORY-SEARCH-NOTES-E2E** — History 的語意搜尋提示還沒有 E2E（2026-10-25 做完 `WORK-HISTORY-SEARCH-NOTES` 的前後端時留下）
  - 已做：History 在結果上方照語意搜尋回的提示說一句（上一個模型、索引是空的、篩掉了、設定變了、連不上服務），連到 Settings → AI；服務算不出查詢向量時改用字詞搜尋並帶 `providerResolutionFailed`。
  - 要做：`semantic-index.spec`（restart 專案）在換模型、新索引還沒建完時，History 的語意搜尋寫「所選模型的索引還沒建完，所以先用上一個模型的索引」；讓假的嵌入服務對查詢回錯，History 寫「連不上嵌入服務…」並列出字詞結果。

- [!] **WORK-SEED-TURSO-ASSERT** — debug 版的 14.4M 種子資料觸發 Turso 的 reserved-bytes 斷言（2026-10-15 Codex 量 Takeout 匯入時看到，release 版沒事，根因沒查）[!blocked: 要 14.4M 的 debug 種子存檔，約 40 GB，這台 Mac 放不下；Modal 上 5 小時種不完（2026-10-25），要一次更長的執行。斷言的原文當時沒記下]
  - 先在 debug 版重現，看是我們寫入的方式還是 Turso 的 bug；是 Turso 的就照 AGENTS.md「不對外」在我們這邊繞過並記下重現。
  - 2026-10-25 在 Modal 試了一次（8 vCPU / 32 GB，磁碟 495 GB，上限 5 小時）：debug 版的 14.4M 種子沒有在 5 小時內結束（工作沒寫下結束碼：停在上限，或 sandbox 中途沒了，分不出來）；輸出寫在 artifact 裡，當時的 offload 只在工作結束時打包 artifact，所以沒有留下（offload 之後改成每 10 分鐘打包一次）。沒有重現，也沒有排除。下次給更長的上限，或只種到斷言出現為止。

- [ ] **WORK-SEARCH-FAMILY-ACROSS-PROFILES** — 同一族搜尋在不同設定檔各算一族，「所有瀏覽器」的清單仍會列兩行（2026-10-25 `WORK-SEARCH-FAMILY-DUPLICATE` 做了一半留下）
  - 已做（2026-10-25）：Insights「最常回來再搜的搜尋」在所有瀏覽器時，同一句（`search_family_key`）在不同設定檔、不同搜尋引擎合成一行，次數相加。原本 `台北天氣` 在兩個設定檔各一行、文字一模一樣的情形不會再有。
  - 還沒做：族是照設定檔與搜尋引擎建的（`QueryFamilyAccumulator::scope`，`family:{profile}:{n}`）。不同設定檔的同一族如果第一句不同（fixture 裡哪一句先搜依它的 `now` 每天不同，兩個設定檔可能不一樣），合併只看第一句，就還是兩行。「你回頭再看的事」（Insights）與 Home 的常回來的主題也照設定檔各列一行（`oneThreadPerAnchor` 的 anchor 是各設定檔的 family id）。
  - 要做：先決定跨設定檔的族怎麼認同一族（兩族有共同的寫法？照族的成員比對？），再改這三處；E2E 用 fixture 的兩個設定檔驗「台北天氣」與「台北天氣預報」在所有瀏覽器只列一行。

- [ ] **WORK-DEPS-LATEST** — 依賴跟到最新版（用戶 2026-10-15：之前的 agent 過度 pin）
  - 0.x 寫 `"0.34.0"` 等於鎖在 0.34.x，要手動升（rig 已在 `WORK-AI-AGENT-TURNS` 換掉）。
  - 列出所有落後的 Rust 與 JS 依賴：先升範圍內的（`cargo update`、`bun update`），major 一個一個升（tokenizers、getrandom、lucide-react 的圖示改名、TypeScript / ESLint、tauri 與它的 plugins），合在一次 gate 裡跑，失敗才二分；要鎖舊版的在宣告旁寫理由（AGENTS.md 供應鏈）。最後跑 `check:supply-chain`。
  - 2026-10-18 發布 v0.4.0 前為了清 RustSec 先升了 rustls、wasmtime、zip（到 9）、rmcp（到 3）、tar、serde_with、openssl（CHANGELOG）。其餘的還沒掃。

- [!] **WORK-WINDOWS-SNAP-LAYOUTS** — Windows 11 的貼齊版面與自畫視窗按鈕的實機確認（2026-10-13 `WORK-UI-POLISH-2026-10` 留下）[!blocked: 要一台 Windows 11 與 GNOME / KDE 的 Linux 桌面來建置與實看]
  - 現況：Windows、Linux 已關掉系統標題列，最小化 / 最大化 / 關閉由 app 畫在右上角（`window-controls.tsx`），從沒在兩個平台的實機上看過。滑鼠停在最大化上時 Windows 11 的貼齊版面不會出現。
  - 為什麼還沒做：要讓系統把我們畫的按鈕當成最大化按鈕，得在 Rust 裡回應 `WM_NCHITTEST`（`windows` crate），但 WebView2 的子視窗蓋住整個內容區，訊息可能到不了頂層視窗；這台 Mac 也編不了 Windows 版（`ring`、`aws-lc-sys` 要 mingw），寫了無法建置也無法試。
  - 要做：在 Windows 上建置，先確認縮放邊、拖到螢幕邊緣（Aero Snap）、雙擊最大化、深淺色、按鈕 hover 與鍵盤都對；再試 `WM_NCHITTEST` 子類化或 WebView2 的 non-client region；Linux 在 GNOME 與 KDE 各看一次。

- [ ] **WORK-RECLASSIFY-STALL** — 14.4M 上改一個網站的類別，建置卡在第一頁之前；修好後仍要 16 分鐘（2026-10-13 `WORK-SCALE-BENCH-RERUN` 找到）
  - 現象：`turso_intelligence` 的 `categories` 階段（覆寫 `site1.test` 的類別後做一次增量建置）在 visit-derived-facts 結束後，Intelligence 寫入執行緒單核跑滿 30 分鐘以上，沒有任何 reclassify 進度；之前「全部頁重新分類」的版本是 62 s。使用者在網站頁改類別就是這條路。
  - 已排除：reclassify 每個網址的兩條讀取（`archive.visits` 依 `idx_visits_visible_url_time` 找、再接 facts / assignments）都走索引，有計畫測試守著。
  - 根本原因（同日找到並修好，未 gate、未提交）：重寫一筆造訪 facts 的 `UPDATE visit_derived_facts_rows … WHERE generation=?1 AND visit_id=?2`，planner 選了 `idx_vdf_generation_canonical_visit` 只用 `generation=?` 找，每筆造訪都讀過這一代全部 14.4M 筆 facts。小資料看不出來。改成 `INDEXED BY idx_vdf_visit_generation`，計畫變成 `(visit_id=? AND generation=?)`；`reclassify.rs` 的計畫測試要求 `visit_id=?`（原本只擋 SCAN，沒擋到這種只用前綴的 SEARCH）。reclassify 4 條測試（含與從頭建一次相等）過。
  - 2026-10-14 在 14.4M 上重量（修正已提交 `f4301c1f`）：跑得完了，958 s，峰值 RSS 1.5 GB。reclassify 只走 site1.test 的 144 個網址（21 s），daily rollups 845 s（1,669 天，site1.test 每天都有造訪，每一天整天重算），概覽快照約 90 s。卡住的問題解決；但「改一個網站的類別」在這台 Mac 上要 16 分鐘，目標機（4 核）更久。
  - 2026-10-25 在 4 vCPU / 8 GB 上量：1,220 s、峰值 1.12 GiB；重分 144 個網址 35 s，daily rollups 1,013 s（1,669 天）。比這台 Mac 慢約 27%，還是整天重算。
  - 還要做：(1) 改了類別時只重算類別那一種彙總（`category_daily_rollups` 與受影響的 summary），用移動的造訪算增減，不要整天重讀（網域、引擎那幾種跟類別無關）；(2) 144 個網址、約 720 筆造訪的 reclassify 要 21 s，每筆改寫約 29 ms，查清楚；(3) 把其他「依鍵改一列」的 UPDATE / DELETE 也用計畫測試掃一遍（同一種 planner 選錯索引）；(4) 結果寫回 `ipc-performance.md`。

- [ ] **WORK-SCALE-BENCH-RERUN** — 補量 14.4M：改回本機跑之後沒量的數字（2026-10-11；用戶指示「先把所有任務做完，再處理這個真實測速」）
  - 讀先：`docs/architecture/ipc-performance.md`、`vault-worker/tests/archive_scale_bench/`（`turso_intelligence.rs`、`turso_reads.rs`）。
  - 前提：14.4M 的測試存檔最多約 40 GB，Modal 已停用，在這台 Mac 上跑。2026-10-11 用戶騰出空間後剩約 77 GB，放得下（另留 5 GB 安全線）；開跑前再看一次，量完刪掉測試存檔。
  - 待量：改一個網站的類別到洞察照新類別建好要多久（`WORK-SITE-CATEGORY-REVIEW`：只重分那個網站的頁，成本照那個網站的網址數與日子數，沒在 14.4M 上量過）；`WORK-EVIDENCE-REIMPORT` 的重新匯入在 14.4M 上多花多少時間（2026-10-12 已改成不再寫重複的證據，20 萬筆的量測見 `ipc-performance.md` §15：第一次匯入的時間不變、再匯入 0 列；14.4M 上要量第一次匯入的唯一索引與再匯入的耗時）。之後的 block 沒量到的 14.4M 數字也記在這裡。
  - 做法：照 `ipc-performance.md` 各節的量法在 14.4M 上跑，結果寫回各節與原 block 的 CHANGELOG 附註。
  - 2026-10-13 第一次在這台 Mac 上跑（`turso_intelligence::encrypted_intelligence_at_scale`，release，加密，14.4M 造訪、2.88M 頁；Apple Silicon 18 核 64 GB，比目標機快得多）：完整建置 4,333 s（visit-derived-facts 1,443 s、daily rollups 863 s、sessions 774 s、refind pages 1,146 s），峰值 RSS 462 MiB，建置中的互動寫入等 0.3 ms；概覽 cold 1.67 s / warm 122 ms，網站頁 32 / 6 ms，sessions 1 ms；全部匯出 5.0 GB、289 s，**峰值 RSS 2.07 GB**（8 GB 目標機上偏高，要查是匯出本身還是 turso 的頁快取）；增量（新增 1 萬筆造訪）140 s，**峰值 RSS 1.26 GB**（比完整建置還高，要查）。
  - 「改網站類別」卡住：`categories` 階段在 visit-derived-facts（11 s）之後，Intelligence 的寫入執行緒單核跑滿 30 分鐘以上，一次 reclassify 進度都沒回報，停掉。之前的量法（全部頁重新分類）是 62 s。查過：每個網址的兩條讀取都走索引（`intelligence/tests/reclassify.rs` 的計畫測試），不是它們。下一步見 `WORK-RECLASSIFY-STALL`。
  - 2026-10-14 重跑一次（含 `f4301c1f`）：完整建置 4,288 s、峰值 RSS 410 MiB；增量 134 s、1.26 GB；改網站類別 958 s、1.5 GB（見 `WORK-RECLASSIFY-STALL`）。兩次的完整建置與增量相差不到 5%。
  - 2026-10-14 補量 `WORK-EVIDENCE-REIMPORT`：14.4M 的存檔上匯入 100 萬筆 256.8 s，同一份再匯入 24.8 s、0 列（`ipc-performance.md` §15）。改網站類別見 §16。這一項只剩兩個記憶體峰值要查：增量 1.26 GB、匯出 2.07 GB。
  - 2026-10-25 第一次在目標機大小的機器上跑（Modal microVM，4 vCPU / 8 GB，load average 1.0，資料最大 22.5 GB；數字在 `ipc-performance.md` §16 最後）：完整建置 5,179 s、462 MiB；增量 213 s、646 MiB；改網站類別 1,220 s；匯出 394 s、993 MiB。兩個記憶體峰值在 8 GB 上都在 1 GB 上下，是隨可用記憶體長的快取，不是工作本身，這兩項結案。
  - 同一次找到的慢讀取：見 `WORK-INSIGHTS-COLD-READS`。

> 2026-07-26 coherence-sweep closeout note：使用者插單的全庫「逆天點」審查（`WORK-REVIEW-2026-07-26-COHERENCE-SWEEP`，見 CHANGELOG）已完成並全 gate 綠。結構化 `CommandError` envelope、十餘條 stable-code copy 通道、六個影子翻譯表剷除、posture/boundary-map 文檔如實化都已落地。待使用者拍板：`?layout=paper` QA mounts 去留、og:image 預設是否翻 `OnDemand`、onboarding 一次性 egress 披露。

- [ ] **WORK-PERF-OG-QUEUE-A** — Background OG Image Refetch Queue
  - 2026-10-25 審查：原本卡住的「前端進度契約」已經不需要（連結預覽在 Settings → 後台任務有自己的一列）。剩下的範圍照審查是「備份之後的連結預覽補抓不擋在備份的路徑上」；開工時先確認這個前提。
  - 讀先：
    `docs/features/explorer-browse.md`
    `docs/architecture/data-model.md`
    `src-tauri/crates/vault-core/src/archive/history/og_images.rs`
    `src-tauri/crates/vault-core/src/archive/history/og_images_fetch.rs`
    `src-tauri/crates/vault-worker/src/archive_flows.rs`
    `src-tauri/src/commands/archive.rs`
    `src-tauri/src/dev_ipc_bridge/dispatch.rs`
    `src/pages/explorer/hooks/use-explorer-og-images.ts`
  - 觀察（2026-05-21）：`trigger_og_image_refetch` 是 fire-and-forget 從 FE 角度看是不阻塞 UI（dev bridge 也已包了 spawn_blocking），但 backend 端單次 batch 20 URL 仍會花 ~18s 把一條 dispatch 連線占住做網路+HTML 解析，背景成本不透明。
  - 目標：把 refetch 拆成 enqueue + worker：command 立即返回 `{ enqueued, alreadyQueued }`，新 `og_image_fetch_jobs` 表（或 reuse 既有 intelligence queue）紀錄 pending URL 狀態；新 worker thread 處理 fetch，per-host rate limit 與 retry policy 集中在 worker；FE 改成 long-poll 或 SSE 觀察進度（沿用 import progress event 模式），UI 上 og:image cell 顯示骨架直到 worker 寫回。
  - 契約：失敗仍要產 negative-cache row（既有合約不變）；UI 不得因 og:image 失敗轉成全頁錯誤；新 worker 要 honor user-agent / rate limit / timeout 既有設定；不得引入新依賴除非經審核。
  - 驗收：batch 100+ URL 時 UI 維持 60fps；`bun run check`；新增 worker + FE polling 測試；docs/features/explorer-browse.md 更新 fetch lifecycle 圖。

- [ ] **WORK-REVIEW-0612-FOLLOWUPS-A** — Deferred findings from the 2026-06-12 deep code review
  - 來源：`WORK-REVIEW-0612-HARDENING-A`（見 `CHANGELOG.md`）。當時已修掉可驗證且範圍受控的安全/正確性 bug；下列是已確認為真、但屬「大型重構 / 需 Accepted-doc 決策 / 需實機視覺驗證 / 低優先」而**刻意延後**的項目。**不要**在沒有對應 slice 與覆蓋計畫下硬塞進別的 block。
  - **Intelligence 規模化效能（最高優先，需先做審查階段）**：以下都在 14.4M-visit 目標機上違反「禁止全量加載」。多數位於 >1000 行的 intelligence owner，必須先走大檔重構兩階段（審查→執行）：
    - 語意 index 全量 build 把整張 visits 表收進 `Vec<IndexedVisit>`（`ai/indexing.rs`，no-limit 分支），且在 v0.3 bail 之前就分配。
    - structural rebuild accumulator 的 `visit_ids: Vec<i64>` 與 evidence_json 的 `visitIds` 無上限（`intelligence/intelligence_structural_aggregates.rs`）。
    - query-family 分組 O(events × families) 且每次比較重新 tokenize（`intelligence/intelligence_structural_build.rs`）。
    - scoped-debug rebuild 的 `limit` 在全量 materialize + 分類之後才裁剪（`intelligence/intelligence_visit_records.rs`）。
    - 增量 / fallback daily-rollup 與 phase_three/phase_four read models 把無上限 `Vec` 收進記憶體（`phase_three.rs`、`phase_four.rs`；2026-10-12 刪掉的 `intelligence_daily_rollups.rs` 只剩測試在用）；另含增量 vs fallback 對 `domain_category` 的「first-seen vs most-frequent」不一致與雙 local-day 實作。
  - **Intelligence job lease / recovery 強化**：`recover_expired_intelligence_jobs` 無 attempt cap 地把過期 `running` job requeue（300s lease + 每 profile 才一次 heartbeat），可造成重跑 / 雙執行；enrichment job 在 claim 後失敗（`store_enrichment` 等）會被 `let _ =` 吞掉而永遠 `running`。修法：背景 heartbeat 續租或每 batch 進度、recovery 加 attempt cap、worker 端 on-Err `mark_intelligence_job_failed`。需與 lease 設計一起做並補測。
  - **Parser（需 Accepted-doc 決策 / 語義變更）**：~~Chromium `visit_duration`（µs）以 `_ms` 命名 verbatim 存~~（2026-10-09 已修：寫入時換成毫秒，升級轉換時把舊資料除以 1000，見 `docs/architecture/visit-duration-unit-tradeoff.md`）；`PRAGMA quick_check`（無界）每次 Browser-Direct import/preview 全庫掃描（`takeout/browser_history/staging.rs`）；orphan visit 因 INNER JOIN 靜默丟棄（chromium/firefox/safari），需 warn 或 LEFT JOIN。
  - **History regex recall 無界**：`list_history_with_regex` 綁 `:pageLimit = -1` 後全量 materialize 再過濾（`archive/history.rs`）。修法需 bounded 候選掃描 + 三語誠實提示「已掃描前 N 筆，請縮小範圍」(en/zh-CN/zh-TW) 與 response contract 欄位。
  - **App-lock KDF 強度（需新增依賴 + 遷移）**：passcode 走 120k 輪 SHA-256，建議改 argon2id（RustCrypto，符合 trust gate 但需依賴授權）+ 提高最小熵 + 線上 attempt 鎖定；同時 rekey 前的 plaintext snapshot（`archive/maintenance.rs`）在 Plaintext→Encrypted 後仍留明文，需加密或誠實提示並清理；`UnlockAppSessionRequest`/`SetAppLockPasscodeRequest` 的 `Debug` 應 redact passcode。
  - **og:image SSRF 殘留**：目前 pre-flight 守住「page URL / og:image 直接指向私網」與最終 host；但 `reqwest` 仍會自動跟隨 redirect 到私網（中間 hop 仍會發 GET）。徹底封堵需自管 redirect-follow + 逐 hop 驗證（hermetic 覆蓋需可注入 resolver/transport）。
  - **Frontend（需實機視覺驗證 / 中等）**：~~Explorer sticky day-header 的 `top` offset 由 ResizeObserver→setState 驅動，filter chips 換行時有 1 幀延遲導致短暫脫離 sticky~~ — **已修（2026-06-13，見 CHANGELOG `WORK-REVIEW-0612-FOLLOWUPS-A`）**：改用 `--pk-toolbar-h` CSS custom property，由 `useLayoutEffect` 內的 ResizeObserver 直接寫入(無 React render 中介);剩餘 interactive chip-wrap/scroll pixel 驗收待在實機 app 補眼。其餘未做：`renderedTimeResults`→`groupEntriesByDay` 在每次 favicon/og cache mutation 對整個累積集 O(N log N) 重排+重建物件（建議把 icon hydration 與 grouped data 解耦）；`useBrowseDayInsightsCache.resolve` 在 render 期間發 IPC；IntersectionObserver `root` 假設 viewport 是 scroller 但實際是 `<main>`；link-previews rebuild 在 `refreshStats()` 完成前就顯示成功。
  - **Frontend lib（中等）**：`readRequestCache` / `overviewCache` 無界增長（建議 LRU）；`invokeCachedRead` 的 `force` 在有非 forced in-flight 時被忽略；`flattenDictionary` bare-alias 跨 namespace 葉鍵碰撞（目前無 live hit，建議加 catalog 測試斷言無碰撞，或移除 bare alias）。
  - **Build/tooling（低）**：desktop-bridge truth gate 用 `Math.random` 端口偏移 + `retries:2`，跨 run / 殘留進程可能撞埠造成偽紅（建議 OS ephemeral port）；native-deps cache 的 broad `restore-keys` 可能在 baseline bump 後重用舊樹；rusqlite `bundled-sqlcipher-vendored-openssl` 的 vendored C compile 與 native-dependency ADR 措辭需補一句一致性說明。
  - 契約：每一項落地時都要先確認屬於哪個 owner / slice、>1000 行檔案先走兩階段、保持 100% JS/Rust coverage 與 mutation gate；Accepted-doc 項目（visit_duration 單位、KDF/snapshot 安全模型）必須先產出 trade-off 文檔並徵得使用者同意。

- [ ] **WORK-AI-MAPREDUCE-A** — Sub-agent time-chunk map-reduce for whole-history analysis（需先做設計階段）
  - 來源：2026-06-24 使用者驗收。使用者要「讓 agent 在知道 what's ahead 的狀況下拿到完整數據」。當時已做 **option A**（`run_code` 在沙箱內分頁聚合完整數據——rows 不進 context，只回 distilled 結果；見 CHANGELOG 的 sandbox 完整聚合條目）。本 block 是 **option C**：當單一 context 裝不下、或需要「逐塊的 LLM 質性分析」（不只是 JS 統計聚合）時，用 sub-agent map-reduce 跨時間塊分析整段歷史。
  - 目標（先設計，再實作）：
    - 一個確定性 orchestrator（worker 端，非讓弱模型自己 spawn）把時間軸切成塊（依時間窗或資料量），每塊派一個**全新 context 的 sub-agent**，給它該塊的完整數據存取（search/run_code/intelligence_report 限定該塊範圍），各自回一個**結構化摘要**，主 agent 再 synthesize 成最終答案。對應「分析我這一年/整段歷史」這種單一 context 裝不下的質性問題。
    - 形狀參考 Claude 的 workflow/sub-agent map-reduce，但落在 PathKeep 的 local-first worker 上：要能 spawn 多個 `drive_agent_run`（或一個 map-reduce harness）、chunk 規劃、結果 schema + 聚合、跨 N 個 agent 的 token budget / 取消 / journaling、以及進度可觀測（使用者看得到 fan-out，呼應 [[WORK-AI-INDEX-OBS-A]] 的 observability 精神）。
    - 與 option A 的邊界要寫清楚：純統計聚合 → A（一個 run_code 沙箱內搞定，便宜）；需要每塊 LLM 質性判讀或資料超過單 context → C。
  - 讀先：
    `docs/architecture/ai-security-posture.md`
    `docs/features/intelligence-and-ai.md`（AI/assistant 規格）
    `src-tauri/crates/vault-core/src/ai/agent_harness.rs`（`drive_agent_run` 迴圈 / budget / journal / cancel）
    `src-tauri/crates/vault-worker/src/intelligence/chat.rs`（`run_agent_stream` / `ai_chat_send` 的 thread + runtime）
    `src-tauri/crates/vault-core/src/ai/agent_tools.rs`（工具表面，sub-agent 也要用）
  - 契約：本機跑 N×LLM 呼叫成本高，要 bound + 進度回報 + 可取消；前端流暢度硬指標（不阻塞主線程）；AI 可選、無 provider 安靜不出現；i18n×3；100% JS/Rust coverage + lethal tests；誠實態（部分失敗的塊要誠實標示，不靜默吞）。先產出 design 文檔（chunk 策略、result schema、budget 模型、failure/部分結果語義、UI）再動代碼。
  - 驗收：對「總結我這一整年的瀏覽」這類問題，orchestrator 正確切塊、各 sub-agent 跑完回結構化結果、主 agent 合成出涵蓋全段、誠實標示任何缺漏的答案；全程進度可見、可取消；`bun run check`；更新 `docs/features/` 與 `docs/architecture/`。

- [ ] **WORK-AI-CTX-BUDGET-A** — Context-budget hardening for the agent loop（從 ai-redesign 審查 defer）
  - 2026-10-25：第 3 項（模型沒寫出回答就停時要說）已做（`noAnswer` 提示）。審查認為第 2 項（視窗）已做、第 4 項已做，tokenizer 的部分被 `intelligence.md` 取代；剩下第 1 項：每一次送出的請求都要在預算內，不只最後的整理回合。開工時先確認。
  - 來源：2026-06-24 移除 8-step cap + 強制 final-synthesis turn 後的審查。當時已修：64-step backstop、live-context gate（量「最後一輪 prompt」而非 cumulative）、無 usage 時 fallback 估算（`estimate_context_tokens`）、ceiling → 強制 tool-free synthesis turn。以下是已知殘留邊界，本 block 把它們做成長期最優解：
    - **synthesis turn 自身可能 overflow**：gate 量的是「剛跑完那輪」的 prompt（`last_prompt_tokens`），但 synthesis turn 還會把「該輪新 threaded 的 tool results」+ directive 再送一次。若最後一輪 `run_code` 回了接近 256KB 的結果且 context 已逼近 budget，這個「救援回合」反而可能撞 context-length（→ 誠實的 provider error / Failed，note 已先發，不是靜默）。要：gate 改用「含本輪新結果的投影大小」，或組 synthesis request 時丟掉/截斷最肥的那筆剛 threaded 的結果（模型本就被要求「用已收集到的證據」作答）。
    - **budget 是寫死的 110k，不知道 provider 真實 window**：`capabilities().max_context_tokens` 目前是 `None`。小 context 模型（如 32k）會在 budget gate 觸發前就被 provider 報 context-length 錯。要把 provider 回報的真實 window 接進來推導 budget（無回報時 fallback 110k），並把 `estimate_context_tokens` 換成真 tokenizer 估算（CJK 比例 char/token 與英文差很多，現在的 bytes/4 只是粗估）。
    - **synthesis turn 回空字串無 fallback**：模型若硬是在 tool-free turn 只吐一個（無效的）tool call、不吐 text，使用者會拿到 note + 證據 chips 但沒有 prose——等於那個 case 又「靜默」了。要：偵測 `accumulated.text` 為空時，串一條 deterministic 的最小摘要（localized），讓「never silent」契約對不聽話的模型也成立。
    - **synthesis directive 未進 journal**：trace replay 重建得到一段沒有可見成因的 assistant 答案。要把 directive（或一個「final-synthesis」marker）寫進 journal payload，維持「trace = 忠實重放」。
  - 讀先：
    `src-tauri/crates/vault-core/src/ai/agent_harness.rs`（`drive_agent_run` 迴圈、`run_final_synthesis_turn`、`estimate_context_tokens`、`DEFAULT_TOKEN_BUDGET` / `DEFAULT_MAX_ITERATIONS`）
    `src-tauri/crates/vault-core/src/ai/llm.rs`（`capabilities()` / `to_llm_usage` / streaming usage）
    `src-tauri/crates/vault-core/src/ai/traits.rs`（`LlmCapabilities::max_context_tokens`）
    `docs/architecture/ai-security-posture.md`
  - 契約：誠實態（寧可誠實 provider error 也不靜默吞；部分結果要標示）；前端流暢度不受影響；AI 可選；i18n×3；100% JS/Rust coverage + lethal tests（含 no-usage、小 window、超大最後一筆結果、空答 fallback 的案例）；`bun run check`。
  - 驗收：構造「context 逼近 budget 時最後一輪回超大結果」的測試，synthesis turn 不再 overflow（或誠實降級且有測試證明）；小 window provider 走 graceful 路徑；no-usage provider 用真 tokenizer 估算；空答有 fallback 摘要；更新 `docs/architecture/`。

- [ ] **WORK-ENCRYPT-AT-REST-ALL** — 評估並（評估後）實作「全量加密」衍生資料庫（需先做設計/性能評估階段）
  - 2026-10 狀態：Turso 版每個原生資料庫（archive、source evidence、intelligence、agent、scratch）都用同一把資料金鑰加密，搜尋索引用加密的 Tantivy Directory。下面提到的 SQLCipher 原語（`at_rest.rs` reconcile、`open_archive_connection`、`search_projection.rs`、`intelligence_projection.rs`）已刪除。剩下要評估的只有語意搜尋的平面檔（`.pkvec` / `.pkmap` / `.pkbin` / `.pki8`）是否加密。
  - 來源：2026-06-28 使用者指示。修 backup OOM 時確認：開啟加密只加密 canonical encrypted-tier（`history-vault.sqlite` + `source-evidence.sqlite`）；衍生 sidecar（`derived/history-search.sqlite` FTS、`derived/history-intelligence.sqlite`、`derived/agent.sqlite` 對話）目前**明文**落盤，內含 URL / 標題 / enrichment 摘要 / 聊天內容。使用者選擇當時先只修 source-evidence，並要求把「encrypt EVERYTHING 的性能與實現設計評估」記成待辦，評估後再決定是否實作。
  - 讀先：`docs/architecture/`（at-rest posture）；`src-tauri/crates/vault-core/src/archive/at_rest.rs`（既有的 reconcile/rewrite 原語，可推廣到 sidecar）；`schema.rs`（`open_archive_connection` 的 `ATTACH … KEY ''` 慣例）、`search_projection.rs`（`open_search_connection` 明文開啟）、`intelligence_projection.rs`（明文 + attach archive）；W-AI-3 `agent.sqlite` migration；`scripts/verify-rust-coverage.mjs`。
  - 設計問題（先評估再下結論，量測為準）：(1) **性能**——SQLCipher 對 14.4M 級 FTS5 / 向量 sidecar 的查詢/重建吞吐與 RAM 影響（目標機 4 核 / 8GB）；semantic plane（`.pkbin`/`.pki8`/`.pkvec`/`.pkmap` flat sidecar）本身不是 SQLite，是否一併加密、怎麼加密（檔案層 vs 應用層）。(2) **實現模型**——衍生資料可丟棄/可重建，加密它們 vs「鎖時不可讀、解鎖才解密」的取捨；rekey 時是否要連 sidecar 一起 lockstep（推廣 `migrate_*_for_rekey`）；明文→加密的遷移/重建路徑與 blast radius。(3) **威脅模型**——衍生明文到底洩漏多少（URL/標題/摘要/對話），是否值得這個性能與複雜度成本。
  - 契約：先交 trade-off 設計文檔 + 性能量測（env-gated bench），使用者確認後才動代碼；不破壞非加密路徑；100% JS/Rust coverage；前端流暢度硬指標。
  - 驗收：設計+量測文檔成立並獲使用者批准 → 若實作：開啟加密後 sidecar 不再明文落盤、查詢/備份/重建在目標機仍流暢、rekey lockstep、`bun run check` 綠。

- [ ] **WORK-TEST-REAL-IO-A** — 真實 I/O 測試化：embedding transport seam + fake-HTTP 整合測試 + dispatch 契約斷言（承 2026-06-28 behavioral-assertion 鐵律）
  - 來源：2026-06-28，note 搜尋與 embedding 一碰就炸但「100% coverage」沒擋。根因＝真實 I/O 引擎被 `#[cfg(not(any(test,coverage)))]` 整段換 stub、行為從不被斷言（見 `quality-matrix.md`「覆蓋率 ≠ 行為」鐵律）。2026-06-28 已把 static engine 的真實 compute 放回 coverage binary + 補 note/embedding 行為斷言；剩兩塊較大的 reform 延後：
  - **(1) external embedding transport 可注入 seam**：把 `ai/embedding_external.rs` 的真實 `reqwest` send/decode/timeout 從「整段 cfg-out 換 stub」改成把 HTTP **endpoint** 抽成可注入（本地 fake server / injected client），讓真實 decode / timeout / 空 `data[]` / mid-batch 500 的邏輯留在被量測 build。加 fake-HTTP `/v1/embeddings` 整合測試斷言「誠實失敗，絕不成功但 0 向量」。
  - ~~**(2) `dispatch_for_coverage` → 契約斷言**~~：2026-10-12 `WORK-TEST-CUT-2026-10` 把那個 walk 整個刪了；IPC 契約由 E2E 經 dev bridge 打真的命令證明。
  - 驗收：external 真實失敗模式有測試覆蓋；`quality-matrix.md` 鐵律全達成；gate 全綠。

- [!] **WORK-RELEASE-SIGNING-A** — macOS Developer ID signing + notarization in CI（需使用者決定 Apple Developer 帳號）[!blocked: 需 Apple Developer Program 帳號（$99/yr）+ 使用者授權]
  - 來源：2026-06-25 備份失效調查。Root cause 不是代碼，而是 **macOS TCC / Full Disk Access**：備份要讀 `~/Library/Application Support/Google/Chrome`，OS 拒絕 → `Operation not permitted` → 備份失敗。FDA 授權綁在 app 的**簽章身份**上；目前 debug/release 都是 **ad-hoc 簽章（無穩定身份）**，所以每次 rebuild / 每次發版更新，macOS 都視為「新 app」→ FDA 授權失效 → 使用者更新後備份再次靜默失效。這是發版的硬阻斷：不能讓使用者每次更新都重新授權 FDA。
  - 目標（release 工程）：
    - 取得 **Apple Developer ID Application** 憑證，存成 GitHub Actions 加密 secrets，在 CI 對每個 release 做 **codesign（hardened runtime）+ notarize + staple**。所有 release 共用同一個穩定身份 → app 的 TCC designated requirement（team + bundle id）跨版本不變 → 使用者授一次 FDA 後**跨更新持續有效**。
    - 設計 CI 簽章流程（憑證/私鑰注入、`tauri build` 的 signing identity、notarytool、更新器 artifact 簽章與 `TAURI_SIGNING_PRIVATE_KEY`）。
    - 文檔化 first-run FDA 授權引導（即使簽章穩定，第一次仍需使用者授權；配合 in-app FDA 引導，見備份透明度修復）。
  - 開發期 interim（已/將做，非本 block）：本機用 **自簽憑證**（stable self-signed）簽 debug build，讓開發測試時 FDA 不會每次 rebuild 失效；自簽 ≠ release（自簽會被 Gatekeeper 擋、無法 notarize），只用於本機 dev。
  - 讀先：`src-tauri/tauri.conf.json`（bundle / identifier / updater）；`.github/workflows/`（CI）；Tauri v2 macOS code-signing + updater 文檔。
  - 契約：憑證/私鑰只進加密 secrets，永不入 repo；reproducible build 原則不破；release rehearsal（`bun run verify`）通過。
  - 驗收：CI 產出 signed + notarized + stapled 的 `.app`/`.dmg`；裝在乾淨機器上 Gatekeeper 不擋；授一次 FDA 後，安裝下一個版本仍保有 FDA（備份不再靜默失效）。

---

## 維護規則

1. `STATUS.md` 清空時，從這裡頂部取最多 2 個未被阻塞的 work blocks
2. 新工作一律先放進這裡，除非使用者明確要求立刻調整 `STATUS.md`
3. 如果某個 block 需要再拆，只在 milestone 文檔的 WBS 裡拆，不把 `STATUS.md` / `BACKLOG.md` 再退回原子 task
4. 依賴解除後，直接更新行尾的 `[!blocked: ...]`
