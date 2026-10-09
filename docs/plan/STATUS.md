# STATUS.md — 當前工作

> Agent 每次開工讀這個檔案。一次只做第一個 `[ ]` work block。
> 2026-10 之前已完成的 blocks 見 `CHANGELOG.md`，或 git 歷史裡舊版的本檔（`main` 上的 `docs/plan/STATUS.md`）。

**當前 Milestone：M18 — Frontend redesign（Claude Design prototype → shadcn）**

---

## CURRENT FOCUS

- [ ] **WORK-FRONTEND-REDESIGN-2026-10** — 用新 prototype 整個替換前端（用戶 2026-10-02 指示）
  - 分支：`redesign/v0.4`（從 `main@551f851d` 開出）。第一個遠端 agent 的工作以 `4bbd8d62` 一次導入，它自己的 commit 歷史沒有保留。
  - 讀先：
    `docs/design/prototype-2026-10/`（prototype HTML + 截圖；用 `python3 -m http.server` 開）
    `AGENTS.md`（測試規則：E2E 為主、不跑 mutation、不強制 coverage）
    `docs/architecture/desktop-command-surface.md`
    `docs/plan/program/quality-matrix.md`
  - 用戶要求：
    - 新前端用 shadcn，盡量用 pre-built components；charts 用 evilcharts（`npx shadcn add @evilcharts/...`，Recharts）；heatmap 若前者沒有，用 amicro mono-charts，視覺風格要一致。
    - prototype 缺的東西順手補上；後端 dead code 順手刪；IPC / 卡頓問題看到就修或記錄。
    - E2E 為主，每次 E2E 產出可驗證、可重現的 artifact。
  - 步驟：
    - [x] 0. 分支、prototype 存進 `docs/design/prototype-2026-10/`
    - [x] 1. 改 gate：刪 mutation / coverage gate，AGENTS.md 加新規則，quality-matrix / TESTING 改寫，E2E artifact 設定
    - [x] 2. 後端：補齊前端已在呼叫的命令、IPC 性能修復、刪死命令，結論寫進 `docs/architecture/`（`desktop-command-surface.md` §2026-10、`ipc-performance.md`）
    - [x] 3. 新前端地基：tokens、窗口外殼 + nav rail、i18n、主題、shadcn 組件、數據層
    - [x] 4. 頁面：Home ✓、History ✓、Insights ✓、Ask ✓、Backup ✓、Lock ✓、Command palette ✓、Onboarding ✓（`bun run dev:demo -- --first-run --fresh`）、Settings ✓（General / Security / AI / Storage / About；舊設定的取捨見 commit `2f6f5880`）
    - [x] 5. 刪舊前端殘留和沒人用的後端命令（`5edf5645`；32 個 intelligence 讀取命令依用戶決定保留給 Insights）
    - [x] 6. E2E：`bun run test:e2e`，在真 Rust 後端上全綠（first-run → read → change → wipe），產物在 `artifacts/e2e/`；2026-10-04 Security 分支跑過時是 10 條場景
    - [ ] 7. 文檔同步（features / design / architecture）、CHANGELOG、`bun run check` 全綠
  - 已知缺口（2026-10-02 盤點）：
    - ~~前端呼叫但 Rust 沒註冊的命令~~：已補（`load_source_stats`、`get_url_detail`、`preview_wipe_all_data` / `wipe_all_data`），Tauri 與 dev bridge 都有。
    - ~~開機時啟動、選單列圖示~~：已做（用戶 2026-10-02 決定補做）。macOS 實機看過；**Windows / Linux 的 tray、登入啟動、語言偵測從沒編譯過**。2026-10-02 已推上 `origin/redesign/v0.4` 並手動觸發 CI 與 Platform Native workflow；編譯過了之後仍要在實機上點過一次。
    - ~~IPC 性能問題~~：已修並量測（14.4M：History 列表第一頁約 26 s → 1.9 ms；加密 `app_snapshot` 237 → 97 ms），見 `docs/architecture/ipc-performance.md`。剩下：常見詞的關鍵字搜尋第一頁 14.4M 約 1.1 s；`app_snapshot` 每次查 keychain 未量測。
    - 「刪除所有資料」會保留已安裝的自動備份排程。用戶 2026-10-02 決定：刪除時一併移除排程。已做（`d0755426`）。
    - 前端 `HistoryQueryResponse` 型別（`src/lib/types/archive.ts`）缺 `totalExact`，`history/queries.ts` 目前用交集型別補。
    - 30 多個前端未用的 intelligence 讀取命令：用戶 2026-10-02 決定保留，留給 Insights 之後擴充（清單見 `desktop-command-surface.md` §2026-10）。`vercel.json`（browser-only 預覽站）已刪。
    - 打包時 Recharts 被併成一個名叫 `heatmap` 的 546 kB chunk（Home 和 Insights 共用、lazy load），不算問題。
  - 2026-10-02 試跑時記下、還沒修的：
    - [x] Turso area 3 搜尋後端：`vault-search` 覆蓋所有匹配頁面，按相關度、符合篩選的最新可見訪問、page id 排序；沒有 25,000 頁窗口、由新到舊退路、match-set cache 或準備階段。owner indexer 消化 durable outbox，重建期間保留可用的舊索引，首次建立時每個回應都帶覆蓋狀態。Tauri / worker / dev IPC 提供 session、漸進結果與精確計數、游標及取消。前端 channel 接線由用戶處理。契約：[search.md](../architecture/search.md)；各上限、失效模式與測試移植：[驗收記錄](../architecture/research/2026-10-search-integration-validation.md)。
    - [x] Turso 遷移區塊 1–6 全部完成（2026-10-07，`feat/turso-storage`）：寫入、讀取、搜尋、Insights / AI / agent、維護與 SQLCipher 轉換、舊格式程式碼收進轉換器。審查與修復進行中，清單在 [turso-review-ledger.md](turso-review-ledger.md)；第一批已提交（2026-10-07，`check:base` 與完整 E2E 全綠），未完成項目見清單。
    - Regex、模糊搜尋、AI 候選、code mode、命令面板、MCP 與搜尋匯出不再截斷覆蓋；原有標註 API 上限由 area 1 的完整游標分頁處理。14.4M 訪問 / 2.9M 頁面的加密 owner 路徑量測、cold / warm 定義、RSS、備份競爭和清理證據見 [benchmark](../architecture/research/2026-10-vault-core-search-benchmark.md)。時間軸與其他 read models 屬 area 2，intelligence / AI queue 屬 area 4，maintenance 屬 area 5。
    - ~~Insights「常搜尋」的次數是整個 query family 的歷史總數，不受日期範圍限制~~：已修。新命令 `get_frequent_searches` 只數範圍內的關鍵字搜尋（intelligence migration 9 給 `search_events` 加上訪問時間和索引），14.4M 下 30 天約 10 ms、一年約 145 ms，見 `ipc-performance.md` §8。
    - ~~排程器沒有沙盒~~：已補 `PATHKEEP_PLATFORM_TEST_SANDBOX_DIR`（只在 debug build 生效），dev:demo 與 E2E 都會設。E2E 的 onboarding 仍選 Manual；`schedule.spec`（2026-10-04）在沙盒裡裝自訂間隔、核對檔案、移除。改了 project root 卻沒設沙盒的 debug run 會被排程器拒絕。
    - ~~`export_history` 先把整個結果集載入記憶體；`doctor_report` / `repair_health` 用 `NOT IN`~~：已修。匯出改成逐頁寫入暫存檔再原子改名（1M：heap 1.2 GB → 0.6 MB）；doctor 改成 `NOT EXISTS` 按主鍵查（14.4M：SQLite 暫存 321 MB → 0）。見 `ipc-performance.md` §7、§9。14.4M 全量匯出量過：heap +0.6 MiB、常駐約 92 MiB、4.7 GB 檔。後來改成單一連線（見下一條），66 分鐘的問題已處理。
    - ~~App Lock 面板與已接受文檔不一致~~：用戶 2026-10-04 選 trade-off 選項 B，已做（Touch ID 開關與解鎖、passcode hint、鎖屏「忘記密碼？」；不做 config 路徑列與上次解鎖時間），文檔已改。
    - ~~更改 archive 密碼 / 關閉加密不需要目前的密碼~~：已修（2026-10-04），後端用目前的密碼實際打開 archive 驗證，dev bridge 也繞不過。
    - E2E 偶發失敗（2026-10-04 見過一次，重跑通過）：`history.spec`「a starred page keeps its note after a reload」輸入搜尋後立刻點結果，那一刻列表有兩列同標題（之後只剩一列），strict mode 報錯。疑似舊查詢的結果還在畫面上，屬 History 範圍，未修。
    - ~~合成 Firefox profile 被 Floorp / LibreWolf / Waterfox 各認一次~~：已修，override 只套用在 Firefox。
    - Home「一年的瀏覽」載入中會先顯示「0 次瀏覽」。
  - 2026-10-04 用戶：「UI 本身就缺很多東西，prototype 做得很寬泛，很多細節沒做出來，你補上就行。」連結預覽：繼續抓、在 UI 顯示。五條平行補齊**都已合併**：Security（App Lock 方案 B、改密碼要驗證目前密碼）、History（預覽圖、標籤、搜尋語法說明、Rust regex 方言、語意狀態）、Backup（自訂間隔、安裝前顯示檔案、Verify、Browser Direct 匯入、匯入批次復原 / 還原、執行詳情、排程沙盒防呆）、Settings → Background work（佇列、derived state、搜尋排序、診斷）、Insights（day / site / search / page drill-in 與四張規律卡）。合併後全套 E2E 24/24 綠、Rust 全部測試通過（2026-10-04）。另外合併：匯入復原改成只刷新該批網址（Codex）、2026 RustSec 公告清掉並升 Rust 1.96（Codex + 主 agent）、Playwright 1.63（每次都留 trace）、匯出串流、常搜尋按範圍統計。
  - 2026-10-04 Backup 補完時記下、還沒修的：
    - `revert_import_batch` / `restore_import_batch` 每次都同步 `rebuild_search_projection` 整個搜尋投影；1440 萬條時一次復原要重建全部索引。應改成只刷新該批影響的 URL（匯入時已經這樣做）。
    - `vite.config.ts` 的 `server.watch.ignored` 含 `**/.claude/**`，在 `.claude/worktrees/` 裡開的 dev server 看不到任何檔案變更（要重開才生效）。
    - 自訂排程間隔上限 30 天是新加的限制（archive.md §2 原文只寫最短 1 分鐘）。
  - 匯出歷史在 14.4M 要約 66 分鐘（每秒約 3,600 筆），Settings 沒有進度也不能取消。決定（2026-10-04）release 前要修：後端單一連線 + keyset 分頁、目標幾分鐘內；加 `exportId`、`get_export_progress`（輪詢，dev bridge 也能用）、`cancel_export`；前端顯示進度與取消，附 E2E。**已做（2026-10-04）**：Codex 改後端（`55d22c0a`），明文 14.4M JSON Lines 51.5 分鐘 → 40 秒；Settings → Storage 有進度、剩餘時間、停止，`export.spec` 覆蓋。加密 14.4M 也量了：113.8 秒（同 14.4M 筆、4,744 MiB；量時 load average 19，所以偏慢），見 `ipc-performance.md` §9。目標機 4 核預計幾分鐘，有進度和停止。
  - 試跑：`bun run dev:demo`（真後端 + 合成 Chrome×2 / Firefox archive），瀏覽器開 http://127.0.0.1:1420。

- [ ] **WORK-INSIGHTS-RESTORE-2026-10** — Insights 回到 v0.3.0 的內容量，預設「全部時間」（用戶 2026-10-08）
  - 起因：用戶在自己的資料上把 Insights 切到 1 年，整個畫面崩潰；並指出舊版 Insights 顯示的資料多得多，而且一打開就是全部時間（有意的設計），新版既不預設全部時間，連選項都沒了。v0.3.0 確實預設 `all`（`src/pages/intelligence/route-state.ts:47`）。redesign 把範圍改成 7d / 30d / 90d / 1y、預設 30 天，並拿掉大半張卡，寫進了 `screens-and-nav.md`，但沒有 trade-off 文檔也沒經用戶同意，違反 AGENTS.md 原則 7。
  - 讀先：`docs/features/intelligence.md`、`docs/features/intelligence-current-state.md`、`docs/design/screens-and-nav.md`（§Insights）、`docs/plan/intelligence-all-time-cache-invalidation.md`、`docs/design/ux-principles.md`、`design-tokens.md`、`ui-review-guardrails.md`、`docs/architecture/ipc-performance.md`。v0.3.0 的畫面用 `git show v0.3.0:src/pages/intelligence/…` 讀；盤點結果見下。
  - 決定：
    - 範圍：全部時間（預設）/ 1 年 / 90 天 / 30 天 / 7 天，加自訂起訖與瀏覽器設定檔篩選（v0.3 都有）。`?range=all` 深連結。
    - 內容：v0.3.0 主畫面與 drill-in 的每一項資料都回來，用新設計語言重畫，不照搬舊版面。保留 v0.4 新增的（活躍時間、常搜尋按範圍統計、Browsers compared、page drill-in 的「為什麼」等）。
    - 全部時間要在 1440 萬條時開得快：後端仍有 v0.3 的 fingerprint 快照（`intelligence_overview_snapshot.rs`，primary / secondary overview 遇到全部時間就讀快照），先在 Modal 上量 14.4M 時每張卡的命令在全部時間下的耗時，再決定全部時間走 overview 快照灌各卡的快取（v0.3 的做法）還是逐卡讀。
  - 步驟：
    - [x] 1. 1 年崩潰（`2025-W41` 被當日期格式化）：圖表改成要日資料、前端按週分組（`insights/buckets.ts`）；卡片渲染失敗只壞那張卡並寫進 log；`insights.spec` 用切換鈕走過每個範圍（含 site drill-in）。
    - [ ] 2. 量測：14.4M、全部時間，每個 Insights 命令（含 overview 快照冷 / 熱、匯出後）的耗時與 RSS。
      - 2026-10-08 進度（用戶要關機，中斷）：`vault-worker/tests/archive_scale_bench/turso_intelligence.rs` 加了 `insights-30d` / `insights-1y` / `insights-all`（每個 Insights 讀取命令各兩次）與 `export-between-reads`（讀、整包匯出、再讀，看匯出後變慢與常駐記憶體），`turso_reads.rs` 加了 `current_rss_bytes`。**尚未提交、尚未編譯過**。第一次 Modal 執行還在編 release 時被中止，沒有數字。重跑：`offload run --key insights-bench --cpu 4 --memory-gib 8 --timeout-min 300 -- '<touch 前綴>; cargo test --manifest-path src-tauri/Cargo.toml -p vault-worker --release --test archive_scale_bench turso_intelligence::encrypted_intelligence_at_scale -- --ignored --nocapture'`，結果寫進 `ipc-performance.md` 與 ledger，再提交。
      - 注意：全部時間的 overview 快照由重建後的 `warm_overview_snapshots` 寫入，鍵是開始日 `1900-01-01` 加今天的本地日期；真實資料測試用 `2000-01-01` 與 UTC 日期，所以那些 overview 數字是沒走快照的冷算成本。新版 Insights 頁逐卡讀取，不用 overview。
      - 真實資料測試的副本仍在 `/Users/tim/LocalData/pathkeep-realdata-test`（APFS clone 與 release build），留到 Insights 修好後在真實資料上驗證，之後刪掉，並刪 Keychain 項目 `com.yi-ting.pathkeep.realdata-test`，以及本機正式版編譯時 worker 寫進專案的 `src-tauri/target`（2026-10-08 量為 2.5 GB，2026-10-07 第一次真實資料編譯時才建立，全是測試用的產物）。用戶的 PathKeep 可以照常打開（測試用副本與它無關）。
      - 真實資料（386,891 次瀏覽，release build，2026-10-08）每個 Insights 讀取的耗時，全部時間 / 30 天：多數 70–130 ms（每個 section 讀取似乎有約 70 ms 的固定成本，`get_activity_mix_trend` 只要 5 ms）；`get_browsing_rhythm` 0.5 s；`get_habit_patterns` 0.7 / 0.3 s；`get_path_flows` 0.4 / 1.5 s（30 天反而更慢）；`get_compare_sets` 2.5 / 1.9 s（一次回傳全部 3,211 組，沒有分頁）；primary overview 2.5 s（第二次 0.9 s）；secondary overview 5.3 / 3.6 s。量測腳本 `pathkeep-realdata-test/cards.mjs`。
      - 同一份資料上 v0.3 卡片背後的資料品質（要恢復就要先修後端，不能只把卡畫回來）：digest 全部時間 `newDomains` 5,244 大於 `distinctDomains` 4,115；search effectiveness 每個引擎 `avgReformulations` 都是 0，但同一份回應的 hardest topics 有 299 次改寫；stable sources 每列都是 `reference`、從不輸出 entry / landing，且同一網站每個設定檔一列（同一網站出現兩次）；friction 恆為空；compare sets 同一查詢重複多組、無分頁；search concepts 前 15 名有單一字母、單一數字與 "pro"、"open" 之類的詞；activity mix 58% 是 `unknown` 類別；observed interactions 只有 foreground 時間有值（一個登入確認頁顯示 65 分鐘），捲動 / 打字恆為空；common paths 前幾名都是「搜尋引擎 → 某網站 → 搜尋引擎」來回。
    - [x] 3. 範圍：全部時間（預設）、自訂、設定檔篩選；全部時間的長條按月（跨度超過約 18 個月時），每條的 key 仍是起始日期。
    - [ ] 4a. 主畫面先補回資料可信的卡（2026-10-08 已做：KPI 加新網站 / 深入瀏覽 / 重新打開的頁面；熱門網站前 40、三種排序、卡內捲動；節奏下方的平日 / 週末每天平均與最忙時段；新網站走勢；在哪裡搜尋；搜尋什麼；頁面按「多少與去哪 / 什麼時候 / 搜尋 / 規律」分段；進頁讀兩批。未做：Spotlight、Discovery rate、多瀏覽器的共同網站〔後端只給網站名單、沒有次數，排不出重要的〕、Common Paths 與 Observed Interactions 移到 4b）（見下方 4 的清單）：Period Summary 的 Deep Reads / Refinds / New Sites（New Sites 已修，`891be3db`）、Spotlight、工作日 vs 週末與尖峰時段、Browsing Rhythm 日曆、Top sites 完整清單、搜尋引擎排名、search concepts（`a509c02b` 已濾掉單一字母與數字）、query families、Interrupted Habits、Discovery Trend、Multi-Browser 的共同網站與類別、Common Paths、Observed Interactions（附說明：只有前景時間）；進頁時先批次讀 primary overview 灌各卡快取，再讀 secondary（accepted 契約：不得一次 fan-out 二十多條讀取）。
    - [ ] 4b. 後端資料修正，修好才把卡補回。2026-10-08 已做：搜尋改寫延長 trail（`4782c63e`）；compare sets 建置時算好、分頁（`ef93154a`）；habits 從日 rollup 取最後造訪（`0a6eee90`）；節奏從日 rollup 的每小時計數加總（`09b5ebeb`）；KPI 列與 top sites / breadth / habits / 各瀏覽器 / common paths / 網站頁改成在 rows 表上依範圍索引 seek，不再經 generation view 整代讀（`452bfeb7`、`fe1687c6`；`intelligence_read_plans` 擋回歸）。14.4M、30 天修前：digest 126 s、top sites 55 s、rhythm 34 s、breadth 60 s、habits 140 s、browsers 149 s、paths 72 s、網站頁 55 s，primary overview 588 s、secondary 351 s。修後（同一環境，`ipc-performance.md` §11）：30 天 primary 3.3 s、secondary 4.3 s；一年 37.8 / 55.0 s（top sites 16 s、habits 18 s、breadth 15 s、各瀏覽器 32 s、common paths 9 s、digest 3 s）；全部時間走快照每批 1.5 s，但快照裡沒有的讀取（改 top sites 排序、下一頁）要 74 s。重建 5,721 → 5,955 s：日 rollup 241 → 1,293 s（新索引，網站索引按網站順序寫入），快照暖機 1,964 → 844 s。bench 的資料幾乎每次瀏覽都是新的網站-日列（每天約 8,600 列），是這些讀取的最壞情況。還沒做：長範圍——top sites / breadth / habits / 各瀏覽器各讀一次同一批網站-日列，改成一次讀完共用；有界範圍的 common paths 仍重放範圍內每次瀏覽；先在真實資料上量每天的網站-日列數，再決定要不要月 rollup；bench 改成 Zipf 分佈另列一步，前後各量一次。加 1 萬筆（14.4M 上）要 57 分鐘、峰值 3.1 GB：aggregate 每次重走全部瀏覽兩遍（43 分鐘），再重暖全部時間快照（14 分鐘，其中全部時間 habits 單獨 1.85 GB）。**違反已定案文檔，要修回**：`intelligence.md`「cold path 仍以 secondary overview batch 為主，不能退回多個 foreground IPC fan-out」，但現在非全部時間的範圍不讀 secondary batch、各卡自己讀（`overview.ts`）。做法：把 secondary 變便宜（common paths 有界範圍改讀按日存好的路線；breadth / 各瀏覽器與 top sites / habits 共用一次網站-日讀取），再讓每個範圍都讀兩批。
      - 順序（2026-10-08 用戶要求把備份後重算排進來）：① common paths 有界範圍改讀 `session_paths`（已做：「perf(insights): common paths for a range read stored session paths」）；② 每個範圍都讀兩批（已做：「fix(insights): every range reads the two batches again」）；③ 網站-日共用一次讀取（已做：「perf(insights): cards that rank or count sites share one read per range」；順帶修了全部時間快照的指紋每次掃整個 visits 表）；14.4M bench（最壞形狀）：全部時間第一批 1.5 → 0.11 s，一年第二批 55 → 0.9 s，快照暖機 14 → 3.5 分鐘；**一年第一批仍 39 s**（共用讀取一次走完一年 315 萬列網站-日，逐列帶進 Rust 比原本 SQL 分組每列更慢；真實資料 74 ms：bench 每次瀏覽一列網站-日，真實資料約 19 次瀏覽一列，bench 密了約 19 倍）。**⑤ 要修，合併前做**（用戶 2026-10-08 確認）：目標是 14.4M 上一年第一批不比 30 天那批（3.3 s）慢，最壞形狀與 Zipf 形狀都量。依序：(a) bench 加 Zipf 形狀，量真實密度下的一年第一批（2026-10-08 已量，`PATHKEEP_BENCH_SHAPE=habits`，14.4M、4 核 8 GB：一年第一批 1.74 s〔再讀 0.24 s〕、第二批 1.28 s〔其中 common paths 0.92 s〕；全部時間兩批走快照 15 ms，common paths 1.35 s；加 1 萬筆 13.7 分鐘〔refind pages 一個階段 12 分鐘，④ 處理〕；整包匯出 5 GB 9.6 分鐘，匯出後常駐記憶體停在 870 MB 不回落，要查。Zipf 形狀已達標，最壞形狀仍 39 s，所以 (b) 照做）；(b) 共用讀取在 SQL 裡先按網站（與設定檔）合併、只帶需要的欄位，不再逐列帶進 Rust；(c) 每次建置後在背景暖 30 天 / 90 天 / 一年，像全部時間的快照一樣，預設範圍一打開就有；(d) 做完 (b)(c) 自訂範圍仍超過目標，就提網站-月 rollup（改 data model，先寫 trade-off 文檔給用戶）。真實資料（2026-10-08 第二次）：重建 145 → 40 s，一年兩批 348 ms / 2.0 s → 67 / 43 ms，全部時間兩批 4 / 3 ms，一年 common paths 1.45 s → 22 ms，只剩全部時間的 common paths（所有設定檔）151 ms（`ipc-performance.md` §11）；④（用戶 2026-10-09 選 B，提案見 `docs/architecture/research/2026-10-incremental-aggregates.md`）aggregate 只算變動的日子：refind pages / habits / common paths / compare sets / query families 改成增量，不再每次備份重走全部瀏覽兩遍（14.4M 上加 1 萬筆 43 分鐘）。每步做完在 14.4M bench 上量加 1 萬筆的時間與峰值記憶體。stable sources 重新定義、friction、activity mix 的 unknown、common paths 的來回、全部時間快照的設定檔按需暖機；以及把 search effectiveness、compare sets、common paths、改寫搜尋、stable sources 的卡補回。
      - 搜尋 trail 每次搜尋都另起一條（`intelligence_structural_stream.rs` `process_visit` 遇到搜尋就 `finish_trail`），所以每條 trail 只有一個查詢、`reformulation_count` 恆為 0；同一 session、`TRAIL_GAP_MS` 內、同一引擎且查詢相近（query family 的相似度）的再搜尋應延長 trail、算作改寫，landing 取最後一次搜尋後打開的頁。影響 search effectiveness、stable sources、compare sets、search drill-in 與 structural 測試。
      - Stable sources：`build_stages.rs` 的 role 是「refind 頁數 ≥ landing 次數就 reference」，從不產生 entry，每個設定檔一列；改成「搜尋落在哪些網站」：跨設定檔合併，按落地的搜尋數排序，附該站反覆打開的頁數。
      - Compare sets：每次讀取都從範圍內所有 trail members 現算（真實資料 2.5 s，14.4M 全部時間會讀數百萬列），無分頁、同一查詢重複；改成建置時算好存表，按查詢合併、分頁。
      - Friction 恆為空；activity mix 58% `unknown`（taxonomy 覆蓋。2026-10-08 真實資料：58.5% 的造訪是 unknown，但前 100 個網站只有 18 個顯示 unknown——網站一天的分類取名稱最小的，少數被歸類的頁面替整站貼了標籤，GitHub、YouTube 這類只有部分路徑有規則的網站，其餘頁面都是 unknown；exact rule 只比 registrable domain，Google 文件、郵件、Gemini 都算搜尋。已改：主機優先比對、各區 pack 約 900 條站點規則、路徑規則網站加整站規則、網站分類取多數；頁面標題的詞只定頁面種類、不再替網站貼類別（「tokio vs smol」的部落格被歸成購物）。真實資料重量：unknown 58.5% → 26.1%；新版打開副本時因規則變了自動重建（65 s）。Activity Mix 以「哪類網站」補回，未分類排最後；讓用戶改網站分類的入口另列 `WORK-SITE-CATEGORY-REVIEW`（BACKLOG）。**⑥ 要修，合併前做**（side agent 2026-10-08 提醒）：規則版本現在含每個 pack 的版本，清單一改就整個重建（14.4M 約 97 分鐘），之後用戶改一個網站的分類也一樣。分類只影響每次造訪存的分類、每日網站 / 類別 rollup、compare sets（頁面種類）與快照，session、trail、路線、習慣、重開頁面、query family 都不看分類。做法：分類規則（matcher、pack、用戶 override）的版本與其他建置規則分開；只有分類規則變了就走「重新分類」：每個不同的頁面分類一次（14.4M 時 288 萬頁，不是 1440 萬次造訪），只改分類真的變了的造訪，只重建那些造訪所在日子的 rollup 與所在 trail 的 compare sets，再暖快照；成本跟著改動的多少走，不跟著 archive 大小。在 14.4M bench 上量一次清單改動的時間與峰值記憶體；`WORK-SITE-CATEGORY-REVIEW` 以此為前提）；common paths 被「搜尋引擎 → 某站 → 搜尋引擎」來回佔滿；`get_path_flows` 30 天比全部時間慢（1.5 s 對 0.4 s，全部時間走預算表）；`get_habit_patterns` 0.7 s、`get_browsing_rhythm` 0.5 s。
    - [ ] 4. 主畫面補回：Period Summary 的 New Sites / Deep Reads / Refinds；Spotlight；工作日 vs 週末、尖峰時段；Browsing Rhythm 日曆（年分頁、點日展開）；Top sites 完整清單（20、排序、篩選）；Stable Answer Sources；Search Activity（引擎、概念、查詢瀏覽、query families）；Activity Mix；Search Effectiveness；Friction & Dead Ends；Interrupted Habits；Discovery Trend（每週新站率）；Common Paths；Compare Sets；Multi-Browser 的共同網站與類別；Observed Interactions；refind 的因子與 explain。
    - [ ] 5. Drill-in 補回：day（Activity Mix、Query Evolution、Refinds）；site（trails KPI、到達方式、來源 referrers、compare / path 焦點——後端 `DomainDeepDive` 拿掉了 `arrival_breakdown` / `top_referrers`，要補回）；session、trail、query family、compare set、year review 頁。
    - [ ] 6. E2E：每張卡的數字對照 fixture 手算（中等以上難度場景），每個範圍、每個 drill-in 都走過。
    - [x] 8. Home（用戶 2026-10-08 回報）：app 剛打開時 dashboard 的數字是空的，還沒算出來時要有載入動畫（skeleton），不是空著。原因有兩個：卡片把「建置中、還沒有資料」當成「沒有資料」；新的 Insights store 沒有東西排第一次建置。現在卡片保留 skeleton，並寫一行「正在第一次整理你的洞察…」或「正在更新你的洞察…」，算完自己填上；打開 archive 時，若有瀏覽紀錄卻從沒建過（或被清除），或是用別版規則建的，就排一次重建（`insights_build_needed`）。`first-run.spec` 在背景工作暫停時打開 Home 驗證。
    - [ ] 7. 文檔：`screens-and-nav.md` §Insights、`intelligence.md`（舊「首次進入預設 Month」的說法本來就與 v0.3.0 不符，一併改正）、CHANGELOG。

- [ ] **WORK-TEST-AUDIT-2026-10** — 舊 unit test 審查（用戶 2026-10-08）
  - 起因：早期讓 AI 寫大量 unit test、追求 100% mutation，東西仍一碰就炸；用戶擔心這些測試在 pin 錯誤或細微的行為，之後每次改動都得背著。專案已改成 E2E 為主（AGENTS.md「測試是契約」）。
  - 做法：四個唯讀 subagent 按區域審查，每個測試分成：pin 錯誤行為（附文檔依據）／pin 實作細節／mutation 遺留／該留。報告與計畫在 `docs/plan/test-audit-2026-10/`（約 2,400 個測試：A 22、B 235、C 825、D 1,338）。動手前逐條核對。
  - [x] 1. 審查報告（2026-10-08）。
  - [x] 2. 安全（P0，2026-10-08）：Ask 的連結改走 `open_visited_page`，`file://` 只在 archive 裡有造訪時才打開，`open_external_url` 不再接受 `file://`（`links.spec`）；og:image 與 content fetch 的每一跳轉址都檢查私有位址，content fetch 不再有 `cfg(test)` 的放行版本；Bilibili 回應邊讀邊限 64 KiB；31 個只需本機 mock server 的網路測試恢復執行。測試 embedding 連線送空的 `Bearer` 只送到用戶自己設定的端點、不帶秘密，移到 3。
  - [ ] 3. 產品錯誤（P1，見 README。已修：Threads 把寫法數當天數〔「fix(insights): a search you came back to counts days, not wordings」〕。2026-10-08 已寫、待 gate：AI 重建索引的時間估計拿掉〔用已移除的 candle 速度，10 萬頁說 22 小時〕，只說要索引幾頁；agent 的日期篩選改用本地日；洞察「部分可用」的原因改用代碼翻成使用者語言；Windows 排程被拒時顯示翻譯後的文字而不是 i18n key；中斷的對話標題不再是英文「Untitled conversation」；清除索引佇列不再把失敗的工作改成已取消；查詢的 embedding 不再對沒有金鑰的端點送空的 `Bearer`〔不只連線測試，每次語意搜尋都會送〕。另寫、待 gate：備份後的警告〔備份之後的工作（Insights、語意索引、連結預覽、內容抓取）產生的警告寫回同一筆 run；執行紀錄依 code 用使用者的語言寫成一句話，附修法；搜尋詞、連結預覽這類順利完成的附帶工作改放摘要下方，不算警告；手動備份有警告時，完成通知寫幾則並可直接打開那筆 run（`?run=`）；`backup.spec` 驗從首頁備份、繁中顯示〕；`.jsonl` 不再被當成 Takeout 匯入〔parser 拿掉 JSON Lines 分支與 `titleUrl` / `pageTitle` / `visitedAt` 這些不是 Takeout 的欄位；測試的 fixture 改成真的 `Chrome/BrowserHistory.json`；新的 `takeout-import.spec` 驗資料夾裡的 `.jsonl` 不匯入、再匯一次不重複〕；History 語意搜尋的日期與瀏覽器篩選交給後端〔後端本來就對每筆造訪先套篩選再排名，只是 History 沒傳；`AiSearchRequest` 加 `browser_kind`，前端不再在每頁 50 筆上自己過濾，篩選時也顯示筆數。E2E 要等 6 的假 embedding server〕；較小的：HTML 匯出宣告 UTF-8；entity reference 欄位用 camelCase；重開的對話保留每個引用的 profile（`agent_citations.profile_id`）；連線測試的工具支援跟 agent 實際送工具的判斷用同一條規則；`run_code` 沒有 return 時告訴模型「沒有回傳值」而不是 `null`；YouTube 網址的 query 裡有沒有 `=` 的參數也找得到影片 id；英文 lexicon 詞要整個詞相符（`m5-taxonomy-v3`）；清除洞察的預覽算進所有會刪的表；compare sets 改看同一層的兄弟頁（`structural-rebuild-v3`）；主機回 429 時照它的 `Retry-After` 整台停送，不再一秒後重試。索引檔最後一筆寫到一半時，下次增量建置先切掉那半筆（之前每次都失敗到全部重建）；sidecar 檔頭壞掉時語意搜尋退回 `.pkmap`，不再整個失敗（兩者的 E2E 隨 6「建置中斷再續」）。舊版 app 留下、讀不懂的 rekey / import 標記：只有在 archive 與 source evidence 的加密狀態一致時才刪，不一致就留著、開啟時走復原畫面（quality-matrix 鐵律 3；之前一律刪掉並「修好」成混合狀態）。等用戶決定：Chrome 停留時間的單位〔微秒存進 `_ms`，讀出來大 1000 倍；trade-off 見 `docs/architecture/visit-duration-unit-tradeoff.md`，建議寫入時換毫秒、舊資料分批轉，會改寫用戶現有 archive〕。Firefox 的 visit type 被當成 Chrome transition 只影響沒有呼叫者的 `get_browse_day_insights`，隨 4 一起刪）：AI 重建估時用已移除引擎的速度（10 萬頁說 22 小時）；agent 日期篩選用 UTC 日；英文或 i18n key 直接上畫面；Threads 把搜尋的不同寫法數當成天數；Chrome 停留時間微秒當毫秒；Firefox visit type 當 Chrome transition；`.jsonl` 被當 Takeout 匯入；History 語意搜尋在前端篩日期；備份後的警告沒人看到；以及 README 列的其餘項目。每項用 E2E 證明，連同 pin 它的測試一起改。
  - [ ] 4. 刪 B / C，連同只有它們在用的程式碼（`cfg(coverage)` 雙胞胎、production 裡的測試替身、沒有呼叫者的命令）；只在畫面上到不了、但已定案文檔說已上線的 candle embedding：用戶 2026-10-09 決定拿掉（連同只測它的測試、退回外部服務的分支、文檔裡「已交付」的說法）。Ask 的「崩潰可續跑」：用戶 2026-10-09 決定改文檔（中斷的回答標成中斷、要重問），只保留重開對話讀得到的紀錄（引用、用量），沒人讀的步驟日誌不再寫。
  - [ ] 5. 從不執行的測試（devtools-bridge、Vitest）進 gate 或刪；用時鐘當種子的測試改固定種子。
  - [ ] 6. 補 E2E：Takeout / Safari 匯入、Ask（假的 OpenAI 相容 server）、連線測試、語意索引建置中斷再續、星號與筆記搜尋、og:image、備份後警告、搜尋排序。
  - [ ] 7. `quality-matrix.md` 與 AGENTS.md 寫明什麼情況才寫 unit test；修 README 列的過期文檔。

- [ ] **WORK-UI-WINDOW-ICONS-2026-10** — 視窗拖動、icon 一致、毛玻璃（用戶 2026-10-08）
  - 讀先：`docs/design/ux-principles.md`、`design-tokens.md`、`screens-and-nav.md`、`ui-review-guardrails.md`；`src/app/shell/`；`src-tauri/tauri.conf.json` 與 capabilities。
  - [x] 1. 視窗從頂部的空白處都能拖動（2026-10-08，「fix(ui): drag the window from its top, like any window」）：每個捲動畫面頂端 28 px、頁首與工具列與側欄的空白處、主面板外圍、鎖定與啟動畫面的背景；按鈕、輸入框、連結照常可點。拖動本身 E2E 驗不到，待用戶在桌面上試。
  - [ ] 2. Icon 一致：Insights 的網站、瀏覽器與設定檔、搜尋引擎等大多沒有 icon，整個 app 都有「有現成 icon 卻不顯示」的情況。2026-10-08 已寫、待 gate：Insights 與 Ask 的網站改用 `SiteIcon`，沒有造訪可查時用網站自己的 icon（新命令 `load_site_favicons`：網站本身的主機，再 www.）；搜尋引擎帶上它的網站（`EngineRanking.site`）；Insights 的各瀏覽器卡、設定檔篩選、page 畫面與 History 詳情的瀏覽器加圖示；fixture 的 Chrome 加 `Favicons`，`insights.spec` 驗有 icon 的網站顯示真的 icon、沒有的顯示字母。
  - [ ] 4. 用戶在真實資料上看到的問題（2026-10-09，截圖）：
    - 兩欄的卡片按列對齊，短的那張被撐高，留下大片空白（最常瀏覽、反覆打開的頁面、在網站之間怎麼走）。兩欄各自往下排，不互相撐高。
    - 清單裡的佔比長條在比例很小時只剩圖示後面一顆小圓角塊，像畫壞了；長條改成相對清單第一名，或不畫。捲動清單的頂端直接切掉一列，底部有漸層頂部沒有。
    - 有網站顯示成別的網站的圖示（`load_site_favicons` 或 SiteIcon 的對應錯了）。
    - 反覆打開的頁面裡有同名的不同頁面，分不出來，要附網站或路徑。
    - 各瀏覽器對比：Takeout 匯入顯示成原始的 `takeout · Imported browser history`，要寫成「Google Takeout」並翻譯；「只在這裡」列出擴充功能的 id（`chrome-extension://` 的頁面被當成網站）：不是網頁的 scheme 不算網站。
    - 探索還是專注：「一半的瀏覽集中在 4 個網站」配上滑桿在「很多網站」那端、又說「分散在很多網站」，自相矛盾；全部時間時「N 個網站是第一次去」等於所有網站，沒有意義。
    - 搜尋得怎麼樣「每次搜尋打開 23 個頁面」偏高（trail 拉太長？）；「試了好幾次的搜尋」把 "this" 這種詞算成一組、改寫 31 次。
    - 毛玻璃看得出半透明但很淡，要更明顯。
  - [ ] 5. 時間用 12 或 24 小時制跟作業系統走（用戶 2026-10-09：偏好 12 小時制）。WebView 的 `Intl` 只看語系，不看系統的「24 小時制」開關：macOS 讀 `AppleICUForce24HourTime` / `AppleICUForce12HourTime`，Windows 讀 `HKCU\Control Panel\International` 的 `sTimeFormat`，傳給前端的格式化；Settings → General 加「跟隨系統 / 12 小時 / 24 小時」。所有顯示時刻的地方都走同一個格式化。
  - [ ] 3. 半透明毛玻璃：UI prototype 一直是半透明毛玻璃視窗（`ux-principles` 也這樣寫），實作卻在 body 畫 prototype 的桌布漸層（`--wall`）當視窗底色。2026-10-08 已寫、待 gate：Settings → General → 視窗（不透明 / 毛玻璃，只在 macOS 與 Windows 出現），毛玻璃時視窗透明、用系統材質（macOS under-window vibrancy、Windows Mica），body 不再畫漸層；預設不透明。要用戶在桌面上試（E2E 的瀏覽器裡沒有原生視窗）。

- [ ] **WORK-REFACTOR-INDEXING**（用戶 2026-06-21 指示，承巨檔審計）— `ai/indexing.rs` 拆分重構。詳見 `main` 上的 STATUS。_非當前 focus_

> `WORK-V03-PAPER-REDESIGN-A`（Paper 風格前端重建）被本次 redesign 取代，不再繼續。
