# M5-RT — Enrichment Runtime And Operations

> 讀這份文檔的時機：當 M4 closeout 已經把 release 相關工作收斂，但 enrichment plugin / queue / rebuild / retry 仍然停留在同步 pipeline 或 truthfully deferred 狀態時。  
> 這份文檔要處理的不是「再多做幾個 plugin」，而是先把 plugin 運行系統做對。

---

## Source Inputs

- [../../features/archive.md](../../features/archive.md)
- [../../features/intelligence.md](../../features/intelligence.md)
- [../m3-intelligence/providers-indexing-and-jobs.md](../m3-intelligence/providers-indexing-and-jobs.md)
- [../m4-full-polish/enrichment-advanced-intelligence-and-remote.md](../m4-full-polish/enrichment-advanced-intelligence-and-remote.md)
- [../program/research-and-decisions.md](../program/research-and-decisions.md)

---

## 本工作包要交付什麼

- enrichment plugin contract、registry、enable / disable、derived-state boundary
- SQLite-backed intelligence queue，至少先覆蓋 enrichment 與 insight refresh
- retry / cancel / rebuild / clear derived data 的一致操作語義
- Settings / Insights 的可觀察 surface，誠實顯示 plugin、queue、degrade state

---

## WBS

### Plugin Contract And Safety Boundary

- [x] `M5-RT-PL-001` 把現有 enrichment 抽成正式 plugin contract，至少包含 input scope、derived output、evidence、freshness、error surface、network usage。（2026-04-10：`title-normalization` / `readable-content-refetch` 現在以 built-in registry + Settings / Insights review surface 凍結）
- [x] `M5-RT-PL-002` 為 built-in plugin 建立 registry，支援 metadata、version、enabled state、manual rebuild 和 invalidation。（2026-04-10：前後端都回補 dual-plugin defaults，避免 config / runtime drift）
- [x] `M5-RT-PL-003` 補齊 canonical vs derived boundary，禁止 plugin 直接寫入 archive 核心事實表。（2026-04-10：Settings / docs / schema note 已明確把 plugin runtime 限縮在 derived tables）
- [x] `M5-RT-PL-004` 為 plugin 定義 honest degrade story，例如 disabled、unsupported browser、network refused、rate limited。（2026-04-10：disabled / failed / queue review / retry / cancel copy 已接到 Settings / Insights）
- [x] `M5-RT-PL-005` 先完成 day-one built-in plugin 組合，至少包含 local-only plugin 和 network-backed plugin 各一種。（2026-04-10：`title-normalization` + `readable-content-refetch` 正式成為 first-party built-ins）

### Queue And Orchestration

- [x] `M5-RT-Q-001` 建立 intelligence queue schema，至少支援 job type、plugin id、state、attempt、priority、payload、artifact / error trace。
- [x] `M5-RT-Q-002` 定義第一版 lifecycle：queued、running、succeeded、failed、cancelled；pause / resume 若未落地需 truthfully defer。
- [x] `M5-RT-Q-003` 實作 claim / execute / retry / cancel 的基礎邏輯，避免每次 insight refresh 都偷偷同步做完所有 enrichment。（2026-04-10：retry / cancel 現在有 state guard，不再允許成功 job 被誤改寫）
- [x] `M5-RT-Q-004` 讓 queue 和 insight run / backup run 建立關聯，使用戶能追到 job 是誰觸發的。
- [x] `M5-RT-Q-005` 為 retry / full rebuild / clear derived data 定義精確語義，避免 UI、worker、schema 三邊各說各話。

### Runtime UX And Operability

- [x] `M5-RT-UX-001` 在 Settings 提供 plugin review surface：enabled state、network boundary、freshness / rebuild hint、no-provider honesty。
- [x] `M5-RT-UX-002` 在 Insights 或 Dashboard 提供 queue status、recent failures、retry / cancel controls 和 evidence boundary copy。（2026-04-10：目前落地在 Insights）
- [x] `M5-RT-UX-003` 明確區分 deterministic insight refresh、network enrichment、LLM enhancement 三類能力，不讓使用者誤會它們是同一件事。
- [x] `M5-RT-UX-004` 為 browser preview、archive locked、plugin disabled、queue idle / failed 設計清楚空狀態和降級文案。（2026-04-10：browser preview runtime fixture、disabled / failed / unavailable copy 已接上；archive locked 仍走既有 session guard refusal）

### Testing And Acceptance

- [x] `M5-RT-QA-001` 建立 plugin contract tests，覆蓋 enable / disable、freshness、failure、clear derived data。（2026-04-10：前端 registry / config merge tests 與既有 rebuild / clear flow 一起簽收）
- [x] `M5-RT-QA-002` 建立 queue acceptance tests，覆蓋 enqueue、claim、success、failure、retry、cancel。
- [x] `M5-RT-QA-003` 建立 Settings / Insights runtime UX tests，驗證 plugin state、queue state 和 no-send / no-run 情境。
- [ ] `M5-RT-QA-004` 建立一輪資料主權 review，確認 plugin / queue 不偷偷突破 local-first 與 evidence-first 邊界。

### 2026-04-10 closeout note

- M5-A 現在正式把 first-party-only enrichment runtime、dual built-in plugin defaults、SQLite-backed `intelligence_jobs` review surface，以及 Settings / Insights 的 retry / cancel / degrade copy 一起簽收。
- `PG-RD-AI-010` 仍保持 open：shipping runtime 只允許 checked-in heuristic / built-in plugin assets；新的 external tokenizer、language-hint、registrable-domain 或 optional model asset 仍不得直接進 bundle。

---

## Exit Artifacts

- plugin registry 和 built-in plugin runtime
- queue-backed enrichment orchestration
- rebuild / retry / clear derived data 操作面
- runtime observability 和 acceptance tests
