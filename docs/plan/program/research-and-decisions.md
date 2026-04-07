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

- [!] `PG-RD-UX-001` 逐頁比對 [screens-and-nav.md](../../design/screens-and-nav.md) 和 `reference/PathKeep — Desktop UI Design/`，列出 prototype 已覆蓋和未覆蓋的畫面。
- [!] `PG-RD-UX-002` 補 onboarding、empty state、error state、long-running operation、rollback confirmation 等 prototype 缺口；缺口畫面先補設計決策，再開做。
- [x] `PG-RD-UX-003` 從 prototype 的 `style.css` 抽取正式 design token 表，明確暗色主題、淺色主題、字體、間距、狀態色和資料密度規範。見 [design-tokens.md](../../design/design-tokens.md)。（2026-04-06）
- [x] `PG-RD-UX-004` 定義 PME（Preview / Manual / Execute）在各類操作上的共用 interaction grammar：stepper、artifact viewer、copy command、verify result、rollback hint。見 [ux-principles.md](../../design/ux-principles.md) 與 [screens-and-nav.md](../../design/screens-and-nav.md)。（2026-04-06，WORK-M1-B）
- [x] `PG-RD-UX-005` 決定 Dashboard / Explorer / Audit Ledger / Assistant 之間的導航策略與 deep-link 規則。見 [screens-and-nav.md](../../design/screens-and-nav.md)。（2026-04-06，WORK-M1-B）

---

## 資料模型 / 架構決策

- [x] `PG-RD-ARCH-001` 決定 archive reset strategy：採 fresh schema，新 canonical schema v1 獨立建立；既有 archive DB 走一次性升級路徑。見 [ADR-001](../../architecture/decisions/001-archive-reset-strategy.md)。（2026-04-06）
- [x] `PG-RD-ARCH-002` 凍結 canonical timestamp contract：欄位命名、毫秒整數欄位、ISO 輔助欄位、run timezone、fallback timezone、前端顯示規則。見 [ADR-002](../../architecture/decisions/002-timestamp-contract.md)。（2026-04-06）
- [x] `PG-RD-ARCH-003` 凍結 run model：`backup` / `import` / `revert` / `doctor` / `snapshot restore` 共用同一 `runs` ledger。見 [ADR-003](../../architecture/decisions/003-run-model.md)。（2026-04-06）
- [x] `PG-RD-ARCH-004` 凍結 rollback visibility model：user-visible facts soft-hide、raw facts immutable、derived state rebuild。見 [ADR-004](../../architecture/decisions/004-rollback-visibility-model.md)。（2026-04-06）
- [x] `PG-RD-ARCH-005` 決定 FTS projection 範圍：canonical FTS 只索引 URL / title / search term 與 whitelist projection，不直接塞完整 refetch 文本。見 [data-model.md](../../architecture/data-model.md)。（2026-04-06）
- [x] `PG-RD-ARCH-006` 設計 aggregation strategy：canonical v1 不把 timeline / heatmap / daily counts 當 source of truth；materialized table 只作 derived state。見 [data-model.md](../../architecture/data-model.md)。（2026-04-06）
- [x] `PG-RD-ARCH-007` 定義 `browser-history-parser` 的 public API 和 versioning policy，明確它不依賴 archive schema 或 Tauri。見 [module-boundary-map.md](../../architecture/module-boundary-map.md)。（2026-04-06）
- [ ] `PG-RD-ARCH-008` 定義 fixture strategy：Chromium / Firefox / Safari / Takeout 都要有可重跑、可公開測試的最小樣本和 edge-case 樣本。

---

## Intelligence 研究

- [!] `PG-RD-AI-001` 驗證 LanceDB 在 Tauri / Rust desktop 環境中的打包、資料目錄、重建成本和升級策略，確認它真的是可替換 sidecar。
- [!] `PG-RD-AI-002` 驗證 rig.rs 在我們需要的 provider matrix 中的能力邊界，明確哪些 request format day one 支援、哪些只保留 preset 形狀。
- [!] `PG-RD-AI-003` 決定 job queue persistence model：SQLite queue table、worker concurrency、retry / backoff、pause / resume、manual replay 的精確語義。
- [!] `PG-RD-AI-004` 決定 semantic index rebuild contract：何時全量重建、何時增量、何時清理 stale embeddings、誰負責觸發。
- [ ] `PG-RD-AI-005` 決定 evidence contract：semantic search、assistant answer、insight card 至少要回傳哪些引用欄位。
- [ ] `PG-RD-AI-006` 決定 enrichment refetch policy：預設是否關閉、頻率、freshness window、rate limiting、robots / 429 handling。

---

## 平台與運維研究

- [x] `PG-RD-PLAT-001` 梳理 macOS / Windows / Linux 的 scheduler artifact、install path、manual instructions、remove / uninstall 路徑、rollback story。見 [archive.md](../../features/archive.md) 的跨平台 timer 約束與 [schedule-security-and-storage.md](../m1-solid-archive/schedule-security-and-storage.md) 的 M1-OPS 實作註記。（2026-04-06）
- [!] `PG-RD-PLAT-002` 研究 Safari / macOS Full Disk Access 的 detection、提示和 manual guidance UX。
- [!] `PG-RD-PLAT-003` 研究 Linux keyring 不可用時的 UX：哪些功能可退化、哪些功能必須阻止、哪些警告要前置。
- [ ] `PG-RD-PLAT-004` 研究 remote backup bundle format：bundle manifest、加密模式、restore story、checksum strategy。
- [ ] `PG-RD-PLAT-005` 研究多平台 installer / signing / notarization / secrets 需求，形成正式 release runbook。

---

## 研究輸出規範

- [ ] `PG-RD-OUT-001` 每個高風險決策至少產出一份 docs 內部文檔或 ADR，而不是只留在 commit message 或聊天記錄裡。
- [ ] `PG-RD-OUT-002` 每個 benchmark 類研究至少保留輸入資料、命令、環境和結論，確保之後能重跑。
- [ ] `PG-RD-OUT-003` 每個設計研究結論都要回鏈到對應的 `features/`、`design/` 或 `architecture/` 文檔。
