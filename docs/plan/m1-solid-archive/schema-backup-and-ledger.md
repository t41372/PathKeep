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

---

## WBS

### Migration And Schema Execution

- [ ] `M1-DB-SC-001` 將 M0 產出的 canonical schema v1 實際落成 migration files，建立初始 migration。
- [ ] `M1-DB-SC-002` 實作 `schema_migrations` ledger、migration executor、checksum 驗證和失敗回報。
- [ ] `M1-DB-SC-003` 建立新 archive 初始化流程，確保空庫建立不再依賴隱式補欄位。
- [ ] `M1-DB-SC-004` 建立 legacy-to-new schema 的升級測試和一次性轉換流程草案。
- [ ] `M1-DB-SC-005` 為 run、manifest、snapshot、source profile、settings、watermark、visibility 欄位建立完整索引和外鍵策略。
- [ ] `M1-DB-SC-006` 為 FTS、聚合表、derived intelligence state 建立暫緩或 canonical 標記，避免 M1 提前污染核心 schema。

### Run Ledger And Audit Model

- [ ] `M1-DB-RN-001` 實作統一 run ledger，支援 `backup` 為 day-one 正式 run type，並保留 M2 `import / revert / doctor` 擴展空間。
- [ ] `M1-DB-RN-002` 為每次 run 記錄 trigger source、profile scope、started / finished timestamp、timezone、result、warnings、artifact index。
- [ ] `M1-DB-RN-003` 為 manifest chain 實作 parent / child 關係、checksum、row counts、source stats、content digest。
- [ ] `M1-DB-RN-004` 定義 run log 和 user-facing artifact 的界線，避免把 debug log 當成正式 audit artifact。
- [ ] `M1-DB-RN-005` 為 Audit UI 產生可列舉的 run summary read model 和 run detail read model。
- [ ] `M1-DB-RN-006` 為高風險操作預留 approval / manual step 欄位，即使 M1 day one 只先用於 backup / snapshot。

### Chromium Backup Pipeline

- [ ] `M1-DB-BK-001` 實作 Chromium profile discovery v1，包含 profile metadata、user label、path validation、lock-aware staging plan。
- [ ] `M1-DB-BK-002` 實作 staging copy pipeline，確保從 live browser DB 複製時具原子性和錯誤診斷能力。
- [ ] `M1-DB-BK-003` 實作 parse-to-canonical ingest pipeline，串接 M0 parser crate 和 canonical schema。
- [ ] `M1-DB-BK-004` 實作 dedupe strategy，明確區分 raw event uniqueness、URL normalization、download / search-term 關聯。
- [ ] `M1-DB-BK-005` 實作 watermark strategy，避免後續同 profile 備份每次全量重灌。
- [ ] `M1-DB-BK-006` 為 partial failure 設計 rollback / cleanup 行為，避免 staging 殘檔和半套 run 寫入。
- [ ] `M1-DB-BK-007` 把 profile-level result、warning、skipped reason、row delta 回寫到 run ledger 和 UI summary read model。

### Snapshot And Restore Safety Net

- [ ] `M1-DB-SN-001` 設計 archive snapshot artifact 格式，至少包含 DB 檔、manifest、版本資訊、校驗資訊。
- [ ] `M1-DB-SN-002` 定義 snapshot 觸發時機：重大 migration 前、rekey 前、import 前、manual command 前。
- [ ] `M1-DB-SN-003` 實作 snapshot 建立流程和 storage layout，確保檔名、大小、建立時間、相關 run id 可追蹤。
- [ ] `M1-DB-SN-004` 設計 snapshot restore preview，至少能告知將回到哪個版本、覆蓋哪些資料、需要哪些手動確認。
- [ ] `M1-DB-SN-005` 為 restore 建立最小 command contract 和測試，即使完整 UI 在 M2 / M4 才深化。

### Doctor Baseline And Integrity

- [ ] `M1-DB-DR-001` 定義 doctor baseline 檢查項：migration drift、manifest chain 斷裂、orphan artifact、missing snapshot、bad checksum。
- [ ] `M1-DB-DR-002` 為每項 doctor finding 定義 severity、recommended action、can-auto-fix 和 evidence。
- [ ] `M1-DB-DR-003` 實作最小 doctor report artifact，供 Audit 或 Settings 中查看。
- [ ] `M1-DB-DR-004` 為損壞或不一致資料建立 fixture，確保 doctor 不是只在 happy path 上工作。

### Query Models For UI

- [ ] `M1-DB-QR-001` 為 Dashboard 實作 archive health、recent runs、storage summary、next action 所需 read model。
- [ ] `M1-DB-QR-002` 為 Explorer v1 實作基礎 query model：keyword、domain、date range、browser / profile filter、sort。
- [ ] `M1-DB-QR-003` 為 Audit v1 實作 run list、run detail、artifact list、warning list 的 query model。
- [ ] `M1-DB-QR-004` 為 Export v1 實作 query-to-export pipeline 所需的 stable row model。

### Test And Acceptance Matrix

- [ ] `M1-DB-QA-001` 建立 end-to-end archive engine 測試：空庫初始化、首次備份、再次備份、無新增資料、部分 profile 失敗。
- [ ] `M1-DB-QA-002` 建立 migration acceptance 測試：fresh install、upgrade from old DB、upgrade with damaged metadata、migration replay prevention。
- [ ] `M1-DB-QA-003` 建立 manifest chain acceptance 測試：checksum 正常、篡改檢測、artifact 遺失、snapshot 關聯。
- [ ] `M1-DB-QA-004` 建立性能基線：中等資料量下首次備份、增量備份、Explorer 基本搜尋的可接受時間。
- [ ] `M1-DB-QA-005` 將所有 M1 archive engine 驗收條件回寫到 docs 和測試名稱，避免 acceptance 只存在聊天記錄。

---

## Exit Artifacts

- migration system 和 canonical schema v1
- 可驗收的 Chromium backup pipeline
- run ledger / manifest chain / snapshot artifact
- doctor baseline
- Dashboard / Explorer / Audit read models
