# INTELLIGENCE CURRENT STATE — 2026-04-18 Reset Transition Truth

> 這份文檔不是在描述「理想上想做什麼」，而是在描述 **PathKeep 現在實際上已經做了什麼、怎麼做、哪些地方只做到一半、哪些舊文檔已經不夠準**。  
> 如果你要重新設計 intelligence UI，或要先搞清楚現在 repo 裡的 intelligence 到底是什麼，先讀這份。
>
> **2026-06-22 AI-shipped note:** optional AI layer 已由 AI redesign 2026 program（W-AI-1..9）交付並 reachable——streaming assistant、durable agent harness、in-app（candle Qwen3）+ external embedding、`FlatVectorIndex` 語義/混合檢索、code-mode 沙箱、MCP 對外面、skills、content-fetch enrichment 都已落地，但整個 AI 面 **off by default + consent-gated**（master `ai.enabled` + 各 sub-flag）。本文中早期標 `v0.2 disabled / Coming in v0.3` 的 AI 行已更新為 shipped 狀態；安全邊界與 threat model 見 [../architecture/ai-security-posture.md](../architecture/ai-security-posture.md)。
>
> **2026-04-15 reset note:** deterministic / Core Intelligence 的 accepted baseline 已改由 [core-intelligence-ultimate-design.md](core-intelligence-ultimate-design.md) 接管，主產品 route 也已 hard-cut 到 `/intelligence`。這份文檔保留的價值，現在主要是說明 pre-reset shipping surface、optional AI layer、Jobs/runtime review 與哪些舊 `insights` / snapshot 概念只剩歷史脈絡；不要再把本文中提到的 `load_insights` / thread-detail snapshot surface 當成新的 product contract。除非特別標示 pre-reset，本文中的「Insights」都應理解為現在的 `/intelligence` deterministic product surface。
>
> **2026-04-15 follow-up note:** Core Intelligence backend 現在還多了一層正式的 trait-backed module registry。`visit-derived-facts`、`daily-rollups`、`sessions`、`search-trails`、`refind-pages`、`activity-mix`、`search-effectiveness`、`domain-deep-dive` 這 8 個 built-ins 的依賴順序、rebuild stage ownership、以及 `explain_entity` 的 entity ownership 都已經收斂到單一 registry。
>
> **2026-04-17 incremental note:** Core Intelligence deterministic queue 現在已補上 per-profile stage checkpoint ledger。append-only `visit-derive`、`daily-rollup`、`structural-rebuild` 會優先走 incremental path，並把 `executionMode`、`dirtyVisitCount`、`dirtyDateKeys`、`fallbackReason` 寫進 runtime artifact；如果 archive visibility regression、stage version drift 或 checkpoint 缺失，系統會誠實回退成 scoped `fallback-full`。這代表「stage queue 不再只是看起來像增量」，但也**不**代表 10M / low-RAM / queue recovery RSS 的最終 large-archive signoff 已完成。
>
> **2026-04-17 benchmark/recovery closeout:** structural stage 的 profile-wide aggregates 現在改成 batch scan：query families 以 `search_events` batches 建構，refind/path-flow/habit 以 `visit_derived_facts` batches 聚合，不再為這些 aggregates 額外 materialize whole-profile `search_events` / `visit_derived_facts` Vec。`visit-derive` / `daily-rollup` 的 `fallback-full` 也已改成 chunked profile scan，不再先把整個 profile 載入成單一 `Vec`。`artifacts/benchmarks/2026-04-17-intelligence-finish-line/` 補齊了 `100k / 60y` low-RAM fallback 與 expired-lease recovery evidence，而 `artifacts/benchmarks/2026-04-17-intelligence-signoff/` 現在則已補齊 corrected `2k / 1m / 10m / queue-recovery` artifact，外加 disposable encrypted app-root 的 `real-replay-signoff.json`。這代表 `WORK-CI-B` 的 backend finish-line 已完成。
>
> **2026-04-18 closeout note:** `WORK-CI-C` 已完成。crate-internal `vault-core::insights` tree 已刪除，readable-content helper 與 queued enrichment ownership 都已移到 `enrichment` / `intelligence`；repo 也已移除 `InsightStatus` alias。`artifacts/benchmarks/2026-04-18-intelligence-long-horizon-signoff/` 現在補齊 `full-14_4m-60y-signoff.json` 與 `expired-lease-recovery-14_4m-signoff.json`，因此 current-host `14.4M / 60y` signoff 已落地。第二台主機 benchmark parity 目前不在當前 scope 內，如要再做必須重新立項。
>
> **2026-04-18 M5 evidence note:** `WORK-M5-C` 已完成。`/intelligence` 與 `/intelligence/domain/:domain` 現在會以 shared section envelope 承接 compact evidence / freshness badge + floating review panel：顯示 generated-at、active scope / window、owning modules、source tables、enrichment flag、以及 stale / disabled / degraded reason；Maintenance / Jobs 繼續保留 rebuild / clear / retry mutation controls。
>
> **2026-04-18 app truth-gate note:** closeout 之後又做了一輪實機驗證修補：source 現在已修正 `/intelligence` section-envelope camel/snake drift、`daily-rollup` duplicate domain-day fallback、encrypted onboarding 無 keychain regression、queue / copy / privacy drift、以及 route-level error fallback。不過這台主機的 Computer Use 在手動驗證時仍可能附著到 stale `target/release/bundle/macos/PathKeep.app`，繼續載入舊 hash bundle（例如 `index-CNXdWxTA.js`、`intelligence-mc5c_cvZ.js`）。看到那類 screenshot 時，要先把它當成 host-specific stale bundle / cache noise，而不是 current source truth。
>
> **2026-04-18 desktop truth repair note:** 又補了一輪 `/intelligence` / Explorer / domain deep dive 的 shipped-truth 修補：source 現在已固定 archive-wide callout copy、`category_community` label、external-output CTA 文案、Explorer 可見 URL redaction、domain deep dive 的 decoded page path，以及 `/intelligence` runtime digest 只讀 `load_intelligence_runtime`（不再主動輪詢 `load_ai_queue_status`）。這一輪之後，原始 Core Intelligence P1–P4 deterministic 主體範圍已可視為完成；真正還沒交付的原規劃，只剩 `browser-snippet-v1` 之外的 external host integration。若這台主機的 fresh Tauri dev app 仍截到 raw key / 舊 CTA 文案 / 舊 queue 行為，請先把它視為 current-host WebView stale cache noise，而不是 source regression：同一時間 `devUrl` 已能直接提供更新後的 `src/pages/intelligence/{index,sections,domain-deep-dive,copy}.tsx`。
>
> **2026-04-18 UI polish note:** `/intelligence` 首屏現在再收一輪注意力排序：不再保留 archive-wide 大橫幅，也不再把 external-output / Settings 提示做成第二張大 callout；runtime digest 改成更緊湊的 review strip，而低價值或目前為空的 secondary sections（stable sources / search effectiveness / friction / reopened investigations 等）會降到底部 secondary grid，必要時直接不占位。`Stable Sources`、`Search Effectiveness`、`Discovery Trend`、`Breadth Index` 與 `Habits` 也都改成更誠實的人話說明：入口/落地來源會講清楚各自代表什麼；搜尋效率不再只剩抽象條狀圖；探索率趨勢改成帶公式說明的逐週列；廣度卡片明講 breadth score 與 concentration / HHI 看的是不同維度；habits 則改成「約每幾天回來一次、出現於幾天、最近一次何時」，不再把 active-day count 冒充成總 visits。
>
> **2026-04-19 calendar heatmap note:** 使用者已明確推翻上一輪「週內 × 小時主圖」的 accepted 假設；current truth 現在改成：`Browsing Rhythm` 主圖是 **GitHub 式真實日期日曆熱力圖**，資料直接來自 `getDiscoveryTrend(..., 'day')`，每個方格都對應一天真實日期。小時分布不再冒充主圖，而是退回 day-level detail。
>
> **2026-04-20 browsing-rhythm override note:** 進一步的 shipped truth 現在是：`/intelligence` overview 與 Dashboard 的 `Browsing Rhythm` 卡片都改成 **preview-first**。點某一天後，先在卡片下方 lazy-load `getDayInsights(date, profileId)` 的 compact preview（當天摘要 / 全寬 24 小時分布 / 全寬重點網站列 / proportion bar 活動構成）；只有使用者再按明確的 `查看詳情` CTA，才進 `/intelligence/day/:date`。Explorer detail rail 與其他 active day surface 仍維持 route-first，不跟著改成卡內 preview。Dashboard 也繼續固定以 calendar year 呈現，但 year pager 現在不再直接照抄 raw `availableYears`：它必須覆蓋「最早有資料的年份」到 `max(當前年份, 最晚有資料的年份)` 的**連續年份帶**，中間空白年份也要顯示空熱力圖，而當前年份永遠要存在並可一鍵回去。卡片頂部的 summary line 現在一律誠實使用「目前畫面內」的 `totalVisits` 口徑：Dashboard 用 `X visits in 2026` 這類 calendar-year summary，`/intelligence` 則按目前實際選定的單日 / 整月 / 整年 / date span 顯示 exact-range wording，同一年 span 不重複年份；日格 hover 也恢復顯示 exact date + visits/new sites tooltip。
>
> **2026-04-19 entity-first note:** `day` 與 `domain` 現在都已升格成 first-class shared insights entity。`/intelligence/day/:date` 是正式的 exact-day route；`/intelligence/domain/:domain` 雖然內部暫保留 `deep_dive` naming 過渡，但 user-facing IA 已正式視為 `Domain Insights`。`/intelligence` 頂部也新增 `Insight Access` strip，Explorer detail rail 則補上 `Open day insights` / `Open domain insights`。這次之後，`day/domain` 的 primary interaction 默認是 `Insights first`，Explorer evidence 降為 secondary CTA；只有 `/intelligence` 與 Dashboard 的 `Browsing Rhythm` 卡片保留 preview-first 特例。
>
> **2026-04-19 M7 entity-promotion note:** generic insight-entity navigation 現在也已正式收斂成 shipping truth：`query family`、`refind page`、`session`、`trail` 都有 first-class shared insights route；`reopened investigation`、`stable source`、`habit`、`friction`、`compare set`、external-output day/domain chips 等 active surface 也都改成解析到 shared destination，而不是各自手搓 `/explorer` 或留 static label。`path flow` 則只在 step 可穩定解析成 registrable domain 時提供 CTA；剩餘 stable identity gap 已改由 M8 追蹤。
>
> **2026-04-20 search-browser note:** `Search Activity` 的第四個 tab 現在已從 additive `Recent Queries` 收斂成正式的 `Search Keywords` browser：它仍然讀 `get_search_queries`、仍保留 `familyId` / `trailId` / `profileId` reusable identity，也仍不重開 Explorer `queries` view 或新的 route grammar；但 UI 已升格成 bounded paged browser，支援 text / engine / nested date subrange / sort / pagination / page size。`Top Concepts` 也不再用詞雲，而是 horizontal bar chart，且 concept / keyword-facing surface 只允許吃 keyword-eligible rows，不再把 pasted URL / hostname-like navigation noise 混進排行。search-engine rule editor 現在維持在 Maintenance derived-state panel，作為 deterministic rebuild 的一部分。
>
> **2026-04-19 M8 aggregate-identity note:** `compare set` 現在已正式升格成 `/intelligence/compare-set/:compareSetId` first-class route；`path flow` 則改成 stable `flowId` + typed `steps`，前端不再 parsing `flowPattern` label。shared non-overview insights routes 也已 additive 支援受限的 `focusType` / `focusId` query grammar，讓 compare-set / path-flow context highlight 可以跨 trail/day/domain route 共用；overview 會在返回時清掉 focus。trusted external-output payload 現在也有 structured entity targets，而 `public snapshot` 明確維持 redacted。
>
> **2026-04-19 M9 composition note:** route grammar 已在 M6–M8 收口後，這輪正式把 route-level shared composition 也拉回 single source：metric strip、`query-family-card`、compare-set page list、Settings structured target label，以及 section title + `證據與新鮮度` badge 的 header chrome 現在都走 shared primitive，不再由 overview / promoted route / Settings 各自手寫。這也順手修掉了桌面實機上 badge 佔整行與 hover hitbox 過大的 drift。`refind` summary/detail chrome、Explorer richer review rows 與 route / desktop glue decomposition 則改由 M10 追蹤。
>
> **2026-04-19 M10 workbench / transport note:** M10 現在也已正式 closeout。`refind` overview/day/detail route 共用同一套 workbench shell；Explorer `session` / `trail` grouped view 與 promoted route member rows 改吃 shared group-card / row primitive；Integrations external-output / local-host review 也共用 shared review row / code preview / target-link grammar。`src/pages/intelligence/promoted-entity-routes.tsx`、`src/lib/core-intelligence/api.ts`、以及 `src-tauri/src/{commands,worker_bridge}/intelligence.rs` 則已按 ownership split，但 route path、query grammar、command name 與 payload shape 全部維持不變。當時刻意不拆的 mixed helper / dev mirror / `vault-worker` pass-through 已在後續 M11 收口，其中 active deferred 只剩 M12 parity inventory。
>
> **2026-04-19 M11 review grammar note:** M11 現在也已正式 closeout。`src/lib/intelligence.ts` 已退回 thin barrel；route href / entity label helper 正式升格回 `src/lib/core-intelligence/routes.ts`，AI/provider/assistant presentation 與 evidence/assistant link helper 也各自回到獨立 owner。app-visible review grammar 則不再只停留在 intelligence subtree：neutral `review-surface`、`PmeTabBar`、`GeneratedArtifactViewer`、`VerifyCheckList` 現在已被 Settings、Maintenance、Integrations、Schedule、Audit 與 Jobs 共用。dev IPC mirror 與 `vault-worker` pass-through 沒有再被機械拆碎，後續只在 M12 的 support-actions / parity automation inventory 內重看。
>
> **2026-04-19 IA cleanup note:** `On This Day` 已從 `/intelligence` 移除，改成 Dashboard-only surface。原因不是功能失效，而是這張卡不受 `/intelligence` route time scope 影響，放在 Dashboard 更誠實；相對地，`/intelligence` 的 spotlight 現在保留 summary / top sites / browsing rhythm，而 storage analytics 也改成 `core history` / `other data` 的兩層 bucket 心智。
>
> **2026-04-18 desktop-only truth repair note:** 這一輪後續修補不再把 browser preview 當成 `/intelligence` 的驗收替身。current-host live desktop 才是正式 truth gate；因此 `/intelligence` 現在除了保留上面的首屏 hierarchy，也進一步把低信度 section 做成更保守的桌面行為：`Stable Sources` 若只有單邊 leaderboard、`Friction` 若沒有可站得住腳的 signal、`Reopened Investigations` 若只剩不像搜尋問題的 label、`Path Flows` 若看起來更像 auth / callback / canonical redirect path，都會直接隱藏而不是硬佔版面。`Activity Mix` 也補上分類說明，讓 category label 旁邊有代表性網站例子，不再只剩抽象名詞。這台 host 最後是靠重打 release `.app` / 直接啟動 `src-tauri/target/release/pathkeep-desktop` 才看見最新前端；Computer Use 已能在 live desktop 上看到 Explorer 的「第 x / y 頁」摘要與 Habits 新 copy，但 CUA 對直接啟動的 release binary 仍偶發 `noWindowsAvailable`，所以較下方 sections 的簽收仍保留一部分 regression-test 支撐。
>
> **2026-04-20 performance recovery note:** 使用者在三個月真實 archive 上回報 `/intelligence` route 與跨頁返回仍會整個 UI 凍住。這輪 stop-ship 修補後，current source truth 變成：overview batch 在 backend 只重用一條 intelligence SQLite connection / attached archive 與一份 runtime snapshot；frontend 則改成 scope-keyed warm cache + in-flight dedupe + stale-while-revalidate。`domain/day/entity -> back -> /intelligence` 的 same-scope revisit 會先把已看過的卡片用 cache 還原，再做 background refresh，而不是重新打出一整串 foreground request。`Search Activity` 的 hidden tabs (`Search Keywords` / `Query Evolution`) 也改成在首屏穩定後自動 idle prewarm，不再等第一次點 tab 才開始冷載。
>
> **2026-04-28 all-time/progressive-loading note:** `/intelligence` time scope bar 現在新增 `All time / 全部时间 / 全部時間` preset，但首次進入仍維持 `Month`。deep link 使用 `?range=all`，不輸出 custom `start/end`；這一 slice 仍走既有 concrete `DateRange` command contract。`Browsing Rhythm` 在 all-time scope 只渲染實際有資料的日期 span，避免把 1900 年以來的空白日期全量展開。`Browsing Rhythm` 以下的 secondary grid 也改成 cache-aware progressive reveal：已 warm 的 card 先顯示，未 resolved 的 card 保持 skeleton，cold load 仍走 secondary overview batch，不退回多個 foreground IPC fan-out。後續 all-time preload/cache/invalidation 設計記在 [intelligence-all-time-cache-invalidation.md](../plan/intelligence-all-time-cache-invalidation.md)。

---

## 1. 先講結論

PathKeep 現在的 intelligence，不是一個單一功能，而是三層東西疊在一起：

1. **deterministic intelligence baseline**
   - 不靠 LLM、不靠 embedding，也能提供可 shipping 的洞察。
   - 這是現在真正的基線，也是 M5 已接受的正式方向。
2. **optional AI layer**
   - semantic search、assistant、MCP、provider 測試、index build。
   - 這些功能存在，而且 default desktop build 會一起 shipping，但它們預設是 optional。
3. **runtime / queue / review layer**
   - 背景重建、plugin runtime、module runtime、retry / cancel / pause / resume、recovery。
   - 這一層不是 debug 小工具，而是正式產品 surface。

換句話說，現在的 intelligence 不是「只有 Insights 頁」。  
它實際上分散在：

- Dashboard
- Explorer
- Assistant
- Intelligence
- Jobs
- Settings
- sidebar footer background strip

---

## 2. 設計師應該先看哪些文檔

### 如果你要理解「現在產品實際上長什麼樣」

建議閱讀順序：

1. [intelligence-current-state.md](intelligence-current-state.md)
2. [deterministic-intelligence.md](deterministic-intelligence.md)
3. [intelligence.md](intelligence.md)
4. [../design/screens-and-nav.md](../design/screens-and-nav.md)
5. [../architecture/decisions/006-deterministic-intelligence-boundary.md](../architecture/decisions/006-deterministic-intelligence-boundary.md)
6. [../architecture/data-model.md](../architecture/data-model.md)
7. [../architecture/desktop-command-surface.md](../architecture/desktop-command-surface.md)

### 如果你要開始做 intelligence UI redesign

在上面的閱讀順序之外，再讀：

1. [../design/intelligence-ui-redesign-brief.md](../design/intelligence-ui-redesign-brief.md)
2. [../design/jobs-ui-redesign-brief.md](../design/jobs-ui-redesign-brief.md)

### 每份文檔現在適合拿來做什麼

| 文檔                                          | 建議用途                                                    | 現況                                |
| --------------------------------------------- | ----------------------------------------------------------- | ----------------------------------- |
| `docs/features/deterministic-intelligence.md` | deterministic baseline 的正式 source of truth               | **目前最準**                        |
| `docs/features/intelligence.md`               | optional AI / assistant / MCP / queue / shipping truth note | **可用，但混有 legacy 段落**        |
| `docs/design/screens-and-nav.md`              | 畫面 IA、導航、queue grammar、Insights / Jobs 分工          | **大致準確**                        |
| `docs/architecture/decisions/006-...`         | 為什麼 deterministic baseline 要這樣切                      | **準確**                            |
| `docs/plan/m3-*` / `docs/plan/m5-*`           | 歷史 WBS 與 closeout 背景                                   | **拿來看脈絡，不要當 feature spec** |

---

## 3. 現在我們說的 intelligence，到底是在做什麼

白話一點說，PathKeep 現在的 intelligence 目標不是「幫你做很玄的 AI 分析」。

它實際上在做三件更務實的事：

1. **把一堆零碎瀏覽紀錄整理成比較像人能讀的結構**
   - 例如：這些搜尋是同一條問題的演化、這幾個頁面其實是同一條研究線、這個頁面是你一直回來看的穩定參考頁。
2. **把背景重建與補資料過程變得誠實**
   - 例如：現在還在排隊、剛剛卡在哪一段、哪個 plugin 失敗、哪個 job 可以重試。
3. **在有 AI 時加值，但沒有 AI 也不能整塊變空**
   - 所以 deterministic surfaces 必須能自己成立，LLM 和 embedding 只是加法，不是地基。

---

## 4. 當前 intelligence 功能總表

下面這張表是「實際有沒有做」的盤點，不是理想藍圖。

| 能力                                      | 使用者在哪裡看到                                                                                           | 目前狀態                                               | 實際做法                                                                                                                                                                                                                                                                                                                                                                                                                                |
| ----------------------------------------- | ---------------------------------------------------------------------------------------------------------- | ------------------------------------------------------ | --------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| On This Day                               | Dashboard                                                                                                  | **Shipping**                                           | 用 canonical history 做本地日曆日比對，只回看過去年份；因為不受 `/intelligence` time scope 影響，現在不再放進 Intelligence route                                                                                                                                                                                                                                                                                                        |
| Site Analytics                            | Insights                                                                                                   | **Shipping**                                           | 讀 `canonical.topDomains`，顯示 top domains 與 deep-link                                                                                                                                                                                                                                                                                                                                                                                |
| Periodic Summary                          | Dashboard、Insights                                                                                        | **Shipping**                                           | deterministic template summary，沒有 AI 也會顯示                                                                                                                                                                                                                                                                                                                                                                                        |
| Contrastive Summary                       | Insights                                                                                                   | **Shipping**                                           | 比較當前視窗與前一個等長視窗                                                                                                                                                                                                                                                                                                                                                                                                            |
| Topic Timeline                            | Insights                                                                                                   | **Shipping，但不是舊文檔寫的那套**                     | 目前是 deterministic topic aggregation，不是 embedding cluster + LLM naming                                                                                                                                                                                                                                                                                                                                                             |
| Query Groups                              | Insights                                                                                                   | **Shipping**                                           | 用 search evidence、token overlap、landing continuity 分群                                                                                                                                                                                                                                                                                                                                                                              |
| Query Ladders / Query Evolution           | Insights                                                                                                   | **Shipping**                                           | 從 query group 的 steps / stages 產生                                                                                                                                                                                                                                                                                                                                                                                                   |
| Threads / Open Loop                       | backend、Insights cards / stats / explain                                                                  | **部分前台呈現**                                       | backend 有完整 thread summary / detail；UI 目前沒有 thread list 作為主 section                                                                                                                                                                                                                                                                                                                                                          |
| Reference Pages                           | Insights                                                                                                   | **Shipping**                                           | 找被跨 query group / thread 重用的穩定頁面                                                                                                                                                                                                                                                                                                                                                                                              |
| Source Effectiveness                      | Insights                                                                                                   | **Shipping**                                           | 看某個來源是否常成為穩定落點                                                                                                                                                                                                                                                                                                                                                                                                            |
| Insight Cards                             | Insights                                                                                                   | **Shipping**                                           | 從 summary / open loop / reference page 產生可 explain 的卡片                                                                                                                                                                                                                                                                                                                                                                           |
| Explainability                            | Insights                                                                                                   | **Shipping**                                           | deterministic explanation + citations，不靠 LLM                                                                                                                                                                                                                                                                                                                                                                                         |
| Day Insights                              | `/intelligence/day/:date`、Dashboard、Explorer                                                             | **Shipping after 2026-04-20 browsing-rhythm override** | `get_day_insights` 現在是 exact local day 的唯一完整 read model；Dashboard 的 yearly rhythm 與 Intelligence 的 `Browsing Rhythm` 會先 lazy-load compact inline preview，再由明確 CTA 進 shared day route。preview 與 day route 也正式共用同一套 flat 24 小時分布與 proportion bar 活動構成；只有 detail route 的 top-sites section 仍保留較豐富的原本呈現。Explorer detail rail 與其他 day entry 則仍直接用 shared route / href grammar |
| Domain Insights                           | `/intelligence/domain/:domain`、Dashboard、Explorer、Intelligence                                          | **Shipping after 2026-04-20 search-browser**           | `/intelligence/domain/:domain` 正式成為 shared `Domain Insights` route；Top Sites、Habits、Search Effectiveness、Explorer domain chip 與其他 active domain surface 都優先走 shared domain href，而不是各自拼 `/explorer` deep-link。對 search-engine domains，route 現在也會 conditional 顯示 domain-scoped `Search Keywords` browser                                                                                                   |
| Query Family Insights                     | `/intelligence/query-family/:familyId`、Intelligence                                                       | **Shipping after 2026-04-19**                          | `Search Activity`、day-insights families、`Search Effectiveness` hardest topics 現在都會走 shared query-family route；`HardTopic.familyId` 也已成為正式 typed contract                                                                                                                                                                                                                                                                  |
| Search Keywords / Search query history    | `/intelligence` `Search Activity`、`/intelligence/domain/:domain`                                          | **Shipping after 2026-04-20 search-browser**           | `Search Keywords` browser 讀 `get_search_queries`：同一視窗內 dedupe 最新 `(engine, normalized query)`，row 保留 `familyId` / `trailId` / `profileId`，並支援 text / engine / nested date subrange / sort / pagination。concept / keyword-facing surface 只讀 keyword-eligible rows，不再把 URL-like navigation noise 混進結果                                                                                                          |
| Refind Page Insights                      | `/intelligence/refind/:canonicalUrl`、Intelligence                                                         | **Shipping after 2026-04-19**                          | `Refind` cards 現在以 refind insights route 作為 primary CTA；Explorer evidence 與 domain insights 改成 secondary CTA，canonical URL 直接作為 encoded path identity                                                                                                                                                                                                                                                                     |
| Profile-scoped insights                   | shell scope + Insights / Assistant / Explorer                                                              | **Shipping**                                           | shared scope 會影響 insight fetch 與 deep-link                                                                                                                                                                                                                                                                                                                                                                                          |
| Storage analytics                         | Dashboard、Insights                                                                                        | **Shipping**                                           | 用 dashboard storage snapshot，不是從 deterministic pipeline 現場重算；top-level summary 先分成 `core history` / `other data`，detail 才展開                                                                                                                                                                                                                                                                                            |
| Latest growth signal                      | Insights、Settings                                                                                         | **Shipping**                                           | 連回 Audit run                                                                                                                                                                                                                                                                                                                                                                                                                          |
| Semantic / hybrid search                  | Explorer                                                                                                   | **Shipping (W-AI-4/5/6), off by default**              | `FlatVectorIndex`（binary-recall → int8-rescore）+ RRF hybrid + bounded starred boost + `is:starred` facet；gated on `ai.enabled && semantic_index_enabled`，未開時退回 keyword / regex / lexical recall v2。搜尋 tuning（RRF k / weights / starred boost）是 Settings consent knob                                                                                                                                                     |
| AI Assistant                              | Assistant                                                                                                  | **Shipping (W-AI-1/2/3/7), off by default**            | streaming chat（token / reasoning / tool-use 即時可視）+ durable agent harness（journal/replay，崩潰可續跑）；5 tools（search ×4 + `run_code`）；chat 持久化在 `derived/agent.sqlite`（transcript 不進 export）；gated on `assistant_enabled`                                                                                                                                                                                           |
| Code-mode sandbox                         | Assistant（tool-use timeline）                                                                             | **Shipping (W-AI-8), default-enabled + sandboxed**     | Wasmtime + Javy 沙箱跑 LLM 生成 JS 做只讀聚合查詢；零 ambient 權限（無 fs/net/clock/random/env）、hash-pinned guest、read-only host API、硬上限；沙箱本身即安全邊界，故不設模型能力門檻                                                                                                                                                                                                                                                 |
| AI provider config / test                 | Settings                                                                                                   | **Shipping (REACH-A)**                                 | per-provider editor（local draft + explicit save）+ per-provider connection test（latency / capability / error）；secret store / clear 獨立動作                                                                                                                                                                                                                                                                                         |
| MCP server / skills                       | Settings、外部 MCP client                                                                                  | **Shipping (W-AI-9-B/C), off by default**              | localhost stdio、read-only（search / status / usage-guide）、unlock-gated、每次外部查詢 audited 為 `mcp_query`、key 不外洩；gated on `mcp_enabled`（skills gated on `skill_enabled`）                                                                                                                                                                                                                                                   |
| AI queue review                           | Jobs、sidebar                                                                                              | **Shipping, off by default**                           | embedding backfill / assistant queue 在 `ai.enabled` 時於 Jobs / sidebar 顯示；未開 AI 時不顯示可操作 AI queue，只呈現 deterministic runtime 與 archive workflow queue truth                                                                                                                                                                                                                                                            |
| Deterministic / enrichment runtime review | Insights、Jobs、Settings、sidebar                                                                          | **Shipping**                                           | `intelligence_jobs` + module / plugin runtime read model                                                                                                                                                                                                                                                                                                                                                                                |
| Sidebar background strip                  | shell footer                                                                                               | **Shipping**                                           | compact queue summary + progress bar + Jobs link                                                                                                                                                                                                                                                                                                                                                                                        |
| Jobs page                                 | `/jobs`                                                                                                    | **Shipping**                                           | 專門看 background work、retry、cancel、recovery                                                                                                                                                                                                                                                                                                                                                                                         |
| Maintenance derived-state panel           | Maintenance                                                                                                | **Shipping**                                           | plugin / module / rebuild / clear review surface；2026-04-19 起也正式承接 search-engine rule editor，built-ins read-only、custom rules 可新增 / 編輯 / 刪除，變更後走既有 rebuild / runtime refresh 流程；recent job details deep-link 到 Jobs                                                                                                                                                                                          |
| Workflow map                              | backend snapshot                                                                                           | **backend-only**                                       | snapshot 有資料，但當前 Insights UI 沒有 render                                                                                                                                                                                                                                                                                                                                                                                         |
| Profile facets                            | backend snapshot                                                                                           | **backend-only**                                       | snapshot 有資料，但當前 Insights UI 沒有 render                                                                                                                                                                                                                                                                                                                                                                                         |
| Session / trail / navigation-path surface | backend command                                                                                            | **shipping after 2026-04-15 reset**                    | `get_sessions`、`get_search_trails`、`get_navigation_path` 已取代舊 thread-detail 想像，成為 Explorer / Intelligence 的新 deterministic 結構單位；Explorer grouped view 仍是 browse-first canonical surface                                                                                                                                                                                                                             |
| Session Insights                          | `/intelligence/session/:sessionId`、Explorer                                                               | **Shipping after 2026-04-19**                          | session grouped view 現在保留 inline expand，同時新增 shared `Open session insights` CTA；detail route 沿用既有 `range` / `start` / `end` / `profileId` query grammar                                                                                                                                                                                                                                                                   |
| Trail Insights                            | `/intelligence/trail/:trailId`、Explorer、compare-set focus drilldown                                      | **Shipping after 2026-04-19**                          | trail grouped view 現在有 shared `Open trail insights` CTA；若目前 focus 來自 compare-set，trail route 也會顯示 aggregate context callout / highlight                                                                                                                                                                                                                                                                                   |
| Compare Set Insights                      | `/intelligence/compare-set/:compareSetId`、Intelligence compare-set cards、focused trail/day/domain routes | **Shipping after 2026-04-19**                          | compare-set 現在有 dedicated detail read model、recent compare days、shared hero/actions 與 explainability；它不再只借 trail route 寄生                                                                                                                                                                                                                                                                                                 |
| Generic explainability                    | backend command                                                                                            | **shipping after follow-up**                           | `explain_entity` 已經成為 accepted cross-entity deterministic explainability entrypoint；`explain_refind` 只剩 facade / compatibility role                                                                                                                                                                                                                                                                                              |
| Section evidence / freshness badge/panel  | Intelligence、domain deep dive                                                                             | **Shipping after 2026-04-18**                          | backend section envelope 現在會帶 `generatedAt`、scope/window、module/source-table provenance、enrichment flag、以及 stale / disabled / degraded reason；UI 以 compact badge + floating review panel 呈現，Maintenance / Jobs 仍擁有 rebuild / clear / retry                                                                                                                                                                            |
| Embed / widget / public snapshot review   | Integrations                                                                                               | **Shipping，manual review + trusted local host**       | Integrations 現在可 preview / copy `embed cards`、`widget snapshot`、`public snapshot`，也可 preview / build / verify `browser-snippet-v1` 的 `index.html` / `bundle.json` 本地 artifact；trusted payload 會帶 structured entity targets 供 app links 復用，而 public snapshot 仍不安裝 OS widget、也不發布 localhost/public API                                                                                                        |
| Module registry                           | backend runtime / config                                                                                   | **shipping after follow-up**                           | trait-backed registry 現在是 built-in module defaults、runtime status、dependency ordering、staged rebuild ownership、explainability ownership 的單一真相                                                                                                                                                                                                                                                                               |

---

## 5. 沒有 LLM、沒有 embedding 時，現在到底有哪些洞察

這一段只講 deterministic intelligence，也就是你特別關心的那層。

### 5.1 On This Day

設計目的：

- 讓使用者快速回看「過去幾年的同一天，我那時候在看什麼」。
- 它是一個回顧卡，不是 timeline 全覽。

現在的實際行為：

- 只看過去幾年的同一天。
- 不會把當前年份今天的紀錄混進來。
- 用本地 timezone 的日曆日，不是 UTC 假裝對齊。
- 只有 Dashboard 會顯示這張卡；`/intelligence` 不再承接它。
- 每一筆都可以 deep-link 回 Explorer。

這張卡現在是可信的 deterministic surface，不需要 AI 才成立。

### 5.2 Site Analytics

設計目的：

- 回答「最近這段時間，我主要在哪些網站花力氣」。
- 它不是做人格分析，也不是判斷網站好壞。

現在的實際行為：

- 顯示最近視窗內的 top domains。
- 以簡單排名與條形寬度呈現相對量級。
- 點 domain 可以回 Explorer 重新看 evidence。

這是 canonical summary，不靠 AI。

### 5.3 Periodic Summary

設計目的：

- 用人能直接讀懂的段落，幫使用者知道最近一段時間大概在研究什麼。

現在的實際行為：

- backend deterministic 直接產出 `periodic-summary`。
- 也會產出 `contrastive-summary`，用來講「這段時間和前一段時間比，有沒有變化」。
- 即使 AI 關掉，summary 仍會顯示。
- 有 AI 時，未來可以再把 deterministic output 改寫得更像人話，但基底不是 AI。

### 5.4 Query Groups

設計目的：

- 把一次問題搜尋與後續點進去看的頁面，收斂成同一組研究動作。
- 不再把每個搜尋頁、每個 landing page 都看成互相無關。

現在的實際行為：

- 優先吃 canonical search term 與 search-result URL 的 query。
- 也看 query token overlap、referrer / redirect chain、landing continuity。
- 會記住：
  - root query
  - latest query
  - steps
  - stages
  - confidence
  - evidence tier
- Insights 頁直接把它當一個正式 section 顯示。

這是目前 deterministic baseline 最重要的 shipping 單位之一。

### 5.5 Query Ladders / Query Evolution

設計目的：

- 讓使用者回看「我的問題是怎麼一步一步變具體的」。

現在的實際行為：

- 從 query groups 裡的 `steps` / `stages` 生成 ladder。
- UI 會把 stage 翻成人可讀 label。
- Chromium search term evidence 最強，其它來源如果只是從 URL 猜 query，可信度要低一級看待。
- 現在已能 deep-link 回 Explorer。

### 5.6 Topic Timeline

設計目的：

- 回答「我最近在關注哪些主題」。

現在的實際做法，和舊文檔最不同：

- **現在不是 embedding 聚類加 LLM 命名。**
- 現在是用 thread title、query group title 的 token similarity 做 deterministic aggregation。
- 換句話說，topic 目前比較像「從現有 thread / query group 摘出來的主題彙整」，不是完整的 semantic topic clustering 系統。
- UI 會用簡單條形與 count 呈現，不是高解析度時間序列圖。

所以如果設計師要重做這個區塊，不能拿舊文檔那種「向量 topic cluster」去設計過度複雜的操作模型。

### 5.7 Threads / Open Loops

設計目的：

- 把跨 burst、跨 query group、可跨天 reopen 的研究線收斂起來。
- 幫使用者看出哪些問題是反覆回來看的，而不是一次就結束。

現在的實際行為：

- backend 會建立 thread summary、thread detail、open-loop score、confidence、evidence tier。
- Insights page 目前會用：
  - thread count
  - open-loop card
  - explainability
  - reference/source surfaces
    來間接表達 thread 結果。
- **但目前前台沒有把 threads 做成一個真正的一等 section。**

這是現在 UI 與 backend 之間最值得設計重做的一個缺口。

### 5.8 Reference Pages

設計目的：

- 找出那些你會反覆回來看的高價值頁面。
- 它們通常是文件、repo、討論串、工具頁、文章，而不是一時點進去的路過頁。

現在的實際行為：

- 看 revisit 次數、cross-day revisit、出現在多少個 query groups / threads。
- Insights 頁把它做成正式 section。
- 每一列都可以 explain，也可以回 Explorer。

### 5.9 Source Effectiveness

設計目的：

- 回答「哪些網站對你來說常常真的有幫助，而不是只有被你打開很多次」。

現在的實際行為：

- 看它是不是 query group 後段的穩定落點。
- 看它是不是被 reference pages 與 reopen support 反覆支撐。
- 現在 UI 會顯示 domain、source role、group / reference / landing counts。

### 5.10 Insight Cards

設計目的：

- 讓使用者不用讀完整頁，就先抓到幾個值得看的重點。

現在的實際行為：

- 從 template summaries 先取前幾張 card。
- 額外補：
  - open-loop card
  - reference-page card
- 每張卡都能 explain。

### 5.11 Explainability

設計目的：

- 不讓洞察變成一句「看起來很合理但不知道從哪來」的黑盒話術。

現在的實際行為：

- `thread`
- `query-group`
- `reference-page`
- `template-summary`
- `topic`
- 一般 `card`

都可以拿到 deterministic explanation。

而且目前 explain 是：

- `usedLlm = false`
- 有 citation
- 有 notes

也就是說，這層 explainability 現在是可 shipping 的 deterministic contract。

---

## 6. optional AI intelligence 現在做到哪

### 6.1 Semantic Search

現在在 Explorer 裡。

使用者可切：

- `keyword`
- `semantic`
- `hybrid`

現在的實際做法（W-AI-4/5/6 已交付，off by default）：

- `semantic` / `hybrid` 在 `ai.enabled && semantic_index_enabled` 開啟後可用；未開時誠實退回 keyword / regex / lexical recall v2。
- 向量走自製 `FlatVectorIndex`（binary-recall → int8-rescore），寫在 `derived/vectors/` sidecar plane，**不連結 LanceDB**，也不做全庫 cosine 掃描。
- hybrid 用 RRF 融合 lexical + semantic，加 bounded starred boost；支援 `is:starred` facet。RRF k / weights / starred boost 是 Settings consent knob（W-AI-9-A）。
- 不允許在 request path 偷偷依賴 SQLite metadata 當假 semantic。
- result 會顯示 score band、match reason、visited time、profile、deep-link。

### 6.2 AI Assistant

現在是 streaming agent chat（W-AI-1/2/3/7），不是 queue-backed 黑盒。

- streaming：token / reasoning / tool-use 即時可視，markdown 串流渲染。
- durable agent harness：journal-before-observe + replay，崩潰可續跑而不重複呼叫模型 / 不重複收費。
- 5 tools：`search_history`（hybrid）/ `search_bm25` / `search_vector` / `search_hybrid` / `run_code`（code-mode 沙箱）。
- 回答強制引用真實 rows（citation 帶 `historyId` / `canonical_url`），可深鏈回 Explorer。
- chat 持久化在 `derived/agent.sqlite`（`conversations` / `messages` + `agent_runs` / `agent_steps` / `agent_citations`）；transcript **不進 export**。
- gated on `ai.enabled && assistant_enabled`（預設關）。

### 6.3 Integrations preview（deterministic external-output）vs AI MCP server

兩者要分清楚：

- **Integrations preview**（deterministic intelligence external-output）：在 Integrations 做 manual review / copy-export（embed cards / widget / public snapshot / `browser-snippet-v1` trusted local host）。定位是給使用者 review，不是假裝已自動安裝。
- **AI MCP server**（W-AI-9-B，已交付）：localhost stdio MCP server，opt-in、hard-default-OFF（`mcp_enabled`）、unlock-gated、**read-only**（`search-history` / `archive-status` / `usage-guide`），SQLCipher key 不外洩，每次外部查詢 audited 為 `mcp_query`。skills（W-AI-9-C）的 usage-guide JSON gated on `skill_enabled`。邊界見 [../architecture/ai-security-posture.md](../architecture/ai-security-posture.md) §3。

---

## 7. intelligence runtime 現在的真實樣子

這一層很重要，因為現在很多 UI 痛點不是演算法本身，而是 runtime review 沒被整理好。

### 7.1 其實有兩套 queue

#### AI queue

用途：

- semantic index build / clear
- assistant jobs

資料表：

- `ai_jobs`

前台 surface：

- Explorer runtime panel
- Assistant
- Jobs page
- sidebar strip（聚合後）

#### intelligence runtime queue

用途：

- deterministic rebuild
- enrichment plugin jobs

資料表：

- `intelligence_jobs`
- `deterministic_module_runtime`

前台 surface：

- Insights runtime digest
- Jobs page
- Maintenance derived-state panel
- sidebar strip（聚合後）

這兩套 queue 同時存在，是現在產品真相。  
如果未來 UI 要重做，不能把它們硬藏成一套「萬用背景任務」而失去邊界，也不能讓使用者被兩套 queue 的細節淹死。

### 7.2 built-in enrichment plugins

現在正式 shipping 的 built-ins 只有兩個：

1. `title-normalization`
   - local-only
   - 目的是把 noisy title 收斂成更穩定 evidence label
2. `readable-content-refetch`
   - network-backed future feature
   - v0.2.0 disabled；目前不補抓可讀正文，也不提供 site-adapter evidence

它們都屬於 derived state runtime，不改 canonical archive facts。

### 7.3 deterministic modules

現在正式 shipping 的 built-ins 有八個：

1. `visit-derived-facts`
2. `daily-rollups`
3. `sessions`
4. `search-trails`
5. `refind-pages`
6. `activity-mix`
7. `search-effectiveness`
8. `domain-deep-dive`

每個 module 現在都已有：

- id
- version
- enabled
- dependsOn
- derivedTables
- status
- lastBuiltAt
- staleReason
- notes

也就是說，module registry 已經不是文檔構想，而是實作中的 runtime contract。

### 7.4 progress / heartbeat / cancel

deterministic rebuild 現在不是只有抽象的 `running`。

worker 會回寫：

- phase
- detail
- completedSteps / totalSteps
- processedItems / totalItems
- progressPercent

cancel 也不是假裝立即中止，而是 cooperative stop：

- UI 設 stop request
- worker 在 phase / chunk 邊界停
- runtime 留下 cancelled trace

### 7.5 recovery

如果 app 上次中斷：

- deterministic rebuild running jobs 會被 recover/requeue
- enrichment running jobs 也會 recover/requeue
- runtime notes 會誠實告訴使用者有 recovered jobs

這一點已經有 backend 實作和測試，不只是文檔要求。

---

## 8. 前端各頁目前各自扮演什麼角色

### Dashboard

目前不是完整 intelligence 頁，而是入口與摘要頁。

它現在做的事：

- 顯示整體 AI / intelligence status
- 顯示 On This Day
- 顯示 yearly browsing rhythm preview
- 提供去 Explorer / Assistant / Insights 的快速入口

### Explorer

目前是：

- canonical evidence 主場
- semantic / hybrid recall 的入口
- AI runtime panel 的修復與 queue review 入口
- `view=time|session|trail` 已共用同一個 `/explorer` route；session / search trail 不再是 repo 裡孤立的 demo panel，而是正式 route state
- session / trail 視角共用同一條 detail rail 與 navigation tracer；使用者在 grouped view 選到的 visit，仍然能回答「你是怎麼找到這裡的」
- 若 Explorer route 沒有顯式 `profileId`，會沿用 shared profile scope；若 URL 有 `profileId`，則頁面級 scope 優先

### Assistant

目前是：

- queue-backed Q&A surface
- provider probe
- queued job reload / run / cancel
- citation review

### Intelligence

目前是：

- deterministic analysis snapshot 主頁
- `Insight Access` strip：直接打開 day/domain 完整頁面
- top-of-page runtime digest
- explainability 主頁
- `/intelligence`、`/intelligence/day/:date`、`/intelligence/domain/:domain`、`/intelligence/query-family/:familyId`、`/intelligence/refind/:canonicalUrl`、`/intelligence/session/:sessionId` 與 `/intelligence/trail/:trailId` 現在已收斂成同一套 shared entity grammar：day route 只保留 `profileId` query，其餘 entity route 沿用 `range` / `start` / `end` / `profileId`
- 主頁上大多數 deterministic section 現在都會吃 effective profile scope；只有天生 archive-wide 的 surface（例如 multi-browser diff）會在 UI 上明講自己沒有跟著 scope 一起收窄
- explainability 已實際接進 Refind Pages、Query Families、Reopened Investigations、Habits、Path Flows；Explorer 的 sessions / search trails 也已能直接展開 explainability
- active entity 的 CTA 現在不再各自決定 destination：`reopened investigation` 解析到 anchor route、`habit/stable source/friction/multi-browser diff` 走 `domain insights`、`compare set` 走自己的 first-class insights route，而 path-flow domain chips 會帶著 `focusType=path-flow` 進 shared `domain insights`
- `/intelligence`、`/intelligence/day/:date`、`/intelligence/domain/:domain`、`/intelligence/query-family/:familyId`、`/intelligence/refind/:canonicalUrl`、`/intelligence/session/:sessionId`、`/intelligence/trail/:trailId`、`/intelligence/compare-set/:compareSetId` 現在共用同一套 entity grammar；overview 不承接 focus，但其餘 shared route 可 additive 接受 `focusType` / `focusId`
- 外部 `embed/widget/public snapshot` 現在已有正式的 Integrations manual review / copy-export surface，並且多了 `browser-snippet-v1` trusted local host 的 preview / build / verify flow；trusted payload 會帶 structured entity targets，`public snapshot` 則維持 redacted，不暴露 internal reusable IDs。`/intelligence` 則改成 CTA，把使用者帶去 Integrations，而不是再假裝這塊還完全不存在
- `refind` summary/detail chrome、Explorer session/trail member row、以及 Integrations external-output / local-host review row 現在都已進入 shared workbench contract；M11 進一步把 neutral review grammar 推到 Settings / Maintenance / Integrations / Schedule / Audit / Jobs，而剩餘 dev mirror / `vault-worker` transport debt 則改由 M12 的 parity inventory 追蹤

目前實際 section 順序是：

1. time range selector + scope note
2. `Insight Access` strip
3. runtime digest
4. spotlight / summary
5. research signals
6. evidence / health
7. explainability panel

### Jobs

目前是：

- background work 的完整 review surface
- pause / resume
- plugin / module / recent jobs / recovery

### Settings / Maintenance / Integrations

目前是：

- Settings：language、Explorer background prefetch、browser profile selection、App Lock preferences、AI provider / API-key configuration、Data migration（Export / Import 整機 `.pathkeep-bundle`）。
- Maintenance：updates、retention cleanup、derived-state rebuild / clear、plugin / module enable-disable、diagnostics、platform troubleshooting
- Integrations：manual external-output review / copy-export、trusted local host preview / build / verify（`browser-snippet-v1`）、MCP / skill generated artifact review
- Jobs：runtime queue / recent jobs / retry / cancel 的 canonical surface

### Sidebar footer strip

目前是：

- 永遠可見的 compact background status
- progress bar
- 快速跳 Jobs

---

## 9. backend 已經有，但 UI 還沒好好用上的東西

這些都很重要，因為它們會直接影響 redesign 是否要補畫面、補 IA、補 interaction。

### 9.1 舊 thread detail 不再是新主契約

pre-reset backend 曾有：

- `load_thread_detail`
- `insight_threads`
- `insight_thread_members`

但 2026-04-15 之後，新的 accepted deterministic structure 改成：

- `get_sessions`
- `get_search_trails`
- `get_navigation_path`
- `get_hub_pages`

如果前台還有 thread / query-group / snapshot-first UX，那都應視為 legacy UI surface，而不是新的 source of truth。

### 9.2 workflow map / profile facets 已存在 snapshot，但目前沒 render

snapshot type 與 backend 都已經有：

- `workflowMap`
- `profileFacets`

但當前 Insights UI 沒把它們畫出來。  
這代表設計師如果要重做，可以決定：

- 要不要把它們正式上架
- 還是先留 backend-only

### 9.3 topic 現在比較像「彙總結果」，不是互動式 topic workspace

UI 只有：

- label
- bar
- count
- deep-link

沒有：

- topic detail
- explain button
- timeline drilldown

### 9.4 還沒完全收尾的前端 truth

- `/intelligence` / `/intelligence/day/:date` / `/intelligence/domain/:domain` 的 route、scope、time-range、runtime digest 與 `/insights` 命名漂移，現在已經收斂回一致的 shipping truth
- grouped Explorer 目前仍以 deterministic date window + profile scope 為主，不會把 keyword query / regex 再硬套進 session / trail API 假裝有 backend 支援
- habit / path-flow explainability 目前只有在明確 profile-scoped view 下才會出現，因為 backend explain contract 需要 `<profile_id>::...` entity id
- external output payload 現在已有 Integrations manual review / copy-export consumer，且第一個 `browser-snippet-v1` trusted local host 已交付；剩下 deferred 的是 OS widget、localhost/public host API、以及其他 alternate hosts，這不再算 `WORK-CI-F` 的未完成漂移

---

## 10. 哪些文檔已經過時，哪些沒有

### 10.1 `docs/features/deterministic-intelligence.md`

判斷：

- **大致準確**

原因：

- 它跟 `ADR-006` 對得上。
- 它講的 burst / query group / thread / module / invalidation / no-AI baseline，跟當前 backend 是一致的。

### 10.2 `docs/features/intelligence.md`

判斷：

- **部分過時，部分準確**

最主要的過時點：

- `Topic Timeline` 仍殘留 embedding cluster + LLM naming 的描述，但目前 shipping 不是這樣。
- 文檔裡同時保留了 M3 optional AI、M4/M5 shipping truth note、以及一些舊 deterministic 想像，讀起來容易混。

仍然準確的部分：

- optional AI / assistant / MCP / queue / runtime surface 的大方向
- Jobs / Insights / Settings / sidebar 的 queue honesty contract

### 10.3 `docs/design/screens-and-nav.md`

判斷：

- **大致準確**

原因：

- 它對 Jobs / Intelligence 的分工、queue grammar、shared profile scope、runtime digest 的規範，和現在前台很接近。

### 10.4 `docs/plan/m3-*`、`docs/plan/m5-*`

判斷：

- **不算過時，但不是現在最該拿來做設計的文檔**

原因：

- 它們比較像「當時怎麼做、做完了哪些 work package」，不是當前 UX source of truth。

---

## 11. 這次盤點後，我認為最值得立刻重做的地方

### 11.1 Intelligence 的核心問題不是資料不夠，而是層次太亂

現在它把：

- summary
- storage
- runtime
- query groups
- topic timeline
- reference pages
- source effectiveness
- module health
- cards
- explainability

全部塞進同一種 panel 語法裡。  
資訊是真的，但頁面不會自己告訴使用者「先看哪裡」。

### 11.2 Jobs 的核心問題不是功能缺，而是沒有把 triage 畫面做對

現在它其實已經有：

- running now
- queued
- failed
- plugin status
- module status
- recovery
- recent AI jobs
- recent runtime jobs

但視覺上還是太像一整頁 operations dump，而不是一個讓人快速判斷優先順序的 review surface。

### 11.3 Threads 現在是 backend 核心概念，但 UI 不是

這是目前 intelligence 設計最可惜的一點。

現在真正有結構意義的是：

- query group
- thread
- reference page

但 UI 目前比較強調：

- cards
- topic bars
- flat lists

所以 designer 這一輪很值得重新思考：

- thread 是否應該成為一等主體
- topic 是否應該退到比較輕的概覽層

---

## 12. 本次調查的結論

如果只用一句話總結：

**PathKeep 現在已經有一套可用、可解釋、可重建的 deterministic intelligence 系統；真正需要大改的，不是「有沒有 intelligence」，而是「怎麼把這套已存在的 intelligence 和 queue truth 整理成使用者第一次看就懂的 UI」。**

這也是為什麼接下來的設計重點應該是：

- 不是再 invent 新功能名詞
- 而是重新整理頁面層次、區塊主次、thread 的主角地位、queue triage 的閱讀順序，以及 explainability 的可達性

---

## 相關文檔

- [deterministic-intelligence.md](deterministic-intelligence.md)
- [intelligence.md](intelligence.md)
- [../design/intelligence-ui-redesign-brief.md](../design/intelligence-ui-redesign-brief.md)
- [../design/jobs-ui-redesign-brief.md](../design/jobs-ui-redesign-brief.md)
- [../design/screens-and-nav.md](../design/screens-and-nav.md)
- [../architecture/decisions/006-deterministic-intelligence-boundary.md](../architecture/decisions/006-deterministic-intelligence-boundary.md)
