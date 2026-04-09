# M3-AI — Providers, Indexing, And Jobs

> 讀這份文檔的時機：當你要在不破壞 archive 核心可信度的前提下，把 optional intelligence 從概念做成穩定可重建的增值層。  
> 這份文檔處理的是 AI 基礎設施，不是最終使用者體驗細節。

---

## Source Inputs

- [../../features/intelligence.md](../../features/intelligence.md)
- [../../architecture/tech-stack.md](../../architecture/tech-stack.md)
- [../../database-selection-decision-2026-04-05.md](../../database-selection-decision-2026-04-05.md)
- [../program/research-and-decisions.md](../program/research-and-decisions.md)
- [../m0-foundation/backend-and-data-rearchitecture.md](../m0-foundation/backend-and-data-rearchitecture.md)
- [../m2-recall-and-trust/imports-browsers-and-rollback.md](../m2-recall-and-trust/imports-browsers-and-rollback.md)

---

## 本工作包要交付什麼

- AI provider registry、model selection、secret storage、connection test
- SQLite-backed job queue 和 background indexing orchestration
- LanceDB 或等價 sidecar index 的正式落地
- 可重建、可清空、可重跑的 embedding / indexing pipeline
- MCP server / IDE integration 的安全基礎

## 實作註記（2026-04-07 / WORK-M3-A）

- provider config 現在正式區分 `purpose`、`requestFormat`、`baseUrl`、`defaultModel`、`modelCatalog`、`temperature` / `maxTokens` / `dimensions` 與 `apiKeySaved`；provider secret save / clear 透過 keyring surface 留下可驗證結果，不會把 preset 與 model selection 一起刪掉。
- provider capability / connection test 已落地：worker / Tauri / frontend contract 會回傳 chat / embedding / streaming / tool-use / structured-output capability、latency、error code、action hint、retry hint。Anthropic 在當前 rig.rs integration 下保持 day-one chat-only。
- `ai_jobs` queue schema 已存在於 canonical archive SQLite；index build / clear 與 assistant request 共用同一 queue。pause / resume 只影響 queued / stale jobs；running job 不支援 mid-flight cancel；manual replay 只允許 failed / cancelled / paused / stale job。
- semantic index 現在由 SQLite `ai_index_ledger` + LanceDB sidecar 組成：SQLite 記 provider / model / sidecar table / source watermark / last run / failure，LanceDB 保存真正的 vector rows 與 ANN index。`clear-only`、`full rebuild`、incremental catch-up 三種模式都會留下 queue / run trace。
- backup auto-index、手動 build、manual replay、queue drain 都走同一組 worker orchestration；沒有 AI provider、queue 被 pause、archive 尚未初始化等情況，都會回傳誠實降級狀態而不是偷偷失敗。
- packaging / supply-chain 註記：目前 LanceDB 依賴鏈仍 transitively 拉入 `tantivy 0.24.2 -> lru 0.12.x`。RustSec `RUSTSEC-2026-0002` 影響 `IterMut`，而 PathKeep 當前只經由 tantivy `StoreReader` 使用 `get` / `put` / `len` / `peek_lru` cache path，因此先保留 allowlist，等待上游兼容升級。

## 2026-04-09 closeout（`WORK-QC-D`）

- semantic retrieval 現在會先查 LanceDB sidecar，sidecar 缺失或失敗時才退回 SQLite compatibility mirror，且會留下明確 notes，而不是靜默掃全表
- index readiness / stale detection 現在已 model-scoped，並會對 import / rollback / visibility watermark 與 readable-content enrichment freshness 變化回傳 `stale`，要求使用者手動 rebuild
- embedding build 現在有 batch sizing、retry、partial success handling；逐筆 fallback 只在 batch 失敗時才發生
- Settings 現在可見 indexed rows、sidecar bytes、mirror bytes、estimated embedding tokens，以及 MCP / skill integration 的 consent / capability / scope / audit preview
- MCP 現在明確保留 read-only lexical fallback：沒有 embedding provider 時可搜尋，但必須明講 semantic recall unavailable；每次 external query 都會留下 `mcp_query` run
- 整體 truth stance 見 [../m4-full-polish/intelligence-60-year-envelope.md](../m4-full-polish/intelligence-60-year-envelope.md)：這代表 M3 intelligence v1 可用，但**不等於**已對 60-year all-features baseline 做最終性能背書

---

## WBS

### Provider Registry And Secret Management

- [x] `M3-AI-PR-001` 凍結 day-one provider matrix，明確哪些 provider 真正支援、哪些只保留 preset 佔位。（2026-04-07，WORK-M3-A）
- [x] `M3-AI-PR-002` 建立 provider capability model，描述 embedding、chat、structured output、streaming、tool use 等能力。（2026-04-07，WORK-M3-A）
- [x] `M3-AI-PR-003` 實作 provider 設定資料模型，包含 endpoint、model id、auth mode、timeout、enabled state。（2026-04-07，WORK-M3-A）
- [x] `M3-AI-PR-004` 實作 secret storage 和 keyring integration，涵蓋 secret save / test / update / remove。（2026-04-07，WORK-M3-A）
- [x] `M3-AI-PR-005` 實作 provider connection test，回傳延遲、錯誤原因、建議修復，而不只是一個 boolean。（2026-04-07，WORK-M3-A）
- [x] `M3-AI-PR-006` 為 provider disabled、secret missing、quota exceeded、unsupported capability 設計一致錯誤模型。（2026-04-07，WORK-M3-A）

### Job Queue And Orchestration

- [x] `M3-AI-JQ-001` 實作 SQLite-backed job queue schema，至少支援 job type、state、priority、attempt、payload、artifact link。（2026-04-07，WORK-M3-A）
- [x] `M3-AI-JQ-002` 定義 queue job lifecycle：queued、running、succeeded、failed、paused、cancelled、stale。（2026-04-07，WORK-M3-A）
- [x] `M3-AI-JQ-003` 實作 worker claim / heartbeat / retry / backoff 基礎邏輯。（2026-04-07，WORK-M3-A）
- [x] `M3-AI-JQ-004` 決定 manual replay、cancel、pause / resume 的精確語義，避免 UI 和 worker 對不上。（2026-04-07，WORK-M3-A）
- [x] `M3-AI-JQ-005` 讓 indexing jobs 和 user-triggered assistant jobs 共用同一套可觀察 queue，而不是各走一套私有背景流程。（2026-04-07，WORK-M3-A）
- [x] `M3-AI-JQ-006` 為 queue 建立 run / audit 關聯，讓每次 intelligence 操作都可追溯。（2026-04-07，WORK-M3-A）

### Index Sidecar

- [x] `M3-AI-IDX-001` 實測 LanceDB sidecar 在桌面環境中的資料目錄、重建成本、版本升級、清除與重新建立流程。（2026-04-07，WORK-M3-A）
- [x] `M3-AI-IDX-002` 定義 sidecar index 和 canonical SQLite 的界線，保證 sidecar 可被整個刪除後再重建。（2026-04-07，WORK-M3-A）
- [x] `M3-AI-IDX-003` 設計 index metadata ledger：index version、embedding model、source watermark、build started / finished、failure reason。（2026-04-07，WORK-M3-A）
- [x] `M3-AI-IDX-004` 決定 index 分片策略：按 archive、browser、date bucket、或單一 index；要求可支撐增量和全量 rebuild。（2026-04-07，WORK-M3-A）
- [x] `M3-AI-IDX-005` 為 stale embeddings、deleted source rows、rolled-back rows、changed visibility 設計失效和清理邏輯。（2026-04-07，WORK-M3-A）
- [x] `M3-AI-IDX-006` 為 index unavailable 狀態提供清楚的 read model，讓前端可誠實顯示 rebuild / disabled / failure。（2026-04-07，WORK-M3-A）

### Embedding And Rebuild Pipeline

- [x] `M3-AI-EM-001` 定義要送入 embedding 的文本 projection，限制在允許的 canonical fields 和 approved enrichment fields。（2026-04-07，WORK-M3-A）
- [x] `M3-AI-EM-002` 建立 batch sizing、token budget、rate limit、retry 和 partial success handling 策略。（2026-04-09，`WORK-QC-D`：semantic build 現在以 batch 為預設、可 retry、並在 batch 失敗時逐筆 fallback，skip / partial failure 會留下 notes。）
- [x] `M3-AI-EM-003` 實作 initial full rebuild job 和 incremental catch-up job。（2026-04-07，WORK-M3-A）
- [x] `M3-AI-EM-004` 為 rollback、import、visibility change、re-enrichment 建立 index invalidation hook。（2026-04-09，`WORK-QC-D`：v1 以 `stale` state + manual rebuild 落地，不假裝已有 background auto-requeue。）
- [x] `M3-AI-EM-005` 實作 user-triggered rebuild / clear index / pause index 的 commands 和 safety prompt。（2026-04-07，WORK-M3-A）
- [x] `M3-AI-EM-006` 產出 embedding cost 和 storage cost 可視化資料，供 Settings / Security / Insights 顯示。（2026-04-09，`WORK-QC-D`）

### MCP And External AI Surfaces

- [x] `M3-AI-MCP-001` 重新審視現有 MCP / server 實作，只保留符合新 evidence 和 permission 原則的表面。（2026-04-09，`WORK-QC-D`：surface 維持 localhost history query，並對齊 visibility、App Lock、audit trace 與 providerless lexical fallback。）
- [x] `M3-AI-MCP-002` 定義對外暴露的 history query contract，確保不會默默繞過 visibility、archive lock、provider gating。（2026-04-08，`WORK-M4-C`：MCP 只能在 AI / MCP 明確啟用且 app unlocked 時啟動；query surface 會沿用 canonical visibility，並在 App Lock 下回傳 locked refusal。）
- [x] `M3-AI-MCP-003` 為 MCP / IDE integration 加入 capability detection、consent copy、scope boundary和 audit trace。（2026-04-09，`WORK-QC-D`）
- [x] `M3-AI-MCP-004` 明確定義「無 AI provider 但要使用本地 recall / MCP read-only」是否允許，以及其安全限制。（2026-04-09，`WORK-QC-D`：允許 lexical-only read mode，但必須保留 AI / MCP explicit opt-in、localhost-only 與 App Lock gating。）

### Testing And Acceptance

- [x] `M3-AI-QA-001` 建立 provider matrix contract tests，覆蓋 secret missing、network error、bad model id、rate limit。（2026-04-07，WORK-M3-A）
- [x] `M3-AI-QA-002` 建立 queue acceptance tests，覆蓋 retry、cancel、pause / resume、worker crash recovery。（2026-04-07，WORK-M3-A）
- [x] `M3-AI-QA-003` 建立 index rebuild acceptance：fresh build、incremental build、clear and rebuild、rollback invalidation。（2026-04-07，WORK-M3-A；rollback / import auto-invalidations 仍待 `M3-AI-EM-004`）
- [x] `M3-AI-QA-004` 建立 no-provider degrade tests，確保 archive 核心和非 AI UI 不受影響。（2026-04-07，WORK-M3-A）
- [x] `M3-AI-QA-005` 建立 MCP surface tests，驗證 visibility、permission、lock state 都會正確套用。（2026-04-08，`WORK-M4-C`）

---

## Exit Artifacts

- provider registry 和 secret management
- queue-backed indexing orchestration
- replaceable vector sidecar
- rebuild / clear / retry 能力
- MCP / external AI 安全基礎
