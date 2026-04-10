# Program — Research And Decisions

> 凡是會影響資料模型、交付順序、平台能力、操作透明性或 AI optional 原則的議題，都不應該只靠實作當下臨場判斷。  
> 這些項目要先形成研究輸出或決策結論，再進對應里程碑。

---

## 使用方式

- `[!]` 代表還沒做完，而且會卡住後面的實作。
- 每個項目都要有明確輸出：文檔、ADR、benchmark、fixture、prototype 或驗收結論。
- 如果研究結果改變了 `features/`、`design/` 或 `architecture/` 的定義，先改文檔，再改代碼。

---

## UX / 設計研究

- [x] `PG-RD-UX-001` 逐頁比對 [screens-and-nav.md](../../design/screens-and-nav.md) 和 `reference/PathKeep — Desktop UI Design/`，列出 prototype 已覆蓋和未覆蓋的畫面。結論：目前 repo 內的 prototype export 主要覆蓋 shell chrome 與 Dashboard 視覺語言；Onboarding、Import / rollback PME、Audit / Schedule / Security 細部狀態，以及 AX / reduced-motion / i18n wrapping 仍需 doc-led 補齊。見 [screens-and-nav.md](../../design/screens-and-nav.md) 的 `Prototype Coverage Snapshot`。（2026-04-07，`WORK-QC-B`）
- [x] `PG-RD-UX-002` 補 onboarding、empty state、error state、long-running operation、rollback confirmation 等 prototype 缺口。結論：這些 non-prototype states 現在已由 [screens-and-nav.md](../../design/screens-and-nav.md) 的 `Non-Prototype State Coverage`、[ux-principles.md](../../design/ux-principles.md) 的 PME / trust grammar，以及 route tests / e2e smoke 共同收斂；剩餘的全站 accessibility / release polish 留到 `M4-RL-PO-003`。（2026-04-07，`WORK-QC-B`）
- [x] `PG-RD-UX-003` 從 prototype 的 `style.css` 抽取正式 design token 表，明確暗色主題、淺色主題、字體、間距、狀態色和資料密度規範。見 [design-tokens.md](../../design/design-tokens.md)。（2026-04-06）
- [x] `PG-RD-UX-004` 定義 PME（Preview / Manual / Execute）在各類操作上的共用 interaction grammar：stepper、artifact viewer、copy command、verify result、rollback hint。見 [ux-principles.md](../../design/ux-principles.md) 與 [screens-and-nav.md](../../design/screens-and-nav.md)。（2026-04-06，WORK-M1-B）
- [x] `PG-RD-UX-005` 決定 Dashboard / Explorer / Audit Ledger / Assistant 之間的導航策略與 deep-link 規則。見 [screens-and-nav.md](../../design/screens-and-nav.md)。（2026-04-06，WORK-M1-B）

---

## 資料模型 / 架構決策

- [x] `PG-RD-ARCH-001` 決定 archive reset strategy：採 fresh schema，新 canonical schema v1 獨立建立；既有 archive DB 走一次性升級路徑。見 [ADR-001](../../architecture/decisions/001-archive-reset-strategy.md)。（2026-04-06）
- [x] `PG-RD-ARCH-002` 凍結 canonical timestamp contract：欄位命名、毫秒整數欄位、ISO 輔助欄位、run timezone、fallback timezone、前端顯示規則。見 [ADR-002](../../architecture/decisions/002-timestamp-contract.md)。（2026-04-06）
- [x] `PG-RD-ARCH-003` 凍結 run model：高風險操作共用同一 `runs` ledger。day-one M0 範圍先涵蓋 `backup` / `import` / `rollback` / `doctor` / `snapshot_restore`；後續 M2 / M3 / M4 再以 additive 方式擴充 `restore` / `ai_index` / `assistant` / `mcp_query`，而不是另開私有 ledger。見 [ADR-003](../../architecture/decisions/003-run-model.md)。（2026-04-06；2026-04-09 truth note）
- [x] `PG-RD-ARCH-004` 凍結 rollback visibility model：user-visible facts soft-hide、raw facts immutable、derived state rebuild。見 [ADR-004](../../architecture/decisions/004-rollback-visibility-model.md)。（2026-04-06）
- [x] `PG-RD-ARCH-005` 決定 FTS projection 範圍：canonical FTS 只索引 URL / title / search term 與 whitelist projection，不直接塞完整 refetch 文本。見 [data-model.md](../../architecture/data-model.md)。（2026-04-06）
- [x] `PG-RD-ARCH-006` 設計 aggregation strategy：canonical v1 不把 timeline / heatmap / daily counts 當 source of truth；materialized table 只作 derived state。見 [data-model.md](../../architecture/data-model.md)。（2026-04-06）
- [x] `PG-RD-ARCH-007` 定義 `browser-history-parser` 的 public API 和 versioning policy，明確它不依賴 archive schema 或 Tauri。見 [module-boundary-map.md](../../architecture/module-boundary-map.md)。（2026-04-06）
- [x] `PG-RD-ARCH-008` 定義 fixture strategy：Chromium / Firefox / Safari / Takeout 都要有可重跑、可公開測試的最小樣本和 edge-case 樣本。見 [imports-browsers-and-rollback.md](../m2-recall-and-trust/imports-browsers-and-rollback.md) 的 QA 基線與對應 parser / archive 測試夾具。（2026-04-07，WORK-M2-A）

---

## Intelligence 研究

- [x] `PG-RD-AI-001` 驗證 LanceDB 在 Tauri / Rust desktop 環境中的打包、資料目錄、重建成本和升級策略，確認它真的是可替換 sidecar。結論：sidecar 以 provider/model 對應的 table name 儲存在 app data 目錄，可由 `clear-only` / `full rebuild` 完整重建；operational metadata 留在 SQLite `ai_index_ledger`，供 UI / worker 誠實顯示 rebuild、failure、clear history。當前上游仍經 `tantivy 0.24.2` transitively 拉入 `lru 0.12.x`；`RUSTSEC-2026-0002` 已以 tantivy `StoreReader` 實際用法為前提納入 allowlist，待上游提供兼容升級。（2026-04-07，WORK-M3-A；見 [providers-indexing-and-jobs.md](../m3-intelligence/providers-indexing-and-jobs.md) 與 [tech-stack.md](../../architecture/tech-stack.md)）
- [x] `PG-RD-AI-002` 驗證 rig.rs 在我們需要的 provider matrix 中的能力邊界，明確哪些 request format day one 支援、哪些只保留 preset 形狀。結論：day-one chat provider 支援 OpenAI-compatible、Anthropic、Google、Ollama、LM Studio；embedding provider 支援 OpenAI-compatible、Google、Ollama、LM Studio，Anthropic 保持 chat-only。connection test 會回傳 capability report、latency、error / action / retry hints，而不是只給 boolean。（2026-04-07，WORK-M3-A；見 [providers-indexing-and-jobs.md](../m3-intelligence/providers-indexing-and-jobs.md) 與 [intelligence.md](../../features/intelligence.md)）
- [x] `PG-RD-AI-003` 決定 job queue persistence model：SQLite queue table、worker concurrency、retry / backoff、pause / resume、manual replay 的精確語義。結論：`ai_jobs` queue schema 存在 canonical archive SQLite；index 與 assistant 共用同一 queue。`pause` 只影響 queued / stale jobs，`running` job 不支援 mid-flight cancel；manual replay 只允許 failed / cancelled / paused / stale jobs；retry 由 worker 依 retryable + attempt budget + backoff 決定。（2026-04-07，WORK-M3-A；見 [providers-indexing-and-jobs.md](../m3-intelligence/providers-indexing-and-jobs.md)）
- [x] `PG-RD-AI-004` 決定 semantic index rebuild contract：何時全量重建、何時增量、何時清理 stale embeddings、誰負責觸發。結論：`build_ai_index` 支援 incremental catch-up、full rebuild、clear-only 三種模式；backup auto-index 與手動 trigger 都先 enqueue job，再由 worker 執行；`ai_index_ledger` 以 source watermark、provider、model、last run 記錄 rebuild 狀態，stale embedding cleanup 在 rebuild 流程內統一處理。（2026-04-07，WORK-M3-A；見 [providers-indexing-and-jobs.md](../m3-intelligence/providers-indexing-and-jobs.md) 與 [data-model.md](../../architecture/data-model.md)）
- [x] `PG-RD-AI-005` 決定 evidence contract：semantic search、assistant answer、insight card 至少要回傳哪些引用欄位。結論：semantic result、assistant citation、insight evidence 全都至少回傳 `historyId`、`profileId`、URL、optional title、visited timestamp；semantic result 另含 `score` / `matchReason`，assistant 另含 `state`、`jobId`、`runId`、provider snapshot，insight card 另含 generated time / data window / explanation citations。所有 intelligence evidence 都必須可 deep-link 回 Explorer 的 canonical filters，不可只停在生成式摘要。（2026-04-07，WORK-M3-B；見 [intelligence.md](../../features/intelligence.md)、[screens-and-nav.md](../../design/screens-and-nav.md) 與 [search-assistant-and-insights.md](../m3-intelligence/search-assistant-and-insights.md)）
- [x] `PG-RD-AI-006` 決定 enrichment refetch policy：M4-A v1 只正式落地內建 `readable-content-refetch` plugin，預設啟用、freshness window 7 天、掛在 `insights` rebuild flow。refetch client 採 10 秒 timeout + 最多 5 次 redirect；`fetch-error`、`decode-error`、`unsupported-content`、`empty` 都記為 non-blocking derived failure，使用者可在 Settings disable / rebuild / clear。明確的 robots / dedicated 429 backoff policy 與獨立 enrichment queue UX 延到後續 M4 work。見 [archive.md](../../features/archive.md)、[intelligence.md](../../features/intelligence.md)、[data-model.md](../../architecture/data-model.md) 與 [enrichment-advanced-intelligence-and-remote.md](../m4-full-polish/enrichment-advanced-intelligence-and-remote.md)。（2026-04-08，`WORK-M4-A`）
- [x] `PG-RD-AI-007` 審核 M3 / M4 intelligence closeout 的 truth boundary：semantic stale / cost read model、MCP consent / scope / audit trace、privacy / data-sovereignty review，以及 60-year support envelope。結論：semantic retrieval 現在以 LanceDB sidecar 為主、以 SQLite mirror 為 fallback；selected provider/model readiness 已 model-scoped；MCP read-only lexical fallback 在沒有 embedding provider 時仍可用，但必須保留 explicit opt-in、localhost-only 與 App Lock gating；repo **不**應宣稱已完成「60 年資料量、所有 AI 開啟、仍可流暢使用全部功能」的最終背書，直到 large-archive perf artifact bundle 真正存在。見 [../../features/intelligence.md](../../features/intelligence.md)、[../m3-intelligence/providers-indexing-and-jobs.md](../m3-intelligence/providers-indexing-and-jobs.md)、[../m4-full-polish/enrichment-advanced-intelligence-and-remote.md](../m4-full-polish/enrichment-advanced-intelligence-and-remote.md) 與 [../m4-full-polish/intelligence-60-year-envelope.md](../m4-full-polish/intelligence-60-year-envelope.md)。（2026-04-09，`WORK-QC-D`）
- [x] `PG-RD-AI-008` 決定 `WORK-M4-I` 的 deterministic intelligence shipping boundary：`open-loop` / `revisit` / `query ladder` 必須完全依賴 lexical + structural evidence，在沒有 embedding / LLM 時也能顯示；browser-retention honesty 放在 Onboarding / Dashboard 的 profile boundary surface，而不是只留在文檔；第一批高價值 site adapters 先收斂到 `readable-content-refetch` 內建 parse（YouTube / Vimeo 影片 metadata），不引入新的 plugin family 或 queue contract。見 [../../features/archive.md](../../features/archive.md)、[../../features/intelligence.md](../../features/intelligence.md)、[../../design/screens-and-nav.md](../../design/screens-and-nav.md)、[../m4-full-polish/enrichment-advanced-intelligence-and-remote.md](../m4-full-polish/enrichment-advanced-intelligence-and-remote.md) 與 [../m4-full-polish/README.md](../m4-full-polish/README.md)。（2026-04-09，`WORK-M4-I`）

---

## 平台與運維研究

- [x] `PG-RD-PLAT-001` 梳理 macOS / Windows / Linux 的 scheduler artifact、install path、manual instructions、remove / uninstall 路徑、rollback story。見 [archive.md](../../features/archive.md) 的跨平台 timer 約束與 [schedule-security-and-storage.md](../m1-solid-archive/schedule-security-and-storage.md) 的 M1-OPS 實作註記。（2026-04-06）
- [x] `PG-RD-PLAT-002` 研究 Safari / macOS Full Disk Access 的 detection、提示和 manual guidance UX。見 [archive.md](../../features/archive.md) 的 Safari support contract 與 [imports-browsers-and-rollback.md](../m2-recall-and-trust/imports-browsers-and-rollback.md) 的 baseline guidance 定義。（2026-04-07，WORK-M2-A）
- [x] `PG-RD-PLAT-003` 研究 Linux keyring 不可用時的 UX：哪些功能可退化、哪些功能必須阻止、哪些警告要前置。見 [archive.md](../../features/archive.md) 的 security contract、[ux-principles.md](../../design/ux-principles.md) 的 trust warning grammar，以及 [trust-ux-i18n-and-platforms.md](../m2-recall-and-trust/trust-ux-i18n-and-platforms.md) 的平台 UX closeout。（2026-04-07，WORK-M2-B）
- [x] `PG-RD-PLAT-004` 研究 remote backup bundle format：M4-A v1 bundle format 定案為 `pathkeep.remote-backup.v1` zip，至少包含 `archive/history-vault.sqlite`、`config/config.json`、`metadata/bundle-manifest.json`，manifest 記錄 `createdAt`、`appVersion`、`archiveMode`、`objectKey` 與逐檔 `sha256` / `sizeBytes`。Settings 以 Preview / Manual / Execute / Verify 落地完整 PME；Verify 會檢查 version、required entries、checksums 與本地 restore readiness，plaintext bundle 明確警告，retention / prune 保持 manual-first。見 [archive.md](../../features/archive.md)、[data-model.md](../../architecture/data-model.md)、[screens-and-nav.md](../../design/screens-and-nav.md) 與 [enrichment-advanced-intelligence-and-remote.md](../m4-full-polish/enrichment-advanced-intelligence-and-remote.md)。（2026-04-08，`WORK-M4-A`）
- [x] `PG-RD-PLAT-005` 研究多平台 installer / signing / notarization / secrets 需求，形成正式 release runbook。結論：repo 現在以 [`RELEASE.md`](../../../RELEASE.md)、[`TROUBLESHOOTING.md`](../../../TROUBLESHOOTING.md)、[`SUPPORT.md`](../../../SUPPORT.md) 與 [release-readiness-runbook.md](../m4-full-polish/release-readiness-runbook.md) 作為發版 closeout source of truth；GitHub `Release` workflow 先做 version-sync preflight，再發佈 multi-platform assets、`SHA256SUMS.txt` 與 `RELEASE-MANIFEST.json`。macOS 為 primary signed / notarized channel；Windows / Linux 形成明確的 preview support stance 與 deferred-with-rationale，而不是假裝 repo 已經替 operator 決定唯一簽章方案。（2026-04-08，`WORK-M4-B`）
- [x] `PG-RD-PLAT-006` 研究 app lock / biometric unlock / passcode fallback 的 security model。結論：PathKeep 目前 shipping 的 App Lock 是 **UI session boundary**，會阻擋 shell、desktop read/query surface 與 MCP history query，但不取代 archive encryption，也不把 shared profile scope 升級成 partition。啟用前必須先設定 passcode；startup / idle timeout / manual lock 都走同一個 `/lock` route；biometric 在 macOS / Windows / Linux 目前都只以 truthful capability / degradation state 呈現，不假裝已經接線。見 [ADR-005](../../architecture/decisions/005-app-lock-session-boundary.md)、[archive.md](../../features/archive.md)、[intelligence.md](../../features/intelligence.md)、[screens-and-nav.md](../../design/screens-and-nav.md) 與 [platform-release-and-polish.md](../m4-full-polish/platform-release-and-polish.md)。（2026-04-08，`WORK-M4-C`）

---

## 研究輸出規範

- [ ] `PG-RD-OUT-001` 每個高風險決策至少產出一份 docs 內部文檔或 ADR，而不是只留在 commit message 或聊天記錄裡。
- [ ] `PG-RD-OUT-002` 每個 benchmark 類研究至少保留輸入資料、命令、環境和結論，確保之後能重跑。
- [ ] `PG-RD-OUT-003` 每個設計研究結論都要回鏈到對應的 `features/`、`design/` 或 `architecture/` 文檔。
- [x] `PG-RD-OUT-004` 定義 repo 的 quality matrix、blocking path、scheduled / release deep checks 與 CI 對應關係。見 [quality-matrix.md](quality-matrix.md)、[../../standards.md](../../standards.md)、[../README.md](../README.md) 與 `.github/workflows/ci.yml` / `.github/workflows/mutation.yml`。（2026-04-07，`WORK-QC-A`）
