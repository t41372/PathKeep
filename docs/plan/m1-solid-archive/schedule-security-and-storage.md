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

## 實作註記（2026-04-06 / WORK-M1-A, 2026-04-06 audit follow-up）

- macOS scheduler 的 preview / manual / apply 流程已落地；Windows / Linux 也已有 preview / manual artifact，Linux timer contract 明確使用 `OnCalendar=` + `Persistent=true`。
- Schedule execute surface 現在同時覆蓋 install / update 與 explicit remove：macOS 會移除 current / legacy LaunchAgent plist、嘗試 `launchctl bootout`，並寫出 remove audit artifact；browser preview mode 則保持 manual-first read-only 說明。
- schedule status read model 已補上：macOS 會檢查 LaunchAgent 是否已安裝、內容是否 mismatch、是否殘留 legacy plist；Windows / Linux 目前明確回報 `manual-review`，不再假裝自動檢測已完成。
- keyring status / get / set / clear、archive unlock session、security status read model、rekey preview 與 snapshot-backed rekey execute 基礎已經存在於 worker / platform surface；Security trust UI 也已落地，Linux keyring 仍維持 truthful degradation / warning stance。
- storage layout、artifact naming 與 Dashboard / Audit 可消費的 storage summary / artifact metadata 已落地；retention policy 現在已明確定為 manual-first，自動清理 UX 仍未完成。
- 2026-04-09 closeout：M1-OPS 現在有明確 acceptance matrix。shipping surface 是 schedule PME + verify、security mode / unlock / rekey foundation、storage summary 與 safe reveal/open boundary；自動 retention、完整 rekey audit summary 與真機 runbook 仍保留為 partial / deferred。

## Acceptance Matrix（2026-04-09 / `WORK-QC-C`）

| Surface                   | Current support                                                                                                                                  | Evidence                                                                               | Truthful boundary                                                          |
| ------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------ | -------------------------------------------------------------------------------------- | -------------------------------------------------------------------------- |
| Schedule domain model     | `SchedulePlan` + `ScheduleStatus` 已表達 platform、interval、install state、detected files、manual steps、warnings                               | `vault-platform::schedule_status`、`src/pages/schedule/index.tsx`、trust-flow tests    | `next_run` / `last_apply_remove` 仍未成為獨立 persisted scheduler ledger。 |
| Schedule PME + verify     | Preview / Manual / Execute / Verify 全部已有 UI 與 command surface                                                                               | `preview_schedule` / `apply_schedule` / `remove_schedule` / `schedule_status`          | Windows / Linux 保持 manual-review，不假裝自動 apply / detect 已完成。     |
| Security mode taxonomy    | `uninitialized` / `plaintext` / `encrypted` / `locked` 已形成正式 read model                                                                     | `security_status`、Security page、Dashboard / Settings keyring warnings                | `rekeying` 仍是 execute-time flow，而不是 persisted long-lived mode。      |
| Rekey safety boundary     | preview 顯示 snapshot / temp DB / warnings；execute 前建立 safety snapshot                                                                       | `preview_rekey_archive`、`rekey_archive_keeps_a_safety_snapshot`                       | rekey 本身尚未寫入 unified run artifact / audit summary matrix。           |
| Storage / reveal boundary | storage summary、artifact metadata、open / reveal helper 已落地；preview file path 現在也會退回到最近存在的父資料夾，避免 PME preview 變成假按鈕 | `StorageSummary`、`open_path_in_file_manager`、Audit / Schedule / Settings / Import UI | 不提供跨磁碟搬移或自動清理；operator-guided reveal 仍是 v1 主線。          |
| Retention policy          | manual-first policy 已定義：manifests / audit summary 永久保留，raw snapshots / exports / remote bundles 不自動 prune                            | `archive.md`、Settings remote-backup manual tab、storage diagnostics                   | 尚未有 auto-prune job、retention ledger 或 UI cleanup workflow。           |

---

## WBS

### Schedule And PME

- [x] `M1-OPS-SC-001` 定義 schedule domain model：enabled / disabled、frequency、next run、last run、install status、warning status。（2026-04-09，`WORK-QC-C`：`ScheduleStatus` 已正式成為 read model；`next run` 仍以 manual verification / platform artifact 為主）
- [x] `M1-OPS-SC-002` 實作 macOS scheduler preview，顯示將建立哪些 artifact、跑什麼命令、需要哪些權限。
- [x] `M1-OPS-SC-003` 實作 macOS scheduler manual mode，提供可複製命令、檔案位置、移除方式和驗證方式。
- [x] `M1-OPS-SC-004` 實作 macOS scheduler apply mode，完成 launch agent 或等價機制的安裝、更新和移除。
- [x] `M1-OPS-SC-005` 建立 scheduler status detection，能回報 mismatch、legacy install、missing artifact、permission issue；macOS 走實際檔案比對，Windows / Linux 明確維持 manual-review。（2026-04-06 audit follow-up）
- [x] `M1-OPS-SC-006` 為 UI 建立 schedule PME 狀態說明和 trust 文案，清楚說明什麼是 preview、manual、execute。（2026-04-09，`WORK-QC-C`：Schedule page 與 trust-flow tests 已覆蓋 PME / verify quick jump）
- [x] `M1-OPS-SC-007` 為 future Windows / Linux 留下 platform adapter 抽象，但不在 M1 假裝跨平台已完成。

### Security And Encryption

- [x] `M1-OPS-SEC-001` 定義 archive security modes：目前正式 shipping contract 為 `uninitialized` / `plaintext` / `encrypted` / `locked`，並帶 warning model；`rekeying` 保留作 execute-time transition，而不是 persisted mode。（2026-04-09，`WORK-QC-C`）
- [x] `M1-OPS-SEC-002` 實作初始 archive 建立時的加密模式選擇和確認流程。（2026-04-09，`WORK-QC-C`：Onboarding 已可選 plaintext / encrypted，並接上 keyring / initialize flow）
- [x] `M1-OPS-SEC-003` 實作 keyring secret 讀寫和錯誤處理，涵蓋 keyring 不可用、權限拒絕、讀取失敗。
- [x] `M1-OPS-SEC-004` 實作 archive unlock / lock 流程和 session state，確保前端可感知當前可用能力。
- [x] `M1-OPS-SEC-005` 實作 rekey preview，清楚說明會建立 safety snapshot、temporary database 與 execute 前置條件。（2026-04-06 audit follow-up）
- [x] `M1-OPS-SEC-006` 實作 rekey execute 基礎流程，先建立 safety snapshot，再做 temp export 與 swap；如果最終替換失敗，會嘗試把原始 archive 放回原位。（2026-04-06 audit follow-up）
- [~] `M1-OPS-SEC-007` 為 Security 頁建立清楚的 trust UI：目前模式、上次 rekey、keyring status、手動恢復指引。現況：目前模式 / keyring / unlock / rekey guidance 已存在；`last rekey` 與更完整 recovery audit 仍未落地。

### Storage Layout And Artifact Management

- [x] `M1-OPS-ST-001` 凍結 archive data dir layout：DB、snapshots、artifacts、logs、exports、future sidecars 的目錄位置。
- [x] `M1-OPS-ST-002` 建立 artifact naming convention，確保 run artifact、snapshot、preview bundle、doctor report 可穩定定位。
- [x] `M1-OPS-ST-003` 實作 storage summary read model，提供 DB size、artifact size、snapshot size、estimated reclaimable size。
- [x] `M1-OPS-ST-004` 決定 artifact retention policy，至少區分永久保留和可清理項目。（2026-04-09，`WORK-QC-C`：v1 policy 明確是 manual-first；audit/manifests 保留，其他 artifacts 不自動 prune）
- [x] `M1-OPS-ST-005` 為 storage move / open folder / reveal in Finder 類操作定義安全邊界和 user guidance。（2026-04-09，`WORK-QC-C`：`open_path_in_file_manager` + UI reveal actions 已是正式 boundary）

### Audit Transparency

- [~] `M1-OPS-AU-001` 為每次 schedule / rekey / backup 操作生成可讀的 audit summary，不只保留機器日志。現況：schedule apply/remove 與 import / backup artifacts 已有人類可讀 summary；rekey 尚未完全接入同等 audit summary。
- [~] `M1-OPS-AU-002` 在 run detail 中顯示 trigger source、security mode、scheduler origin、manual intervention steps。現況：trigger / warnings / artifact 已有；security mode / scheduler origin / manual intervention 尚未完整進 run detail。
- [~] `M1-OPS-AU-003` 為高風險操作加入 explicit confirmation reason 和 rollback hint。現況：UI preview 已有 confirmation / rollback guidance，但 unified audit ledger 尚未完整持久化這些欄位。
- [x] `M1-OPS-AU-004` 實作 artifact viewer 所需的 metadata，例如 content type、size、created at、source run、copy path。

### Testing And Acceptance

- [x] `M1-OPS-QA-001` 為 macOS scheduler 建立 preview / manual / apply 三種模式的 acceptance tests。
- [~] `M1-OPS-QA-002` 為 keyring 可用 / 不可用、plaintext / encrypted、rekey success / failure 建立測試矩陣。現況：bridge / worker / frontend 已覆蓋主要分支，但還未整理成完整矩陣文檔。
- [~] `M1-OPS-QA-003` 為 storage summary、artifact retention、snapshot before rekey 建立整合測試。現況：storage summary 與 rekey snapshot 已各自有測試；retention 仍是 doc-led manual-first policy。
- [x] `M1-OPS-QA-004` 為 PME 文案和 UI 流程建立前端測試，確保 trust 信息不會在重構時被隨手刪掉。（2026-04-09，`WORK-QC-C`：`src/pages/trust-flows.test.tsx` 已作為正式 acceptance anchor）
- [~] `M1-OPS-QA-005` 在 M1 結束前，產出一輪真機驗收記錄，至少覆蓋 macOS schedule install / remove 和 encrypted archive rekey。現況：macOS apply / remove Rust acceptance 已有；人工真機 runbook 仍保留到 release / support docs。

---

## Exit Artifacts

- macOS day-one scheduler PME 流程
- security mode / unlock / rekey 基礎
- storage layout 和 artifact policy
- audit transparency data for UI
