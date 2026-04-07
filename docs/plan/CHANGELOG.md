# CHANGELOG — 已完成任務紀錄

> Agent 完成任務後把它從 STATUS.md 剪切到這裡（append 到底部）。
> 這份文檔是歷史紀錄，正常工作時不需要讀它。

---

## PG — Program / Baseline

- [x] `PG-BL-001` 讀完整份 `docs/` 體系，新的 vision / features / architecture / design 文檔完成重寫
- [x] `PG-BL-002` 掃完 `src/`, `src-tauri/`, `tests/`, `scripts/`, `.github/workflows/` 主要入口
- [x] `PG-BL-003` `bun run typecheck` — 通過
- [x] `PG-BL-004` `bun run test:unit` — 通過 (8 test files / 142 tests as of 2026-04-05)
- [x] `PG-BL-005` `cargo test --workspace` — 通過
- [x] `PG-BL-006` `bun run test:e2e` — 確認失敗點是舊 shell 斷言，不是環境壞掉
- [x] 建立整份 `docs/plan/` 體系：program + m0 + m1 + m2 + m3 + m4 文檔架構
- [x] 建立 repo baseline 盤點 (`program/repo-baseline.md`)
- [x] 建立 research & decisions backlog (`program/research-and-decisions.md`)
- [x] 建立 traceability map (`program/traceability-map.md`)
- [x] 建立所有 milestone 的詳細 WBS 文檔

## M0 — Foundation

- [x] `TASK-019` 修復 repo-wide doc formatting baseline，讓 `bun run check` 重新可作為硬 gate
  - 2026-04-06：清理 repo-wide Markdown / Prettier debt，並一併修正驗收途中浮出的 JS ESLint 與 Rust Clippy 基線問題
  - 驗收：`bun run check`、`bun run build`

## PG / M0 Decisions

- [x] `TASK-001` 寫 ADR: Archive Reset Strategy
  - 2026-04-06：新增 [ADR-001](../architecture/decisions/001-archive-reset-strategy.md)，正式凍結 fresh schema 策略：canonical schema v1 獨立建立，legacy archive DB 走一次性 upgrade path
  - 同步回寫 `docs/architecture/data-model.md`、`docs/plan/program/research-and-decisions.md`、`docs/plan/m0-foundation/backend-and-data-rearchitecture.md`、`docs/plan/BACKLOG.md`、`docs/architecture/decisions/README.md`
  - 驗收：`bun run check`、`bun run build`
  - Commit：`docs(adr): add ADR-001 archive reset strategy`

## Planning System

- [x] `PLAN-2026-04-06` 工作追蹤從原子 `TASK-*` 改成 half-milestone `WORK-*` blocks
  - 刪除舊的中繼式規劃敘事，改成直接重寫與直接刪舊
  - 更新 `STATUS.md`、`BACKLOG.md`、`docs/plan/README.md`、`docs/milestones.md`、`docs/standards.md`、`AGENTS.md` 與 M0 planning docs
  - 重寫期品質規則改成：repo-wide coverage / mutation 暫不擋主線；新碼與整段重寫模組仍要求 100% coverage + mutation verification
