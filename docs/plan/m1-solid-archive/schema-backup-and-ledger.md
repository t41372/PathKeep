# M1-DB — Schema, Backup, And Ledger

> 讀這份文檔的時機：當你要把 PathKeep 的 archive plane 做成真正可信的基礎設施，而不是僅能備份一次的 demo。  
> 這是整個產品最核心的一層，M2 之後所有 trust / recall / intelligence 都依賴它。

---

## Source Inputs

- [../../features/archive.md](../../features/archive.md)
- [../../features/recall.md](../../features/recall.md)
- [../../architecture/data-model.md](../../architecture/data-model.md)
- [../../database-selection-decision-2026-04-05.md](../../database-selection-decision-2026-04-05.md)
- [../program/research-and-decisions.md](../program/research-and-decisions.md)
- [../m0-foundation/backend-and-data-rearchitecture.md](../m0-foundation/backend-and-data-rearchitecture.md)

---

## 本工作包要交付什麼

- 可正式演進的 canonical SQLite schema 和 migration system
- Chromium profile 的 manual backup pipeline、watermark、dedupe、manifest、run ledger
- snapshot / restore safety net
- doctor baseline 和 archive integrity 基礎
- 對 Explorer / Audit / Dashboard 可直接供應的 read model

## 實作註記（2026-04-06 / WORK-M1-A）

- archive init / upgrade path 已統一走 `archive/schema.rs` migration executor；v2 runtime migration 補上 canonical ingest 必需欄位、`profile_watermarks` 與 compatibility views / triggers。
- Chromium manual backup 已經使用 `browser-history-parser` + staging copy + canonical ingest，並把 run ledger、manifest、snapshot、raw row versions 串成同一條 day-one 審計鏈。
- Dashboard / Audit / Explorer / Export 可直接使用 canonical read model foundation，但 restore preview、tamper-oriented doctor 深化和完整 migration upgrade matrix 仍在後續 WBS。
- 2026-04-09 closeout：M1 這裡現在多了一份明確的 truth matrix。shipping surface 是 canonical migration / backup / manifest / snapshot safety net / doctor baseline；import batch 的 un-revert 也已使用獨立 `restore` run type，而不是再冒充 `rollback`。尚未 shipping 的 snapshot restore preview / execute、legacy DB one-shot converter 與 richer approval metadata 明確留在 deferred 欄位，不再被模糊寫成「M1 已全做完」。

## Closeout Truth Matrix（2026-04-09 / `WORK-QC-C`）

| Surface                        | Current support                                                                                                                                       | Evidence                                                                                                         | Truthful boundary                                                                                 |
| ------------------------------ | ----------------------------------------------------------------------------------------------------------------------------------------------------- | ---------------------------------------------------------------------------------------------------------------- | ------------------------------------------------------------------------------------------------- |
| Migration / canonical init     | `001_initial.sql` + `002_archive_runtime_foundation.sql` + `003_history_search_fts.sql` 均由 `archive/schema.rs` 驅動                                 | migration tests、`schema_migrations` checksum、runtime migration executor                                        | legacy DB one-shot upgrade harness 仍未 shipping。                                                |
| Unified run / manifest / audit | backup / import / rollback / restore / doctor 已共用 `runs`，manifest / snapshot 透過 `AuditRunDetail` 暴露 user-facing artifact                      | `load_audit_run_detail`、Dashboard / Audit read models、M2 import revert / restore tests                         | approval reason / manual-step metadata 尚未成為 schema 欄位；`snapshot_restore` 仍未 shipping。   |
| Snapshot safety net            | 每次 profile backup 會落 `snapshots` artifact；rekey execute 前一定建立 safety snapshot                                                               | `canonical_backup_pipeline_writes_runs_manifests_snapshots_and_queries`、`rekey_archive_keeps_a_safety_snapshot` | 本地 snapshot restore preview / execute command 尚未 shipping。                                   |
| Doctor baseline                | config / browser sources / archive DB / unlock / schema version / manifest chain / snapshot / import audit / visibility / stale derived checks 已存在 | `doctor_detects_missing_snapshot_artifacts`、takeout / doctor / repair tests                                     | orphan artifact scan、checksum drift severity taxonomy、auto-fix capability matrix 尚未完整。     |
| Query models for UI            | Dashboard / Explorer / Audit / Export 已走 canonical read model                                                                                       | `DashboardSnapshot`、`HistoryQuery`、`AuditRunDetail`、frontend trust-flow coverage                              | large-archive tuning 留到 M4 runbook；這不是 M1 原始 closeout 的性能背書。                        |
| Recoverability contract        | rollback visibility、snapshot safety net、doctor repair 都已接到 `runs` story                                                                         | ADR-003 / ADR-004、M2 import revert / restore tests                                                              | 真正的 snapshot restore UX 仍 deferred；M1 只背書「先保住可回滾 artifacts」，不背書完整 restore。 |

---

## WBS

### Migration And Schema Execution

- [x] `M1-DB-SC-001` 將 M0 產出的 canonical schema v1 實際落成 migration files，建立初始 migration。
- [x] `M1-DB-SC-002` 實作 `schema_migrations` ledger、migration executor、checksum 驗證和失敗回報。
- [x] `M1-DB-SC-003` 建立新 archive 初始化流程，確保空庫建立不再依賴隱式補欄位。
- [~] `M1-DB-SC-004` 建立 legacy-to-new schema 的升級測試和一次性轉換流程草案。現況：fresh canonical init / replay / checksum drift 都已驗證；explicit legacy DB converter 仍 deferred，因目前沒有正式 legacy user base。
- [~] `M1-DB-SC-005` 為 run、manifest、snapshot、source profile、settings、watermark、visibility 欄位建立完整索引和外鍵策略。現況：核心 ingest / query hot path 索引已存在，但這還不是完整的 schema-hardening 終態。
- [x] `M1-DB-SC-006` 為 FTS、聚合表、derived intelligence state 建立暫緩或 canonical 標記，避免 M1 提前污染核心 schema。（2026-04-09，`WORK-QC-C`：`history_search` 已明確標成 derived projection；AI / insights sidecar 邊界已寫回 source docs）

### Run Ledger And Audit Model

- [x] `M1-DB-RN-001` 實作統一 run ledger，支援 `backup` 為 day-one 正式 run type，並保留 M2 `import / revert / doctor` 擴展空間。
- [x] `M1-DB-RN-002` 為每次 run 記錄 trigger source、profile scope、started / finished timestamp、timezone、result、warnings、artifact index。
- [x] `M1-DB-RN-003` 為 manifest chain 實作 parent / child 關係、checksum、row counts、source stats、content digest。
- [x] `M1-DB-RN-004` 定義 run log 和 user-facing artifact 的界線，避免把 debug log 當成正式 audit artifact。（2026-04-09，`WORK-QC-C`：`AuditRunDetail` 只暴露 manifest / snapshot / warning / stats，不把 worker debug log 冒充正式 artifact）
- [x] `M1-DB-RN-005` 為 Audit UI 產生可列舉的 run summary read model 和 run detail read model。
- [~] `M1-DB-RN-006` 為高風險操作預留 approval / manual step 欄位，即使 M1 day one 只先用於 backup / snapshot。現況：preview artifact / UI copy 已有 manual step，但 unified `runs` 尚未持久化 approval metadata。

### Chromium Backup Pipeline

- [x] `M1-DB-BK-001` 實作 Chromium profile discovery v1，包含 profile metadata、user label、path validation、lock-aware staging plan。
- [x] `M1-DB-BK-002` 實作 staging copy pipeline，確保從 live browser DB 複製時具原子性和錯誤診斷能力。
- [x] `M1-DB-BK-003` 實作 parse-to-canonical ingest pipeline，串接 M0 parser crate 和 canonical schema。
- [x] `M1-DB-BK-004` 實作 dedupe strategy，明確區分 raw event uniqueness、URL normalization、download / search-term 關聯。
- [x] `M1-DB-BK-005` 實作 watermark strategy，避免後續同 profile 備份每次全量重灌。
- [x] `M1-DB-BK-006` 為 partial failure 設計 rollback / cleanup 行為，避免 staging 殘檔和半套 run 寫入。
- [x] `M1-DB-BK-007` 把 profile-level result、warning、skipped reason、row delta 回寫到 run ledger 和 UI summary read model。

### Snapshot And Restore Safety Net

- [x] `M1-DB-SN-001` 設計 archive snapshot artifact 格式，至少包含 DB 檔、manifest、版本資訊、校驗資訊。
- [~] `M1-DB-SN-002` 定義 snapshot 觸發時機：重大 migration 前、rekey 前、import 前、manual command 前。現況：backup 與 rekey 已有明確 snapshot story；migration / import 的 preflight snapshot 尚未全面自動化。
- [x] `M1-DB-SN-003` 實作 snapshot 建立流程和 storage layout，確保檔名、大小、建立時間、相關 run id 可追蹤。
- [ ] `M1-DB-SN-004` 設計 snapshot restore preview，至少能告知將回到哪個版本、覆蓋哪些資料、需要哪些手動確認。
- [ ] `M1-DB-SN-005` 為 restore 建立最小 command contract 和測試，即使完整 UI 在 M2 / M4 才深化。

### Doctor Baseline And Integrity

- [~] `M1-DB-DR-001` 定義 doctor baseline 檢查項：migration drift、manifest chain 斷裂、orphan artifact、missing snapshot、bad checksum。現況：manifest / snapshot / import audit / visibility / stale derived state 已覆蓋；orphan artifact / checksum drift 仍未完整進 doctor。
- [~] `M1-DB-DR-002` 為每項 doctor finding 定義 severity、recommended action、can-auto-fix 和 evidence。現況：doctor report 有 `ok/detail`，repair report 有 notes；正式 severity / action taxonomy 仍未完整。
- [x] `M1-DB-DR-003` 實作最小 doctor report artifact，供 Audit 或 Settings 中查看。
- [x] `M1-DB-DR-004` 為損壞或不一致資料建立 fixture，確保 doctor 不是只在 happy path 上工作。

### Query Models For UI

- [x] `M1-DB-QR-001` 為 Dashboard 實作 archive health、recent runs、storage summary、next action 所需 read model。
- [x] `M1-DB-QR-002` 為 Explorer v1 實作基礎 query model：keyword、domain、date range、browser / profile filter、sort。
- [x] `M1-DB-QR-003` 為 Audit v1 實作 run list、run detail、artifact list、warning list 的 query model。
- [x] `M1-DB-QR-004` 為 Export v1 實作 query-to-export pipeline 所需的 stable row model。

### Test And Acceptance Matrix

- [~] `M1-DB-QA-001` 建立 end-to-end archive engine 測試：空庫初始化、首次備份、再次備份、無新增資料、部分 profile 失敗。現況：canonical ingest / query / snapshot 主路徑已有 acceptance-style tests，但矩陣仍不算完整。
- [~] `M1-DB-QA-002` 建立 migration acceptance 測試：fresh install、upgrade from old DB、upgrade with damaged metadata、migration replay prevention。現況：fresh install / replay / checksum mismatch 已有，legacy upgrade / damaged metadata 尚未完整。
- [~] `M1-DB-QA-003` 建立 manifest chain acceptance 測試：checksum 正常、篡改檢測、artifact 遺失、snapshot 關聯。現況：manifest / snapshot presence 與 artifact drift 已有 coverage，但未形成完整 tamper matrix。
- [~] `M1-DB-QA-004` 建立性能基線：中等資料量下首次備份、增量備份、Explorer 基本搜尋的可接受時間。現況：性能 artifact 與 FTS hardening 已移到 M4 runbook，M1 本身不再冒充有正式性能 signoff。
- [x] `M1-DB-QA-005` 將所有 M1 archive engine 驗收條件回寫到 docs 和測試名稱，避免 acceptance 只存在聊天記錄。（2026-04-09，`WORK-QC-C`：本頁 truth matrix + traceability acceptance surfaces 已回鏈）

---

## Exit Artifacts

- migration system 和 canonical schema v1
- 可驗收的 Chromium backup pipeline
- run ledger / manifest chain / snapshot artifact
- doctor baseline
- Dashboard / Explorer / Audit read models
