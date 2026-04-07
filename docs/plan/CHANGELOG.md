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
