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

- [x] **WORK-M1-A** — Archive Engine Foundation
  - 2026-04-06：把 [`archive/schema.rs`](../../src-tauri/crates/vault-core/src/archive/schema.rs) 接成 canonical migration / init 正式入口，新增 [`002_archive_runtime_foundation.sql`](../../src-tauri/crates/vault-core/src/migrations/002_archive_runtime_foundation.sql)，補上 runtime 欄位、`profile_watermarks` 與 rewrite 過渡期的 `profiles` / `visit_events` compatibility views
  - 重寫 [`archive/mod.rs`](../../src-tauri/crates/vault-core/src/archive/mod.rs) 的 archive engine，使 Chromium manual backup 經由 `browser-history-parser` + staging copy 寫入 canonical `runs` / `source_profiles` / `urls` / `visits` / `downloads` / `search_terms` / `favicons` / `raw_row_versions`，並串起 manifest chain、snapshot artifact、doctor baseline、storage summary、Dashboard / Audit read models
  - 更新 [`models.rs`](../../src-tauri/crates/vault-core/src/models.rs)、[`lib.rs`](../../src-tauri/crates/vault-core/src/lib.rs)、[`takeout.rs`](../../src-tauri/crates/vault-core/src/takeout.rs)、[`ai.rs`](../../src-tauri/crates/vault-core/src/ai.rs)、[`insights.rs`](../../src-tauri/crates/vault-core/src/insights.rs) 與 worker / desktop bridge，讓既有 AI / insights / takeout surface 在 canonical runtime 上繼續工作
  - 修正 [`vault-platform/src/lib.rs`](../../src-tauri/crates/vault-platform/src/lib.rs) 的 Linux scheduler timer contract，明確使用 `OnCalendar=` + `Persistent=true`，並保留 macOS preview / manual / apply 與 Windows / Linux manual guidance
  - 同步回寫 `data-model.md`、`research-and-decisions.md`、`repo-baseline.md`、M1 README / WBS、`STATUS.md`、`BACKLOG.md`
  - 驗收：`bun run check`、`bun run build`

- [x] **WORK-M1-B** — Archive UX And Operations
  - 2026-04-06：新增 [`src/app/shell-data.tsx`](../../src/app/shell-data.tsx) / [`src/app/shell-data-context.ts`](../../src/app/shell-data-context.ts) 真實 shell data provider，並把 [`src-tauri/src/lib.rs`](../../src-tauri/src/lib.rs)、[`src-tauri/src/worker_bridge.rs`](../../src-tauri/src/worker_bridge.rs)、[`src-tauri/crates/vault-worker/src/lib.rs`](../../src-tauri/crates/vault-worker/src/lib.rs) 接上 dashboard snapshot / audit run detail command surface
  - 重寫 [`src/pages/onboarding/index.tsx`](../../src/pages/onboarding/index.tsx)、[`src/pages/dashboard/index.tsx`](../../src/pages/dashboard/index.tsx)、[`src/pages/explorer/index.tsx`](../../src/pages/explorer/index.tsx)、[`src/pages/audit/index.tsx`](../../src/pages/audit/index.tsx)、[`src/pages/security/index.tsx`](../../src/pages/security/index.tsx)、[`src/pages/schedule/index.tsx`](../../src/pages/schedule/index.tsx) 與 sidebar / topbar，讓 Onboarding、Dashboard、Explorer、Audit、Export、Security、Schedule 全部讀取 canonical archive read model，並補齊 empty / locked / loading / zero-state、open / copy path 與 PME trust copy
  - 擴寫 [`src/lib/backend.ts`](../../src/lib/backend.ts) browser preview mock，使 onboarding、manual backup、Explorer query、Export、Audit detail、keyring / AI preview 都有 stateful fixture；同步更新 `types.d.ts`、format helpers、shell/unit tests 與 [`tests/e2e/shell.spec.ts`](../../tests/e2e/shell.spec.ts)
  - 收斂 review findings：Chromium backup 現在嚴格尊重 `selected_profile_ids` 邊界，Google Takeout rollback 改成 soft-hide imported rows 並寫入 rollback run，而不是硬刪資料
  - 同步回寫 `archive.md`、`screens-and-nav.md`、`ux-principles.md`、`research-and-decisions.md`、M1 README / WBS、`STATUS.md`、`BACKLOG.md`
  - 驗收：`bun run check`、`bun run build`、`bun run test:e2e`

## Post-Audit Corrections

- [x] `AUDIT-2026-04-06` 修正 M1 closeout 的非前端真實性問題
  - 補上 shell foundation slice 的明確 gate：新增 `bun run check:shell-slice`，並把 Vitest / Stryker 的 targeted scope 與 `src-tauri/target` 等大目錄排除規則對齊，避免 shell mutation 因 sandbox copy 掃到 Rust build artifacts 而直接崩潰
  - 回寫 `AGENTS.md`、`docs/standards.md`，明確規定 shell foundation slice 的保護範圍與收工命令，避免 agent 只跑 `bun run check` 就誤判 targeted coverage / mutation 已經通過
  - 回寫 `docs/plan/README.md` 與 `docs/plan/m1-solid-archive/README.md`，把 M1 從「已完全完成」修正為「feature baseline 已落地，但 non-frontend ops / QA closeout 仍開著」

- [x] `AUDIT-2026-04-06-B` 把假裝驗完前端的 shell gate 收斂成可兌現的 desktop contract gate
  - 將 `vitest.shell.config.ts` / `stryker.shell.config.json` 改成只保護 `src/main.tsx` 與 `src/lib/ipc/bridge.ts` 的 desktop contract slice，並重新命名腳本為 `test:unit:desktop-contract`、`coverage:js:desktop-contract`、`mutation:js:desktop-contract`、`check:desktop-contract`
  - 將 `bun run check` 直接納入 `bun run check:desktop-contract`，讓非前端 contract gate 成為真正的 blocking path，而不是靠 agent 額外記憶
  - 回寫 `AGENTS.md`、`docs/standards.md`、M0 planning docs 與 `docs/plan/README.md`，明確標記前端 shell / route / sidebar 驗收仍需由前端 owner 補獨立驗收，不再誤報成 shell slice 已完成

- [x] `AUDIT-2026-04-06-C` 補上 M1-OPS 缺的非前端 command surface 與 recoverability 基線
  - 新增 `schedule_status`、`security_status`、`preview_rekey_archive` worker / Tauri command，讓 schedule / security 至少有 read model 與 preview surface，而不是只剩 UI placeholder
  - macOS schedule status 會檢查已安裝 LaunchAgent 是否 mismatch、是否殘留 legacy plist；Windows / Linux 明確回報 `manual-review`
  - rekey execute 先建立 safety snapshot，再做 temp export / swap；若最終替換失敗，會嘗試把原始 archive 放回原位
  - 新增對應的 Rust tests，並回寫 `desktop-command-surface.md`、M1-OPS 文檔與 M1 README，讓後續 agent 知道哪些後端能力已落地、哪些仍然是 UI / acceptance gap

## M2 — Recall & Trust (Work Blocks)

- [x] **WORK-M2-A** — Imports, Rollback, And Multi-Browser
  - 2026-04-07：把 Google Takeout 流程補齊到 dry-run / preview / quarantine / import / revert / restore，並讓 import batch audit artifact 可預覽、可重建、可透過 worker / Tauri / frontend surface 操作
  - `browser-history-parser` 現在提供 Firefox 與 Safari history baseline parser；`vault-core` backup pipeline 也改為 ingest Chromium、Firefox 與具備權限的 Safari profile，Safari 在缺少 Full Disk Access 時會保留 profile 並顯示 needs-access guidance
  - visibility-aware filtering 擴到 query / dashboard / insights 相關 read models；doctor / repair run 新增 missing import artifact、broken visibility reference、stale AI / insight derived state 檢查與修復，並補齊對應 fixture 與 acceptance-style Rust tests
  - 同步回寫 `archive.md`、`recall.md`、`module-boundary-map.md`、`desktop-command-surface.md`、`research-and-decisions.md`、`repo-baseline.md`、M2 README / WBS、`STATUS.md`、`BACKLOG.md`
  - 驗收：`bun run check`、`bun run build`

- [x] **WORK-M2-B** — Trust UX, I18n, And Platforms
  - 2026-04-07：把 `src/lib/i18n.ts` 巨檔拆成 [`src/lib/i18n/`](../../src/lib/i18n/) catalog / provider / hooks 結構，正式落地 `en` / `zh-CN` / `zh-TW` namespace-based i18n、route metadata 翻譯、locale-aware relative time / bytes formatting、缺 key coverage test 與 pseudo-locale smoke
  - 重寫 [`src/pages/import/index.tsx`](../../src/pages/import/index.tsx)、[`src/pages/security/index.tsx`](../../src/pages/security/index.tsx)、[`src/pages/schedule/index.tsx`](../../src/pages/schedule/index.tsx)、[`src/pages/settings/index.tsx`](../../src/pages/settings/index.tsx)、[`src/pages/dashboard/index.tsx`](../../src/pages/dashboard/index.tsx)、[`src/pages/audit/index.tsx`](../../src/pages/audit/index.tsx)，補齊 PME workflow、rollback / restore / doctor repair / rekey preview、trust callout、platform capability guidance 與跨頁修復入口
  - 後續 closeout 修補把 Audit run filter / summary delta、unified run ledger run type / source scope contract、trust-critical status / mode 翻譯、跨頁 refresh correctness、keyboard-only walkthrough、reduced-motion fallback 與 locale-length wrapping 一起補齊，避免 M2 提前關帳
  - 新增 [`src/lib/platform-guidance.ts`](../../src/lib/platform-guidance.ts) 與 [`src/components/primitives/status-callout.tsx`](../../src/components/primitives/status-callout.tsx)，把 Windows / Linux scheduler story、Safari Full Disk Access、keyring unavailable、scheduler mismatch / legacy install 收斂成正式 warning grammar
  - 擴寫 [`src/lib/backend.ts`](../../src/lib/backend.ts) browser preview mock，讓 import batch preview / revert / restore、doctor / repair、schedule / security support state 與 Windows / Linux schedule variants 都能支撐 trust-critical UI；同步補齊相關 unit tests 與 trust flow acceptance tests，並做一輪 keyboard / locale smoke review（含 `zh-CN` / `zh-TW`）
  - 同步回寫 `ux-principles.md`、`screens-and-nav.md`、`archive.md`、`recall.md`、`standards.md`、`research-and-decisions.md`、M2 README / WBS、`plan/README.md`、`STATUS.md`、`BACKLOG.md`
  - 驗收：`bun run check`、`bun run build`
