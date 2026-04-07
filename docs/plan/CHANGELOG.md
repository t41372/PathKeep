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

## M0 — Foundation (Work Blocks)

- [x] **WORK-M0-A** — Data Plane Reset
  - 2026-04-06：新增 [ADR-002](../architecture/decisions/002-timestamp-contract.md)、[ADR-003](../architecture/decisions/003-run-model.md)、[ADR-004](../architecture/decisions/004-rollback-visibility-model.md)，並同步凍結 `PG-RD-ARCH-002` ~ `PG-RD-ARCH-007`
  - 建立 [`browser-history-parser`](../../src-tauri/crates/browser-history-parser/) workspace crate，定義 provided-path inspection / incremental cursor / parsed row boundary，並加入 Chromium parser 測試
  - 建立 [`001_initial.sql`](../../src-tauri/crates/vault-core/src/migrations/001_initial.sql) 與 `run_migrations` / `current_version` migration foundation；把 `archive.rs` 的 schema bootstrapping 抽到 [`archive/schema.rs`](../../src-tauri/crates/vault-core/src/archive/schema.rs)
  - 新增 [module-boundary-map.md](../architecture/module-boundary-map.md) 與 [desktop-command-surface.md](../architecture/desktop-command-surface.md)，凍結 crate 邊界、derived-state boundary 和 Tauri command surface draft
  - 同步回寫 `data-model.md`、`archive.md`、`repo-baseline.md`、M0 backend docs、M0 checklist、research backlog 與 BACKLOG blocked marker
  - 驗收：`cargo test --manifest-path src-tauri/Cargo.toml -p browser-history-parser`、`cargo test --manifest-path src-tauri/Cargo.toml -p vault-core migration`、`bun run check`、`bun run build`
  - 驗證註記：targeted 100% coverage / mutation verification 依重寫期 policy 明確記錄在 `backend-and-data-rearchitecture.md`，待 M1 canonical runtime 接線後做 focused sweep

- [x] **WORK-M0-B** — Product Shell Reset
  - 2026-04-06：建立 [`src/app/`](../../src/app/) shell / router / onboarding shell / preview data、[`src/components/sidebar/`](../../src/components/sidebar/) 與 [`src/components/topbar/`](../../src/components/topbar/) 共用 chrome、[`src/components/primitives/`](../../src/components/primitives/) 狀態元件，以及 [`src/pages/*/index.tsx`](../../src/pages/) route-scoped skeleton 頁面
  - 新增 [design-tokens.md](../design/design-tokens.md)、[`src/styles/tokens.css`](../../src/styles/tokens.css)、[`src/styles/app.css`](../../src/styles/app.css)、[`src/lib/tokens.ts`](../../src/lib/tokens.ts)，把 prototype token / density / theme contract 正式落地
  - 刪除 `AppNew` / `App.css` / 舊 flat page files 與舊 shell assertions；新增 `vitest.shell.config.ts`、`stryker.shell.config.json`，完成 shell slice 的 100% coverage + mutation verification
  - 重寫 [`tests/e2e/shell.spec.ts`](../../tests/e2e/shell.spec.ts) 以驗證新 shell、onboarding 與 dashboard preview，並完成 PathKeep rename sweep（package / Tauri / README / release workflow / public strings）
  - 同步回寫 `research-and-decisions.md`、`repo-baseline.md`、`docs/plan/README.md`、M0 README / WBS、`standards.md`、`AGENTS.md`、`screens-and-nav.md`、`module-boundary-map.md`
  - 驗收：`bun run test:unit:shell`、`bun run coverage:js:shell`、`bun run mutation:js:shell`、`bun run test:e2e`、`bun run check`、`bun run build`
