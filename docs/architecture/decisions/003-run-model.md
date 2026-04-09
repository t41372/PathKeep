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
- 所有 manifest、snapshot、preview artifact、warning/error 摘要都透過 `run_id` 關聯，而不是各類操作再各建一張專屬 run 表。
- Tauri / worker / frontend 的 command surface 也以這個 unified run model 為前提，區分 read model、preview、execute、artifact fetch 與 job progress，而不是以舊 UI 的頁面名稱命名命令。

2026-04-09 follow-up:

- 這個 ADR 凍結的是「共用同一張 `runs` 表」的原則，不是永久凍結唯一的 `run_type` 字串清單。
- repo 現況已在同一個 unified ledger 中**增量擴充** `rollback`、`restore`、`ai_index`、`assistant`、`mcp_query` 等 run types。這沒有推翻 ADR；相反地，這正是 ADR 所要求的演化方式。
- `restore` 明確保留給 import batch 的 un-revert / restore；`snapshot_restore` 則保留給未來真正的 snapshot restore flow，兩者不能再混成同一個 run type。

## Rationale

- run ledger 的核心責任是「描述一次可審計操作」，不是只描述 backup。
- 共用 `runs` 表讓 artifact index、run detail、status chip、PME stepper 和 audit ledger 可以共用一套資料模型。
- 這個模型同時讓 schedule 觸發和 manual 觸發走同一條核心語義，避免後續 worker / UI 分岔成兩套流程。

## Consequences

- 舊 `backup_runs` 只作為 legacy runtime bridge，不能再被視為 canonical archive 的正式 run 模型。
- 之後新增的高風險操作若沒有 `run_id`、`run_type`、`trigger`、`status` 與 artifact 關聯，就算設計未完成。
- worker bridge 需要逐步把「返回一段 ad-hoc JSON」的接口收斂到 run-oriented envelope。

## Related

- `WORK-M0-A`
- [docs/architecture/data-model.md](../data-model.md)
- [docs/architecture/desktop-command-surface.md](../desktop-command-surface.md)
- [docs/plan/program/research-and-decisions.md](../../plan/program/research-and-decisions.md)
