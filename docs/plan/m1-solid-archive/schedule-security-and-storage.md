# M1-OPS — Schedule, Security, And Storage

> 讀這份文檔的時機：當你要讓 Archive 真正能長期自動運作，並在安全、透明、可驗證的前提下交給使用者。  
> 這份文檔涵蓋的是「運行中的產品」而不是單次手動 demo。

---

## Source Inputs

- [../../features/archive.md](../../features/archive.md)
- [../../design/ux-principles.md](../../design/ux-principles.md)
- [../../architecture/data-model.md](../../architecture/data-model.md)
- [../program/research-and-decisions.md](../program/research-and-decisions.md)
- [schema-backup-and-ledger.md](schema-backup-and-ledger.md)

---

## 本工作包要交付什麼

- macOS day-one 可用的 schedule PME 流程
- plaintext / encrypted archive、keyring integration、rekey 基礎流程
- storage layout、artifact retention、space visibility
- 讓高風險操作具備 preview、manual instructions、audit trace

---

## WBS

### Schedule And PME

- [ ] `M1-OPS-SC-001` 定義 schedule domain model：enabled / disabled、frequency、next run、last run、install status、warning status。
- [ ] `M1-OPS-SC-002` 實作 macOS scheduler preview，顯示將建立哪些 artifact、跑什麼命令、需要哪些權限。
- [ ] `M1-OPS-SC-003` 實作 macOS scheduler manual mode，提供可複製命令、檔案位置、移除方式和驗證方式。
- [ ] `M1-OPS-SC-004` 實作 macOS scheduler apply mode，完成 launch agent 或等價機制的安裝、更新和移除。
- [ ] `M1-OPS-SC-005` 建立 scheduler status detection，能回報 mismatch、stale install、missing artifact、permission issue。
- [ ] `M1-OPS-SC-006` 為 UI 建立 schedule PME 狀態說明和 trust 文案，清楚說明什麼是 preview、manual、execute。
- [ ] `M1-OPS-SC-007` 為 future Windows / Linux 留下 platform adapter 抽象，但不在 M1 假裝跨平台已完成。

### Security And Encryption

- [ ] `M1-OPS-SEC-001` 定義 archive security modes：plaintext、encrypted、locked、rekeying、error。
- [ ] `M1-OPS-SEC-002` 實作初始 archive 建立時的加密模式選擇和確認流程。
- [ ] `M1-OPS-SEC-003` 實作 keyring secret 讀寫和錯誤處理，涵蓋 keyring 不可用、權限拒絕、讀取失敗。
- [ ] `M1-OPS-SEC-004` 實作 archive unlock / lock 流程和 session state，確保前端可感知當前可用能力。
- [ ] `M1-OPS-SEC-005` 實作 rekey preview，清楚說明會建立哪些 snapshot、要多久、成功後哪些舊資料會失效。
- [ ] `M1-OPS-SEC-006` 實作 rekey execute 基礎流程，確保失敗可回退或至少保留可恢復 snapshot。
- [ ] `M1-OPS-SEC-007` 為 Security 頁建立清楚的 trust UI：目前模式、上次 rekey、keyring status、手動恢復指引。

### Storage Layout And Artifact Management

- [ ] `M1-OPS-ST-001` 凍結 archive data dir layout：DB、snapshots、artifacts、logs、exports、future sidecars 的目錄位置。
- [ ] `M1-OPS-ST-002` 建立 artifact naming convention，確保 run artifact、snapshot、preview bundle、doctor report 可穩定定位。
- [ ] `M1-OPS-ST-003` 實作 storage summary read model，提供 DB size、artifact size、snapshot size、estimated reclaimable size。
- [ ] `M1-OPS-ST-004` 決定 artifact retention policy，至少區分永久保留和可清理項目。
- [ ] `M1-OPS-ST-005` 為 storage move / open folder / reveal in Finder 類操作定義安全邊界和 user guidance。

### Audit Transparency

- [ ] `M1-OPS-AU-001` 為每次 schedule / rekey / backup 操作生成可讀的 audit summary，不只保留機器日志。
- [ ] `M1-OPS-AU-002` 在 run detail 中顯示 trigger source、security mode、scheduler origin、manual intervention steps。
- [ ] `M1-OPS-AU-003` 為高風險操作加入 explicit confirmation reason 和 rollback hint。
- [ ] `M1-OPS-AU-004` 實作 artifact viewer 所需的 metadata，例如 content type、size、created at、source run、copy path。

### Testing And Acceptance

- [ ] `M1-OPS-QA-001` 為 macOS scheduler 建立 preview / manual / apply 三種模式的 acceptance tests。
- [ ] `M1-OPS-QA-002` 為 keyring 可用 / 不可用、plaintext / encrypted、rekey success / failure 建立測試矩陣。
- [ ] `M1-OPS-QA-003` 為 storage summary、artifact retention、snapshot before rekey 建立整合測試。
- [ ] `M1-OPS-QA-004` 為 PME 文案和 UI 流程建立前端測試，確保 trust 信息不會在重構時被隨手刪掉。
- [ ] `M1-OPS-QA-005` 在 M1 結束前，產出一輪真機驗收記錄，至少覆蓋 macOS schedule install / remove 和 encrypted archive rekey。

---

## Exit Artifacts

- macOS day-one scheduler PME 流程
- security mode / unlock / rekey 基礎
- storage layout 和 artifact policy
- audit transparency data for UI
