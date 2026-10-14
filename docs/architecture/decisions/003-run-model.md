# ADR-003: Unified Run Ledger

## Status

Accepted

## Context

PathKeep 不只需要 backup run。M1 / M2 之後還會有 import、rollback、doctor、snapshot restore、rekey preview/execute 等高風險操作；它們都需要可追溯的 run id、artifact index、warning/error surface 和 rollback story。

舊 schema 只有 `backup_runs`，導致：

- 非 backup 操作沒有一致的 ledger 和 serialization contract。
- audit / manifest / snapshot metadata 只能圍繞 backup flow 建模。
- Tauri commands 和 worker bridge 容易長成「每種操作各自一套字串拼裝」。

## Decision

PathKeep 採用 **unified run ledger**：

- `backup`、`import`、`revert`、`doctor`、`snapshot_restore` 共用同一張 `runs` 表。
- `runs.run_type` 表示操作類型；`runs.trigger` 表示觸發來源（如 `manual`、`schedule`、`cli`）。
- Native scheduler invocation 在能開啟 canonical archive 前，先寫入 archive-independent 的 bounded attempt ledger；一旦 archive 可用，再用 `backup_run_id` 關聯 canonical `runs`。這不是第二套業務 run source of truth，而是 canonical plane 本身不可用時仍能證明「系統曾嘗試、為何沒有進入 run」的 delivery envelope。
- `skipped-not-due`、`deferred-lock`、pre-archive `failed` 與 crash-recovered `interrupted` 都是可稽核 outcome；不能以 `run = null` + stdout 當作已 surfaced。
- 所有 manifest、snapshot、preview artifact、warning/error 摘要都透過 `run_id` 關聯，而不是各類操作再各建一張專屬 run 表。
- Tauri / worker / frontend 的 command surface 也以這個 unified run model 為前提，區分 read model、preview、execute、artifact fetch 與 job progress，而不是以舊 UI 的頁面名稱命名命令。

2026-04-09 follow-up:

- 這個 ADR 凍結的是「共用同一張 `runs` 表」的原則，不是永久凍結唯一的 `run_type` 字串清單。
- repo 現況已在同一個 unified ledger 中**增量擴充** `rollback`、`restore`、`ai_index`、`assistant`、`mcp_query` 等 run types。這沒有推翻 ADR；相反地，這正是 ADR 所要求的演化方式。
- `restore` 明確保留給 import batch 的 un-revert / restore；`snapshot_restore` 則保留給未來真正的 snapshot restore flow，兩者不能再混成同一個 run type。

2026-10-12 follow-up（`WORK-RUN-LIFECYCLE-GUARDS`）：

- run 結束後的狀態不再被改寫：`write_runs::finish_in` 只從 `running` 轉出（AI 的 `native_ledger` 同樣）。
- 工作已經 commit 之後的收尾步驟（審計檔、manifest、讀回預覽）失敗，不是這次 run 的錯：run 維持 `success`，`warnings_json` 記一筆 `review-record-failed`（哪一步、原始錯誤），命令照常回結果（`write_runs::after_success`）。以前 run 寫成功、命令卻回錯，保留清理已刪了檔案卻被改成失敗，匯入還把讀過的檔案標成「解析失敗」。
- 失敗時記錄結束本身也失敗，不蓋掉原本的錯（`write_runs::end_failed`、`end_failed_ai_run`）：印出來，下次開存檔時被標成 `interrupted`。
- 開存檔時，所有還是 `running` 的 run 都標成 `interrupted`，不分種類：同一時間只有一個行程擁有存檔，開的時候還在跑的一定是被打斷的。
- Backup 的「最近的執行」只列存檔操作；AI 的 run（`ai_index`、`assistant`、`mcp_query`）還在同一張表，但不在這裡列。

## Rationale

- run ledger 的核心責任是「描述一次可審計操作」，不是只描述 backup。
- 共用 `runs` 表讓 artifact index、run detail、status chip、PME stepper 和 audit ledger 可以共用一套資料模型。
- 這個模型同時讓 schedule 觸發和 manual 觸發走同一條核心語義，避免後續 worker / UI 分岔成兩套流程。

## Consequences

- 舊 `backup_runs` 只作為 legacy runtime bridge，不能再被視為 canonical archive 的正式 run 模型。
- 之後新增的高風險操作若沒有 `run_id`、`run_type`、`trigger`、`status` 與 artifact 關聯，就算設計未完成。
- worker bridge 需要逐步把「返回一段 ad-hoc JSON」的接口收斂到 run-oriented envelope。
- Manual backup success 不得作為 scheduled-backup health 的替代證據；Schedule read model 必須分開 last attempt、last scheduled success 與 last manual success。

## Related

- `WORK-M0-A`
- [docs/architecture/data-model.md](../data-model.md)
- [docs/architecture/desktop-command-surface.md](../desktop-command-surface.md)
- [docs/plan/program/research-and-decisions.md](../../plan/program/research-and-decisions.md)
