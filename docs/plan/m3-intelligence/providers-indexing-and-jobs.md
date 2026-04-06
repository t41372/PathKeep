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

---

## WBS

### Provider Registry And Secret Management

- [ ] `M3-AI-PR-001` 凍結 day-one provider matrix，明確哪些 provider 真正支援、哪些只保留 preset 佔位。
- [ ] `M3-AI-PR-002` 建立 provider capability model，描述 embedding、chat、structured output、streaming、tool use 等能力。
- [ ] `M3-AI-PR-003` 實作 provider 設定資料模型，包含 endpoint、model id、auth mode、timeout、enabled state。
- [ ] `M3-AI-PR-004` 實作 secret storage 和 keyring integration，涵蓋 secret save / test / update / remove。
- [ ] `M3-AI-PR-005` 實作 provider connection test，回傳延遲、錯誤原因、建議修復，而不只是一個 boolean。
- [ ] `M3-AI-PR-006` 為 provider disabled、secret missing、quota exceeded、unsupported capability 設計一致錯誤模型。

### Job Queue And Orchestration

- [ ] `M3-AI-JQ-001` 實作 SQLite-backed job queue schema，至少支援 job type、state、priority、attempt、payload、artifact link。
- [ ] `M3-AI-JQ-002` 定義 queue job lifecycle：queued、running、succeeded、failed、paused、cancelled、stale。
- [ ] `M3-AI-JQ-003` 實作 worker claim / heartbeat / retry / backoff 基礎邏輯。
- [ ] `M3-AI-JQ-004` 決定 manual replay、cancel、pause / resume 的精確語義，避免 UI 和 worker 對不上。
- [ ] `M3-AI-JQ-005` 讓 indexing jobs 和 user-triggered assistant jobs 共用同一套可觀察 queue，而不是各走一套私有背景流程。
- [ ] `M3-AI-JQ-006` 為 queue 建立 run / audit 關聯，讓每次 intelligence 操作都可追溯。

### Index Sidecar

- [ ] `M3-AI-IDX-001` 實測 LanceDB sidecar 在桌面環境中的資料目錄、重建成本、版本升級、清除與重新建立流程。
- [ ] `M3-AI-IDX-002` 定義 sidecar index 和 canonical SQLite 的界線，保證 sidecar 可被整個刪除後再重建。
- [ ] `M3-AI-IDX-003` 設計 index metadata ledger：index version、embedding model、source watermark、build started / finished、failure reason。
- [ ] `M3-AI-IDX-004` 決定 index 分片策略：按 archive、browser、date bucket、或單一 index；要求可支撐增量和全量 rebuild。
- [ ] `M3-AI-IDX-005` 為 stale embeddings、deleted source rows、rolled-back rows、changed visibility 設計失效和清理邏輯。
- [ ] `M3-AI-IDX-006` 為 index unavailable 狀態提供清楚的 read model，讓前端可誠實顯示 rebuild / disabled / failure。

### Embedding And Rebuild Pipeline

- [ ] `M3-AI-EM-001` 定義要送入 embedding 的文本 projection，限制在允許的 canonical fields 和 approved enrichment fields。
- [ ] `M3-AI-EM-002` 建立 batch sizing、token budget、rate limit、retry 和 partial success handling 策略。
- [ ] `M3-AI-EM-003` 實作 initial full rebuild job 和 incremental catch-up job。
- [ ] `M3-AI-EM-004` 為 rollback、import、visibility change、re-enrichment 建立 index invalidation hook。
- [ ] `M3-AI-EM-005` 實作 user-triggered rebuild / clear index / pause index 的 commands 和 safety prompt。
- [ ] `M3-AI-EM-006` 產出 embedding cost 和 storage cost 可視化資料，供 Settings / Security / Insights 顯示。

### MCP And External AI Surfaces

- [ ] `M3-AI-MCP-001` 重新審視現有 MCP / server 實作，只保留符合新 evidence 和 permission 原則的表面。
- [ ] `M3-AI-MCP-002` 定義對外暴露的 history query contract，確保不會默默繞過 visibility、archive lock、provider gating。
- [ ] `M3-AI-MCP-003` 為 MCP / IDE integration 加入 capability detection、consent copy、scope boundary和 audit trace。
- [ ] `M3-AI-MCP-004` 明確定義「無 AI provider 但要使用本地 recall / MCP read-only」是否允許，以及其安全限制。

### Testing And Acceptance

- [ ] `M3-AI-QA-001` 建立 provider matrix contract tests，覆蓋 secret missing、network error、bad model id、rate limit。
- [ ] `M3-AI-QA-002` 建立 queue acceptance tests，覆蓋 retry、cancel、pause / resume、worker crash recovery。
- [ ] `M3-AI-QA-003` 建立 index rebuild acceptance：fresh build、incremental build、clear and rebuild、rollback invalidation。
- [ ] `M3-AI-QA-004` 建立 no-provider degrade tests，確保 archive 核心和非 AI UI 不受影響。
- [ ] `M3-AI-QA-005` 建立 MCP surface tests，驗證 visibility、permission、lock state 都會正確套用。

---

## Exit Artifacts

- provider registry 和 secret management
- queue-backed indexing orchestration
- replaceable vector sidecar
- rebuild / clear / retry 能力
- MCP / external AI 安全基礎
