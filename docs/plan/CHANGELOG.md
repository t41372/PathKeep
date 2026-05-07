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

## M3 — Intelligence (Work Blocks)

- [x] **WORK-M3-A** — Providers, Queue, And Indexing
  - 2026-04-07：把 [`src-tauri/crates/vault-core/src/ai.rs`](../../src-tauri/crates/vault-core/src/ai.rs) 接成 provider capability / connection-test surface、`ai_index_ledger`、run-linked build / clear / rebuild foundation，並新增 [`src-tauri/crates/vault-core/src/ai_queue.rs`](../../src-tauri/crates/vault-core/src/ai_queue.rs) 與 [`src-tauri/crates/vault-core/src/ai_sidecar.rs`](../../src-tauri/crates/vault-core/src/ai_sidecar.rs)，正式落地 SQLite queue + LanceDB sidecar orchestration
  - 擴寫 [`src-tauri/crates/vault-worker/src/lib.rs`](../../src-tauri/crates/vault-worker/src/lib.rs)、[`src-tauri/src/lib.rs`](../../src-tauri/src/lib.rs) 與 [`src-tauri/src/worker_bridge.rs`](../../src-tauri/src/worker_bridge.rs)，新增 provider connection test、queue status / drain / replay / cancel、queue-backed build index / assistant worker commands 與 CLI `ai-queue`
  - 同步更新 [`src/lib/types.d.ts`](../../src/lib/types.d.ts)、[`src/lib/backend.ts`](../../src/lib/backend.ts) 與相關 unit tests，讓前端 contract 能表達 provider capability、queue status、recent jobs、job-backed index / assistant run ids，並保留無 AI 配置時的安全降級
  - 回寫 `intelligence.md`、`tech-stack.md`、`data-model.md`、`research-and-decisions.md`、M3 README / WBS、`STATUS.md`、`BACKLOG.md`；同時把 LanceDB 依賴鏈帶來的 RustSec `RUSTSEC-2026-0002` 與 `0BSD` / `BSL-1.0` 授權需求同步記錄到 supply-chain gate 設定
  - 驗收：`bun run check`、`bun run build`

- [x] **WORK-M3-B** — Search, Assistant, And Insights
  - 2026-04-07：重寫 [`src/pages/explorer/index.tsx`](../../src/pages/explorer/index.tsx)、[`src/pages/assistant/index.tsx`](../../src/pages/assistant/index.tsx)、[`src/pages/insights/index.tsx`](../../src/pages/insights/index.tsx) 與 [`src/pages/dashboard/index.tsx`](../../src/pages/dashboard/index.tsx) 的 intelligence surface，正式落地 `keyword` / `semantic` / `hybrid` recall、assistant citation thread、On This Day / Site Analytics / Periodic Summary / Topic Timeline，以及跨頁 deep-link / explainability / queue controls
  - 新增 [`src/lib/intelligence.ts`](../../src/lib/intelligence.ts) 與 [`src/lib/intelligence.test.ts`](../../src/lib/intelligence.test.ts)，把 AI status、score band、evidence href、assistant response state 等前端 contract 收斂成可測的 helper；同步更新 [`src/lib/backend.ts`](../../src/lib/backend.ts)、[`src/lib/types.d.ts`](../../src/lib/types.d.ts)、[`src/styles/app.css`](../../src/styles/app.css) 與相關 UI tests
  - 收斂上一輪 review findings：`ai_index_status` 改為只看 index jobs 與 provider readiness、assistant 在沒有可保留 citation 時回傳 `insufficient-evidence`、assistant / index jobs 凍結 enqueue-time provider snapshot、`ai_assistant_runs` 連回 `runs.id`、queue reconcile / sidecar sync 不再靜默吞錯，long-running AI jobs 會寫 heartbeat 以支撐 stale reclaim
  - 回寫 `intelligence.md`、`screens-and-nav.md`、`data-model.md`、`research-and-decisions.md`、M3 README / WBS、`STATUS.md`、`BACKLOG.md`
  - 驗收：`bun run check`、`bun run build`

## PG — Quality Closeout Before M4

- [x] **WORK-QC-A** — Restore Global Quality Gates Before M4
  - 2026-04-07：新增 [quality-matrix.md](program/quality-matrix.md)，正式寫下 pre-M4 的 blocking path、deep checks、coverage / mutation surface 與 desktop-vs-preview 驗收邊界
  - 新增 [`vitest.quality.config.ts`](../../vitest.quality.config.ts)，把 `coverage:js` 收斂到 living M0-M3 JS quality surface；同步更新 [`stryker.config.json`](../../stryker.config.json) 與 [`package.json`](../../package.json)，讓 `mutation:js` / `coverage:js` 不再只保護少數 helper 或 desktop contract slice
  - 更新 [`scripts/verify-rust-coverage.mjs`](../../scripts/verify-rust-coverage.mjs) 與 [CI workflow](../../.github/workflows/ci.yml)，把 `coverage:rust` 定義成誠實的 Tauri desktop command / bridge quality gate，並讓 mainline CI 直接執行 `coverage:js`、`coverage:rust`、`check:desktop-contract`、`build`、`test:e2e`
  - 補齊 JS / Rust quality surface 的測試缺口，包含 `src/app/shell-data.tsx`、`src/lib/backend.ts`、`src/lib/format.ts`、`src/lib/intelligence.ts`、`src/lib/platform-guidance.ts`、`src/lib/trust-review.ts`、`src/lib/i18n/*` 與對應的 worker / parser / vault-worker Rust tests
  - 同步回寫 `docs/standards.md`、`docs/plan/README.md`、`docs/plan/program/research-and-decisions.md`、`docs/plan/program/repo-baseline.md`、M0 / M1 / M3 / M4 README 與 `AGENTS.md`，移除「checker 已恢復但其實沒開」的失真敘事
  - 驗收：`bun run coverage:js`、`bun run coverage:rust`、`bun run mutation:js`、`bun run test:e2e`、`bun run check`、`bun run build`

- [x] **WORK-QC-B** — Close Remaining M0-M3 Product And Doc Debt
  - 2026-04-08：修正 [`src/pages/onboarding/index.tsx`](../../src/pages/onboarding/index.tsx) 的授權 trust copy，將 welcome 文案從錯誤的 MIT 改回 GPL v3，並補上對應的 onboarding acceptance 斷言
  - 更新 [`src/pages/dashboard/index.tsx`](../../src/pages/dashboard/index.tsx)、[`src/pages/insights/index.tsx`](../../src/pages/insights/index.tsx)、[`src/lib/format.ts`](../../src/lib/format.ts) 與相關 tests，讓 Dashboard / Insights 正式顯示 On This Day、Periodic Summary、evidence deep-link，避免 raw `common.disabled` i18n key 洩漏，並把「今天」的判斷改成使用者本地 timezone 的日曆日
  - 擴寫 [`tests/e2e/shell.spec.ts`](../../tests/e2e/shell.spec.ts)，把 onboarding GPL 文案、dashboard intelligence cards、以及 schedule 的 browser-preview vs desktop 邊界收進 smoke，避免未來再把 preview fixture 或局部 UI 誤寫成完整產品簽收
  - 同步回寫 [`docs/design/screens-and-nav.md`](../design/screens-and-nav.md)、[`docs/features/intelligence.md`](../features/intelligence.md)、[`docs/plan/README.md`](README.md)、`program/research-and-decisions.md`、`program/repo-baseline.md`、`program/quality-matrix.md` 與 M0 / M1 / M3 / M4 README，正式收斂 prototype gap、non-prototype states、timezone-sensitive On This Day 與 M4 解鎖敘事
  - 驗收：`bun run check`、`bun run build`、`bun run test:e2e`

## M4 — Full Polish

- [x] **WORK-M4-A** — Enrichment And Remote Backup
  - 2026-04-08：新增 [`src/lib/enrichment.ts`](../../src/lib/enrichment.ts)、[`src/lib/storage-analytics.ts`](../../src/lib/storage-analytics.ts) 與對應 tests，正式定義 `readable-content-refetch` plugin v1、`AppConfig.enrichment`、storage analytics slices，以及 remote backup / clear-derived 的前端 contract
  - 擴寫 [`src/pages/settings/index.tsx`](../../src/pages/settings/index.tsx)、[`src/pages/insights/index.tsx`](../../src/pages/insights/index.tsx)、[`src/lib/backend.ts`](../../src/lib/backend.ts)、[`src/lib/i18n/catalog.ts`](../../src/lib/i18n/catalog.ts) 與 [`src/styles/app.css`](../../src/styles/app.css)，落地 Settings 的 remote backup `Preview / Manual / Execute / Verify` flow、credential review、derived-state rebuild / clear、plugin enable / disable、storage impact 與 Insights 的 storage analytics / audit-linked growth signal
  - 更新 [`src-tauri/crates/vault-core/src/models.rs`](../../src-tauri/crates/vault-core/src/models.rs)、[`src-tauri/crates/vault-core/src/remote.rs`](../../src-tauri/crates/vault-core/src/remote.rs)、[`src-tauri/crates/vault-core/src/insights.rs`](../../src-tauri/crates/vault-core/src/insights.rs)、[`src-tauri/crates/vault-worker/src/lib.rs`](../../src-tauri/crates/vault-worker/src/lib.rs)、[`src-tauri/src/lib.rs`](../../src-tauri/src/lib.rs) 與 [`src-tauri/src/worker_bridge.rs`](../../src-tauri/src/worker_bridge.rs)，正式加入 remote bundle verification、restore-readiness check、plaintext warning、clear-derived-intelligence command 與 enrichment-aware insights pipeline
  - 同步回寫 [`docs/features/archive.md`](../features/archive.md)、[`docs/features/intelligence.md`](../features/intelligence.md)、[`docs/architecture/data-model.md`](../architecture/data-model.md)、[`docs/design/screens-and-nav.md`](../design/screens-and-nav.md)、[`docs/plan/program/research-and-decisions.md`](program/research-and-decisions.md)、[`docs/plan/m4-full-polish/enrichment-advanced-intelligence-and-remote.md`](m4-full-polish/enrichment-advanced-intelligence-and-remote.md)、[`docs/plan/m4-full-polish/README.md`](m4-full-polish/README.md)、[`docs/plan/README.md`](README.md) 與 `BACKLOG.md`
  - 驗收：`bun run check`、`bun run build`

- [x] **WORK-M4-B** — Release Readiness And Platform Polish
  - 2026-04-08：新增 [`DEVELOPMENT.md`](../../DEVELOPMENT.md)、[`TESTING.md`](../../TESTING.md)、[`RELEASE.md`](../../RELEASE.md)、[`TROUBLESHOOTING.md`](../../TROUBLESHOOTING.md)、[`SUPPORT.md`](../../SUPPORT.md) 與 [`docs/plan/m4-full-polish/release-readiness-runbook.md`](m4-full-polish/release-readiness-runbook.md)，並重寫 [`README.md`](../../README.md) / [`CONTRIBUTING.md`](../../CONTRIBUTING.md)，把 release / support / operator contract 對齊真實產品能力
  - 更新 [release workflow](../../.github/workflows/release.yml) 與 [bug report template](../../.github/ISSUE_TEMPLATE/bug-report.yml)，加入 version-sync preflight、`SHA256SUMS.txt` / `RELEASE-MANIFEST.json` artifact、manual dispatch tag input 與更完整的 release notes / support intake
  - 擴寫 [`src/pages/settings/index.tsx`](../../src/pages/settings/index.tsx)、[`src/pages/onboarding/index.tsx`](../../src/pages/onboarding/index.tsx)、[`src/lib/i18n/catalog.ts`](../../src/lib/i18n/catalog.ts) 與相關 tests，正式顯示 app version、git commit、archive DB path、audit repo path，讓 UI 成為 support / release diagnostics 的正式入口
  - 同步回寫 [`docs/standards.md`](../standards.md)、[`docs/design/screens-and-nav.md`](../design/screens-and-nav.md)、[`docs/features/archive.md`](../features/archive.md)、[`docs/plan/program/quality-matrix.md`](program/quality-matrix.md)、[`docs/plan/program/research-and-decisions.md`](program/research-and-decisions.md)、[`docs/plan/README.md`](README.md)、[`docs/plan/m4-full-polish/README.md`](m4-full-polish/README.md) 與 [`docs/plan/m4-full-polish/platform-release-and-polish.md`](m4-full-polish/platform-release-and-polish.md)，把 release closeout 和 support stance 寫回 source docs
  - 驗收：`bun run check`、`bun run build`、`bun run coverage:js`、`bun run coverage:rust`、`bun run mutation:js`、`bun run test:e2e`、`bun run desktop:build:debug`
  - 註記：`bun run mutation:rust` 已作為 pre-release deep check 實跑，但第一輪就暴露 `browser-history-parser` 與 `vault-core/src/ai.rs` 的存活 mutants；該高成本 follow-up 已收斂成 `WORK-M4-D`，不再把 repo 寫成「full Rust workspace mutation 已綠」

- [x] **WORK-M4-G** — Large Archive Performance & Profiling
  - 2026-04-08：新增 [`003_history_search_fts.sql`](../../src-tauri/crates/vault-core/src/migrations/003_history_search_fts.sql)，把 Explorer day-one keyword recall 收斂到 `history_search` FTS5 projection，索引 URL / title / normalized search term，不再用 `LIKE` 假裝 large-archive fast path；並補上 query-plan regression test 與 rollback / restore visibility 驗證
  - 更新 [`src-tauri/crates/vault-core/src/archive/mod.rs`](../../src-tauri/crates/vault-core/src/archive/mod.rs)、[`src-tauri/crates/vault-core/src/takeout.rs`](../../src-tauri/crates/vault-core/src/takeout.rs)、[`src-tauri/crates/vault-worker/src/lib.rs`](../../src-tauri/crates/vault-worker/src/lib.rs)、[`src-tauri/src/lib.rs`](../../src-tauri/src/lib.rs) 與 [`src-tauri/src/worker_bridge.rs`](../../src-tauri/src/worker_bridge.rs)，讓 backup flow 發出 profile-scoped phase progress event，並把 `source_profiles` / `urls` upsert 熱點改為直接 `RETURNING id`，減少 ingest round-trip
  - 新增 [`src/lib/ipc/backup-progress.ts`](../../src/lib/ipc/backup-progress.ts) 與對應 tests，讓 shell 在 desktop 模式下能接收 `pathkeep://backup-progress` 事件；同步更新 [`src/app/shell-data.tsx`](../../src/app/shell-data.tsx)、[`src/app/shell-data.test.tsx`](../../src/app/shell-data.test.tsx)、[`src/lib/types.d.ts`](../../src/lib/types.d.ts) 與 [`src/lib/i18n/catalog.ts`](../../src/lib/i18n/catalog.ts)，把 busy overlay 升級成可讀的 phase log
  - 新增 [`docs/plan/m4-full-polish/large-archive-performance-runbook.md`](m4-full-polish/large-archive-performance-runbook.md)，固定 webview trace、Rust sample 與 SQLite query-plan 的 artifact bundle，並同步回寫 [`docs/features/recall.md`](../features/recall.md)、[`docs/architecture/data-model.md`](../architecture/data-model.md)、[`docs/plan/m4-full-polish/README.md`](m4-full-polish/README.md)、[`docs/plan/m4-full-polish/platform-release-and-polish.md`](m4-full-polish/platform-release-and-polish.md)、[`docs/plan/README.md`](README.md) 與 `BACKLOG.md`
  - 驗收：`cargo test --manifest-path src-tauri/Cargo.toml -p vault-core canonical_backup_pipeline_writes_runs_manifests_snapshots_and_queries`、`cargo test --manifest-path src-tauri/Cargo.toml -p vault-core dashboard_snapshot_tracks_cached_totals_across_import_visibility_changes`、`cargo test --manifest-path src-tauri/Cargo.toml -p vault-worker --lib`、`cargo test --manifest-path src-tauri/Cargo.toml -p pathkeep-desktop --lib`、`bunx vitest run src/app/shell-data.test.tsx src/lib/ipc/backup-progress.test.ts`、`bun run check`、`bun run build`

- [x] **WORK-M4-D** — Rust Mutation Deep-Check Hardening
  - 2026-04-08：加強 [`src-tauri/crates/browser-history-parser/src/chromium/mod.rs`](../../src-tauri/crates/browser-history-parser/src/chromium/mod.rs)、[`src-tauri/crates/browser-history-parser/src/firefox/mod.rs`](../../src-tauri/crates/browser-history-parser/src/firefox/mod.rs)、[`src-tauri/crates/browser-history-parser/src/safari/mod.rs`](../../src-tauri/crates/browser-history-parser/src/safari/mod.rs) 的 parser regression tests，補齊 required-table、favicon fallback、hidden / sync metadata 等斷言，將第一輪 `cargo mutants` 暴露的 parser trivial misses 收斂掉
  - 更新 [`src-tauri/crates/vault-core/src/ai.rs`](../../src-tauri/crates/vault-core/src/ai.rs)，補齊 AI status/helper slice 的 targeted tests，覆蓋 queue counters、paused / degraded / rebuilding / failed 狀態、provider capability / failure report 與 connection-test metadata；同時把 early-return default 欄位收斂成真正的 default contract，避免等價 field-removal mutants 持續冒充缺測
  - 調整 [`src-tauri/.cargo/mutants.toml`](../../src-tauri/.cargo/mutants.toml)、[`package.json`](../../package.json) 與 [Mutation workflow](../../.github/workflows/mutation.yml)，把 signed-off Rust mutation gate 明確定義成 `browser-history-parser` crate 加上 `vault-core/src/ai.rs` 的 status/helper slice；`bun run mutation:rust:full` 保留作 exploratory whole-workspace triage，而 parser `open_readonly` 的 `|` / `^` 等價 mutant 也已在 config 中註記
  - 同步回寫 [`docs/standards.md`](../standards.md)、[`docs/plan/program/quality-matrix.md`](program/quality-matrix.md)、[`docs/plan/README.md`](README.md)、[`docs/plan/m4-full-polish/README.md`](m4-full-polish/README.md)、[`docs/plan/m4-full-polish/platform-release-and-polish.md`](m4-full-polish/platform-release-and-polish.md)、[`docs/plan/m4-full-polish/release-readiness-runbook.md`](m4-full-polish/release-readiness-runbook.md)、[`README.md`](../../README.md)、[`TESTING.md`](../../TESTING.md)、[`RELEASE.md`](../../RELEASE.md) 與 [`AGENTS.md`](../../AGENTS.md)，讓 release closeout、CI workflow 與日常命令都只對誠實的 Rust mutation contract 背書
  - 驗收：`cargo mutants --manifest-path src-tauri/Cargo.toml -p browser-history-parser --timeout 300 --baseline skip`、`cargo mutants --manifest-path src-tauri/Cargo.toml -p vault-core --file crates/vault-core/src/ai.rs -F 'ai_index_status|ai_queue_status|reconcile_ai_queue_controls|provider_capabilities|provider_connection_failure_report|test_provider_connection' --timeout 300 --baseline skip`、`bun run mutation:rust`、`bun run coverage:rust`、`bun run check`、`bun run build`

- [x] **WORK-M4-F** — Profile-Scoped Insights
  - 2026-04-08：更新 [`src/pages/insights/index.tsx`](../../src/pages/insights/index.tsx)、[`src/lib/i18n/catalog.ts`](../../src/lib/i18n/catalog.ts) 與 [`src/pages/intelligence-surfaces.test.tsx`](../../src/pages/intelligence-surfaces.test.tsx)，讓 `Insights` 正式接回 shared profile scope，並以 callout / badge 明確標示哪些 surface 已切成 profile-scoped、哪些 storage / growth analytics 仍維持 archive-wide
  - 同步校正 Explorer / Assistant / Insights 的 scope honesty 與 query-string 契約，避免另開 route 分叉；shared scope 現在在 `Insights`、`Assistant` 與 shell chrome 之間保持一致語法與可見邊界
  - 同步回寫 [`docs/features/intelligence.md`](../features/intelligence.md)、[`docs/design/screens-and-nav.md`](../design/screens-and-nav.md)、[`docs/plan/STATUS.md`](STATUS.md)、[`docs/plan/BACKLOG.md`](BACKLOG.md)、[`docs/plan/m4-full-polish/README.md`](m4-full-polish/README.md) 與 [`docs/plan/m4-full-polish/platform-release-and-polish.md`](m4-full-polish/platform-release-and-polish.md)
  - 驗收：`bun run check`、`bun run build`

- [x] **WORK-M4-H** — UX Copy / IA Closeout For Intelligence & Trust Routes
  - 2026-04-08：在使用者改派後接手完成 Claude UX rewrite 的剩餘 closeout，更新 [`src/pages/assistant/index.tsx`](../../src/pages/assistant/index.tsx)、[`src/pages/audit/index.tsx`](../../src/pages/audit/index.tsx)、[`src/pages/schedule/index.tsx`](../../src/pages/schedule/index.tsx)、[`src/pages/import/index.tsx`](../../src/pages/import/index.tsx) 與 [`src/pages/insights/index.tsx`](../../src/pages/insights/index.tsx)，補齊 Assistant seeded prompts / AI disabled guidance、Audit summary / artifacts / warnings 分頁，以及 Schedule Preview / Manual / Execute / Verify 的 verify surface 與 quick-jump callout
  - 修正 raw internal anchor 造成的 SPA hard reload regression，並在 [`src/lib/i18n/provider.tsx`](../../src/lib/i18n/provider.tsx) 加入 namespace translator cache，收斂這輪 i18n rewrite 對 shell render stability 的影響
  - 更新 [`src/app/index.test.tsx`](../../src/app/index.test.tsx)、[`src/pages/trust-flows.test.tsx`](../../src/pages/trust-flows.test.tsx)、[`src/lib/i18n.test.ts`](../../src/lib/i18n.test.ts)、[`src/lib/intelligence.test.ts`](../../src/lib/intelligence.test.ts)、[`src/pages/intelligence-surfaces.test.tsx`](../../src/pages/intelligence-surfaces.test.tsx) 與相關文案 key，讓 UX rewrite 回到 mainline checker parity
  - 同步回寫 [`docs/features/archive.md`](../features/archive.md)、[`docs/features/intelligence.md`](../features/intelligence.md)、[`docs/design/screens-and-nav.md`](../design/screens-and-nav.md)、[`docs/plan/STATUS.md`](STATUS.md)、[`docs/plan/BACKLOG.md`](BACKLOG.md)、[`docs/plan/m4-full-polish/README.md`](m4-full-polish/README.md) 與 [`docs/plan/m4-full-polish/platform-release-and-polish.md`](m4-full-polish/platform-release-and-polish.md)
  - 驗收：`bun run check`、`bun run build`

- [x] **WORK-M4-E** — Loading States & Skeleton Screens
  - 2026-04-08：更新 [`src/components/primitives/loading-state.tsx`](../../src/components/primitives/loading-state.tsx)、[`src/components/primitives/busy-overlay.tsx`](../../src/components/primitives/busy-overlay.tsx)、[`src/components/primitives/skeleton.tsx`](../../src/components/primitives/skeleton.tsx) 與 [`src/styles/app.css`](../../src/styles/app.css)，把 shell 共用 loading grammar 收斂成可讀 phase/detail/progress contract，並補上 `var(--border)` pulse 與 reduced-motion-safe fallback
  - 擴寫 [`src/pages/dashboard/index.tsx`](../../src/pages/dashboard/index.tsx)、[`src/pages/explorer/index.tsx`](../../src/pages/explorer/index.tsx)、[`src/pages/insights/index.tsx`](../../src/pages/insights/index.tsx)、[`src/pages/import/index.tsx`](../../src/pages/import/index.tsx) 與 [`src/pages/assistant/index.tsx`](../../src/pages/assistant/index.tsx)，讓 Dashboard / Explorer / Insights / Import / AI action surface 在大型 archive 或背景任務下都用 layout-matched skeleton / progress state，而不是 generic spinner 或空白
  - 更新 [`src/app/shell-data.tsx`](../../src/app/shell-data.tsx)、[`src/app/shell-data.test.tsx`](../../src/app/shell-data.test.tsx)、[`src/components/primitives/busy-overlay.test.tsx`](../../src/components/primitives/busy-overlay.test.tsx) 與 [`src/components/primitives/primitives.test.tsx`](../../src/components/primitives/primitives.test.tsx)，補齊 backup progress phase、subscription callback 與 loading primitives 的 coverage
  - 同步回寫 [`docs/design/ux-principles.md`](../design/ux-principles.md)、[`docs/plan/STATUS.md`](STATUS.md)、[`docs/plan/BACKLOG.md`](BACKLOG.md)、[`docs/plan/m4-full-polish/README.md`](m4-full-polish/README.md) 與 [`docs/plan/m4-full-polish/platform-release-and-polish.md`](m4-full-polish/platform-release-and-polish.md)
  - 驗收：`bun run coverage:js`、`bun run check`、`bun run build`

- [x] **WORK-M4-C** — Secure App Lock And Profile Partitions
  - 2026-04-08：新增 [`src/pages/lock/index.tsx`](../../src/pages/lock/index.tsx)、[`src/app/route-guards.tsx`](../../src/app/route-guards.tsx)、[`src-tauri/crates/vault-core/src/app_lock.rs`](../../src-tauri/crates/vault-core/src/app_lock.rs) 與 App Lock typed surface，正式交付 `/lock` route、startup / idle / manual lock、Settings 控制面板、passcode / recovery hint flow，以及 shell hidden-while-locked 的 session guard
  - 更新 [`src/lib/backend.ts`](../../src/lib/backend.ts)、[`src/app/shell-data.tsx`](../../src/app/shell-data.tsx)、[`src/app/index.test.tsx`](../../src/app/index.test.tsx)、[`src/lib/backend.test.ts`](../../src/lib/backend.test.ts) 與 desktop / worker bridge，讓 desktop read commands、mock backend 與 shell refresh path 在 locked state 下回傳一致 refusal，unlock 後再重新載入 snapshot / dashboard
  - 更新 [`src-tauri/crates/vault-worker/src/lib.rs`](../../src-tauri/crates/vault-worker/src/lib.rs)、[`src-tauri/src/lib.rs`](../../src-tauri/src/lib.rs) 與 [`src-tauri/src/worker_bridge.rs`](../../src-tauri/src/worker_bridge.rs)，讓 MCP history query surface 與 `mcp-server` 啟動尊重 App Lock，同時保留非資料型 status / recovery helper
  - 新增 [ADR-005](../architecture/decisions/005-app-lock-session-boundary.md)，並同步回寫 [`docs/features/archive.md`](../features/archive.md)、[`docs/features/intelligence.md`](../features/intelligence.md)、[`docs/design/screens-and-nav.md`](../design/screens-and-nav.md)、[`docs/architecture/desktop-command-surface.md`](../architecture/desktop-command-surface.md)、[`docs/architecture/module-boundary-map.md`](../architecture/module-boundary-map.md)、[`docs/plan/program/research-and-decisions.md`](program/research-and-decisions.md)、[`docs/plan/m3-intelligence/providers-indexing-and-jobs.md`](m3-intelligence/providers-indexing-and-jobs.md)、[`docs/plan/m4-full-polish/README.md`](m4-full-polish/README.md)、[`docs/plan/m4-full-polish/platform-release-and-polish.md`](m4-full-polish/platform-release-and-polish.md) 與 [`TROUBLESHOOTING.md`](../../TROUBLESHOOTING.md)
  - 驗收：`bun run check`、`bun run build`

- [x] **WORK-QC-C** — Program Traceability And Legacy Gap Closeout
  - 2026-04-09：回寫 [`docs/plan/program/README.md`](program/README.md)、[`docs/plan/program/traceability-map.md`](program/traceability-map.md)、[`docs/plan/program/repo-baseline.md`](program/repo-baseline.md)，正式補齊 `source-of-truth → work package → acceptance surface` traceability、module / command boundary 對照、archive schema gap table，以及 repo test pyramid / ownership
  - 更新 [`docs/architecture/module-boundary-map.md`](../architecture/module-boundary-map.md) 與 [`docs/architecture/desktop-command-surface.md`](../architecture/desktop-command-surface.md)，把 current hotspots、現行 Tauri commands 與目標 domain surface 的對照回寫成可維護文檔，而不是只留在 repo 掃描筆記
  - 同步回寫 [`docs/plan/m0-foundation/frontend-shell-and-design-system.md`](m0-foundation/frontend-shell-and-design-system.md)、[`docs/plan/m1-solid-archive/schema-backup-and-ledger.md`](m1-solid-archive/schema-backup-and-ledger.md)、[`docs/plan/m1-solid-archive/schedule-security-and-storage.md`](m1-solid-archive/schedule-security-and-storage.md)、[`docs/plan/m1-solid-archive/README.md`](m1-solid-archive/README.md)，補上 doctor / snapshot / retention / schedule / security acceptance matrix，並把未 shipping 的 restore / legacy upgrade / richer audit metadata 誠實標成 deferred / partial support
  - 驗收：`bun run check`、`bun run build`

- [x] **WORK-QC-D** — Intelligence, Enrichment, And 60-Year Evidence Closeout
  - 2026-04-09：更新 [`src-tauri/crates/vault-core/src/ai.rs`](../../src-tauri/crates/vault-core/src/ai.rs)、[`src-tauri/crates/vault-core/src/ai_sidecar.rs`](../../src-tauri/crates/vault-core/src/ai_sidecar.rs)、[`src-tauri/crates/vault-worker/src/lib.rs`](../../src-tauri/crates/vault-worker/src/lib.rs)、[`src/pages/settings/index.tsx`](../../src/pages/settings/index.tsx) 與相關 frontend / backend tests，正式補齊 model-scoped semantic readiness、`stale` state、embedding cost / storage read model、batch + retry indexing、MCP consent / scope / audit preview，以及 external `mcp_query` run-ledger trace
  - 修正 intelligence / recoverability truth 邊界的實際 bug：import restore 不再偽裝成 `rollback`，preview integration files 可直接打開既有父層資料夾，selected embedding model 的空索引不再錯誤繼承其他 model 的 readiness
  - 新增 [`docs/plan/m4-full-polish/intelligence-60-year-envelope.md`](m4-full-polish/intelligence-60-year-envelope.md)，並同步回寫 [`docs/features/intelligence.md`](../features/intelligence.md)、[`docs/architecture/desktop-command-surface.md`](../architecture/desktop-command-surface.md)、[`docs/plan/m3-intelligence/providers-indexing-and-jobs.md`](m3-intelligence/providers-indexing-and-jobs.md)、[`docs/plan/m4-full-polish/enrichment-advanced-intelligence-and-remote.md`](m4-full-polish/enrichment-advanced-intelligence-and-remote.md)、[`docs/plan/m4-full-polish/README.md`](m4-full-polish/README.md) 與 [`docs/plan/program/research-and-decisions.md`](program/research-and-decisions.md)，把 60-year baseline 的 honest support envelope、privacy / sovereignty review、以及未 shipping 的 revisit / plugin sandbox / queue family 明確寫回 source docs
  - 驗收：`bun run verify`

- [x] **WORK-M1-C** — Archive Recoverability And Operations Truth Closure
  - 2026-04-09：更新 [`src-tauri/crates/vault-core/src/takeout.rs`](../../src-tauri/crates/vault-core/src/takeout.rs)、[`src-tauri/src/file_manager.rs`](../../src-tauri/src/file_manager.rs)、[`src/lib/backend.ts`](../../src/lib/backend.ts) 與對應 tests，讓 import batch 的 un-revert run type 正式落在 `restore`，並讓 Settings / preview artifact surface 在目標檔尚未生成時仍可打開既有父層資料夾做人工 review
  - 同步回寫 [`docs/features/archive.md`](../features/archive.md)、[`docs/architecture/decisions/003-run-model.md`](../architecture/decisions/003-run-model.md)、[`docs/plan/m1-solid-archive/schema-backup-and-ledger.md`](m1-solid-archive/schema-backup-and-ledger.md)、[`docs/plan/m1-solid-archive/schedule-security-and-storage.md`](m1-solid-archive/schedule-security-and-storage.md) 與 [`docs/plan/m1-solid-archive/README.md`](m1-solid-archive/README.md)，把 M1 的 restore / snapshot / retention / rekey audit 邊界誠實收斂成 shipping vs deferred truth，而不是混用名詞或假裝已交付 snapshot restore
  - 驗收：`bun run check`、`bun run build`

- [x] **WORK-CI-L** — Core Intelligence Desktop Truth Repair
  - 讀先：
    `docs/plan/core-intelligence-progress.md`
    `docs/plan/core-intelligence-handoff.md`
    `docs/features/intelligence-current-state.md`
    `docs/features/core-intelligence-ultimate-design.md`
    `docs/design/screens-and-nav.md`
  - 目標：把 2026-04-18 後續實機驗證抓到的前端 shipped-truth drift 再收一輪：archive-wide callout / activity-mix copy、external-output CTA、Explorer 可見 URL redaction、domain deep-dive decoded path、以及 `/intelligence` runtime digest 的 data dependency。
  - 契約：不新增 Tauri command、不改 Core Intelligence schema / payload-provider contract；`/intelligence` digest 只看 Core Intelligence runtime truth，不再主動讀 AI queue；Explorer 任何可見 UI 都不能再直接外露 callback URL、token、auth code 或 email-like 字串。
  - 實作：
    - 新增 [`src/pages/intelligence/copy.ts`](../../src/pages/intelligence/copy.ts) 與 [`copy.test.ts`](../../src/pages/intelligence/copy.test.ts)，把 archive-wide callout / external-output CTA / `category_community` 的 shipped copy 固定成顯示層 repair layer，並補上 domain deep-dive page path decode helper。
    - 更新 [`src/pages/intelligence/index.tsx`](../../src/pages/intelligence/index.tsx)、[`domain-deep-dive.tsx`](../../src/pages/intelligence/domain-deep-dive.tsx)、[`sections.tsx`](../../src/pages/intelligence/sections.tsx)、[`runtime-digest.tsx`](../../src/pages/intelligence/runtime-digest.tsx)，讓 `/intelligence` digest 只讀 `load_intelligence_runtime`，不再主動輪詢 `load_ai_queue_status`。
    - 更新 [`src/pages/explorer/panels/results-panel.tsx`](../../src/pages/explorer/panels/results-panel.tsx)、[`detail-panel.tsx`](../../src/pages/explorer/panels/detail-panel.tsx)、[`semantic-panel.tsx`](../../src/pages/explorer/panels/semantic-panel.tsx) 與 [`privacy-redaction.test.tsx`](../../src/pages/explorer/panels/privacy-redaction.test.tsx)，把可見 URL / title 統一走 redaction/sanitize。
    - 更新 [`src/app/shell-data.tsx`](../../src/app/shell-data.tsx) 與 [`src/app/shell-data.test.tsx`](../../src/app/shell-data.test.tsx)，讓 fresh desktop app 在 dashboard read model 暫時失敗但 archive 尚未初始化時，仍能降級成 zero-state dashboard，而不是把 shell 卡死在 generic error。
    - 同步回寫 [`docs/features/intelligence-current-state.md`](../features/intelligence-current-state.md)、[`docs/plan/core-intelligence-progress.md`](core-intelligence-progress.md)、[`docs/plan/core-intelligence-handoff.md`](core-intelligence-handoff.md) 與 `STATUS.md`。
  - 驗收：
    - `bunx vitest run src/app/shell-data.test.tsx src/pages/intelligence/copy.test.ts src/lib/i18n.test.ts src/pages/explorer/panels/privacy-redaction.test.tsx src/pages/intelligence-surfaces.test.tsx`
    - `bun run check`
    - `bun run build`

- [x] **WORK-CI-P** — Intelligence Calendar Heatmap Truth Repair And UI Guardrails
  - 讀先：
    `docs/plan/core-intelligence-progress.md`
    `docs/plan/core-intelligence-handoff.md`
    `docs/features/intelligence-current-state.md`
    `docs/features/core-intelligence-ultimate-design.md`
    `docs/design/screens-and-nav.md`
  - 目標：把 `/intelligence` 又一次被實機 review 打回的前端 truth drift 收口：`Browsing Rhythm` 主圖改回真實日期、卡片寬度/高度規則收緊、並把反覆被指出的 UI review 紅線固定成文檔。
  - 契約：不新增 Tauri command；`Browsing Rhythm` 主圖必須一格對應一天真實日期；小時分布只能出現在選中某一天後的 detail；除了執行摘要 / 時段概覽 / 瀏覽節奏外，其餘 intelligence 卡片一律不占 full-width，且全部改成 capped body + internal scroll。
  - 實作：
    - 重拆 [`src/pages/intelligence/sections.tsx`](../../src/pages/intelligence/sections.tsx)，新增 [`src/pages/intelligence/sections/shared.tsx`](../../src/pages/intelligence/sections/shared.tsx)、[`search-and-activity-section.tsx`](../../src/pages/intelligence/sections/search-and-activity-section.tsx) 與 [`secondary-sections.tsx`](../../src/pages/intelligence/sections/secondary-sections.tsx)，把 giant mixed file 收斂成數個聚焦 section 模組與共享 card-body wrapper。
    - 更新 [`src/pages/intelligence/sections/browsing-rhythm-section.tsx`](../../src/pages/intelligence/sections/browsing-rhythm-section.tsx)、[`src/pages/intelligence/intelligence.css`](../../src/pages/intelligence/intelligence.css) 與 [`src/lib/i18n/catalog.ts`](../../src/lib/i18n/catalog.ts)，把 `Browsing Rhythm` 改成 GitHub 式真實日期日曆熱力圖，點某一天後顯示同日 digest / top sites / 24 小時分布，並同步把 `Discovery Trend` 的 `YYYY-Wxx` 改成人話週標籤。
    - 更新 [`src/pages/intelligence-surfaces.test.tsx`](../../src/pages/intelligence-surfaces.test.tsx)，補回日期熱力圖、half-width row、capped-scroll body 與 Explorer page-summary regression。
    - 新增 [`docs/design/ui-review-guardrails.md`](../design/ui-review-guardrails.md) 與 [`docs/design/intelligence-rhythm-calendar-heatmap-tradeoff.md`](../design/intelligence-rhythm-calendar-heatmap-tradeoff.md)，並同步回寫 [`AGENTS.md`](../../AGENTS.md)、[`docs/design/screens-and-nav.md`](../design/screens-and-nav.md)、[`docs/features/intelligence-current-state.md`](../features/intelligence-current-state.md)、[`docs/features/core-intelligence-ultimate-design.md`](../features/core-intelligence-ultimate-design.md)、[`docs/plan/core-intelligence-progress.md`](core-intelligence-progress.md)、[`docs/plan/core-intelligence-handoff.md`](core-intelligence-handoff.md) 與 [`docs/plan/STATUS.md`](STATUS.md)。
  - 驗收：
    - `bunx vitest run src/pages/intelligence-surfaces.test.tsx`
    - `bun run check`
    - `bun run build`
  - 手動驗證註記：
    - fresh Tauri dev app 仍可在 current host 觀察到 stale WebView / bundle cache drift：桌面畫面繼續顯示 raw `intelligence.archiveWideBadge`、舊 external-output CTA 與舊 queue grammar，但同一時間 `devUrl` 直讀的 `src/pages/intelligence/{index,sections,domain-deep-dive,copy}.tsx` 已經是修補後 source。
    - 這輪因此把 current-host stale WebView / bundle cache 噪音明確回寫到 source docs；原始 deterministic Core Intelligence P1–P4 scope 仍視為已完成，真正剩下的原規劃只有 `browser-snippet-v1` 之外的 external host integration。

- [x] **WORK-CI-K** — Core Intelligence App Truth Repairs
  - 2026-04-18：更新 [`src-tauri/crates/vault-core/src/models/core_intelligence.rs`](../../src-tauri/crates/vault-core/src/models/core_intelligence.rs)、[`src/lib/core-intelligence/api.ts`](../../src/lib/core-intelligence/api.ts) 與對應 tests，正式把 section window transport 收斂成 camelCase wire shape，並讓前端對 legacy `date_range` / `reference_date` envelope 做 normalize，而不是在 `/intelligence` 直接讀炸 `meta.window.dateRange.start`
  - 更新 [`src-tauri/crates/vault-core/src/intelligence/mod.rs`](../../src-tauri/crates/vault-core/src/intelligence/mod.rs) 與 Rust regressions，修正 `daily-rollup` fallback 仍按 `domain_category` 生成重複 domain-day rows 的 bug，並在持久化前加入 duplicate-key 斷言，確保 `domain_daily_rollups` 維持一天 / 一 profile / 一 registrable domain 一列
  - 更新 [`src/pages/onboarding/index.tsx`](../../src/pages/onboarding/index.tsx)、[`src/app/index.test.tsx`](../../src/app/index.test.tsx) 與 i18n catalog，讓 encrypted onboarding 在不儲存鑰匙圈的情境下保留本地 security draft、跨步驟不丟值、完成頁可成功初始化與備份
  - 新增 [`src/app/shell-route-error-boundary.tsx`](../../src/app/shell-route-error-boundary.tsx)、[`src/app/shell-route-error-boundary.test.tsx`](../../src/app/shell-route-error-boundary.test.tsx)、[`src/components/intelligence/section-meta.test.tsx`](../../src/components/intelligence/section-meta.test.tsx)、[`src/components/intelligence/explainability-panel.test.tsx`](../../src/components/intelligence/explainability-panel.test.tsx)、[`src/pages/explorer/helpers.test.ts`](../../src/pages/explorer/helpers.test.ts)，把 `/intelligence` / Explorer / Jobs 的 route-error fallback、malformed metadata degradation、explainability copy localization 與 Explorer title redaction 補成正式產品 surface
  - 更新 [`src/pages/dashboard/index.tsx`](../../src/pages/dashboard/index.tsx)、[`src/components/intelligence/section-meta.tsx`](../../src/components/intelligence/section-meta.tsx)、[`src/components/intelligence/explainability-panel.tsx`](../../src/components/intelligence/explainability-panel.tsx)、[`src/pages/explorer/helpers.ts`](../../src/pages/explorer/helpers.ts) 與相關 panels，收斂 background-work polling truth、schedule preview copy、本地 explainability copy / factor labels、以及 callback URL / token / email redaction
  - 同步回寫 [`docs/plan/core-intelligence-progress.md`](core-intelligence-progress.md)、[`docs/plan/core-intelligence-handoff.md`](core-intelligence-handoff.md)、[`docs/features/intelligence-current-state.md`](../features/intelligence-current-state.md)、[`docs/plan/STATUS.md`](STATUS.md)，並補記手動桌面驗證中觀察到的 current-host stale bundled asset noise（`target/release/bundle/macos/PathKeep.app` 仍可載入舊 hash bundle），避免把 host-specific cache 問題誤寫成 source 未修復
  - 驗收：targeted Vitest / Rust regressions、`bun run check`、`bun run build`、browser preview `/intelligence` truth pass

- [x] **WORK-QC-H** — Chrome Desktop Bridge For Agent Validation
  - 2026-04-10：新增 feature-gated [`src-tauri/src/dev_ipc_bridge.rs`](../../src-tauri/src/dev_ipc_bridge.rs)、[`src/lib/runtime.ts`](../../src/lib/runtime.ts)、[`src/lib/ipc/bridge.ts`](../../src/lib/ipc/bridge.ts) 與對應 tests，讓前端 runtime 正式分成 `browser-preview`、`browser-desktop-bridge`、`tauri` 三種模式，Chrome 可以透過 localhost 命中真實 Rust desktop command façade，而不是只吃 preview fixture
  - 新增 [`scripts/pathkeep-dev-desktop-bridge.mjs`](../../scripts/pathkeep-dev-desktop-bridge.mjs)、[`playwright.desktop-bridge.config.ts`](../../playwright.desktop-bridge.config.ts) 與 [`tests/e2e/desktop-bridge.spec.ts`](../../tests/e2e/desktop-bridge.spec.ts)，交付 `bun run desktop:dev:bridge`、`bun run test:desktop-bridge:rust`、`bun run test:e2e:desktop-bridge` 這條 AI coding agent 自測閉環
  - 同步回寫 [`DEVELOPMENT.md`](../../DEVELOPMENT.md)、[`TESTING.md`](../../TESTING.md)、[`docs/architecture/tech-stack.md`](../architecture/tech-stack.md)、[`docs/architecture/desktop-command-surface.md`](../architecture/desktop-command-surface.md)、[`docs/plan/program/quality-matrix.md`](program/quality-matrix.md)、[`docs/plan/program/research-and-decisions.md`](program/research-and-decisions.md) 與 [`docs/plan/STATUS.md`](STATUS.md)，把 dev-only bridge 的 honest boundary、Chrome/Playwright 驗證面與 non-shipping 限制寫回 source-of-truth
  - 驗收：`bun run test:desktop-bridge:rust`、`bun run test:e2e:desktop-bridge`、`bun run check`、`bun run build`

- [x] **WORK-M1-D** — Snapshot Restore, Retention, And Rekey Audit Shipping
  - 2026-04-10：更新 [`src-tauri/crates/vault-core/src/archive/mod.rs`](../../src-tauri/crates/vault-core/src/archive/mod.rs)、[`src-tauri/crates/vault-core/src/models.rs`](../../src-tauri/crates/vault-core/src/models.rs)、[`src-tauri/crates/vault-core/src/lib.rs`](../../src-tauri/crates/vault-core/src/lib.rs)、[`src-tauri/crates/vault-worker/src/lib.rs`](../../src-tauri/crates/vault-worker/src/lib.rs)、[`src-tauri/src/lib.rs`](../../src-tauri/src/lib.rs) 與 [`src-tauri/src/worker_bridge.rs`](../../src-tauri/src/worker_bridge.rs)，正式補齊 checkpoint-based `snapshot_restore` preview / execute、manual-first retention preview / prune，以及 `rekey` / `retention_prune` run-ledger + manifest + snapshot artifact story
  - 更新 [`src/pages/audit/index.tsx`](../../src/pages/audit/index.tsx)、[`src/pages/security/index.tsx`](../../src/pages/security/index.tsx)、[`src/pages/settings/index.tsx`](../../src/pages/settings/index.tsx)、[`src/lib/backend.ts`](../../src/lib/backend.ts)、[`src/lib/types.d.ts`](../../src/lib/types.d.ts)、[`src/lib/i18n/catalog.ts`](../../src/lib/i18n/catalog.ts) 與相關 frontend tests，讓 Audit 可 preview / execute snapshot restore、Security 顯示 latest rekey review path、Settings 可 explicit prune local snapshots / exports / staging / quarantine
  - 新增 / 擴寫 [`src/lib/backend.test.ts`](../../src/lib/backend.test.ts)、[`src/pages/trust-flows.test.tsx`](../../src/pages/trust-flows.test.tsx)、[`src/lib/storage-analytics.test.ts`](../../src/lib/storage-analytics.test.ts)、[`src/lib/trust-review.test.ts`](../../src/lib/trust-review.test.ts) 與 Rust tests，將 JS / Rust quality surface 拉回 100% coverage，並補上 worker bridge coverage for新 command surface
  - 同步回寫 [`docs/features/archive.md`](../features/archive.md)、[`docs/architecture/desktop-command-surface.md`](../architecture/desktop-command-surface.md)、[`docs/plan/program/research-and-decisions.md`](program/research-and-decisions.md)、[`docs/plan/m1-solid-archive/schema-backup-and-ledger.md`](m1-solid-archive/schema-backup-and-ledger.md)、[`docs/plan/m1-solid-archive/schedule-security-and-storage.md`](m1-solid-archive/schedule-security-and-storage.md)、[`docs/plan/m1-solid-archive/README.md`](m1-solid-archive/README.md)、[`docs/plan/README.md`](README.md) 與 [`docs/plan/STATUS.md`](STATUS.md)，把 M1 recoverability contract 的 shipping / manual-first boundary 寫回 source docs
  - 驗收：`bun run verify`

- [x] **WORK-M4-J** — Explorer Recall Stabilization, Import Trust UX, And Desktop Feel
  - 2026-04-09：更新 [`src/pages/explorer/index.tsx`](../../src/pages/explorer/index.tsx)、[`src/lib/backend.ts`](../../src/lib/backend.ts)、[`src/lib/types.d.ts`](../../src/lib/types.d.ts)、[`src/pages/import/index.tsx`](../../src/pages/import/index.tsx)、[`src/pages/audit/index.tsx`](../../src/pages/audit/index.tsx)、[`src/styles/app.css`](../../src/styles/app.css)、[`src/index.css`](../../src/index.css) 與 [`src/lib/i18n/catalog.ts`](../../src/lib/i18n/catalog.ts)，正式交付 Explorer 第一頁 / 最後一頁 / 指定頁跳轉、翻頁保留滾動位置、topbar 搜索不再把 app 打掛、Import workflow 預設折疊說明 + detected profiles + native picker、run-centric Audit review，以及 shell desktop feel / collapsed sidebar 修復
  - 更新 [`src-tauri/crates/vault-core/src/models.rs`](../../src-tauri/crates/vault-core/src/models.rs)、[`src-tauri/crates/vault-core/src/archive/mod.rs`](../../src-tauri/crates/vault-core/src/archive/mod.rs)、[`src-tauri/crates/vault-core/src/ai.rs`](../../src-tauri/crates/vault-core/src/ai.rs)、[`src-tauri/src/lib.rs`](../../src-tauri/src/lib.rs) 與 [`src-tauri/Cargo.toml`](../../src-tauri/Cargo.toml)，把 History query response 補成 page-aware contract、接上 Tauri dialog plugin 與 import deep-link / audit run detail 所需的 desktop bridge；同步新增 [`src/pages/trust-flows.test.tsx`](../../src/pages/trust-flows.test.tsx) / [`src/lib/backend.test.ts`](../../src/lib/backend.test.ts) / [`tests/e2e/shell.spec.ts`](../../tests/e2e/shell.spec.ts) 的 regression coverage，補齊 page cursor edge case 與新的 Audit heading 斷言
  - 同步回寫 [`docs/design/screens-and-nav.md`](../design/screens-and-nav.md)、[`docs/features/recall.md`](../features/recall.md) 與 [`docs/plan/m4-full-polish/README.md`](m4-full-polish/README.md)，把 Explorer page deep-link、Import batch review deep-link、Audit 從 manifest-centric 改為 run-centric 的 source-of-truth 寫回文檔
  - 驗收：`bun run verify`

- [x] **WORK-M4-I** — Deterministic Insights, Retention Honesty, And Site Adapters
  - 2026-04-09：新增 [`src-tauri/crates/vault-core/src/browser_retention.rs`](../../src-tauri/crates/vault-core/src/browser_retention.rs) 與 [`src-tauri/crates/vault-core/src/insights/site_adapters.rs`](../../src-tauri/crates/vault-core/src/insights/site_adapters.rs)，並更新 [`src-tauri/crates/vault-core/src/chrome.rs`](../../src-tauri/crates/vault-core/src/chrome.rs)、[`src-tauri/crates/vault-core/src/models.rs`](../../src-tauri/crates/vault-core/src/models.rs)、[`src-tauri/crates/vault-core/src/insights.rs`](../../src-tauri/crates/vault-core/src/insights.rs) 與對應 Rust tests，正式補齊 browser-retention boundary、Chromium query ladders，以及 `readable-content-refetch` 內建的 YouTube / Vimeo video metadata adapters
  - 更新 [`src/lib/browser-retention.ts`](../../src/lib/browser-retention.ts)、[`src/lib/types.d.ts`](../../src/lib/types.d.ts)、[`src/lib/backend.ts`](../../src/lib/backend.ts)、[`src/pages/insights/index.tsx`](../../src/pages/insights/index.tsx)、[`src/pages/dashboard/index.tsx`](../../src/pages/dashboard/index.tsx)、[`src/pages/onboarding/index.tsx`](../../src/pages/onboarding/index.tsx)、[`src/app/shell-data.test.tsx`](../../src/app/shell-data.test.tsx)、[`src/pages/intelligence-surfaces.test.tsx`](../../src/pages/intelligence-surfaces.test.tsx) 與 [`src/lib/i18n/catalog.ts`](../../src/lib/i18n/catalog.ts)，把 deterministic query-evolution surface、browser-retention honesty 與 preview fixtures / coverage 一起收斂成 shipping UI contract
  - 同步回寫 [`docs/features/archive.md`](../features/archive.md)、[`docs/features/intelligence.md`](../features/intelligence.md)、[`docs/design/screens-and-nav.md`](../design/screens-and-nav.md)、[`docs/plan/program/research-and-decisions.md`](program/research-and-decisions.md)、[`docs/plan/m4-full-polish/enrichment-advanced-intelligence-and-remote.md`](m4-full-polish/enrichment-advanced-intelligence-and-remote.md)、[`docs/plan/m4-full-polish/README.md`](m4-full-polish/README.md)、[`docs/plan/STATUS.md`](STATUS.md) 與 [`docs/plan/BACKLOG.md`](BACKLOG.md)
  - 驗收：`bun run verify`

- [x] **WORK-M4-K** — Security, Privacy, And Update Boundary
  - 2026-04-10：新增 [`ADR-007`](../architecture/decisions/007-macos-biometric-session-unlock.md)，先以 source docs 誠實重開 macOS Touch ID 的 accepted-contract change，再落地真正的 shipping boundary
  - 更新 [`src-tauri/crates/vault-platform/src/biometric.rs`](../../src-tauri/crates/vault-platform/src/biometric.rs)、[`src-tauri/crates/vault-core/src/app_lock.rs`](../../src-tauri/crates/vault-core/src/app_lock.rs)、[`src-tauri/crates/vault-worker/src/lib.rs`](../../src-tauri/crates/vault-worker/src/lib.rs)、[`src-tauri/crates/vault-core/src/models.rs`](../../src-tauri/crates/vault-core/src/models.rs) 與 [`src/lib/types.d.ts`](../../src/lib/types.d.ts)，讓 macOS 真正接上 Touch ID session unlock，同時保留 passcode-first / session-only / truthful refusal 與 Windows / Linux unsupported path
  - 新增 [`src/lib/update.ts`](../../src/lib/update.ts) 與相關 tests，並更新 [`src/pages/settings/index.tsx`](../../src/pages/settings/index.tsx)、[`src/pages/lock/index.tsx`](../../src/pages/lock/index.tsx)、[`src/app/shell.tsx`](../../src/app/shell.tsx)、[`src/lib/i18n/catalog.ts`](../../src/lib/i18n/catalog.ts) 與 preview backend，正式補上 manual update review / download / install / restart，以及 Touch ID truthful copy
  - 同步回寫 [`README.md`](../../README.md)、[`RELEASE.md`](../../RELEASE.md)、[`docs/vision-and-requirements.md`](../vision-and-requirements.md)、[`docs/standards.md`](../standards.md)、[`docs/features/archive.md`](../features/archive.md)、[`docs/design/screens-and-nav.md`](../design/screens-and-nav.md)、[`docs/architecture/decisions/005-app-lock-session-boundary.md`](../architecture/decisions/005-app-lock-session-boundary.md)、[`docs/plan/program/research-and-decisions.md`](program/research-and-decisions.md) 與 [`docs/plan/m4-full-polish/README.md`](m4-full-polish/README.md)
  - 驗收：`bun run verify`

- [x] **WORK-M4-L** — Package Rename, Release Flow, Size Audit, And Code Health
  - 2026-04-10：更新 [`src-tauri/tauri.conf.json`](../../src-tauri/tauri.conf.json)、[`src-tauri/crates/vault-core/src/config.rs`](../../src-tauri/crates/vault-core/src/config.rs)、[`src-tauri/crates/vault-platform/src/lib.rs`](../../src-tauri/crates/vault-platform/src/lib.rs)、[`src/lib/backend.ts`](../../src/lib/backend.ts) 與相關 tests，正式把 bundle / keyring / scheduler / data-root namespace 統一切成 `com.yi-ting.pathkeep`，且不再保留 `dev.codex.pathkeep` 自動兼容路徑
  - 更新 [release workflow](../../.github/workflows/release.yml)、[`package.json`](../../package.json)、[`scripts/bump-version.mjs`](../../scripts/bump-version.mjs)、[`scripts/build-release-size-audit.mjs`](../../scripts/build-release-size-audit.mjs) 與 [`RELEASE.md`](../../RELEASE.md)，讓 updater `latest.json` / signatures / checksums、single-script version bump 與 size attribution 成為正式 release contract
  - 新增 [`docs/plan/m4-full-polish/release-size-audit.md`](m4-full-polish/release-size-audit.md) 與 [`docs/plan/m4-full-polish/code-health-audit.md`](m4-full-polish/code-health-audit.md)，並同步回寫 [`docs/plan/program/research-and-decisions.md`](program/research-and-decisions.md)、[`docs/plan/program/repo-baseline.md`](program/repo-baseline.md)、[`docs/plan/m4-full-polish/platform-release-and-polish.md`](m4-full-polish/platform-release-and-polish.md)、[`docs/plan/m4-full-polish/release-readiness-runbook.md`](m4-full-polish/release-readiness-runbook.md)、[`docs/plan/m4-full-polish/README.md`](m4-full-polish/README.md)、[`docs/plan/README.md`](README.md)、[`docs/plan/BACKLOG.md`](BACKLOG.md) 與 [`docs/plan/STATUS.md`](STATUS.md)，把 M4 remaining truth 明確收口並移交到 M5
  - 驗收：`bun run verify`

- [x] **WORK-M5-A** — Deterministic Evidence Contract, Foundation, And Taxonomy
  - 2026-04-10：完成 [`vault-core::deterministic`](../../src-tauri/crates/vault-core/src/deterministic.rs) 的 URL normalization、registrable-domain / search-parser baseline、script-aware tokenization、taxonomy v2 precedence、China Mainland / US core packs、user override 與 persisted evidence / taxonomy trace，並讓 deterministic importance 正式脫離 `duration_ms`
  - 收斂 first-party-only enrichment runtime：更新 [`src-tauri/crates/vault-core/src/models.rs`](../../src-tauri/crates/vault-core/src/models.rs)、[`src-tauri/crates/vault-core/src/intelligence_runtime.rs`](../../src-tauri/crates/vault-core/src/intelligence_runtime.rs)、[`src/lib/enrichment.ts`](../../src/lib/enrichment.ts)、[`src/lib/intelligence-runtime.ts`](../../src/lib/intelligence-runtime.ts) 與 [`src/lib/backend.ts`](../../src/lib/backend.ts)，正式補齊 dual built-in plugin defaults（`title-normalization` + `readable-content-refetch`）、runtime queue review、retry / cancel state guard，以及可在 sandbox 內重現的 RustSec advisory DB audit path
  - 更新 [`src/pages/settings/index.tsx`](../../src/pages/settings/index.tsx)、[`src/pages/insights/index.tsx`](../../src/pages/insights/index.tsx)、[`src/pages/intelligence-surfaces.test.tsx`](../../src/pages/intelligence-surfaces.test.tsx)、[`src/app/index.test.tsx`](../../src/app/index.test.tsx)、[`src/lib/backend.test.ts`](../../src/lib/backend.test.ts) 與對應 Rust tests，讓 Settings / Insights 正式顯示 plugin boundary、queue state、recent runtime jobs、retry / cancel controls 與 degrade copy
  - 同步回寫 [`docs/features/intelligence.md`](../features/intelligence.md)、[`docs/features/deterministic-intelligence.md`](../features/deterministic-intelligence.md)、[`docs/architecture/data-model.md`](../architecture/data-model.md)、[`docs/architecture/module-boundary-map.md`](../architecture/module-boundary-map.md)、[`docs/plan/program/research-and-decisions.md`](program/research-and-decisions.md)、[`docs/plan/m5-runtime-and-extensions/enrichment-runtime-and-operations.md`](m5-runtime-and-extensions/enrichment-runtime-and-operations.md)、[`docs/plan/m5-deterministic-intelligence/foundation-and-taxonomy.md`](m5-deterministic-intelligence/foundation-and-taxonomy.md)、[`docs/plan/m5-deterministic-intelligence/README.md`](m5-deterministic-intelligence/README.md)、[`docs/plan/STATUS.md`](STATUS.md) 與 [`docs/plan/BACKLOG.md`](BACKLOG.md)，並把下一個 active block 切到 `WORK-M5-B`
  - 驗收：`bun run check`、`bun run build`

- [x] **WORK-QC-G** — Platform Native Automation, Updater Desktop Boundary, And Platform Layer Refactor
  - 2026-04-10：將 [`vault-platform`](../../src-tauri/crates/vault-platform/src/lib.rs) 拆成 [`keyring.rs`](../../src-tauri/crates/vault-platform/src/keyring.rs)、[`scheduler.rs`](../../src-tauri/crates/vault-platform/src/scheduler.rs)、[`launcher.rs`](../../src-tauri/crates/vault-platform/src/launcher.rs)、[`host_capability.rs`](../../src-tauri/crates/vault-platform/src/host_capability.rs)、[`discovery.rs`](../../src-tauri/crates/vault-platform/src/discovery.rs) 與 [`test_support.rs`](../../src-tauri/crates/vault-platform/src/test_support.rs)，把 keyring service / scheduler label / launch agent path 的 test override 正式收斂成可重用 harness
  - 新增 [`native_host.rs`](../../src-tauri/crates/vault-platform/tests/native_host.rs) 與對應 scripts / CI：`bun run test:platform:rust` 會依 host 跑 native keyring、launchctl / systemd / schtasks scheduler 驗證、launcher PATH shim、browser discovery smoke 與 biometric capability smoke；`bun run test:platform:desktop` 會跑 debug desktop build、updater / launcher desktop slice 與 updater progress JS tests；`bun run check` 現在固定納入 [`check:platform`](../../package.json)
  - 新增 [`src-tauri/src/updater.rs`](../../src-tauri/src/updater.rs) 與 updater IPC types，把 updater review / download / install / relaunch 收回 typed desktop command surface；前端 [`src/lib/update.ts`](../../src/lib/update.ts) 與 [`src/lib/ipc/updater-progress.ts`](../../src/lib/ipc/updater-progress.ts) 改為走 `check_for_app_update`、`download_and_install_app_update`、`relaunch_after_update` 和 `pathkeep://updater-progress` event，不再直接調用 `@tauri-apps/plugin-updater` / `@tauri-apps/plugin-process`
  - 同步回寫 [`docs/plan/program/quality-matrix.md`](program/quality-matrix.md)、[`docs/architecture/desktop-command-surface.md`](../architecture/desktop-command-surface.md)、[`docs/architecture/module-boundary-map.md`](../architecture/module-boundary-map.md)、[`docs/plan/program/research-and-decisions.md`](program/research-and-decisions.md) 與 [`docs/plan/STATUS.md`](STATUS.md)，並把 GitHub CI 補成 `platform-macos`、`platform-linux`、`platform-windows` matrix
  - 驗收：`bun run check`、`bun run build`

- [x] **WORK-QC-E** — Backend Binary Size Audit And macOS Release Slim-Down
  - 2026-04-10：審核 `src-tauri/target/release/pathkeep-desktop` 為何膨脹到 `190M`，確認問題不在前端 bundle，而在 Rust desktop binary 本體；Mach-O pre-fix 主要由巨大 `__TEXT` / `__LINKEDIT`、optional intelligence stack，以及不必要的 macOS keyring transitive dependency 一起撐大
  - 更新 [`src-tauri/Cargo.toml`](../../src-tauri/Cargo.toml)、[`src-tauri/crates/vault-platform/Cargo.toml`](../../src-tauri/crates/vault-platform/Cargo.toml) 與 [`src-tauri/crates/vault-platform/src/lib.rs`](../../src-tauri/crates/vault-platform/src/lib.rs)，把 macOS keyring wiring 改成直接走 `apple-native-keyring-store`、Windows / Linux / FreeBSD 則各自接 native store crate，移除 umbrella `keyring` 帶來的 `db-keystore` / `turso*` baggage；同時開啟 `strip = "symbols"`、`lto = "thin"`、`codegen-units = 1`
  - 刪除已不在 crate graph 的 [`src-tauri/vendor/cfg_block`](../../src-tauri/vendor/cfg_block) patch/vendor dead code，避免之後的 supply-chain / size audit 再被無效 patch 噪音污染
  - 同步回寫 [`docs/architecture/tech-stack.md`](../architecture/tech-stack.md)、[`docs/plan/m4-full-polish/release-size-audit.md`](m4-full-polish/release-size-audit.md)、[`docs/plan/m4-full-polish/code-health-audit.md`](m4-full-polish/code-health-audit.md)、[`docs/plan/STATUS.md`](STATUS.md) 與 [`docs/plan/BACKLOG.md`](BACKLOG.md)，把這輪 size attribution、remaining dominant crates 與後續 blocked boundary work 寫回 source docs
  - 結果：`cargo build --release --bin pathkeep-desktop` 後的 release executable 從 `190M` 降到 `104M`
  - 驗收：`cargo build --release --bin pathkeep-desktop`、`bun run check`、`bun run build`

- [x] **WORK-M5-B** — Query Groups, Threads, Reference Pages, And Module Registry
  - 讀先：
    `docs/features/deterministic-intelligence.md`
    `docs/plan/m5-deterministic-intelligence/README.md`
    `docs/plan/m5-deterministic-intelligence/groups-threads-and-surfaces.md`
    `docs/plan/m4-full-polish/intelligence-60-year-envelope.md`
    `docs/design/screens-and-nav.md`
  - 目標：把 query groups、query ladders、cross-day thread merge、open loops、source effectiveness、reference pages 與 deterministic module registry 做成 explainable、可重建、可 profile-scope、可 invalidate 的正式 shipping surface。
  - 併入的 M4 deferred work：longer-horizon topic timeline / periodic summary、returning topics / session-pattern families，以及其對應 reference-page / thread surfaces。
  - 2026-04-10：新增 [`src-tauri/crates/vault-core/src/insights/m5b.rs`](../../src-tauri/crates/vault-core/src/insights/m5b.rs)，並更新 [`src-tauri/crates/vault-core/src/insights.rs`](../../src-tauri/crates/vault-core/src/insights.rs)、[`src-tauri/crates/vault-core/src/intelligence_runtime.rs`](../../src-tauri/crates/vault-core/src/intelligence_runtime.rs)、[`src-tauri/crates/vault-core/src/models.rs`](../../src-tauri/crates/vault-core/src/models.rs) 與 [`src-tauri/crates/vault-core/src/archive/mod.rs`](../../src-tauri/crates/vault-core/src/archive/mod.rs)，把 deterministic pipeline 正式改成 `visit features -> burst -> query group -> thread -> reference/source/summaries`，補上 `burst_id` / `query_group_id`、query-group / reference-page / source-effectiveness tables、deterministic module registry trace，以及 visibility / clear-derived stale honesty。
  - 更新 [`src/lib/types.d.ts`](../../src/lib/types.d.ts) 與 [`src/lib/backend.ts`](../../src/lib/backend.ts)，把 additive desktop payload / browser-preview fixture 補到 `AppConfig.deterministic.modules`、`InsightSnapshot.queryGroups / referencePages / sourceEffectiveness / templateSummaries`、thread / ladder confidence 欄位、runtime module status，以及 new clear / rebuild report counts。
  - 更新 [`src/pages/insights/index.tsx`](../../src/pages/insights/index.tsx)、[`src/pages/settings/index.tsx`](../../src/pages/settings/index.tsx)、[`src/lib/intelligence-runtime.ts`](../../src/lib/intelligence-runtime.ts)、[`src/lib/insight-canonical.ts`](../../src/lib/insight-canonical.ts)、[`src/lib/i18n/catalog.ts`](../../src/lib/i18n/catalog.ts)、[`src/pages/intelligence-surfaces.test.tsx`](../../src/pages/intelligence-surfaces.test.tsx) 與 [`src/lib/insight-canonical.test.ts`](../../src/lib/insight-canonical.test.ts)，讓 query groups、reference pages、source effectiveness、template summaries 與 deterministic module registry 成為正式 UI / i18n / browser-preview / explainability contract。
  - 同步回寫 [`docs/features/deterministic-intelligence.md`](../features/deterministic-intelligence.md)、[`docs/architecture/data-model.md`](../architecture/data-model.md)、[`docs/design/screens-and-nav.md`](../design/screens-and-nav.md)、[`docs/plan/m5-deterministic-intelligence/README.md`](m5-deterministic-intelligence/README.md)、[`docs/plan/README.md`](README.md) 與 [`docs/plan/STATUS.md`](STATUS.md)，把 M5-B closeout、new derived tables / module trace、Settings / Insights shipping surface 與目前剩餘 blocked state 寫回 source docs。
  - 驗收：`cargo test -p vault-core insights::`、`cargo test -p vault-core doctor_repair_restores_missing_import_artifacts_visibility_and_derived_state`、`bun x tsc --noEmit`、`bun x vitest run src/pages/intelligence-surfaces.test.tsx src/lib/insight-canonical.test.ts`、`bun run check`、`bun run build`

- [x] **WORK-QC-F** — Optional Intelligence Runtime Boundary And Bundle Size Follow-Up
  - 讀先：
    `docs/architecture/tech-stack.md`
    `docs/features/intelligence.md`
    `docs/features/deterministic-intelligence.md`
    `docs/plan/m4-full-polish/release-size-audit.md`
    `docs/plan/m4-full-polish/code-health-audit.md`
  - 目標：釐清 `lancedb` / `lance` / `datafusion` / `rig-core` 這條 optional intelligence stack 是否應繼續和 archive / shell-critical desktop runtime 同 binary shipping，或改成可選 sidecar / helper / feature-gated build boundary。
  - 驗收：產出有 trade-off 的設計決策與可重跑的 size / packaging evidence；若涉及改變 default shipping surface，必須先取得使用者明確 sign-off。
  - 2026-04-10：使用者明確 sign off 保留 default desktop build 內建 optional AI / MCP / semantic runtime；新增 [ADR-009](../architecture/decisions/009-default-desktop-optional-intelligence-shipping.md)，把「optional = disabled-by-default，而不是 packaging-gated」正式凍結成 source-of-truth。
  - 同步回寫 [`docs/architecture/decisions/README.md`](../architecture/decisions/README.md)、[`docs/architecture/tech-stack.md`](../architecture/tech-stack.md)、[`docs/features/intelligence.md`](../features/intelligence.md)、[`docs/plan/program/research-and-decisions.md`](program/research-and-decisions.md)、[`docs/plan/m4-full-polish/release-size-audit.md`](m4-full-polish/release-size-audit.md)、[`docs/plan/m4-full-polish/code-health-audit.md`](m4-full-polish/code-health-audit.md)、[`docs/plan/README.md`](README.md)、[`docs/plan/STATUS.md`](STATUS.md) 與 [`docs/plan/BACKLOG.md`](BACKLOG.md)，把 blocked state、accepted trade-off 與 current truth 一起收口。
  - 2026-04-11 UTC：重跑 `bun run release:size-audit`，生成 [`artifacts/release/2026-04-11-size-audit/summary.md`](../../artifacts/release/2026-04-11-size-audit/summary.md) 與 [`artifacts/release/2026-04-11-size-audit/size-attribution.json`](../../artifacts/release/2026-04-11-size-audit/size-attribution.json)；結果顯示 web payload `903585` bytes、base shell entry `387414` bytes、`src/pages/settings/index.tsx` route chunk `63696` bytes，而 unsigned macOS executable `macos/PathKeep.app/Contents/MacOS/pathkeep-desktop` 約 `109198880` bytes（約 `104 MiB`）。這個重量現在屬 accepted trade-off，不再作為 active blocker。
  - 驗收：`bun run release:size-audit`

- [x] **WORK-QC-I** — Trust / Recoverability Review Sweep
  - 2026-04-10：更新 [`src-tauri/crates/vault-core/src/remote.rs`](../../src-tauri/crates/vault-core/src/remote.rs)、[`docs/features/archive.md`](../features/archive.md)、[`docs/architecture/data-model.md`](../architecture/data-model.md) 與 [`docs/plan/program/research-and-decisions.md`](program/research-and-decisions.md)，讓 `pathkeep.remote-backup.v1` bundle 補上 `metadata/bundle-manifest.sha256`、Verify 檢查實際 zip entry set 與 detached manifest checksum，並把這條 integrity story 明確定義為 corruption / drift detection，而不是 remote authenticity attestation。
  - 更新 [`src-tauri/crates/vault-core/src/app_lock.rs`](../../src-tauri/crates/vault-core/src/app_lock.rs)、[`src-tauri/crates/vault-core/src/archive/mod.rs`](../../src-tauri/crates/vault-core/src/archive/mod.rs)、[`src-tauri/crates/vault-core/src/takeout.rs`](../../src-tauri/crates/vault-core/src/takeout.rs)、[`src-tauri/crates/vault-core/src/diagnostics.rs`](../../src-tauri/crates/vault-core/src/diagnostics.rs)、[`src-tauri/crates/vault-worker/src/lib.rs`](../../src-tauri/crates/vault-worker/src/lib.rs)、[`src-tauri/crates/browser-history-parser/src/firefox/mod.rs`](../../src-tauri/crates/browser-history-parser/src/firefox/mod.rs) 與 [`src-tauri/crates/browser-history-parser/src/safari/mod.rs`](../../src-tauri/crates/browser-history-parser/src/safari/mod.rs)，補上 biometric-enabled enforcement、passcode removal persist-before-clear、locked Security 仍保留 latest rekey review、post-swap rekey audit trail、shared import audit-artifact repair、malformed crash report non-fatal、temp-file crash report writes，以及 Firefox / Safari equal-watermark reread + dedupe correctness。
  - 更新 [`src/lib/backend.ts`](../../src/lib/backend.ts)、[`src/pages/lock/index.tsx`](../../src/pages/lock/index.tsx)、[`src/pages/audit/index.tsx`](../../src/pages/audit/index.tsx)、[`src/pages/import/index.tsx`](../../src/pages/import/index.tsx)、[`src/app/index.test.tsx`](../../src/app/index.test.tsx) 與 [`src/pages/trust-flows.test.tsx`](../../src/pages/trust-flows.test.tsx)，讓 preview trust surfaces 對齊新的 App Lock truth、Audit per-run cache degradation，以及 Import preview failure 時的 stale-detail cleanup / committed-result preservation。
  - 同步回寫 [`docs/design/screens-and-nav.md`](../design/screens-and-nav.md) 與 [`docs/plan/STATUS.md`](STATUS.md)，把 remote verify / recoverability / trust UI 的 shipping truth 收口。
  - 驗收：`bun run check`、`bun run build`

- [x] **WORK-QC-J** — Intelligence / Release Hardening Review Sweep
  - 2026-04-10：更新 [`src/pages/insights/index.tsx`](../../src/pages/insights/index.tsx)、[`src-tauri/crates/vault-core/src/intelligence_runtime.rs`](../../src-tauri/crates/vault-core/src/intelligence_runtime.rs)、[`src-tauri/crates/vault-core/src/insights.rs`](../../src-tauri/crates/vault-core/src/insights.rs) 與 [`src-tauri/crates/vault-core/src/insights/surfaces.rs`](../../src-tauri/crates/vault-core/src/insights/surfaces.rs)，讓 Insights 在 scope / refresh / explain failure 時清空 stale state、所有 drilldown 保留 `profileId`、enrichment queue 繼續掃描直到找到可執行 jobs、derived clear / rebuild 與 deterministic module runtime 同 transaction 落地，並把 source-effectiveness domain key 對齊 registrable-domain normalization。
  - 更新 [`src/lib/update.ts`](../../src/lib/update.ts)、[`src/lib/ipc/bridge.ts`](../../src/lib/ipc/bridge.ts)、[`src/pages/settings/index.tsx`](../../src/pages/settings/index.tsx)、[`tests/e2e/desktop-bridge.spec.ts`](../../tests/e2e/desktop-bridge.spec.ts) 與 [`scripts/build-release-size-audit.mjs`](../../scripts/build-release-size-audit.mjs)，讓 `browser-desktop-bridge` 可透過 typed desktop command transport 驗證 updater install / relaunch、Settings updater panel 不再卡在 `checking`、typed IPC bridge 會回傳 PathKeep-specific unreachable errors，且 release size audit 改為 manifest-graph walk + shared asset dedupe + full timestamped artifact directories。
  - 新增 / 擴寫 [`src/pages/intelligence-surfaces.test.tsx`](../../src/pages/intelligence-surfaces.test.tsx)、[`src/lib/update.test.ts`](../../src/lib/update.test.ts)、[`src/lib/ipc/bridge.test.ts`](../../src/lib/ipc/bridge.test.ts)、[`src/app/index.test.tsx`](../../src/app/index.test.tsx)、[`scripts/build-release-size-audit.test.ts`](../../scripts/build-release-size-audit.test.ts) 與相關 Rust tests，把 scoped stale-state、bridge updater parity、bridge failure shaping、release size provenance、unknown backup phases / punctuation-only FTS / parser cursor boundary 等 residual review findings 收回 regression surface。
  - 同步回寫 [`docs/features/intelligence.md`](../features/intelligence.md)、[`docs/architecture/desktop-command-surface.md`](../architecture/desktop-command-surface.md)、[`docs/plan/program/quality-matrix.md`](program/quality-matrix.md)、[`docs/plan/m4-full-polish/release-size-audit.md`](m4-full-polish/release-size-audit.md)、[`docs/plan/program/research-and-decisions.md`](program/research-and-decisions.md) 與 [`docs/plan/STATUS.md`](STATUS.md)，把 scoped-view truth、bridge updater boundary 與 release evidence contract 寫回 source docs。
  - 驗收：`bun run check`、`bun run build`

- [x] **WORK-QC-K** — Frontend Doc-Comment Map And Maintainability Sweep
  - 讀先：
    `docs/plan/m0-foundation/frontend-shell-and-design-system.md`
    `docs/plan/m4-full-polish/code-health-audit.md`
    `docs/design/screens-and-nav.md`
    `docs/design/ux-principles.md`
    `docs/design/design-tokens.md`
  - 2026-04-11：為活躍前端 `src/` surface 補上 file header 與 declaration-level doc comments，讓 route / shell / primitive / helper / typed contract files 一打開就能看懂職責、主要宣告與對應的 design / trust source of truth。
  - 針對 hot spots 做實際維護性修補：新增 [`src/pages/settings/helpers.ts`](../../src/pages/settings/helpers.ts) 與 [`src/pages/settings/helpers.test.ts`](../../src/pages/settings/helpers.test.ts)，把 Settings 的 AI draft / retention-selection 純 helper 抽離主 route，並修正 retention preview merge 對新 bucket 的預設選取行為；同時刪除 stale duplicate [`src/lib/i18n/messages.ts`](../../src/lib/i18n/messages.ts)。
  - 更新 [`scripts/check-rust-security.mjs`](../../scripts/check-rust-security.mjs) 與 [`deny.toml`](../../deny.toml)，把新出現的 transitive `RUSTSEC-2026-0097` `rand` advisory 納入 allowlist 並寫下依賴鏈 / owned-surface rationale，讓供應鏈 gate 與目前 repo truth 對齊。
  - 同步回寫 [`docs/plan/m4-full-polish/code-health-audit.md`](m4-full-polish/code-health-audit.md)、[`docs/plan/STATUS.md`](STATUS.md) 與本檔，明確標出 frontend documentation closeout、Settings helper extraction 與 remaining large-file hotspots 的現況。
  - 驗收：`bun run check`、`bun run build`

- [x] **WORK-QC-L** — Intelligence Recovery And Desktop Truth Gate
  - 2026-04-12：更新 [`src/pages/jobs/index.tsx`](../../src/pages/jobs/index.tsx)、[`src/pages/insights/index.tsx`](../../src/pages/insights/index.tsx)、[`src/lib/intelligence-presentation.ts`](../../src/lib/intelligence-presentation.ts)、[`src/lib/i18n/catalog.ts`](../../src/lib/i18n/catalog.ts)、[`src/styles/app.css`](../../src/styles/app.css) 與 [`src-tauri/crates/vault-core/src/insights.rs`](../../src-tauri/crates/vault-core/src/insights.rs)，把 Jobs / Insights 重做成 truthful runtime review surface：deferred backlog、不需人工介入的 queue 漂移、`unsupported-content` / `fetch-error` 等內容抓取狀態，以及 deterministic snapshot / evidence cards 現在都用真實 queue 與 archive runtime 誠實呈現，不再把 backlog 假裝成整條功能壞掉。
  - 補上 [`docs/plan/manual-jobs-insights-truth-pass.md`](manual-jobs-insights-truth-pass.md) 的完整手動驗收路線與 2026-04-12 實跑結果：以真實 `browser-desktop-bridge` / 本機 archive 驗證 Jobs、Insights、explainability 與 Explorer deep-link；同時保存 `artifacts/jobs-page-real-data.png`、`artifacts/insights-page-real-data.png` 作為真實 surface evidence。
  - 更新 [`.github/workflows/ci.yml`](../../.github/workflows/ci.yml)、[`.github/workflows/platform-native.yml`](../../.github/workflows/platform-native.yml)、[`docs/standards.md`](../standards.md)、[`docs/plan/program/quality-matrix.md`](program/quality-matrix.md) 與 [`docs/plan/program/research-and-decisions.md`](program/research-and-decisions.md)，把 hosted-runner platform-native / desktop bridge truth gate 收斂到 manual workflow，避免每次 push / branch commit 都燒掉昂貴且 host-sensitive 的 GitHub Actions 分鐘。
  - 2026-04-12 follow-up：更新 [`playwright.desktop-bridge.config.ts`](../../playwright.desktop-bridge.config.ts) 與 [`tests/e2e/desktop-bridge.spec.ts`](../../tests/e2e/desktop-bridge.spec.ts)，修補 Playwright multi-process fixture drift、fresh temporary `CARGO_TARGET_DIR` 冷啟重編、stale bridge port collision，以及過度脆弱的 desktop-bridge assertions；desktop truth gate 現在會重用 fixture env / repo-local build cache，並以真實 fixture root / live backup+insights flow 作為穩定斷言。
  - 同步回寫 [`docs/features/intelligence.md`](../features/intelligence.md)、[`docs/design/screens-and-nav.md`](../design/screens-and-nav.md)、[`docs/plan/manual-jobs-insights-truth-pass.md`](manual-jobs-insights-truth-pass.md)、[`docs/plan/program/quality-matrix.md`](program/quality-matrix.md)、[`docs/plan/program/research-and-decisions.md`](program/research-and-decisions.md)、[`docs/plan/STATUS.md`](STATUS.md) 與本檔，讓 intelligence recovery、desktop truth gate 與 CI cost boundary 的 source-of-truth 回到一致。
  - 驗收：`bunx vitest run src/lib/intelligence-presentation.test.ts src/pages/intelligence-surfaces.test.tsx`、`cargo test --manifest-path src-tauri/Cargo.toml -p vault-core enrichment_failure_message_turns_known_fetch_states_into_honest_copy`、`bun run test:e2e:desktop-bridge:truth`（修補後連續兩次通過）、`bun run check`、`bun run build`

- [x] **WORK-QC-P** — Browser Support Truth Pass And Adapter Playbook
  - 2026-04-13：新增 [`docs/architecture/browser-support-and-adapter-playbook.md`](../architecture/browser-support-and-adapter-playbook.md)，正式定義 browser support taxonomy、adapter promotion gate、discovery / parser / ingest / UI / i18n / validation checklist，讓 README / onboarding / release docs 的 public promise 只跟著當前 validation evidence 走，而不是跟著最寬鬆的內部實作面積走。
  - 更新 [`README.md`](../../README.md)、[`docs/features/archive.md`](../features/archive.md)、[`TESTING.md`](../../TESTING.md)、[`RELEASE.md`](../../RELEASE.md)、[`docs/plan/m4-full-polish/release-readiness-runbook.md`](m4-full-polish/release-readiness-runbook.md)、[`docs/reference-review.md`](../reference-review.md)、[`docs/plan/program/research-and-decisions.md`](program/research-and-decisions.md)、[`docs/plan/m2-recall-and-trust/README.md`](m2-recall-and-trust/README.md) 與 [`docs/plan/m2-recall-and-trust/imports-browsers-and-rollback.md`](m2-recall-and-trust/imports-browsers-and-rollback.md)，把 current truth 收斂成：公開承諾只包含 `Google Chrome` 與 macOS `Safari` baseline；Firefox 與其他 Chromium / Firefox-family adapter 保留為 implemented-but-not-yet-publicly-promised。
  - 更新 [`src/lib/i18n/catalog.ts`](../../src/lib/i18n/catalog.ts)、[`src/lib/i18n.test.ts`](../../src/lib/i18n.test.ts) 與 [`src/app/index.test.tsx`](../../src/app/index.test.tsx)，移除 onboarding 的過度承諾文案，並補上三語 promise-copy regression，避免 UI 又把 Firefox / Edge / Brave 等寫回已 shipping promise。
  - 更新 [`src-tauri/crates/vault-core/src/archive/mod.rs`](../../src-tauri/crates/vault-core/src/archive/mod.rs)，補上不依賴 Firefox / Chrome fallback 的 Safari baseline backup acceptance，以及「選到 unreadable Safari 也不該阻擋 Chrome backup」的 regression；同時保留既有 `discover_safari_profile_marks_missing_history_without_hiding_the_profile` discovery truth。
  - 驗收：`bun x vitest run src/lib/i18n.test.ts src/app/index.test.tsx`、`cargo test --manifest-path src-tauri/Cargo.toml -p vault-core safari_backup_baseline_ingests_history_without_firefox_or_chrome_dependency`、`cargo test --manifest-path src-tauri/Cargo.toml -p vault-core backup_keeps_chrome_successful_when_selected_safari_is_unreadable`、`cargo test --manifest-path src-tauri/Cargo.toml -p vault-core discover_safari_profile_marks_missing_history_without_hiding_the_profile`、`bun run check`、`bun run build`

- [x] **WORK-QC-R** — Storage Plane Reset And Large-Archive Hard Rebase
  - 2026-04-14：把 archive/search/intelligence/sidecars 正式收斂成 4-layer storage plane。canonical archive 現在只保留 source-of-truth facts 與 immutable audit trace；`derived/history-search.sqlite` 承接 keyword recall / rollups；`derived/history-intelligence.sqlite` 承接 AI queue、assistant trace、deterministic runtime 與 compact semantic metadata；`sidecars/{semantic-index,intelligence-blobs}` 承接向量索引與 content-addressed readable-text blobs。
  - canonical archive 已移除 `raw_row_versions` hot-path persistence；來源證據改由 checkpoint / snapshot / manifest trace 承接。Takeout preview 也改為直接從 canonical import-batch facts 生成，不再依賴 per-row raw JSON 表。
  - intelligence plane 不再使用 temp compatibility views、runtime `ensure_*_column` patching、或 SQLite 向量 payload mirror；所有 intelligence / semantic read path 直接讀 attached `archive.*` canonical facts，向量 payload 僅存在 LanceDB sidecar，SQLite 只保留 compact semantic metadata / rebuild accounting。
  - 補上 rerunnable large-archive benchmark recipe，並生成 `artifacts/benchmarks/2026-04-14-storage-plane-reset/100k-60y.json` 與 `1m-60y.json`，作為 reset 後 deterministic rebuild / snapshot read 的 replayable artifact baseline。
  - 同步回寫 [`docs/architecture/data-model.md`](../architecture/data-model.md)、[`docs/architecture/tech-stack.md`](../architecture/tech-stack.md)、[`docs/features/archive.md`](../features/archive.md)、[`docs/features/intelligence.md`](../features/intelligence.md)、[`docs/features/intelligence-current-state.md`](../features/intelligence-current-state.md)、[`docs/plan/program/repo-baseline.md`](program/repo-baseline.md)、[`docs/plan/program/research-and-decisions.md`](program/research-and-decisions.md)、[`docs/plan/m4-full-polish/intelligence-60-year-envelope.md`](m4-full-polish/intelligence-60-year-envelope.md)、[`docs/architecture/decisions/010-storage-plane-reset.md`](../architecture/decisions/010-storage-plane-reset.md)、[`docs/plan/README.md`](README.md)、[`docs/plan/STATUS.md`](STATUS.md) 與 [`docs/plan/BACKLOG.md`](BACKLOG.md)，讓 storage reset 的 source-of-truth、benchmark recipe 與後續 work block 依賴回到一致。
  - 驗收：`cargo test --manifest-path src-tauri/Cargo.toml -p vault-core --lib --quiet`、`bun run check`、`bun run build`

- [x] **WORK-QC-S** — Source-Evidence Archive And Capability Contract Reset
  - 2026-04-14：新增 [`docs/architecture/decisions/011-source-evidence-archive-and-capability-contract.md`](../architecture/decisions/011-source-evidence-archive-and-capability-contract.md) 與 [`docs/dev/`](../dev/README.md) guides，正式把多瀏覽器 schema / evidence 保存 contract 收斂成 `source-preserving + capability-driven + hot/cold split`：archive plane 現在明確包含 hot canonical `archive/history-vault.sqlite` 與 cold `archive/source-evidence.sqlite`，feature enablement 以 capability snapshot 為主，browser/version metadata 退回 provenance / heuristics / debug。
  - 更新 [`docs/architecture/data-model.md`](../architecture/data-model.md)、[`docs/architecture/module-boundary-map.md`](../architecture/module-boundary-map.md)、[`docs/architecture/browser-support-and-adapter-playbook.md`](../architecture/browser-support-and-adapter-playbook.md)、[`docs/features/deterministic-intelligence.md`](../features/deterministic-intelligence.md)、[`docs/plan/program/research-and-decisions.md`](program/research-and-decisions.md) 與 ADR index，讓 source-evidence plane、source batches、capability family、field promotion 與 browser schema evolution procedure 成為正式 source-of-truth，而不是只留在聊天或實作註解。
  - `browser-history-parser` 現在會輸出 `SchemaObservation`、`CapabilitySnapshot`、typed evidence family 與 `NativeEntity`，並在 Chromium / Firefox / Safari baseline parser 中冷保存 base-table source rows；新增 [`src-tauri/crates/browser-history-parser/src/observation.rs`](../../src-tauri/crates/browser-history-parser/src/observation.rs) 與 developer examples [`source-report.rs`](../../src-tauri/crates/browser-history-parser/examples/source-report.rs) / [`source-diff.rs`](../../src-tauri/crates/browser-history-parser/examples/source-diff.rs)。
  - `vault-core` 新增 [`src-tauri/crates/vault-core/src/archive/source_evidence.rs`](../../src-tauri/crates/vault-core/src/archive/source_evidence.rs) 與 [`006_source_evidence_provenance.sql`](../../src-tauri/crates/vault-core/src/migrations/006_source_evidence_provenance.sql)，把 `source_profiles.browser_family/browser_product`、`profile_watermarks.last_source_batch_id`、`source_batches`、schema observation、typed evidence、`native_entities`、bundle inclusion與 restore verification 一起接上；新增 [`field-promotion-dry-run.rs`](../../src-tauri/crates/vault-core/examples/field-promotion-dry-run.rs) 作為第一版 promotion / re-extract tooling。
  - 前端 storage contract 也補上 `sourceEvidenceDatabaseBytes`，讓 storage analytics 不再把新的 cold plane 當成不存在。
  - 驗收：`cargo test --manifest-path src-tauri/Cargo.toml -p browser-history-parser --lib`、`cargo test --manifest-path src-tauri/Cargo.toml -p vault-core --lib`、`cargo build --manifest-path src-tauri/Cargo.toml -p browser-history-parser --example source-report --example source-diff`、`cargo build --manifest-path src-tauri/Cargo.toml -p vault-core --example field-promotion-dry-run`、`bun run check`、`bun run build`

- [x] **WORK-QC-N** — Backend Rustdoc Sweep And Module Decomposition
  - 2026-04-14：把 [`src-tauri/crates/vault-worker/src/lib.rs`](../../src-tauri/crates/vault-worker/src/lib.rs)、[`src-tauri/crates/vault-core/src/archive/mod.rs`](../../src-tauri/crates/vault-core/src/archive/mod.rs)、[`src-tauri/crates/vault-core/src/chrome.rs`](../../src-tauri/crates/vault-core/src/chrome.rs)、[`src-tauri/crates/vault-core/src/ai.rs`](../../src-tauri/crates/vault-core/src/ai.rs)、[`src-tauri/crates/vault-core/src/insights.rs`](../../src-tauri/crates/vault-core/src/insights.rs) 的巨大 regression suites 拆到 sibling `tests.rs` 模組，讓 runtime 主檔重新回到 orchestration / API boundary / helper contract 的可讀骨架。
  - 補齊 runtime-facing rustdoc：為 browser staging snapshot、AI provider runtime / integration preview、semantic index / assistant entrypoint、archive visible totals、insight enrichment helpers 等 boundary symbol 加上 declaration-level doc comments，讓 backend crate 不再需要靠 implementation dive 才理解責任。
  - 同步回寫 [`docs/architecture/module-boundary-map.md`](../architecture/module-boundary-map.md) 與 [`docs/plan/m4-full-polish/code-health-audit.md`](m4-full-polish/code-health-audit.md)，把「runtime 主檔 vs regression suite」的新邊界寫回 source docs，避免後續又把 test fixture / env setup 混回 mega-file。
  - 驗收：`cargo test --manifest-path src-tauri/Cargo.toml -p vault-core --lib`、`cargo test --manifest-path src-tauri/Cargo.toml -p vault-worker --lib`、`bun run check`、`bun run build`

- [x] **WORK-QC-T** — Core Intelligence Backend Reset And Contract Cutover
  - 2026-04-15：把 [`docs/features/core-intelligence-ultimate-design.md`](../features/core-intelligence-ultimate-design.md) 升為 accepted baseline，並同步標記 [`docs/features/deterministic-intelligence.md`](../features/deterministic-intelligence.md) 為 superseded、更新 [`docs/features/intelligence.md`](../features/intelligence.md)、[`docs/features/intelligence-current-state.md`](../features/intelligence-current-state.md)、[`docs/architecture/data-model.md`](../architecture/data-model.md)、[`docs/architecture/desktop-command-surface.md`](../architecture/desktop-command-surface.md)、[`docs/plan/program/research-and-decisions.md`](program/research-and-decisions.md)、[`docs/plan/m5-deterministic-intelligence/README.md`](m5-deterministic-intelligence/README.md) 與 [`docs/plan/README.md`](README.md)，把 Core Intelligence hard reset、new command surface 與 `PG-RD-AI-010` closeout 寫回 source-of-truth。
  - 新增 [`src-tauri/crates/vault-core/src/intelligence/`](../../src-tauri/crates/vault-core/src/intelligence/) 與 [`src-tauri/crates/vault-core/src/models/core_intelligence.rs`](../../src-tauri/crates/vault-core/src/models/core_intelligence.rs)，把 deterministic backend ownership 切到 `vault-core::intelligence`，建立 Core Intelligence schema（`visit_derived_facts`、daily rollups、`sessions`、`search_trails`、`search_events`、`query_families`、`refind_pages`、`source_effectiveness`、`habit_patterns`、`reopened_investigations`、`path_flows`），並在 intelligence bootstrap 時清掉 legacy `insight_*` tables；`visit_content_enrichments` 則保留作為 optional AI / enrichment readable-text evidence plane。
  - 更新 [`src-tauri/crates/vault-core/src/intelligence_runtime.rs`](../../src-tauri/crates/vault-core/src/intelligence_runtime.rs)、[`src-tauri/crates/vault-worker/src/intelligence.rs`](../../src-tauri/crates/vault-worker/src/intelligence.rs)、[`src-tauri/src/commands/intelligence.rs`](../../src-tauri/src/commands/intelligence.rs)、[`src-tauri/src/worker_bridge/intelligence.rs`](../../src-tauri/src/worker_bridge/intelligence.rs)、[`src-tauri/src/dev_ipc_bridge.rs`](../../src-tauri/src/dev_ipc_bridge.rs) 與相關 model / worker re-exports，讓主產品 deterministic contract hard-cut 到 `run_core_intelligence_now`、`queue_core_intelligence_rebuild`、`load_intelligence_runtime` 與 P1/P2 query commands（sessions / trails / digest / top sites / refind / activity mix / stable sources / friction / discovery / on-this-day 等），不再以 `run_insights_now` / `load_insights` / `explain_insight` 為 accepted shipping path。
  - archive intelligence bootstrap / doctor / backup follow-up 也同步改線：[`src-tauri/crates/vault-core/src/archive/intelligence_projection.rs`](../../src-tauri/crates/vault-core/src/archive/intelligence_projection.rs)、[`src-tauri/crates/vault-core/src/archive/doctor.rs`](../../src-tauri/crates/vault-core/src/archive/doctor.rs)、[`src-tauri/crates/vault-worker/src/archive_flows.rs`](../../src-tauri/crates/vault-worker/src/archive_flows.rs) 現在會對新 Core Intelligence schema 做 bootstrap、stale-state invalidation 與 rebuild enqueue；runtime queue / module registry 也已收斂到 Core Intelligence modules / jobs，而不是 legacy insight snapshot rebuild。
  - 一併修補 supporting quality surface：`vault-core/examples/intelligence-benchmark.rs` 改成 benchmark Core Intelligence rebuild + query surfaces；`src/app/index.test.tsx` 與 `/intelligence` navigation/test contract 跟著 hard-cut rename；[`scripts/check-rust-security.mjs`](../../scripts/check-rust-security.mjs) 也補上 non-advisory cargo-audit warning handling，讓新增的 yanked `core2@0.4.0` transitive warning 以明確 allowlist / provenance note 通過 supply-chain gate，而不是讓腳本因 JSON shape 崩潰。
  - 驗收：`cargo test --manifest-path src-tauri/Cargo.toml -p vault-core --lib -- --nocapture`、`bun run check:rust`、`bun run test:unit -- src/app/index.test.tsx`、`bun run audit:rust`、`bun run check`、`bun run build`

- [x] **WORK-CI-B (2026-04-17 slice)** — Core Intelligence Incremental Foundation
  - 2026-04-17：在 [`src-tauri/crates/vault-core/src/intelligence/incremental.rs`](../../src-tauri/crates/vault-core/src/intelligence/incremental.rs) 與 [`src-tauri/crates/vault-core/src/intelligence/mod.rs`](../../src-tauri/crates/vault-core/src/intelligence/mod.rs) 新增 per-profile `core_intelligence_stage_checkpoints`，把 `visit-derive`、`daily-rollup`、`structural-rebuild` 從「stage queue 存在，但實際仍全量重算」收斂成 checkpoint-backed append-only incremental foundation；visibility regression、stage-version drift、缺失 checkpoint、manual full rebuild 與 debug `limit` path 會誠實回退成 scoped `fallback-full`。
  - 更新 [`src-tauri/crates/vault-core/src/models/core_intelligence.rs`](../../src-tauri/crates/vault-core/src/models/core_intelligence.rs)、[`src-tauri/crates/vault-core/src/models/intelligence.rs`](../../src-tauri/crates/vault-core/src/models/intelligence.rs)、[`src-tauri/crates/vault-core/src/intelligence_runtime.rs`](../../src-tauri/crates/vault-core/src/intelligence_runtime.rs) 與 [`src-tauri/crates/vault-worker/src/intelligence.rs`](../../src-tauri/crates/vault-worker/src/intelligence.rs)，讓 rebuild report / runtime artifact 補上 `executionMode`、`affectedProfiles`、`dirtyVisitCount`、`dirtyDateKeys`、`fallbackReason`，供 Jobs / Settings / 後續前端 surface 誠實呈現增量或 fallback 真相。
  - 修補 deterministic contract 細節：`build_sessions` / `build_search_trails` 改成穩定的 visit-anchored ids、`search_event_terms` dedupe 同一 `(visit_id, term)`、`path_flows` builder / query / explainability 全鏈路支援 2..=4 steps。
  - 擴寫 [`src-tauri/crates/vault-core/examples/intelligence-benchmark.rs`](../../src-tauri/crates/vault-core/examples/intelligence-benchmark.rs)，新增 `full`、`append-delta`、`visibility-regression-fallback` 三種 scenario，並生成 [`artifacts/benchmarks/2026-04-17-intelligence-incremental-foundation/`](../../artifacts/benchmarks/2026-04-17-intelligence-incremental-foundation) 下的 replayable evidence。
  - 新增 / 擴寫 `vault-core` regression：stage checkpoint / append-only path、dirty-day rollups、tail structural rebuild、visibility regression fallback、`path_flows` 4-step、runtime artifact metadata；同時補上 `vault-worker` runtime tests，確認 auto-enqueue / queue review / runtime snapshot 仍維持 green。
  - 同步回寫 [`docs/features/core-intelligence-ultimate-design.md`](../features/core-intelligence-ultimate-design.md)、[`docs/features/intelligence-current-state.md`](../features/intelligence-current-state.md)、[`docs/plan/program/research-and-decisions.md`](program/research-and-decisions.md)、[`docs/plan/core-intelligence-progress.md`](core-intelligence-progress.md)、[`docs/plan/core-intelligence-handoff.md`](core-intelligence-handoff.md) 與 [`docs/plan/STATUS.md`](STATUS.md)，把「incremental foundation 已落地，但 `PG-RD-AI-011` / low-RAM / 10M signoff 仍 open」寫回 planning truth。
  - 驗收：`cargo test --manifest-path src-tauri/Cargo.toml -p vault-core --lib`、`cargo test --manifest-path src-tauri/Cargo.toml -p vault-worker --lib`、`cargo run --manifest-path src-tauri/Cargo.toml -p vault-core --example intelligence-benchmark -- --visits 100000 --window-days 365 --horizon-days 21900 --scenario {full,append-delta,visibility-regression-fallback}`、`bun run check`、`bun run build`

- [x] **WORK-CI-B** — Core Intelligence Backend Finish Line
  - 2026-04-17：把 backend finish-line 真正收口：canonical runtime/status contract 現在以 `intelligenceStatus` / `IntelligenceStatus` 為準，`get_intelligence_embed_cards`、`get_intelligence_widget_snapshot`、`get_intelligence_public_snapshot` 也已補齊 typed IPC wrapper；live queued enrichment execution 則移到 [`src-tauri/crates/vault-core/src/enrichment.rs`](../../src-tauri/crates/vault-core/src/enrichment.rs)，不再把 active runtime path 綁在 legacy `insights.rs`。
  - large-host / replay evidence 也正式補齊：`artifacts/benchmarks/2026-04-17-intelligence-signoff/` 現在包含 corrected `full-2k-smoke-signoff.json`、`full-1m-60y-signoff.json`、`full-10m-60y-signoff.json`、`expired-lease-recovery-10m-signoff.json`，以及 disposable encrypted app-root 的 [`real-replay-signoff.json`](../../artifacts/benchmarks/2026-04-17-intelligence-signoff/real-replay-signoff.json)。其中 `stageTimingsMs` 已修正為跨 profile 累加，real replay 也會把 `--session-key` command shape redact 成 `<redacted>`。
  - 同步回寫 [`docs/plan/program/research-and-decisions.md`](program/research-and-decisions.md)、[`docs/plan/core-intelligence-progress.md`](core-intelligence-progress.md)、[`docs/plan/core-intelligence-handoff.md`](core-intelligence-handoff.md)、[`docs/features/intelligence-current-state.md`](../features/intelligence-current-state.md)、[`artifacts/benchmarks/2026-04-17-intelligence-signoff/README.md`](../../artifacts/benchmarks/2026-04-17-intelligence-signoff/README.md)、[`docs/plan/STATUS.md`](STATUS.md) 與 [`docs/plan/BACKLOG.md`](BACKLOG.md)，把 `PG-RD-AI-011` / `WORK-CI-B` closeout、`WORK-CI-F` 仍 active、以及 `WORK-CI-C` residual boundary 寫回 source of truth。
  - 驗收：`cargo test --manifest-path src-tauri/Cargo.toml -p vault-core --lib`、`bun run check`、`bun run build`

- [x] **WORK-CI-F** — Core Intelligence Frontend Finish Line
  - 2026-04-17：更新 [`src/pages/intelligence/index.tsx`](../../src/pages/intelligence/index.tsx)、[`src/pages/intelligence/runtime-digest.tsx`](../../src/pages/intelligence/runtime-digest.tsx)、[`src/pages/intelligence/intelligence.css`](../../src/pages/intelligence/intelligence.css) 與 [`src/lib/i18n/catalog.ts`](../../src/lib/i18n/catalog.ts)，把 `/intelligence` 主產品頁面補上 top-of-page runtime digest、shared scope honesty copy、以及 external `embed/widget/public snapshot` 的 deferred callout；Dashboard / onboarding / navigation 相關 CTA 也同步改回 Core Intelligence vocabulary，不再把 deterministic route 說成 legacy Insights。
  - 更新 [`src/pages/intelligence-surfaces.test.tsx`](../../src/pages/intelligence-surfaces.test.tsx)、[`tests/e2e/shell.spec.ts`](../../tests/e2e/shell.spec.ts) 與既有 product-flow assertions，收掉 `/insights` route / test drift，並把 browser preview 的 `/intelligence` truth 改成驗 runtime digest + deferred external-output boundary，而不是沿用舊 `Insights` 頁面假設。
  - 同步回寫 [`docs/plan/core-intelligence-progress.md`](core-intelligence-progress.md)、[`docs/plan/core-intelligence-handoff.md`](core-intelligence-handoff.md)、[`docs/features/intelligence-current-state.md`](../features/intelligence-current-state.md)、[`docs/design/screens-and-nav.md`](../design/screens-and-nav.md)、[`docs/plan/STATUS.md`](STATUS.md) 與 [`docs/plan/BACKLOG.md`](BACKLOG.md)，把前端 finish-line truth、CI-H deferred boundary 與下一輪 current focus 收回 planning source of truth。
  - 驗收：`bunx vitest run src/pages/intelligence-surfaces.test.tsx src/app/index.test.tsx`、`node scripts/run-playwright.mjs tests/e2e/shell.spec.ts`、`bun run check`、`bun run build`

- [x] **WORK-CI-H** — Core Intelligence External Output Consumers And Host Integrations
  - 2026-04-17：新增 [`src/pages/settings/external-outputs-panel.tsx`](../../src/pages/settings/external-outputs-panel.tsx)，把既有 `get_intelligence_embed_cards`、`get_intelligence_widget_snapshot`、`get_intelligence_public_snapshot` 接成 Settings 內的 manual review / copy-export surface，支援 shared profile scope、local time range、trusted-only badge、public-redaction honesty 與 raw JSON copy action。
  - 更新 [`src/pages/settings/index.tsx`](../../src/pages/settings/index.tsx)、[`src/pages/intelligence/index.tsx`](../../src/pages/intelligence/index.tsx)、[`src/styles/app.css`](../../src/styles/app.css) 與 [`src/lib/i18n/catalog.ts`](../../src/lib/i18n/catalog.ts)，讓 `/intelligence` 把 external outputs 改成導向 Settings 的 CTA，而不是繼續顯示 deferred placeholder；所有新文案同步補齊 `en` / `zh-CN` / `zh-TW`。
  - 擴寫 [`src/pages/intelligence-surfaces.test.tsx`](../../src/pages/intelligence-surfaces.test.tsx)、[`src/app/index.test.tsx`](../../src/app/index.test.tsx)、[`src/pages/trust-flows.test.tsx`](../../src/pages/trust-flows.test.tsx) 與 [`tests/e2e/shell.spec.ts`](../../tests/e2e/shell.spec.ts)，覆蓋 Settings external-output panel 的 fetch / scope / time-range refetch / honesty states，以及 `/intelligence` 的新 CTA。
  - 同步回寫 [`docs/design/screens-and-nav.md`](../design/screens-and-nav.md)、[`docs/features/core-intelligence-ultimate-design.md`](../features/core-intelligence-ultimate-design.md)、[`docs/features/intelligence-current-state.md`](../features/intelligence-current-state.md)、[`docs/architecture/data-model.md`](../architecture/data-model.md)、[`docs/plan/core-intelligence-progress.md`](core-intelligence-progress.md)、[`docs/plan/core-intelligence-handoff.md`](core-intelligence-handoff.md) 與 [`docs/plan/STATUS.md`](STATUS.md)，把「manual Settings consumer 已完成、但 deeper host integration 仍 deferred」寫回 source of truth。
  - 驗收：`bun run test:unit:product-flows`、`bun run check`、`bun run build`

- [x] **WORK-CI-C** — Core Intelligence Legacy Cleanup And Long-Horizon Signoff
  - 2026-04-18：刪除 [`src-tauri/crates/vault-core/src/insights.rs`](../../src-tauri/crates/vault-core/src/insights.rs) 與整個 legacy `insights/` tree，並把 readable-content / queued-enrichment ownership 收回 [`src-tauri/crates/vault-core/src/enrichment.rs`](../../src-tauri/crates/vault-core/src/enrichment.rs) 與 [`src-tauri/crates/vault-core/src/enrichment_site_adapters.rs`](../../src-tauri/crates/vault-core/src/enrichment_site_adapters.rs)。同一輪也移除 snapshot-era `Insight*` transport、legacy module-id alias、`insight_status` wrapper、以及舊 deterministic clear-report wording，讓 repo 只保留 registry-backed Core Intelligence ids、canonical table names、與 grouped clear-state counts。
  - 更新 frontend shared types / runtime helpers / preview fixtures / Settings clear-state copy，讓主產品與 browser-preview 都只使用 live built-ins `visit-derived-facts`、`daily-rollups`、`sessions`、`search-trails`、`refind-pages`、`activity-mix`、`search-effectiveness`、`domain-deep-dive`，以及 canonical tables `visit_derived_facts`、daily rollups、`sessions`、`search_trails`、`query_families`、`refind_pages`、`source_effectiveness`、`habit_patterns`、`reopened_investigations`、`path_flows`。
  - 新增 [`artifacts/benchmarks/2026-04-18-intelligence-long-horizon-signoff/README.md`](../../artifacts/benchmarks/2026-04-18-intelligence-long-horizon-signoff/README.md)，並生成 [`full-14_4m-60y-signoff.json`](../../artifacts/benchmarks/2026-04-18-intelligence-long-horizon-signoff/full-14_4m-60y-signoff.json) 與 [`expired-lease-recovery-14_4m-signoff.json`](../../artifacts/benchmarks/2026-04-18-intelligence-long-horizon-signoff/expired-lease-recovery-14_4m-signoff.json)。current-host full replay 驗到 `14,400,000` visits、baseline rebuild 約 `4,758,160 ms`、query surfaces 約 `8,969 ms`、peak RSS 約 `1.74 GiB`；expired-lease replay 用同一 durable root 驗到 `--skip-baseline-rebuild`、query surfaces 約 `2,013 ms`、peak RSS 約 `598.6 MiB`，且 queued lease 會被 recover、cancelled lease 維持 cancelled。
  - 同步回寫 [`docs/plan/program/research-and-decisions.md`](program/research-and-decisions.md)、[`docs/plan/core-intelligence-progress.md`](core-intelligence-progress.md)、[`docs/plan/core-intelligence-handoff.md`](core-intelligence-handoff.md)、[`docs/features/intelligence-current-state.md`](../features/intelligence-current-state.md)、[`docs/features/intelligence.md`](../features/intelligence.md)、[`docs/features/core-intelligence-ultimate-design.md`](../features/core-intelligence-ultimate-design.md)、[`docs/architecture/data-model.md`](../architecture/data-model.md)、[`docs/plan/program/repo-baseline.md`](program/repo-baseline.md)、[`docs/plan/README.md`](README.md) 與 [`docs/plan/STATUS.md`](STATUS.md)，把 closeout truth 固定成：`WORK-CI-C` 已完成、legacy `insights` retired、alternate-host evidence 明確 deferred。
  - 驗收：`cargo test --manifest-path src-tauri/Cargo.toml -p vault-core --lib`、`bun run check`、`bun run build`

- [x] **WORK-M5-C** — Core Intelligence Evidence Metadata And Degraded-State Truth Pass
  - 讀先：
    `docs/plan/m5-runtime-and-extensions/README.md`
    `docs/plan/m5-runtime-and-extensions/deterministic-insights-and-evidence-controls.md`
    `docs/features/intelligence-current-state.md`
    `docs/design/screens-and-nav.md`
    `docs/plan/core-intelligence-progress.md`
  - 2026-04-18：新增 backend-owned section envelope：Rust / Tauri / worker / TS contract 現在正式有 `CoreIntelligenceSectionMeta` 與 `CoreIntelligenceSectionResult<T>`，`/intelligence` 與 `/intelligence/domain/:domain` 使用的 section commands 會一起帶回 `generatedAt`、structured `window`、`moduleIds`、`sourceTables`、`includesEnrichment`、`state`、`stateReason` 與 `notes`，不再讓前端自己猜 freshness / provenance。
  - 新增 [`src-tauri/crates/vault-core/src/intelligence_sections.rs`](../../src-tauri/crates/vault-core/src/intelligence_sections.rs) 與對應 Rust tests，把 section id、module ownership、source-table provenance、capability-gated degrade rule、以及 module-backed stale / disabled / degraded derivation收斂成單一 registry；worker transport 只包裝 route-facing section queries，不重寫底層 deterministic query implementation。
  - 更新 [`src/components/intelligence/section-meta.tsx`](../../src/components/intelligence/section-meta.tsx)、[`src/pages/intelligence/sections.tsx`](../../src/pages/intelligence/sections.tsx)、[`src/pages/intelligence/domain-deep-dive.tsx`](../../src/pages/intelligence/domain-deep-dive.tsx)、[`src/pages/intelligence/intelligence.css`](../../src/pages/intelligence/intelligence.css) 與 [`src/lib/i18n/catalog.ts`](../../src/lib/i18n/catalog.ts)，讓 `/intelligence` 與 domain deep dive 現在用同一個 shared evidence / freshness drawer 顯示 generated-at、active scope / window、owning modules、source tables、enrichment flag、以及 stale / disabled / degraded reason；Settings / Jobs 則繼續保留 rebuild / clear / retry mutation surface。
  - 擴寫 [`src/pages/intelligence-surfaces.test.tsx`](../../src/pages/intelligence-surfaces.test.tsx)，覆蓋 section-level stale / disabled / degraded rendering，以及 shared scope / time-range 改變後 metadata 會跟著 refetch；Dashboard 的 `On This Day` 也已改成消費新 envelope 而不丟失既有 UI。
  - 同步回寫 [`docs/plan/m5-runtime-and-extensions/README.md`](m5-runtime-and-extensions/README.md)、[`docs/plan/m5-runtime-and-extensions/deterministic-insights-and-evidence-controls.md`](m5-runtime-and-extensions/deterministic-insights-and-evidence-controls.md)、[`docs/features/intelligence-current-state.md`](../features/intelligence-current-state.md)、[`docs/design/screens-and-nav.md`](../design/screens-and-nav.md)、[`docs/plan/core-intelligence-progress.md`](core-intelligence-progress.md)、[`docs/plan/STATUS.md`](STATUS.md) 與 [`docs/plan/BACKLOG.md`](BACKLOG.md)，把 shared evidence drawer truth、M5 evidence closeout 與下一個 host-integration follow-up block 寫回 planning/source docs。
  - 驗收：`cargo test --manifest-path src-tauri/Cargo.toml -p vault-core --lib`、`bun run test:unit`、`bun run typecheck`、`bun run check`、`bun run build`

- [x] **WORK-CI-I** — External Output Host Integration Foundation
  - 讀先：
    `docs/plan/core-intelligence-progress.md`
    `docs/plan/core-intelligence-handoff.md`
    `docs/features/core-intelligence-ultimate-design.md`
    `docs/features/intelligence-current-state.md`
    `docs/design/screens-and-nav.md`
  - 2026-04-18：新增 [`src-tauri/crates/vault-core/src/intelligence/host_artifacts.rs`](../../src-tauri/crates/vault-core/src/intelligence/host_artifacts.rs)，把第一個真正可重用的 external-output host 定為 trusted local `browser-snippet-v1`。backend / worker / desktop / TS contract 現在正式有 `preview_intelligence_local_host` / `build_intelligence_local_host`、`IntelligenceLocalHostRequest`、`IntelligenceLocalHostPreview`、`IntelligenceLocalHostBundle`、`IntelligenceLocalHostBuildResult`，並固定把 artifact 寫到 `app_root/integrations/core-intelligence/browser-snippet-v1/{index.html,bundle.json}`。
  - `browser-snippet-v1` 嚴格重用既有 `get_intelligence_embed_cards`、`get_intelligence_widget_snapshot`、`get_intelligence_public_snapshot` payload providers：`bundle.json` 保存同一份 typed machine contract，`index.html` 直接內嵌 bundle data 靜態渲染 trusted local snippet，不依賴 localhost server、也不會從 `file://` 再 fetch JSON。preview 會回傳 generated files、boundary notes、manual steps、warnings，以及已安裝 host 的 verify state。
  - 新增 [`src/pages/settings/external-output-local-host-panel.tsx`](../../src/pages/settings/external-output-local-host-panel.tsx)，讓 Settings external outputs 在既有 manual review / copy-export baseline 之下，多出獨立的 trusted local host subsection：同一個 shared profile scope + local time range 下可 preview artifact root、review `index.html` / `bundle.json`、執行 `Create/Update local snippet`、以及 verify / open local host / open folder。所有新增文案都已補齊 `en` / `zh-CN` / `zh-TW`。
  - 更新 [`src-tauri/crates/vault-platform/src/launcher.rs`](../../src-tauri/crates/vault-platform/src/launcher.rs) 與 support/file-manager helpers，讓 `openExternalUrl(file://...)` 正式可用於打開本地 host entry；同時補上 Rust/Vitest regression tests，覆蓋 local-host preview/build contract、Settings gating、scope / time-range refetch、build success、verify state，以及 open path/open host actions。
  - 同步回寫 [`docs/features/core-intelligence-ultimate-design.md`](../features/core-intelligence-ultimate-design.md)、[`docs/features/intelligence-current-state.md`](../features/intelligence-current-state.md)、[`docs/design/screens-and-nav.md`](../design/screens-and-nav.md)、[`docs/architecture/data-model.md`](../architecture/data-model.md)、[`docs/architecture/desktop-command-surface.md`](../architecture/desktop-command-surface.md)、[`docs/plan/core-intelligence-progress.md`](core-intelligence-progress.md)、[`docs/plan/core-intelligence-handoff.md`](core-intelligence-handoff.md)、[`docs/plan/STATUS.md`](STATUS.md) 與 [`docs/plan/BACKLOG.md`](BACKLOG.md)，把「manual baseline + `browser-snippet-v1` trusted local host 已完成，剩 OS widget / localhost / public API / alternate hosts deferred」寫回 source of truth。
  - 驗收：`cargo test --manifest-path src-tauri/Cargo.toml -p vault-core --lib`、`bun run check`、`bun run build`

- **2026-04-18 planning note** — 使用者已明確把 `WORK-CI-J` 從當前計劃移除，改為不在 scope 內的 deferred-by-user follow-up。
  - 這次變更只收口 planning / living docs：`STATUS.md`、`BACKLOG.md`、Core Intelligence progress / handoff、`README.md`、`PG-RD-AI-011`、以及對應 feature notes 現在都改成以 current-host `14.4M / 60y` signoff 為 stop point，不再把第二台主機 benchmark parity 當成 active work block 或預設待辦。
  - 現有 repo truth 不變：current-host signoff 仍有效；CI 與 manual `Platform Native` workflow 仍只是既有驗收邊界，不被改寫成 host-native truth 的替代品。

- [x] **WORK-CI-M** — Desktop Truth Audit And Locked-Archive Bootstrap Repair
  - 2026-04-18：更新 [`src/lib/runtime.ts`](../../src/lib/runtime.ts) 與 [`src/lib/ipc/bridge.ts`](../../src/lib/ipc/bridge.ts)，補上 Tauri runtime detection（`tauri:` protocol / injected internals）與 raw `invoke()` string rejection → `Error` shaping，避免 desktop shell 因 transport 層直接把 actionable refusal message 掉成 generic dashboard fallback；同時擴寫 [`src/lib/runtime.test.ts`](../../src/lib/runtime.test.ts) 與 [`src/lib/ipc/bridge.test.ts`](../../src/lib/ipc/bridge.test.ts) 鎖住這條 contract。
  - 更新 [`src/pages/dashboard/index.tsx`](../../src/pages/dashboard/index.tsx) 與 [`src/pages/intelligence-surfaces.test.tsx`](../../src/pages/intelligence-surfaces.test.tsx)，讓 Dashboard 在 shell bootstrap 已經失敗時，仍能透過既有 `securityStatus()` command 區分 `未初始化 archive`、`encrypted but locked`、與真正 generic failure，並在 source-level UI 上補回 onboarding zero-state 與 `/security#unlock-archive` repair CTA。
  - 更新 [`src-tauri/crates/vault-worker/src/app.rs`](../../src-tauri/crates/vault-worker/src/app.rs)、[`src-tauri/crates/vault-worker/src/tests.rs`](../../src-tauri/crates/vault-worker/src/tests.rs) 與 [`src-tauri/crates/vault-worker/Cargo.toml`](../../src-tauri/crates/vault-worker/Cargo.toml)，把 browser discovery / runtime diagnostics 改成 best-effort degradation，不再因非關鍵 shell bootstrap read 把整個 `app_snapshot` 打掛；同時新增 invalid browser discovery fixture regression。這輪也在 [`src-tauri/src/worker_bridge/mod.rs`](../../src-tauri/src/worker_bridge/mod.rs) 補上 command-error logging，方便 current-host desktop audit 留下真正的 refusal trace。
  - 新增 [`docs/plan/core-intelligence-desktop-truth-audit.md`](core-intelligence-desktop-truth-audit.md) 與 [`artifacts/perf/2026-04-18-desktop-truth-audit/`](../../artifacts/perf/2026-04-18-desktop-truth-audit/)（含 `context.md`、`notes.md`、`unlock-hang-sample.txt`），把這輪 Computer Use 真機觀察與第一份 non-browser profiling evidence 固定下來：fresh desktop app 仍在 Dashboard 顯示 generic `無法讀取封存`、Security route 可讀到真實 encrypted+locked 狀態、但 `000000` unlock flow 在觀察窗口內未 settle，且 sample 顯示 pre-unlock background churn 仍在主線執行。
  - 同步回寫 [`docs/plan/STATUS.md`](STATUS.md)、[`docs/plan/BACKLOG.md`](BACKLOG.md)、[`docs/plan/README.md`](README.md)、[`docs/plan/core-intelligence-progress.md`](core-intelligence-progress.md) 與 [`docs/plan/core-intelligence-handoff.md`](core-intelligence-handoff.md)，把 planning truth 從「原始 P1-P4 scope 完成」進一步收斂成「source-level 完成仍成立，但 current-host desktop truth 重新開出 locked-archive bootstrap / unlock / full real-data audit follow-up block」。`WORK-CI-N` 現在正式留在 `BACKLOG.md`，等待 bootstrap / unlock 修穩，或等待使用者明確允許 reset 當前 app root。
  - 驗收：`bun vitest run src/lib/runtime.test.ts src/lib/ipc/bridge.test.ts`、`bun vitest run src/pages/intelligence-surfaces.test.tsx --testNamePattern='dashboard|archive-key|unlock|onboarding zero-state'`、current-host Computer Use security/unlock audit、以及 `artifacts/perf/2026-04-18-desktop-truth-audit/unlock-hang-sample.txt`

- [x] **WORK-CI-O** — Locked-Archive Shell Truth Follow-Up And Build Revision Diagnostics
  - 2026-04-18：更新 [`src-tauri/crates/vault-worker/src/app.rs`](../../src-tauri/crates/vault-worker/src/app.rs) 與 [`src-tauri/crates/vault-worker/src/tests.rs`](../../src-tauri/crates/vault-worker/src/tests.rs)，讓 locked encrypted archive 的 `app_snapshot` 不再直接把 shell bootstrap 打掛，而是回傳 usable locked snapshot（`archiveStatus.warning` + 空 recent ledger lists）；這樣 Dashboard 仍可透過既有 dashboard-read failure grammar 引導使用者去解鎖，而 sidebar / topbar 也能讀到 initialized+encrypted truth。
  - 更新 [`src/pages/security/index.tsx`](../../src/pages/security/index.tsx) 與 [`src/pages/trust-flows.test.tsx`](../../src/pages/trust-flows.test.tsx)，讓 Security unlock flow 在 candidate key 寫進 session 後，先用既有 `securityStatus()` command 驗證 archive 是否真的解鎖；若沒有，就清掉 transient session key 並立即回傳人話錯誤，而不是先把使用者卡在 full shell refresh 的 busy overlay 裡。
  - 更新 [`src/components/sidebar/background-status.tsx`](../../src/components/sidebar/background-status.tsx)、[`src/components/sidebar/index.tsx`](../../src/components/sidebar/index.test.tsx) 與 [`src/lib/i18n/catalog.ts`](../../src/lib/i18n/catalog.ts)，把 locked archive 的 compact background-work strip 改成停止輪詢 runtime、直接導向 `/security#unlock-archive`，並補上新的 locked-state 文案。
  - 新增 [`src/lib/build-info.ts`](../../src/lib/build-info.ts) 與 [`src/lib/build-info.test.ts`](../../src/lib/build-info.test.ts)，並更新 [`src/components/sidebar/index.tsx`](../../src/components/sidebar/index.tsx)、[`src/pages/onboarding/index.tsx`](../../src/pages/onboarding/index.tsx)、[`src/pages/lock/index.tsx`](../../src/pages/lock/index.tsx) 與 [`src/pages/settings/index.tsx`](../../src/pages/settings/index.tsx)，正式補回 compact `version · short-sha[+]` build label；dirty worktree 會在 short SHA 右邊加 `+`，方便 QA / support 辨認不是純 commit build 的桌面包。
  - 同步回寫 [`docs/plan/core-intelligence-desktop-truth-audit.md`](core-intelligence-desktop-truth-audit.md)、[`artifacts/perf/2026-04-18-desktop-truth-audit/{context,notes}.md`](../../artifacts/perf/2026-04-18-desktop-truth-audit/notes.md)、[`docs/plan/README.md`](README.md)、[`docs/plan/core-intelligence-progress.md`](core-intelligence-progress.md)、[`docs/plan/core-intelligence-handoff.md`](core-intelligence-handoff.md)、[`docs/plan/STATUS.md`](STATUS.md) 與 [`docs/plan/BACKLOG.md`](BACKLOG.md)，記下 source-level follow-up 已完成，但 current-host fresh `bun run desktop:dev` relaunch 仍顯示舊 generic dashboard copy 與不帶 SHA 的 shell chrome；這仍屬 host-side stale WebView / bundle cache drift，而不是 source truth 回退。
  - 驗收：`cargo test --manifest-path src-tauri/Cargo.toml -p vault-worker app_snapshot_`、`bun vitest run src/lib/build-info.test.ts src/components/sidebar/index.test.tsx src/pages/trust-flows.test.tsx src/pages/intelligence-surfaces.test.tsx src/lib/runtime.test.ts src/lib/ipc/bridge.test.ts`

- [x] **WORK-UI-A** — Explorer And Intelligence Interaction Polish
  - 2026-04-18：更新 [`src/components/topbar/index.tsx`](../../src/components/topbar/index.tsx)、[`src/components/topbar/index.test.tsx`](../../src/components/topbar/index.test.tsx)、[`src/styles/app.css`](../../src/styles/app.css) 與 [`src/lib/i18n/catalog.ts`](../../src/lib/i18n/catalog.ts)，在 shell topbar 左上角補上全局上一頁 / 下一頁按鈕，語意對齊 app 內 route history；同時補齊三語 aria / label copy 與 regression test。
  - 更新 [`src/pages/explorer/helpers.ts`](../../src/pages/explorer/helpers.ts)、[`src/pages/explorer/hooks/use-explorer-url-state.ts`](../../src/pages/explorer/hooks/use-explorer-url-state.ts)、[`src/pages/explorer/index.tsx`](../../src/pages/explorer/index.tsx)、[`src/pages/explorer/panels/results-panel.tsx`](../../src/pages/explorer/panels/results-panel.tsx)、[`src/pages/explorer/panels/privacy-redaction.test.tsx`](../../src/pages/explorer/panels/privacy-redaction.test.tsx)、[`src/pages/intelligence-surfaces.test.tsx`](../../src/pages/intelligence-surfaces.test.tsx) 與 [`src/styles/app.css`](../../src/styles/app.css)，讓 Explorer 分頁列直接顯示當前頁 / 總頁數、提供每頁筆數控制、移除強制 scroll restore 邏輯，並把右側 detail rail 改成 sticky + fit-content，避免列表很長時 detail 被推回頁面頂端。
  - 更新 [`src/pages/intelligence/index.tsx`](../../src/pages/intelligence/index.tsx)、[`src/pages/intelligence/sections.tsx`](../../src/pages/intelligence/sections.tsx)、[`src/pages/intelligence/intelligence.css`](../../src/pages/intelligence/intelligence.css)、[`src/pages/intelligence-surfaces.test.tsx`](../../src/pages/intelligence-surfaces.test.tsx) 與 [`src/lib/i18n/catalog.ts`](../../src/lib/i18n/catalog.ts)，把 `/intelligence` 首屏注意力重新排序：移除頂部 archive-wide / external-output 大 callout、縮小 runtime digest、把 habits 提到前段、把 refind 改成半寬 row、activity mix 補上分類示例、browsing rhythm 改成可點日格 + 當日 digest / top sites，並把 stable sources / search effectiveness / friction / reopened / discovery / breadth / path flow 等 secondary cards 移到底部 secondary grid；ready-but-empty 的低價值 section 直接不占位。
  - 一併修正 habits 文案語意：`meanIntervalDays` 現在明講成「平均每幾天回來一次」，`visitCount` 改寫成 active-day count，不再把它誤包裝成總造訪次數；breadth card 也去掉互相打架的 verdict，改成較誠實的 score explainer + concentration detail。
  - 同步回寫 [`docs/design/screens-and-nav.md`](screens-and-nav.md)、[`docs/features/recall.md`](../features/recall.md)、[`docs/features/intelligence-current-state.md`](../features/intelligence-current-state.md) 與 [`docs/plan/STATUS.md`](STATUS.md)，讓 Explorer 分頁 / sticky detail rail、Intelligence 首屏 hierarchy、secondary grid demotion、與 browsing-rhythm day digest 回到 source-of-truth。
  - 驗收：`bun run test:unit -- src/components/topbar/index.test.tsx src/pages/intelligence-surfaces.test.tsx`、`bun run check`、`bun run build`；另外用 Computer Use 實機檢查 current-host `bun run desktop:dev` 時，仍可重現這台主機的 stale WebView / bundle cache noise：PathKeep 視窗繼續顯示舊版 `/intelligence` 大橫幅，而 `http://127.0.0.1:1420/#/security` 的 browser preview / DOM snapshot 已能看到最新的 topbar back-forward buttons 與 source-level route chrome。

- [x] **WORK-UI-B** — Intelligence Card Truth And Heatmap Follow-Up
  - 2026-04-18：更新 [`src/pages/intelligence/sections.tsx`](../../src/pages/intelligence/sections.tsx)、[`src/pages/intelligence/intelligence.css`](../../src/pages/intelligence/intelligence.css)、[`src/lib/i18n/catalog.ts`](../../src/lib/i18n/catalog.ts) 與 [`src/pages/intelligence-surfaces.test.tsx`](../../src/pages/intelligence-surfaces.test.tsx)，把上一輪被改壞的 `Browsing Rhythm` 還原成週內 × 小時熱力圖，保留分類篩選，並在同一卡片補上 bucket selection summary、近期實際日期 chooser、與當天 digest / top sites；這樣既保留原本 heatmap 視覺語法，也補回使用者要的點選後詳情。
  - 同一輪重新設計 `/intelligence` 後段的幾張低價值卡：`Stable Sources` 現在會直接講「入口」與「落地點」各自代表什麼；`Search Effectiveness` 改成 plain-language 摘要句，直接說明平均改寫次數、深度與 trail 數量，不再只剩抽象數字牆；`Discovery Trend` 從看不懂的雙柱圖改成帶公式說明的逐週列；`Breadth Index` 明講 breadth score、top-half concentration 與 HHI 是不同維度；`Habits` 則改成「約每幾天回來一次 / 出現於幾天 / 最近一次」的誠實摘要，不再顯示語意模糊的裸數字。
  - 同步回寫 [`docs/features/intelligence-current-state.md`](../features/intelligence-current-state.md)、[`docs/design/screens-and-nav.md`](../design/screens-and-nav.md) 與 [`docs/plan/STATUS.md`](STATUS.md)，把「保留原 heatmap，再附加當天摘要 detail」與這一輪 low-value card truth 重寫回 source docs。
  - 驗收：`bun run test:unit -- src/pages/intelligence-surfaces.test.tsx`、`bun run build`

- [x] **WORK-UI-C** — Desktop-Only Explorer And Intelligence Truth Repair
  - 2026-04-18：更新 [`src/pages/explorer/index.tsx`](../../src/pages/explorer/index.tsx)、[`src/pages/explorer/panels/results-panel.tsx`](../../src/pages/explorer/panels/results-panel.tsx)、[`src/styles/app.css`](../../src/styles/app.css)、[`src/lib/i18n/catalog.ts`](../../src/lib/i18n/catalog.ts) 與 [`src/pages/intelligence-surfaces.test.tsx`](../../src/pages/intelligence-surfaces.test.tsx)，把 Explorer 的頁碼真相補齊到桌面可讀狀態：timeline summary 與底部分頁列都會顯示「當前頁 / 總頁數」，底部分頁列保留跳頁與每頁筆數控制，且 regression test 明確鎖住翻頁不會呼叫 `window.scrollTo()` 把使用者拉回頁首。
  - 同步收斂 `/intelligence` 的 low-signal truth filter：更新 [`src/pages/intelligence/sections.tsx`](../../src/pages/intelligence/sections.tsx) 與 [`src/lib/i18n/catalog.ts`](../../src/lib/i18n/catalog.ts)，讓 `Activity Mix` 補上分類說明；`Stable Sources` 若只剩單邊 leaderboard、`Friction` 若只剩站不住腳的弱 signal、`Reopened Investigations` 若 label 不像搜尋問題、`Path Flows` 若看起來更像 auth / callback / canonical redirect path，都會直接隱藏而不是硬佔 secondary grid。
  - 同步回寫 [`docs/design/screens-and-nav.md`](../design/screens-and-nav.md)、[`docs/features/intelligence-current-state.md`](../features/intelligence-current-state.md) 與 [`docs/plan/STATUS.md`](STATUS.md)，把這輪從 browser-preview assumption 收回成 desktop-only truth gate 的修正寫回 source docs。
  - 驗收：`bun run test:unit -- src/components/topbar/index.test.tsx src/pages/intelligence-surfaces.test.tsx`、`bun run check`、`bun run build`；另外在 current-host 重打 release `.app` / 直接啟動 `src-tauri/target/release/pathkeep-desktop` 後，用 Computer Use 實機確認 `/explorer` 會顯示新的頂部「第 1 / 1301 頁」摘要、右側 detail rail 仍可見，`/intelligence` 則已拿到新的 habits copy（`xx 天沒來了… 最近一次…`）。這台 host 上的 CUA click/scroll 對直接啟動的 release binary 仍偶發 `noWindowsAvailable`，所以底部分頁列與較下方的 intelligence sections 主要由 regression tests + live top-of-screen evidence 一起簽收，而不是假裝做了完整滑到底驗收。

- [x] **WORK-CI-Q** — Intelligence And Onboarding Performance Decoupling
  - 2026-04-19：更新 [`src-tauri/crates/vault-worker/src/intelligence.rs`](../../src-tauri/crates/vault-worker/src/intelligence.rs)、[`src-tauri/src/commands/intelligence.rs`](../../src-tauri/src/commands/intelligence.rs)、[`src-tauri/src/worker_bridge/intelligence.rs`](../../src-tauri/src/worker_bridge/intelligence.rs)、[`src/lib/core-intelligence/api.ts`](../../src/lib/core-intelligence/api.ts)、[`src/pages/intelligence/index.tsx`](../../src/pages/intelligence/index.tsx)、[`src/pages/intelligence/use-staged-intelligence-overview.ts`](../../src/pages/intelligence/use-staged-intelligence-overview.ts) 與 [`src/pages/intelligence/sections.tsx`](../../src/pages/intelligence/sections.tsx)，把 `/intelligence` 改成 route-level staged overview：primary overview 先批次載入 digest / 首屏 cards 並 prime section cache，secondary grid 在 first paint / idle 後再補；route 離開後的過期 request 也不再 commit 回 UI。
  - 更新 [`src/pages/intelligence/sections/browsing-rhythm-section.tsx`](../../src/pages/intelligence/sections/browsing-rhythm-section.tsx)、[`src/app/shell-data.tsx`](../../src/app/shell-data.tsx)、[`src/components/sidebar/background-status.tsx`](../../src/components/sidebar/background-status.tsx)、[`src/pages/dashboard/index.tsx`](../../src/pages/dashboard/index.tsx) 與 [`src/pages/intelligence/runtime-digest.tsx`](../../src/pages/intelligence/runtime-digest.tsx)，讓 `Browsing Rhythm` 初次進頁不再自動抓同日 detail，並把 sidebar / Dashboard / intelligence digest 的 runtime truth 合併成單一 shell-level polling source，避免 route 切換時重複讀 queue/runtime。
  - 更新 [`src-tauri/crates/vault-core/src/takeout.rs`](../../src-tauri/crates/vault-core/src/takeout.rs)、[`src-tauri/src/commands/import.rs`](../../src-tauri/src/commands/import.rs)、[`src/lib/ipc/import-progress.ts`](../../src/lib/ipc/import-progress.ts)、[`src/pages/import/index.tsx`](../../src/pages/import/index.tsx)、[`src/components/primitives/busy-overlay.tsx`](../../src/components/primitives/busy-overlay.tsx) 與 [`src/components/primitives/loading-state.tsx`](../../src/components/primitives/loading-state.tsx)，把 import/onboarding finalization 的前景工作改成 typed progress/log streaming（`phase/current/total/percent/detail/logLines`）；backup event 也同步補齊 coarse progress/log fields，shell refresh 則拆成 minimal hydrate + background revalidation，不再把 overlay 動畫整段卡死。
  - 新增 [`src/lib/backend-client/shared.ts`](../../src/lib/backend-client/shared.ts) 的 desktop command metrics 記錄，並同步回寫 [`docs/design/ux-principles.md`](../design/ux-principles.md)、[`docs/features/intelligence.md`](../features/intelligence.md)、[`docs/features/archive.md`](../features/archive.md)、[`docs/plan/program/research-and-decisions.md`](program/research-and-decisions.md)、[`docs/plan/m4-full-polish/large-archive-performance-runbook.md`](m4-full-polish/large-archive-performance-runbook.md)、[`docs/plan/STATUS.md`](STATUS.md) 與 [`docs/plan/BACKLOG.md`](BACKLOG.md)，把 staged loading、shared runtime polling、streamed import progress 與 remaining destructive-reset boundary 寫回 source of truth。
  - 驗收：`bun run test:unit -- src/pages/intelligence-surfaces.test.tsx src/components/sidebar/index.test.tsx src/app/shell-data.test.tsx`、`bun run check`、`bun run build`；另外以 current-host live desktop + Computer Use 驗到 dashboard ↔ `/intelligence` 切換、manual backup 後 background rebuild、與 secondary grid 延後載入都不再把 shell 直接凍住。完整 onboarding re-import truth pass 仍 deferred，因為重做 `yi-ting` profile import 需要先對當前 app root 做 destructive reset，必須另取使用者確認。

- [x] **WORK-UI-D** — Dashboard Rhythm Merge And Intelligence IA Cleanup
  - 2026-04-19：以 `cherry-pick -n 5563dc6` 為基底，保留目前分支已 accepted 的 staged `/intelligence` shell、shared runtime polling 與真實日期 `Browsing Rhythm` contract，只把有價值的 dashboard yearly rhythm / storage regrouping contract 手工移植回來；原分支那版 weekday × hour 主圖與較舊的 `/intelligence` layout 沒有被恢復。
  - 重寫 [`src/components/intelligence/browsing-rhythm-card.tsx`](../../src/components/intelligence/browsing-rhythm-card.tsx) 與 [`src/components/intelligence/browsing-rhythm-card.css`](../../src/components/intelligence/browsing-rhythm-card.css)，把共用 card 改成真正沿用目前的 calendar-date heatmap：Dashboard 會固定以 calendar year 呈現，年份切換來自 `getDiscoveryTrend(..., 'day').availableYears`；同日 digest / top sites / hourly strip 仍維持 lazy-load，只有真的選中某一天才會抓 detail。
  - 更新 [`src-tauri/crates/vault-core/src/intelligence/mod.rs`](../../src-tauri/crates/vault-core/src/intelligence/mod.rs)、[`src-tauri/crates/vault-core/src/models/core_intelligence.rs`](../../src-tauri/crates/vault-core/src/models/core_intelligence.rs)、[`src/lib/core-intelligence/types.ts`](../../src/lib/core-intelligence/types.ts)、[`src/lib/backend.ts`](../../src/lib/backend.ts)、[`src/lib/core-intelligence/api.test.ts`](../../src/lib/core-intelligence/api.test.ts) 與 [`src/lib/core-intelligence/hooks.test.ts`](../../src/lib/core-intelligence/hooks.test.ts)，把 `availableYears` 正式掛到 `DiscoveryTrend`，並補上 Rust / Vitest 契約，避免 hourly detail API 再被誤當成年份列表來源。
  - 更新 [`src/pages/dashboard/index.tsx`](../../src/pages/dashboard/index.tsx)、[`src/lib/storage-analytics.ts`](../../src/lib/storage-analytics.ts)、[`src/lib/storage-analytics.test.ts`](../../src/lib/storage-analytics.test.ts)、[`src/pages/intelligence/sections.tsx`](../../src/pages/intelligence/sections.tsx)、[`src/pages/intelligence/sections/health.tsx`](../../src/pages/intelligence/sections/health.tsx)、[`src/components/intelligence/storage-analytics.css`](../../src/components/intelligence/storage-analytics.css) 與 [`src/pages/intelligence-surfaces.test.tsx`](../../src/pages/intelligence-surfaces.test.tsx)，把 storage analytics 的 top-level summary 改成 `core history` / `other data`，並把 health tail 補回 `/intelligence` secondary grid。
  - 同一輪把 `On This Day` 從 `/intelligence` 拿掉，改成 Dashboard-only：更新 [`docs/architecture/data-model.md`](../architecture/data-model.md)、[`docs/design/screens-and-nav.md`](../design/screens-and-nav.md)、[`docs/design/intelligence-ui-redesign-brief.md`](../design/intelligence-ui-redesign-brief.md)、[`docs/features/intelligence.md`](../features/intelligence.md)、[`docs/features/intelligence-current-state.md`](../features/intelligence-current-state.md)、[`docs/features/core-intelligence-ultimate-design.md`](../features/core-intelligence-ultimate-design.md)、[`docs/plan/program/research-and-decisions.md`](program/research-and-decisions.md)、[`docs/plan/e2e-workflow-tests.md`](e2e-workflow-tests.md)、[`docs/plan/manual-jobs-insights-truth-pass.md`](manual-jobs-insights-truth-pass.md)、[`docs/plan/core-intelligence-progress.md`](core-intelligence-progress.md) 與 [`docs/plan/core-intelligence-handoff.md`](core-intelligence-handoff.md)，把 dashboard-only ownership、calendar-year preview 與 storage regrouping 寫回 source docs。

- [x] **WORK-M6-A** — Shared Day And Domain Insights
  - 讀先：
    `docs/features/intelligence.md`
    `docs/features/intelligence-current-state.md`
    `docs/features/core-intelligence-ultimate-design.md`
    `docs/design/screens-and-nav.md`
    `docs/design/ui-review-guardrails.md`
    `docs/architecture/desktop-command-surface.md`
  - 目標：把 `day` 與 `domain` 升格成 Core Intelligence 的 first-class shared entity surface，讓 `/intelligence`、Dashboard、Explorer 與其他 active cards 不再各自重做完整 detail / route grammar。
  - 契約：`/intelligence/day/:date` 是 exact local day 的唯一完整頁面；`/intelligence/domain/:domain` user-facing IA 正式視為 `Domain Insights`；`Insights first` 成為 day/domain 的主互動，Explorer evidence 降為 secondary CTA；mutation controls 仍留在 Settings / Jobs。
  - 2026-04-19：新增 [`src-tauri/crates/vault-core/src/intelligence/day_insights.rs`](../../src-tauri/crates/vault-core/src/intelligence/day_insights.rs) focused read model、[`get_day_insights`](../../src-tauri/src/commands/intelligence.rs) typed desktop command、對應的 worker / bridge wiring，以及 `DayInsights*` Rust/TS contract；同時補上 day/domain consistency regression tests，讓 exact-day window、profile scope、domain trend 與 Explorer drilldown metadata 都有單一來源。
  - 更新 [`src/lib/core-intelligence/routes.ts`](../../src/lib/core-intelligence/routes.ts)、[`src/lib/intelligence.ts`](../../src/lib/intelligence.ts)、[`src/pages/intelligence/day-insights.tsx`](../../src/pages/intelligence/day-insights.tsx)、[`src/pages/intelligence/index.tsx`](../../src/pages/intelligence/index.tsx) 與 [`src/app/router.tsx`](../../src/app/router.tsx)，正式落地 `/intelligence/day/:date`、shared href grammar、`Insight Access` strip，以及 route-level day/domain entry。
  - 更新 [`src/components/intelligence/browsing-rhythm-card.tsx`](../../src/components/intelligence/browsing-rhythm-card.tsx)、[`src/pages/dashboard/index.tsx`](../../src/pages/dashboard/index.tsx)、[`src/pages/intelligence/domain-deep-dive.tsx`](../../src/pages/intelligence/domain-deep-dive.tsx)、[`src/pages/intelligence/sections.tsx`](../../src/pages/intelligence/sections.tsx)、[`src/pages/explorer/panels/detail-panel.tsx`](../../src/pages/explorer/panels/detail-panel.tsx) 與相關 tests，讓 Dashboard、Intelligence、Explorer 的 active day/domain surface 都改吃 shared route-first contract；Explorer detail rail 也補上 `Open day insights` / `Open domain insights` / exact-day evidence CTA。
  - 新增 [`docs/design/intelligence-entity-route-tradeoff.md`](../design/intelligence-entity-route-tradeoff.md)、[`docs/plan/m6-shared-insight-surfaces/README.md`](m6-shared-insight-surfaces/README.md) 與 [`docs/plan/m7-reuse-audit/README.md`](m7-reuse-audit/README.md)，並同步回寫 [`docs/plan/README.md`](README.md)、[`docs/milestones.md`](../milestones.md)、[`docs/plan/program/research-and-decisions.md`](program/research-and-decisions.md)、[`docs/features/intelligence.md`](../features/intelligence.md)、[`docs/features/intelligence-current-state.md`](../features/intelligence-current-state.md)、[`docs/features/core-intelligence-ultimate-design.md`](../features/core-intelligence-ultimate-design.md)、[`docs/design/screens-and-nav.md`](../design/screens-and-nav.md)、[`docs/design/ui-review-guardrails.md`](../design/ui-review-guardrails.md)、[`docs/architecture/desktop-command-surface.md`](../architecture/desktop-command-surface.md)、[`docs/plan/STATUS.md`](STATUS.md) 與 [`docs/plan/BACKLOG.md`](BACKLOG.md)，把 `Insights first`、`TODO: M7` 與下一輪 reuse audit 計劃寫回 source-of-truth。
  - 驗收：`bun run check`、`bun run build`

- [x] **WORK-M7-A** — Cross-App Reuse Audit And Insight Entity Consolidation
  - 讀先：
    `docs/plan/m7-reuse-audit/README.md`
    `docs/plan/m6-shared-insight-surfaces/README.md`
    `docs/design/intelligence-entity-route-tradeoff.md`
    `docs/features/intelligence-current-state.md`
    `docs/features/core-intelligence-ultimate-design.md`
    `docs/design/screens-and-nav.md`
  - 目標：全面盤點 app 內仍然重複造輪子的 intelligence entity surface，抽出 generic insight-entity navigation / digest / route grammar，並清理本輪留下的 `TODO: M7`。
  - 契約：不得推翻 M6 已接受的 `Insights first` / entity-first route baseline；本輪重點是 reuse audit 與 single source of truth，不是再擴一輪大型新 feature。凡是尚未納入 generic entity contract 的 active surface，都要在 docs / status / code TODO 之間保持可追蹤對應。
  - 2026-04-19：更新 [`src/lib/intelligence.ts`](../../src/lib/intelligence.ts)、[`src/components/intelligence/entity-actions.tsx`](../../src/components/intelligence/entity-actions.tsx)、[`src/components/intelligence/entity-hero.tsx`](../../src/components/intelligence/entity-hero.tsx)、[`src/pages/intelligence/promoted-entity-routes.tsx`](../../src/pages/intelligence/promoted-entity-routes.tsx) 與 [`src/app/router.tsx`](../../src/app/router.tsx)，正式落地 generic `InsightEntityTarget` / href contract、shared entity CTA chrome，以及 `/intelligence/query-family/:familyId`、`/intelligence/refind/:canonicalUrl`、`/intelligence/session/:sessionId`、`/intelligence/trail/:trailId` 四條 first-class shared insights route。
  - 更新 [`src/lib/core-intelligence/types.ts`](../../src/lib/core-intelligence/types.ts)、[`src/lib/core-intelligence/api.ts`](../../src/lib/core-intelligence/api.ts)、[`src/lib/backend.ts`](../../src/lib/backend.ts)、[`src-tauri/crates/vault-core/src/models/core_intelligence.rs`](../../src-tauri/crates/vault-core/src/models/core_intelligence.rs)、[`src-tauri/crates/vault-core/src/intelligence/mod.rs`](../../src-tauri/crates/vault-core/src/intelligence/mod.rs)、[`src-tauri/crates/vault-worker/src/intelligence.rs`](../../src-tauri/crates/vault-worker/src/intelligence.rs)、[`src-tauri/src/commands/intelligence.rs`](../../src-tauri/src/commands/intelligence.rs)、[`src-tauri/src/worker_bridge/intelligence.rs`](../../src-tauri/src/worker_bridge/intelligence.rs) 與 [`src-tauri/src/dev_ipc_bridge.rs`](../../src-tauri/src/dev_ipc_bridge.rs)，補齊 `get_query_family_detail`、`get_refind_page_detail`、`HardTopic.familyId`、`CompareSet.trailId`、以及 promoted route read model 的 typed desktop command surface。
  - 更新 [`src/pages/intelligence/index.tsx`](../../src/pages/intelligence/index.tsx)、[`src/pages/intelligence/day-insights.tsx`](../../src/pages/intelligence/day-insights.tsx)、[`src/pages/intelligence/domain-deep-dive.tsx`](../../src/pages/intelligence/domain-deep-dive.tsx)、[`src/pages/intelligence/sections.tsx`](../../src/pages/intelligence/sections.tsx)、[`src/pages/intelligence/sections/search-and-activity-section.tsx`](../../src/pages/intelligence/sections/search-and-activity-section.tsx)、[`src/pages/intelligence/sections/secondary-sections.tsx`](../../src/pages/intelligence/sections/secondary-sections.tsx)、[`src/pages/explorer/panels/detail-panel.tsx`](../../src/pages/explorer/panels/detail-panel.tsx)、[`src/pages/explorer/panels/session-group.tsx`](../../src/pages/explorer/panels/session-group.tsx)、[`src/pages/explorer/panels/trail-group.tsx`](../../src/pages/explorer/panels/trail-group.tsx) 與 [`src/pages/settings/external-outputs-panel.tsx`](../../src/pages/settings/external-outputs-panel.tsx)，把 `query family`、`refind page`、`session`、`trail`、`reopened investigation`、`habit/stable source/friction/multi-browser diff`、`compare set`、path-flow domain chips、以及 external-output day/domain chips 全部收斂到 shared destination。
  - 清理 M6 遺留 `TODO: M7`：[`src-tauri/crates/vault-core/src/intelligence/day_insights.rs`](../../src-tauri/crates/vault-core/src/intelligence/day_insights.rs) 改用 day-specific helper naming，[`src/pages/intelligence/sections/secondary-sections.tsx`](../../src/pages/intelligence/sections/secondary-sections.tsx) 的 path-flow follow-up 改記 `TODO: M8`，[`src/pages/settings/external-outputs-panel.tsx`](../../src/pages/settings/external-outputs-panel.tsx) 則不再保留 static chips。
  - 擴寫 [`src/lib/intelligence.test.ts`](../../src/lib/intelligence.test.ts)、[`src/lib/core-intelligence/api.test.ts`](../../src/lib/core-intelligence/api.test.ts) 與 [`src/pages/intelligence-surfaces.test.tsx`](../../src/pages/intelligence-surfaces.test.tsx)，覆蓋 generic href grammar、encoded refind URL、anchor resolution、以及 promoted routes 的 load/error/CTA 行為；本輪驗收已通過 `bun run check` 與 `bun run build`。
  - 新增 [`docs/design/intelligence-generic-entity-navigation-tradeoff.md`](../design/intelligence-generic-entity-navigation-tradeoff.md) 與 [`docs/plan/m8-aggregate-entity-identity/README.md`](m8-aggregate-entity-identity/README.md)，並同步回寫 [`docs/plan/m7-reuse-audit/README.md`](m7-reuse-audit/README.md)、[`docs/plan/m6-shared-insight-surfaces/README.md`](m6-shared-insight-surfaces/README.md)、[`docs/plan/program/research-and-decisions.md`](program/research-and-decisions.md)、[`docs/features/intelligence.md`](../features/intelligence.md)、[`docs/features/intelligence-current-state.md`](../features/intelligence-current-state.md)、[`docs/features/core-intelligence-ultimate-design.md`](../features/core-intelligence-ultimate-design.md)、[`docs/design/screens-and-nav.md`](../design/screens-and-nav.md)、[`docs/design/ui-review-guardrails.md`](../design/ui-review-guardrails.md)、[`docs/architecture/desktop-command-surface.md`](../architecture/desktop-command-surface.md)、[`docs/plan/README.md`](README.md)、[`docs/milestones.md`](../milestones.md)、[`docs/plan/STATUS.md`](STATUS.md) 與 [`docs/plan/BACKLOG.md`](BACKLOG.md)，把 M7 accepted truth 與 M8 deferred scope 寫回 source-of-truth。
  - 驗收：`bun run check`、`bun run build`

- [x] **WORK-M8-A** — Aggregate Entity Identity And Context Reuse
  - 讀先：
    `docs/plan/m8-aggregate-entity-identity/README.md`
    `docs/plan/m7-reuse-audit/README.md`
    `docs/design/intelligence-generic-entity-navigation-tradeoff.md`
    `docs/features/intelligence-current-state.md`
    `docs/features/core-intelligence-ultimate-design.md`
    `docs/design/screens-and-nav.md`
  - 目標：補齊 M7 故意 deferred 的 aggregate entity identity、context focus 與 reusable payload id 缺口，避免 shared destination 仍靠 best-effort parsing 或 page-local state 維持。
  - 契約：不得推翻 M6/M7 已接受的 `Insights first` / entity-first route baseline；本輪重點是 identity / context reuse，不是再擴一輪大型新 feature 或重開 consumer-local deep-link。凡是剩餘的 deferred reuse gap，都要改用 `TODO: M8` 並在 docs / status / code TODO 之間保持可追蹤對應。
  - 2026-04-19：新增 [`docs/design/intelligence-aggregate-entity-focus-tradeoff.md`](../design/intelligence-aggregate-entity-focus-tradeoff.md)，正式接受 compare-set promotion、shared `focusType` / `focusId` query grammar、path-flow typed identity，以及 trusted external-output structured targets；同時新增 [`docs/plan/m9-cross-app-reuse/README.md`](m9-cross-app-reuse/README.md)，把下一輪 reuse inventory / extraction 立項。
  - 更新 [`src-tauri/crates/vault-core/src/models/core_intelligence.rs`](../../src-tauri/crates/vault-core/src/models/core_intelligence.rs)、[`src-tauri/crates/vault-core/src/intelligence/{mod.rs,phase_three.rs,phase_four.rs}`](../../src-tauri/crates/vault-core/src/intelligence/mod.rs)、[`src-tauri/crates/vault-core/src/intelligence_catalog.rs`](../../src-tauri/crates/vault-core/src/intelligence_catalog.rs)、[`src-tauri/crates/vault-worker/src/{intelligence.rs,lib.rs}`](../../src-tauri/crates/vault-worker/src/intelligence.rs)、[`src-tauri/src/{commands/intelligence.rs,worker_bridge/intelligence.rs,dev_ipc_bridge.rs,lib.rs}`](../../src-tauri/src/commands/intelligence.rs) 與 [`src/lib/core-intelligence/{types.ts,api.ts,routes.ts}`](../../src/lib/core-intelligence/types.ts)，補齊 `CompareSetDetail` / `get_compare_set_detail`、`InsightEntityReference`、trusted payload structured targets、`flowId` / typed path-flow steps，以及對應的 desktop command / worker / TS contract。
  - 更新 [`src/lib/intelligence.ts`](../../src/lib/intelligence.ts)、[`src/pages/intelligence/{index.tsx,route-state.ts,day-insights.tsx,domain-deep-dive.tsx,promoted-entity-routes.tsx}`](../../src/pages/intelligence/index.tsx)、[`src/pages/intelligence/sections{.tsx,/secondary-sections.tsx}`](../../src/pages/intelligence/sections.tsx)、[`src/app/router.tsx`](../../src/app/router.tsx)、[`src/pages/settings/external-outputs-panel.tsx`](../../src/pages/settings/external-outputs-panel.tsx)、[`src/components/intelligence/explainability-panel.tsx`](../../src/components/intelligence/explainability-panel.tsx)、[`src/lib/i18n/catalog.ts`](../../src/lib/i18n/catalog.ts) 與 [`src/lib/backend.ts`](../../src/lib/backend.ts)，正式落地 compare-set route、focus-aware trail/day/domain highlighting、path-flow focus chips、structured `Open insights` links，以及 compare-set explainability / i18n surface。
  - 擴寫 [`src/lib/intelligence.test.ts`](../../src/lib/intelligence.test.ts)、[`src/lib/core-intelligence/{api.test.ts,routes.test.ts}`](../../src/lib/core-intelligence/api.test.ts) 與 [`src/pages/intelligence-surfaces.test.tsx`](../../src/pages/intelligence-surfaces.test.tsx)，覆蓋 compare-set href / route、focus query grammar、path-flow typed steps、structured target links，以及 compare-set context highlighting。
  - 同步回寫 [`docs/design/intelligence-generic-entity-navigation-tradeoff.md`](../design/intelligence-generic-entity-navigation-tradeoff.md)、[`docs/design/screens-and-nav.md`](../design/screens-and-nav.md)、[`docs/features/intelligence-current-state.md`](../features/intelligence-current-state.md)、[`docs/features/core-intelligence-ultimate-design.md`](../features/core-intelligence-ultimate-design.md)、[`docs/features/intelligence.md`](../features/intelligence.md)、[`docs/architecture/data-model.md`](../architecture/data-model.md)、[`docs/architecture/desktop-command-surface.md`](../architecture/desktop-command-surface.md)、[`docs/plan/program/research-and-decisions.md`](program/research-and-decisions.md)、[`docs/plan/README.md`](README.md)、[`docs/milestones.md`](../milestones.md)、[`docs/plan/STATUS.md`](STATUS.md) 與 [`docs/plan/BACKLOG.md`](BACKLOG.md)，把 M8 accepted truth 與 M9 seed plan 寫回 source-of-truth。
  - 驗收：`bun run check`、`bun run build`

- [x] **WORK-M9-A** — Remaining Reuse Inventory And Single-Source Map
  - 讀先：
    `docs/plan/m9-cross-app-reuse/README.md`
    `docs/plan/m8-aggregate-entity-identity/README.md`
    `docs/design/intelligence-aggregate-entity-focus-tradeoff.md`
    `docs/features/intelligence-current-state.md`
    `docs/features/core-intelligence-ultimate-design.md`
    `docs/design/screens-and-nav.md`
  - 目標：掃描整個 app 內仍然存在的 consumer-local composition、duplicated helper / read-model glue、以及跨 Dashboard / Intelligence / Explorer / Settings 的 shared review chrome 漂移，建立 single-source map。
  - 契約：不得推翻 M6–M8 已接受的 entity-first / focus / trusted-output 邊界；這一輪先做 inventory、抽象邊界與 source-of-truth 收斂，不把它擴成新的分析算法里程碑。凡是新的 deferred gap，都要改記 `TODO: M9` 或 `TODO: M10`，並在 docs / status / code TODO 間保持可追蹤。
  - 2026-04-19：新增 [`docs/design/intelligence-shared-route-composition-tradeoff.md`](../design/intelligence-shared-route-composition-tradeoff.md)，正式接受「M9 先抽 route-level shared composition，不擴成 backend transport refactor」的邊界；同時新增 [`docs/plan/m10-workbench-reuse/README.md`](m10-workbench-reuse/README.md)，把下一輪 workbench reuse / glue decomposition seed 正式立項。
  - 同步回寫 [`docs/plan/m9-cross-app-reuse/README.md`](m9-cross-app-reuse/README.md)、[`docs/plan/program/research-and-decisions.md`](program/research-and-decisions.md)、[`docs/design/screens-and-nav.md`](../design/screens-and-nav.md)、[`docs/design/ui-review-guardrails.md`](../design/ui-review-guardrails.md)、[`docs/features/intelligence-current-state.md`](../features/intelligence-current-state.md)、[`docs/features/core-intelligence-ultimate-design.md`](../features/core-intelligence-ultimate-design.md)、[`docs/milestones.md`](../milestones.md) 與 [`docs/plan/README.md`](README.md)，把 shared composition inventory、section-meta header chrome 與 M10 邊界寫回 source-of-truth。
  - 驗收：`bun run check`、`bun run build`

- [x] **WORK-M9-B** — Shared Digest / CTA / Evidence Composition Extraction
  - 讀先：
    `docs/plan/m9-cross-app-reuse/README.md`
    `docs/design/screens-and-nav.md`
    `docs/features/intelligence-current-state.md`
    `docs/features/core-intelligence-ultimate-design.md`
    `docs/design/ui-review-guardrails.md`
  - 目標：根據 `WORK-M9-A` 的 inventory，把至少一輪高價值 shared composition（例如 digest/meta rows、CTA hierarchy、evidence/focus review chrome、target-link derivation）從多個 consumer 中正式抽離，降低 drift 與重複造輪子。
  - 契約：只抽明確跨 consumer 重複且能降低 drift 風險的 composition；不得為了抽象而引入新的 global state、又或重新把 route / focus contract page-local 化。
  - 2026-04-19：新增 [`src/components/intelligence/{metric-grid.tsx,query-family-card.tsx,compare-set-page-list.tsx}`](../../src/components/intelligence/metric-grid.tsx) 與 [`src/lib/intelligence.ts`](../../src/lib/intelligence.ts) 的 structured target label helper，正式收斂 route-level metric strip、`query-family-card`、compare-set page list、以及 trusted output target label 的 shared primitive。
  - 更新 [`src/pages/intelligence/{sections.tsx,day-insights.tsx,promoted-entity-routes.tsx}`](../../src/pages/intelligence/sections.tsx)、[`src/pages/intelligence/sections/{search-and-activity-section.tsx,secondary-sections.tsx}`](../../src/pages/intelligence/sections/search-and-activity-section.tsx)、[`src/pages/settings/external-outputs-panel.tsx`](../../src/pages/settings/external-outputs-panel.tsx) 與 [`src/pages/intelligence/intelligence.css`](../../src/pages/intelligence/intelligence.css)，讓 overview / promoted routes / Settings 都改吃 shared composition；`證據與新鮮度` badge 也回到 inline-end header chrome，hover 命中區不再吃滿整個 card header。
  - 擴寫 [`src/lib/intelligence.test.ts`](../../src/lib/intelligence.test.ts) 與 [`src/pages/intelligence-surfaces.test.tsx`](../../src/pages/intelligence-surfaces.test.tsx)，覆蓋 structured target labels、Settings external-output chips，以及 route-level shared composition adoption；本輪驗收通過 `bun run check` 與 `bun run build`。
  - 新增 `TODO: M10` 追蹤：[`src/pages/intelligence/sections.tsx`](../../src/pages/intelligence/sections.tsx) 與 [`src/pages/intelligence/day-insights.tsx`](../../src/pages/intelligence/day-insights.tsx) 現在都明確標記 `refind` summary/detail chrome 留待下一輪 workbench reuse 收斂。
  - 同步回寫 [`docs/plan/STATUS.md`](STATUS.md)、[`docs/plan/BACKLOG.md`](BACKLOG.md) 與 [`docs/plan/README.md`](README.md)，把 M9 closeout 與 M10 active current-focus 寫回 planning tracking。
  - 驗收：`bun run check`、`bun run build`

- [x] **WORK-M10-A** — Shared Review Rows And Workbench Surface Reuse
  - 讀先：
    `docs/plan/m10-workbench-reuse/README.md`
    `docs/plan/m9-cross-app-reuse/README.md`
    `docs/design/intelligence-shared-route-composition-tradeoff.md`
    `docs/features/intelligence-current-state.md`
    `docs/features/core-intelligence-ultimate-design.md`
    `docs/design/screens-and-nav.md`
  - 目標：收斂仍然 consumer-local 的 workbench / review row composition，優先處理 `refind` summary/detail、Explorer detail/session/trail row，以及 richer Settings review chrome 的重複造輪子。
  - 契約：不得推翻 M6–M9 已接受的 entity-first / focus / trusted-output / shared-composition 邊界；本輪重點是 reusable workbench surface，不是新的 route grammar 或大型視覺重設。凡是新的 deferred gap，都要改記後續 milestone，且在 docs / status / code TODO 間保持可追蹤。
  - 2026-04-19：新增 [`src/components/intelligence/workbench/`](../../src/components/intelligence/workbench) shared layer，正式收斂 `refind` summary/factor shell、Explorer grouped group-card / member-row primitive，以及 Settings external-output / local-host 的 review row / code preview / target-link building blocks。
  - 更新 [`src/pages/intelligence/{sections.tsx,day-insights.tsx,promoted-entity-routes.tsx}`](../../src/pages/intelligence/sections.tsx)、[`src/pages/explorer/panels/{session-group.tsx,trail-group.tsx}`](../../src/pages/explorer/panels/session-group.tsx)、[`src/pages/settings/{external-outputs-panel.tsx,external-output-local-host-panel.tsx}`](../../src/pages/settings/external-outputs-panel.tsx) 與 [`src/pages/intelligence-surfaces.test.tsx`](../../src/pages/intelligence-surfaces.test.tsx)，讓 overview / promoted route / Explorer / Settings 都改吃 shared workbench contract，並清掉 M9 留下的 `TODO: M10`。
  - 新增 [`src/components/intelligence/workbench/workbench.test.tsx`](../../src/components/intelligence/workbench/workbench.test.tsx) 與對應 route regression，覆蓋 shared refind card、expandable group-card、selectable workbench row、以及 target-link row 的核心互動。
  - 驗收：targeted Vitest intelligence surface / workbench / API regression 已通過；全量驗收見本輪 closeout commit。

- [x] **WORK-M10-B** — Intelligence Route And Desktop Glue Decomposition
  - 讀先：
    `docs/plan/m10-workbench-reuse/README.md`
    `docs/design/intelligence-shared-route-composition-tradeoff.md`
    `docs/design/screens-and-nav.md`
    `docs/features/intelligence-current-state.md`
    `docs/features/core-intelligence-ultimate-design.md`
    `docs/architecture/desktop-command-surface.md`
  - 目標：盤點 intelligence route files、Tauri command / worker bridge / TS invoke wrapper 仍存在的重複 glue，決定哪些值得正式拆分與去重，哪些只留 inventory。
  - 契約：不得把 M10-B 擴成大規模 desktop contract rewrite；只處理 ownership 清晰、能降低 drift 或 mega-file 壓力的 split。route grammar、payload shape 與 accepted transport boundary 不得因為「想簡化」就被改寫。
  - 2026-04-19：新增 [`docs/design/intelligence-workbench-transport-hygiene-tradeoff.md`](../design/intelligence-workbench-transport-hygiene-tradeoff.md)，正式接受「shared workbench + ownership-based split、但 public contract 不變」的 M10 邊界；並新增 [`docs/plan/m11-app-wide-reuse/README.md`](m11-app-wide-reuse/README.md) 作為下一輪 inventory / extraction seed。
  - 更新 [`src/pages/intelligence/promoted-entity-routes.tsx`](../../src/pages/intelligence/promoted-entity-routes.tsx) 與新拆出的 [`src/pages/intelligence/promoted-entity-routes/`](../../src/pages/intelligence/promoted-entity-routes)，把 query-family / refind / session / trail / compare-set route page 拆回 per-route ownership。
  - 更新 [`src/lib/core-intelligence/api.ts`](../../src/lib/core-intelligence/api.ts) 與新拆出的 [`src/lib/core-intelligence/api/`](../../src/lib/core-intelligence/api)，把 invoke/normalize/cache helpers、overview read models、entity read models 與 runtime/output payload provider 正式分桶，同時保留既有 `./api` import path。
  - 更新 [`src-tauri/src/{commands,worker_bridge}/intelligence.rs`](../../src-tauri/src/commands/intelligence.rs) 與新拆出的 `ai` / `core` / `runtime` submodules，降低 command facade / worker bridge mega-file 壓力，但維持 command name 與 transport payload 完全不變；同時把 `src/lib/intelligence.ts`、`src-tauri/src/dev_ipc_bridge.rs`、與剩餘 `vault-worker` pass-through debt 明確改記 `TODO: M11`。
  - 同步回寫 [`docs/plan/{STATUS.md,BACKLOG.md,README.md}`](STATUS.md)、[`docs/plan/m10-workbench-reuse/README.md`](m10-workbench-reuse/README.md)、[`docs/plan/program/research-and-decisions.md`](program/research-and-decisions.md)、[`docs/features/intelligence-current-state.md`](../features/intelligence-current-state.md)、[`docs/design/screens-and-nav.md`](../design/screens-and-nav.md) 與 [`docs/milestones.md`](../milestones.md)，把 M10 closeout 與 M11 active current-focus 寫回 source-of-truth。
  - 驗收：`bun run check`、`bun run build`

- [x] **WORK-M11-A** — App-Wide Reuse Inventory And Single-Source Map
  - 讀先：
    `docs/plan/m11-app-wide-reuse/README.md`
    `docs/plan/m10-workbench-reuse/README.md`
    `docs/design/intelligence-workbench-transport-hygiene-tradeoff.md`
    `docs/design/screens-and-nav.md`
    `docs/features/intelligence-current-state.md`
  - 目標：盤點全 app 仍然重複造輪子的 review / PME / diagnostics surface，以及 `src/lib/intelligence.ts`、dev IPC mirror、`vault-worker` pass-through 等 mixed helper / transport glue，建立 single-source map。
  - 契約：不得重開 M6–M10 已接受的 route grammar、payload shape、trusted-output boundary；這一輪先做 inventory、boundary 與 source-of-truth 收斂，不把 M11 又擴成新的 feature milestone。
  - 2026-04-19：新增 [`docs/design/app-wide-review-grammar-tradeoff.md`](../design/app-wide-review-grammar-tradeoff.md)，正式接受 M11 的 canonical owner map：entity route grammar 回 [`src/lib/core-intelligence/routes.ts`](../../src/lib/core-intelligence/routes.ts)，AI/provider/assistant presentation 與 evidence/assistant link helper 分別回各自 owner，neutral review shell 則升格到 app-wide primitive；transport 只做 inventory，不重開 codegen。
  - 更新 [`docs/plan/m11-app-wide-reuse/README.md`](m11-app-wide-reuse/README.md)、[`docs/plan/program/research-and-decisions.md`](program/research-and-decisions.md)、[`docs/milestones.md`](../milestones.md) 與 [`docs/plan/README.md`](README.md)，把 single-source map、consumer-local drift inventory、`PG-RD-UX-012` 與 M12 seed plan 寫回 source-of-truth。
  - 新增 [`docs/plan/m12-support-actions-and-diagnostics/README.md`](m12-support-actions-and-diagnostics/README.md) 並同步回寫 [`docs/plan/BACKLOG.md`](BACKLOG.md) 與 [`docs/plan/STATUS.md`](STATUS.md)，把下一輪 `WORK-M12-A` / `WORK-M12-B` seed 先落回 planning docs，同時保留 `WORK-M11-B` 為當前 active block。
  - 驗收：source docs、inventory map、`TODO: M11` 對應與後續抽取策略存在

- [x] **WORK-M11-B** — Shared Review / PME / Diagnostics Surface Extraction
  - 讀先：
    `docs/plan/m11-app-wide-reuse/README.md`
    `docs/design/screens-and-nav.md`
    `docs/design/ux-principles.md`
    `docs/features/intelligence-current-state.md`
    `docs/plan/e2e-workflow-tests.md`
  - 目標：根據 `WORK-M11-A` 的 inventory，把至少一輪跨 route 的 shared review / PME / diagnostics primitive 抽離，優先處理 Settings / Jobs / Import / Audit 之間仍漂移的 review row、code preview、target-link、verify/result grammar。
  - 契約：只抽明確跨 consumer 重複且能降低 drift 的 grammar；不得為了抽象而引入新的 global state、也不得回退成 page-local trust / PME copy。
  - 2026-04-19：新增 [`src/components/review/`](../../src/components/review/index.ts) neutral layer，正式升格 `review-surface`、`PmeTabBar`、`GeneratedArtifactViewer`、`VerifyCheckList`；[`src/components/intelligence/workbench/review-surface.tsx`](../../src/components/intelligence/workbench/review-surface.tsx) 現在只保留 compatibility re-export。
  - 更新 [`src/lib/{intelligence.ts,intelligence-ai-presentation.ts,intelligence-links.ts}`](../../src/lib/intelligence.ts) 與 [`src/lib/core-intelligence/routes.ts`](../../src/lib/core-intelligence/routes.ts)，把 route grammar、AI/provider/assistant presentation、以及 evidence/assistant links 各自拆回 canonical owner；`src/lib/intelligence.test.ts` 也改成直接驗 canonical modules，而不是只透過 mixed barrel。
  - 更新 [`src/pages/settings/{index.tsx,external-outputs-panel.tsx,external-output-local-host-panel.tsx}`](../../src/pages/settings/index.tsx)、[`src/pages/schedule/index.tsx`](../../src/pages/schedule/index.tsx)、[`src/pages/audit/panels/run-detail.tsx`](../../src/pages/audit/panels/run-detail.tsx) 與 [`src/pages/jobs/index.tsx`](../../src/pages/jobs/index.tsx)，讓 Settings remote backup / AI integration / local-host review、Schedule PME、Audit artifact rows、以及 Jobs recent job rows 都改吃 shared review grammar。
  - 更新 [`src-tauri/src/{dev_ipc_bridge.rs,worker_bridge/intelligence.rs}`](../../src-tauri/src/dev_ipc_bridge.rs)，把剩餘 transport follow-up 從 `TODO: M11` 正式改記到 M12 parity inventory；本輪不再機械拆 transport glue。
  - 新增 [`src/components/review/review.test.tsx`](../../src/components/review/review.test.tsx) 並維持 [`src/pages/intelligence-surfaces.test.tsx`](../../src/pages/intelligence-surfaces.test.tsx)、[`src/pages/trust-flows.test.tsx`](../../src/pages/trust-flows.test.tsx) 與 [`src/lib/intelligence.test.ts`](../../src/lib/intelligence.test.ts) 綠燈，覆蓋 code preview、target links、generated artifact viewer、verify rows、PME tabs，以及受影響的 Settings / Schedule / Audit / Jobs surface。
  - 同步回寫 [`docs/features/intelligence-current-state.md`](../features/intelligence-current-state.md)、[`docs/design/screens-and-nav.md`](../design/screens-and-nav.md)、[`docs/plan/m11-app-wide-reuse/README.md`](m11-app-wide-reuse/README.md)、[`docs/plan/{STATUS.md,BACKLOG.md,README.md}`](STATUS.md)，把 M11 closeout 與 M12 active current-focus 寫回 source-of-truth。
  - 驗收：`bun run check`、`bun run build`

- [x] **WORK-M12-A** — Shared Support Actions And Diagnostics Inventory
  - 讀先：
    `docs/plan/m12-support-actions-and-diagnostics/README.md`
    `docs/plan/m11-app-wide-reuse/README.md`
    `docs/design/app-wide-review-grammar-tradeoff.md`
    `docs/design/screens-and-nav.md`
    `docs/features/intelligence-current-state.md`
  - 目標：盤點全 app 還未進 shared review grammar 的 support actions、diagnostics rows、以及 Settings mega-route 的下一輪拆分機會，建立 single-source map 與 extraction boundary。
  - 契約：延續 M11 已 accepted 的 neutral review primitive / route / trusted-output 邊界；這一輪先做 inventory 與 owner map，不直接擴成大規模 route rewrite 或 transport automation 專案。
  - 2026-04-19：新增 [`docs/design/support-actions-and-diagnostics-tradeoff.md`](../design/support-actions-and-diagnostics-tradeoff.md)，正式接受 M12 的 canonical owner map：support-action grammar 只能掛在 [`src/components/review/`](../../src/components/review/index.ts)，`PathRow` 不再視為 active single source。
  - 更新 [`docs/plan/m12-support-actions-and-diagnostics/README.md`](m12-support-actions-and-diagnostics/README.md)、[`docs/plan/program/research-and-decisions.md`](program/research-and-decisions.md)、[`docs/plan/README.md`](README.md) 與 [`docs/milestones.md`](../milestones.md)，把 M12 inventory、owner map、deferred list 與 M13 seed milestone 回寫到 source-of-truth。
  - 新增 [`docs/plan/m13-broad-reuse-audit/README.md`](m13-broad-reuse-audit/README.md)，把 broad reuse audit 正式立項成下一輪 milestone；主題固定為 support / trust / workflow reuse，而不是 Settings-only split 或 transport-first 專案。
  - 驗收：source docs、inventory map、後續 `TODO: M13` 與 extraction strategy 存在

- [x] **WORK-M12-B** — Support Action / Diagnostics Primitive Extraction
  - 讀先：
    `docs/plan/m12-support-actions-and-diagnostics/README.md`
    `docs/design/screens-and-nav.md`
    `docs/design/ux-principles.md`
    `docs/features/intelligence-current-state.md`
    `docs/plan/e2e-workflow-tests.md`
  - 目標：根據 `WORK-M12-A` 的 inventory，把至少一輪高價值的 shared support action / diagnostics primitive 抽離，優先處理 open-path / copy action、general diagnostics rows、以及 Settings / Import / Audit / Jobs 的 support summary drift。
  - 契約：只抽明確跨 consumer 重複且能降低 drift 的 grammar；不得為了抽象而重開 M11 已收斂的 route / payload / review shell contract。
  - 2026-04-19：新增 [`src/components/review/clipboard.ts`](../../src/components/review/clipboard.ts) 與 [`src/components/review/support-actions.tsx`](../../src/components/review/support-actions.tsx)，正式升格 shared clipboard helper 與 `ReviewPathActionRow`；[`src/components/ui.tsx`](../../src/components/ui.tsx) 的 `PathRow` 則退回 legacy fallback。
  - 更新 [`src/pages/settings/{index.tsx,external-outputs-panel.tsx,external-output-local-host-panel.tsx}`](../../src/pages/settings/index.tsx)、[`src/pages/audit/{index.tsx,hooks/use-audit-data.ts,panels/run-detail.tsx}`](../../src/pages/audit/index.tsx)、[`src/pages/import/index.tsx`](../../src/pages/import/index.tsx)、[`src/pages/schedule/index.tsx`](../../src/pages/schedule/index.tsx)、[`src/pages/security/index.tsx`](../../src/pages/security/index.tsx)、[`src/pages/lock/index.tsx`](../../src/pages/lock/index.tsx)、[`src/pages/explorer/{index.tsx,hooks/use-explorer-data.ts,panels/results-panel.tsx}`](../../src/pages/explorer/index.tsx) 與 [`src/pages/explorer/panels/privacy-redaction.test.tsx`](../../src/pages/explorer/panels/privacy-redaction.test.tsx)，讓 Settings general diagnostics / App Lock、Audit manifest / artifact review、Import selected-batch audit path、Schedule detected-file / audit path、Security / Lock config path，以及 Explorer export path 全部改吃 shared review-layer support-action contract。
  - 更新 [`src/pages/jobs/index.tsx`](../../src/pages/jobs/index.tsx)、[`src-tauri/src/dev_ipc_bridge.rs`](../../src-tauri/src/dev_ipc_bridge.rs) 與 [`src-tauri/src/worker_bridge/intelligence.rs`](../../src-tauri/src/worker_bridge/intelligence.rs) 的 deferred comments，把 Jobs plugin/module summary rows與 transport parity follow-up 明確改記 `TODO: M13`。
  - 同步回寫 [`docs/design/screens-and-nav.md`](../design/screens-and-nav.md)、[`docs/plan/m12-support-actions-and-diagnostics/README.md`](m12-support-actions-and-diagnostics/README.md)、[`docs/plan/{STATUS.md,BACKLOG.md,README.md}`](STATUS.md)，把 M12 closeout 與 M13 active current-focus 寫回 source-of-truth。
  - 驗收：`bun run check`、`bun run build`

- [x] **WORK-PERF-A** — Intelligence Performance Hot-Path Recovery
  - 讀先：
    `docs/plan/m4-full-polish/large-archive-performance-runbook.md`
    `docs/features/core-intelligence-ultimate-design.md`
    `docs/features/intelligence-current-state.md`
    `docs/design/ux-principles.md`
    `docs/database-selection-decision-2026-04-05.md`
  - 目標：先把 `/intelligence` 大資料量體驗從 stop-ship 狀態拉回可用，優先修掉 overview request-path 的重複 runtime/meta 讀取、重複 intelligence connection / attach、route revisit fan-out，以及 first-band / hidden-tab hydration drift。
  - 契約：保留現有 `/intelligence` route grammar、section command 名稱與 payload shape；不引入 click-to-load；DuckDB 仍維持 deferred / optional analytics sidecar，除非 real-data artifact 證明 SQLite hot-path 修補後仍不足。
  - 2026-04-20：更新 [`src-tauri/crates/vault-core/src/intelligence/{mod.rs,phase_three.rs,phase_four.rs}`](../../src-tauri/crates/vault-core/src/intelligence/mod.rs)、[`src-tauri/crates/vault-core/src/{intelligence_runtime.rs,intelligence_sections.rs}`](../../src-tauri/crates/vault-core/src/intelligence_runtime.rs) 與 [`src-tauri/crates/vault-worker/src/intelligence.rs`](../../src-tauri/crates/vault-worker/src/intelligence.rs)，把 primary / secondary overview 改成 batch-scoped read path：同一批只重用一條 intelligence connection、一次 archive attach、以及一份 runtime snapshot，section meta 也改從 shared runtime 生成，保留既有 command 名稱與 payload shape。
  - 更新 [`src/lib/core-intelligence/api/{shared.ts,overview.ts}`](../../src/lib/core-intelligence/api/shared.ts)、[`src/lib/core-intelligence/{hooks.ts,hooks.test.ts,api.test.ts}`](../../src/lib/core-intelligence/hooks.ts)、[`src/pages/intelligence/{use-staged-intelligence-overview.ts,sections.tsx}`](../../src/pages/intelligence/use-staged-intelligence-overview.ts)、[`src/pages/intelligence/sections/{search-and-activity-section.tsx,secondary-sections.tsx}`](../../src/pages/intelligence/sections/search-and-activity-section.tsx)、[`src/components/intelligence/browsing-rhythm-card.tsx`](../../src/components/intelligence/browsing-rhythm-card.tsx) 與 [`src/pages/intelligence-surfaces.test.tsx`](../../src/pages/intelligence-surfaces.test.tsx)，補上 scope-keyed warm cache、in-flight dedupe、stale-while-revalidate、overview seed cache，以及 Search Activity hidden tabs 的 idle prewarm；same-scope revisit 不再重打一串 per-card foreground invoke。
  - 同步回寫 [`docs/plan/{STATUS.md,BACKLOG.md}`](STATUS.md)、[`docs/plan/m13-broad-reuse-audit/README.md`](m13-broad-reuse-audit/README.md)、[`docs/plan/program/research-and-decisions.md`](program/research-and-decisions.md)、[`docs/plan/m4-full-polish/large-archive-performance-runbook.md`](m4-full-polish/large-archive-performance-runbook.md)、[`docs/features/{intelligence-current-state.md,core-intelligence-ultimate-design.md}`](../features/intelligence-current-state.md) 與 [`docs/design/ux-principles.md`](../design/ux-principles.md)，把這輪 stop-ship closeout、SQLite-first 立場，以及 warm-cache / prewarm contract 寫回 source-of-truth。
  - 驗收：targeted Rust regression（single-connection / single-runtime overview batch）、targeted Vitest intelligence cache/hydration regressions、`bun run check`、`bun run build`

- [x] **WORK-CI-R** — Search Activity Cleanup And Search Keyword Browser
  - 讀先：
    `docs/features/core-intelligence-ultimate-design.md`
    `docs/features/intelligence-current-state.md`
    `docs/design/screens-and-nav.md`
    `docs/plan/program/research-and-decisions.md`
    `docs/design/ui-review-guardrails.md`
  - 目標：修正 `Search Activity` keyword-facing surface 的 truth boundary，替換低品質的 word cloud / additive query list，並讓 search-engine domain deep-dive 正式重用同一套 keyword browser。
  - 契約：keyword / concept-facing surface 只能吃 keyword-eligible search rows，不得把 pasted URL / hostname-like navigation noise 混進排行；沿用既有 route grammar / query-family / trail / evidence deep-link；大量 keyword rows 必須走 server-backed pagination。
  - 2026-04-20：新增 [`docs/design/search-activity-keyword-browser-tradeoff.md`](../design/search-activity-keyword-browser-tradeoff.md)，正式接受 `Top Concepts` bar chart、bounded `Search Keywords` browser、domain-scoped keyword reuse，以及 compact domain scope strip 的 UI / IA 邊界；並同步回寫 [`docs/features/core-intelligence-ultimate-design.md`](../features/core-intelligence-ultimate-design.md)、[`docs/features/intelligence-current-state.md`](../features/intelligence-current-state.md)、[`docs/design/screens-and-nav.md`](../design/screens-and-nav.md)、[`docs/plan/program/research-and-decisions.md`](program/research-and-decisions.md) 與 [`docs/plan/m13-broad-reuse-audit/README.md`](m13-broad-reuse-audit/README.md)。
  - 更新 [`src-tauri/crates/vault-core/src/intelligence/mod.rs`](../../src-tauri/crates/vault-core/src/intelligence/mod.rs) 與 [`src-tauri/crates/vault-core/src/models/core_intelligence.rs`](../../src-tauri/crates/vault-core/src/models/core_intelligence.rs)，新增 `search_events.query_kind` / migration、query-shape + landing-domain noise classification、keyword-only `search_event_terms` indexing、`TopSearchConcepts` / `SearchQueryListRequest` domain filter 與對應 Rust regression tests，讓 `https` / hostname-like inputs 不再污染 keyword surfaces。
  - 新增 shared [`src/components/intelligence/search-keywords-browser.tsx`](../../src/components/intelligence/search-keywords-browser.tsx)，並更新 [`src/lib/core-intelligence/api/overview.ts`](../../src/lib/core-intelligence/api/overview.ts)、[`src/lib/backend.ts`](../../src/lib/backend.ts)、[`src/lib/i18n/catalog.ts`](../../src/lib/i18n/catalog.ts)、[`src/pages/intelligence/sections/search-and-activity-section.tsx`](../../src/pages/intelligence/sections/search-and-activity-section.tsx)、[`src/pages/intelligence/domain-deep-dive.tsx`](../../src/pages/intelligence/domain-deep-dive.tsx)、[`src/pages/intelligence/intelligence.css`](../../src/pages/intelligence/intelligence.css) 與 [`src/pages/intelligence-surfaces.test.tsx`](../../src/pages/intelligence-surfaces.test.tsx)，把 `Recent Queries` 升格成 shared `Search Keywords` browser、`Top Concepts` 改成 ranked horizontal bar chart、search-engine domain route 加上 conditional keyword history section，且 domain route 頂部改成 compact inline scope strip。
  - 驗收：`cargo test --manifest-path src-tauri/Cargo.toml -p vault-core classifies_url_like_search_queries_as_navigational_noise -- --nocapture`、`cargo test --manifest-path src-tauri/Cargo.toml -p vault-core keyword_surfaces_filter_navigational_noise_and_support_domain_reads -- --nocapture`、`cargo test --manifest-path src-tauri/Cargo.toml -p vault-core search_queries_reuse_family_and_trail_identity -- --nocapture`、`bun x vitest run src/lib/core-intelligence/api.test.ts src/pages/intelligence-surfaces.test.tsx`、`bun run check`、`bun run build`

- [x] **WORK-CI-N** — Full Desktop Truth Pass After Locked-Archive Bootstrap Recovery
  - 讀先：
    `docs/plan/core-intelligence-desktop-truth-audit.md`
    `docs/plan/core-intelligence-progress.md`
    `docs/plan/core-intelligence-handoff.md`
    `docs/plan/e2e-workflow-tests.md`
    `docs/plan/m4-full-polish/large-archive-performance-runbook.md`
  - 目標：在 current-host shell bootstrap / Security unlock 可以穩定進入 unlocked state 之後，完成原本要求的 real-data import + Core Intelligence + 全 app Computer Use truth pass，並補齊 post-unlock profiling bundle。
  - 契約：使用 Computer Use 跑 Chrome `yi-ting` profile、archive encryption `000000`（不寫入鑰匙圈）、Import / Intelligence / Domain Deep Dive / Explorer session-trail / Settings external outputs / Jobs / Audit / Schedule / Assistant 全路由；若現有 app root 仍不符合「已清資料」前提，必須先取得使用者明確確認後才能做 destructive reset。
  - 2026-04-20：更新 [`index.html`](../../index.html)、[`src/components/ui.tsx`](../../src/components/ui.tsx)、[`src/pages/settings/index.tsx`](../../src/pages/settings/index.tsx)、[`src/components/intelligence/entity-actions.tsx`](../../src/components/intelligence/entity-actions.tsx) 與對應 regression tests [`src/index-html.test.ts`](../../src/index-html.test.ts)、[`src/App.helpers.test.tsx`](../../src/App.helpers.test.tsx)、[`src/components/intelligence/entity-actions.test.tsx`](../../src/components/intelligence/entity-actions.test.tsx)、[`src/pages/intelligence-surfaces.test.tsx`](../../src/pages/intelligence-surfaces.test.tsx)、[`src/pages/explorer/panels/privacy-redaction.test.tsx`](../../src/pages/explorer/panels/privacy-redaction.test.tsx)，把 remote glyph font 改回 local SVG glyph、讓 decorative glyph 從 a11y tree 隱身、把 Settings 分組搬回 i18n catalog，並讓 shared internal entity CTAs 交給 React Router 產生 HashRouter 友善的 `#/...` href。
  - 重打 current-host release `.app`（`bunx tauri build --bundles app --no-sign`）後，再用 Computer Use 在最新 bundle 上重跑 Chrome `Yi-Ting` onboarding / re-import / `000000` 加密（未寫入鑰匙圈）。latest desktop 現在顯示 `6412ad59+`、`config.json` 記錄 `rememberDatabaseKeyInKeyring: false`、Dashboard 載入 `64,498` visits / `35,110` URLs、`/intelligence` 不再外露 raw glyph ids，且 `/intelligence/domain/google.com?range=month` 的 `打開網域證據` 會正確進入 `#/explorer?...` 並保持 Explorer grouped trail view 可用。
  - 同步回寫 [`docs/plan/{STATUS.md,BACKLOG.md,core-intelligence-desktop-truth-audit.md,core-intelligence-progress.md,core-intelligence-handoff.md}`](STATUS.md)，把 stale-bundle blocker closeout、latest rerun evidence、以及 M13 再次回到 active focus 寫回 source-of-truth。
  - 驗收：`bunx vitest run src/components/intelligence/entity-actions.test.tsx src/pages/explorer/panels/privacy-redaction.test.tsx src/index-html.test.ts src/App.helpers.test.tsx src/pages/intelligence-surfaces.test.tsx`、`bun run check`、`bun run build`

- [x] **WORK-BE-A** — Backend Hotspot Decomposition And Import Boundary Split
  - 讀先：
    `docs/plan/backend-hotspot-decomposition.md`
    `docs/architecture/data-model.md`
    `docs/architecture/module-boundary-map.md`
    `docs/architecture/desktop-command-surface.md`
    `docs/architecture/tech-stack.md`
  - 目標：把 2026-04-21 backend 架構審查轉成可執行的拆分軌道，先處理 `takeout` / parser / archive ingest 這條大數據量風險最高的 import boundary，再往 intelligence runtime 與 core intelligence hotspot 推進。
  - 契約：維持現有 Tauri command、worker CLI、serde payload、audit artifact 與 canonical schema 語義穩定；不得把 frontend M13 reuse block 的未提交改動捲進來；所有新建或整段重寫的 backend 模塊都必須帶完整 file header 與 declaration-level doc comments。
  - 2026-04-21 到 2026-04-22：`takeout` boundary 已拆成 focused owners，Takeout execute / preview / source-evidence path 也已切進 streamed or bounded-memory contract；`archive/mod.rs` 已把 ingest、backup、manifest/support helper 全部下沉到 focused submodules，live backup parser family 也已改成 streamed canonical ingest。後續又把 `intelligence_runtime.rs` 拆成 queue / claims / recovery / snapshot owners，並把 `intelligence/mod.rs` 的 overview / summary / domain / outputs、refind / explain、schema / bootstrap / rebuild orchestration 逐步抽成 focused modules。
  - 2026-04-22 structural closeout：最新 execution slice 再把 structural rebuild internals 拆成 `intelligence_structural_{state,build,aggregates,persist,stream,stage}.rs`，把 streamed replay、write-side replacements、aggregate builders、shared state machines 與 stage orchestration 正式分開。`intelligence/mod.rs` 因此從最初的 `11043` 行降到 `5561` 行，且所有新建 structural modules 都回到 `600` 行硬限制內。
  - 同步回寫 [`docs/plan/{STATUS.md,BACKLOG.md,README.md,backend-hotspot-decomposition.md}`](STATUS.md)，把 `WORK-BE-A` closeout truth 與 `WORK-BE-B` next-hop 寫回 source-of-truth。
  - 驗收：relevant targeted Rust regressions、`bun run check`、`bun run build`

- [x] **WORK-PERF-B** — Archive / Import Main-Thread Freeze Repair
  - 讀先：
    `docs/features/archive.md`
    `docs/design/ux-principles.md`
    `docs/design/screens-and-nav.md`
    `docs/architecture/desktop-command-surface.md`
    `docs/plan/m13-broad-reuse-audit/README.md`
  - 目標：修掉 Onboarding 初始化、手動備份、Takeout scan / import 仍會把桌面 UI 整段凍住的 stop-ship 問題，讓 busy overlay 先 repaint、再持續更新，而不是等整串 Rust 工作結束後才快轉補播。
  - 契約：不新增或刪除 Tauri command；不改 command args / return shape；不改 `pathkeep://backup-progress` 與 `pathkeep://import-progress` payload；不把重工作業搬去前端 Web Worker。
  - 2026-04-20：新增 [`src-tauri/src/commands/blocking.rs`](../../src-tauri/src/commands/blocking.rs)，把 join error shaping 與 `tauri::async_runtime::spawn_blocking` 收斂成 shared helper；[`src-tauri/src/commands/{archive.rs,import.rs}`](../../src-tauri/src/commands/archive.rs) 的 `initialize_archive`、`run_backup_now`、`inspect_takeout`、`import_takeout` 現在全部改成 off-main-thread async facade；[`src-tauri/src/commands/intelligence/runtime.rs`](../../src-tauri/src/commands/intelligence/runtime.rs) 也同步改用同一條 helper。
  - 更新 [`src/pages/import/index.tsx`](../../src/pages/import/index.tsx)，在 `scan` 與 `confirm/import` 兩條前景流程加入 explicit paint-first yield，確保 route-level BusyOverlay 能先出現在桌面上，再啟動實際的 inspect / import work。
  - 更新 [`src/app/shell-data.test.tsx`](../../src/app/shell-data.test.tsx) 與 [`src/pages/trust-flows.test.tsx`](../../src/pages/trust-flows.test.tsx)，新增 initialize / backup pending busy-state 與 import scan / import paint-first regressions，保護 overlay 先顯示、進度事件持續更新、以及不再等任務結束才一次補播。
  - 同步回寫 [`docs/architecture/desktop-command-surface.md`](../architecture/desktop-command-surface.md)、[`docs/plan/{STATUS.md,BACKLOG.md}`](STATUS.md) 與 [`docs/plan/m13-broad-reuse-audit/README.md`](m13-broad-reuse-audit/README.md)，把 archive/import long-running command 的 off-main-thread contract 與 M13 pause history 寫回 source-of-truth。
  - 驗收：`bun x vitest run src/app/shell-data.test.tsx src/pages/trust-flows.test.tsx`、`bun run check`、`bun run build`

- [x] **WORK-M13-A** — Broad Reuse Inventory Across Support / Trust / Workflow Surfaces
  - 讀先：
    `docs/plan/m13-broad-reuse-audit/README.md`
    `docs/plan/m12-support-actions-and-diagnostics/README.md`
    `docs/design/support-actions-and-diagnostics-tradeoff.md`
    `docs/design/screens-and-nav.md`
    `docs/design/ux-principles.md`
  - 目標：沿著 M12 的 support-action single-source 方法，盤點全 app 剩餘的 support / trust / workflow reuse drift，建立下一輪 canonical owner map 與 extraction priority。
  - 契約：不得把 M13 收斂成單純的 Settings route split 或 transport-first 專案；Jobs summary、workflow follow-through 與 support composition 必須一起納入 inventory，transport parity 只保留 subordinate role。
  - 2026-04-21：更新 [`docs/plan/m13-broad-reuse-audit/README.md`](m13-broad-reuse-audit/README.md)、[`docs/plan/program/research-and-decisions.md`](program/research-and-decisions.md)、[`docs/design/screens-and-nav.md`](../design/screens-and-nav.md) 與 [`docs/plan/README.md`](README.md)，正式補齊 M13 single-source map、remaining hotspot priority、`PG-RD-UX-016`，以及「runtime-boundary review grammar 掛回 `src/components/review/`」的 source-of-truth。
  - 這輪 inventory 也同步把後續順序固定為：shell-data owner、Security / Import workflow follow-through、Dashboard fallback owner、`Browsing Rhythm` layering；不再讓 M13 退化成單純的 Settings split 或 transport 專案。
  - 驗收：source docs、inventory map、`TODO: M13` 與 extraction priority 存在

- [x] **WORK-BE-B** — Core Intelligence Domain Boundary And Worker Follow-Through
  - 讀先：
    `docs/plan/backend-hotspot-decomposition.md`
    `docs/architecture/data-model.md`
    `docs/architecture/module-boundary-map.md`
    `docs/architecture/desktop-command-surface.md`
    `docs/architecture/tech-stack.md`
  - 目標：把 `intelligence/mod.rs` 剩餘的 query/read-model helper clusters、host-artifact ownership，以及 `vault-worker/src/intelligence.rs` / `src-tauri/crates/vault-core/src/ai.rs` 的 mixed orchestration 再往外拆，讓 backend giant-file 風險從 core intelligence parent module 正式轉移到更小的明確 owner。
  - 契約：維持現有 Tauri command、worker CLI、serde payload、Core Intelligence query ids / rebuild semantics、`IntelligenceRuntimeSnapshot` 與 off-main-thread background task contract 穩定；不得因為 giant-file 拆分而重開已接受的 `/intelligence` route / payload grammar。
  - 2026-04-22：`WORK-BE-B` 先後把 `intelligence/mod.rs` 的 session/navigation/search read-model layer 抽成 `intelligence_{sessions,navigation,search_metrics,search_queries}.rs`，再把 `vault-worker/src/intelligence.rs` 的 queue/runtime/read-surface ownership 收進 `intelligence/{ai_queue,runtime,route_queries,section_queries}.rs`，並把 `src-tauri/crates/vault-core/src/ai.rs` 拆成 `ai/{control,provider,indexing,ledger,search,read_model}.rs`。
  - 同一天的 helper-cluster closeout 又把 `intelligence/mod.rs` 的 residual internal owner 抽成 `intelligence_{shared,visit_records,visit_derive,daily_rollup_state,daily_rollups,core_persist}.rs`。shared date/query heuristics、visit-derived stage、daily-rollup stage、以及 scoped full-rebuild persistence 現在都有 focused owner；`intelligence/mod.rs` 因而從 `4508` 行再降到 `2583` 行，只剩 exported surface、core record types、batch cursors、常數與 regression suite。
  - 同步回寫 [`docs/plan/{STATUS.md,BACKLOG.md,README.md,backend-hotspot-decomposition.md}`](STATUS.md)，把 `WORK-BE-B` closeout truth 與 `WORK-BE-C` next-hop 寫回 source-of-truth。
  - 驗收：relevant targeted Rust regressions、`bun run check`、`bun run build`

- [x] **WORK-BE-C** — Remaining Backend Hotspot Decomposition Beyond Core Intelligence Parent
  - 讀先：
    `docs/plan/backend-hotspot-decomposition.md`
    `docs/architecture/data-model.md`
    `docs/architecture/module-boundary-map.md`
    `docs/architecture/desktop-command-surface.md`
    `docs/architecture/tech-stack.md`
  - 目標：把 backend 軌道剩餘的 giant-file 從 `core intelligence parent` 之外繼續往外拆，優先處理 `models/core_intelligence.rs`、`remote.rs`、`intelligence/site_dictionary.rs`，並把 `intelligence/mod.rs` 仍內嵌的 regression suite / support types 繼續下沉到 focused owners。
  - 契約：維持現有 Tauri command、worker CLI、serde payload、Core Intelligence DTO shape、visit-taxonomy classification semantics、remote bundle manifest/upload/verify contract、以及 `IntelligenceRuntimeSnapshot` 的 off-main-thread background task 邊界穩定；不得因為 giant-file 清理而重開已接受的 `/intelligence` route / payload grammar。
  - 2026-04-23：原 `deterministic` module 已改名並拆成 `visit_taxonomy/{mod,types,url,text,rules,classification,tests}.rs`，保留 `crate::visit_taxonomy::*` façade 與既有 taxonomy / URL / tokenization semantics；`intelligence/site_dictionary.rs` 也已拆成 `site_dictionary/{mod,types,overrides,search_rules,classification,tests}.rs`，維持 search rule / override schema、Settings payload、visit classification 與 search-query extraction semantics。
  - 同一天的 DTO / remote follow-through 又把 `models/core_intelligence.rs` 拆成 `core_intelligence/{mod,shared,requests,reads,analytics,overview,exports,tests}.rs`，並把 `remote.rs` 拆成 `remote/{mod,bundle,manifest,transfer,verify,tests}.rs`。Tauri / worker / frontend-facing serde shape、request aliases、remote bundle manifest、curl upload 與 restore-verification DTO contract 都維持不變；remote bundle build / verify 也改成 chunked SHA + zip streaming，避免大 SQLite payload 被整檔載入記憶體。
  - 最後的 regression-suite closeout 把 `intelligence/mod.rs` 內嵌 28 個 regression 與 fixture helpers 下沉到 `intelligence/tests/{schema_overview,stage_rebuild,structural_incremental,batch_equivalence,fixtures}.rs`。parent module 因而從 `2584` 行降到 `418` 行，只剩 module map、public façade、core records、batch cursors 與 constants；最大新 test owner 是 `stage_rebuild.rs` (`601` 行)。
  - 同步回寫 [`docs/plan/{STATUS.md,BACKLOG.md,README.md,backend-hotspot-decomposition.md}`](STATUS.md)，把 `WORK-BE-C` closeout truth 與下一個 current-focus 回到 `WORK-M13-B` 寫回 source-of-truth。
  - 驗收：`cargo test --manifest-path src-tauri/Cargo.toml -p vault-core remote`、`cargo test --manifest-path src-tauri/Cargo.toml -p vault-core intelligence`、`bun run check`、`bun run build`

- [x] **WORK-M13-B** — Shared Support / Workflow Composition Extraction
  - 讀先：
    `docs/plan/m13-broad-reuse-audit/README.md`
    `docs/design/screens-and-nav.md`
    `docs/design/ux-principles.md`
    `docs/plan/e2e-workflow-tests.md`
  - 目標：根據 `WORK-M13-A` 的 inventory，把至少一輪高價值的 support / trust / workflow composition 抽離，優先處理 Jobs plugin/module summary、workflow follow-through 與剩餘 support summary drift。
  - 契約：只抽明確跨 consumer 重複且能降低 drift 的 grammar；不得為了抽象而重開 M6–M12 已收斂的 route / payload / review / support-action contract。
  - 2026-04-21 到 2026-04-23：M13-B 先把 Jobs runtime health / plugin / module summary 接回 shared `runtime-boundary-card` / review grammar，再把 Import workflow reading order、file classification summary、recent batch review 與 doctor repair surface 收斂成較清楚的 route composition。
  - 同一輪後續把 shell shared AI queue / Core Intelligence runtime refresh、in-flight dedupe、active/idle polling cadence 下沉到 `src/app/shell-runtime-status.ts`；Security route 的 posture load、unlock/keyring、lock 與 rekey mutation state machine 下沉到 `src/pages/security/use-security-workflow.ts`；Dashboard bootstrap error 的 Security status probe 下沉到 `src/pages/dashboard/route-fallback-access.ts`；`BrowsingRhythmCard` 的 discovery-trend load、selected-year/day state、summary / range hint 與 lazy day-preview 下沉到 `src/components/intelligence/browsing-rhythm-card-state.ts`。
  - 最後的 legacy `PathRow` retirement 候選經 repo search 確認已無 active component / consumer；path/copy/open grammar 的實際 single source 是 `src/components/review/support-actions.tsx` 的 `ReviewPathActionRow`，因此這項以 stale-planning cleanup 收口，而不是新增無意義代碼。
  - 同步回寫 [`docs/plan/{STATUS.md,BACKLOG.md}`](STATUS.md) 與 [`docs/plan/m13-broad-reuse-audit/README.md`](m13-broad-reuse-audit/README.md)，把 M13-B closeout、剩餘 hotspot 歸零、以及 BACKLOG 暫無可提升 block 寫回 source-of-truth。
  - 驗收：focused Vitest slices、`git diff --check`、`bun run check`、`bun run build`

- [x] **WORK-BE-D** — Backend Completion Audit And AI Queue Test Boundary
  - 讀先：
    `docs/plan/STATUS.md`
    `docs/plan/backend-hotspot-decomposition.md`
    `docs/architecture/module-boundary-map.md`
    `docs/architecture/desktop-command-surface.md`
  - 目標：深度審查 backend hotspot 軌道是否真的完成，對照 live Rust line counts、doc-comment coverage、active plan/backlog truth、recent commits 與 current quality gate，再落一個可獨立提交的後端 follow-up slice。
  - 契約：只做 review-backed 的窄切片；不改 Tauri command name、worker export surface、serde payload、AI queue schema / lifecycle semantics、或前端 IPC contract。
  - 2026-04-23：live scan 確認 `WORK-BE-A/B/C` 已經把 `intelligence/mod.rs`、`ai.rs`、`remote.rs`、`models/core_intelligence`、`site_dictionary`、`takeout` 與 archive ingest 這批主 giant-file 戰場大幅收口，但不能宣稱整個 backend 已完成。production Rust 仍有 `src-tauri/src/dev_ipc_bridge.rs` (`1141` 行) 超過 1000 行，且 command / worker-bridge intelligence façade 還有 declaration-level rustdoc gaps。
  - 這輪 code slice 把 `src-tauri/crates/vault-core/src/ai_queue.rs` 內嵌 queue lifecycle regression suite 下沉到 [`src-tauri/crates/vault-core/src/ai_queue/tests.rs`](../../src-tauri/crates/vault-core/src/ai_queue/tests.rs)，讓 runtime module 從 `1019` 行降到 `768` 行；新 test owner 帶檔頭責任說明與 test-level doc comments，既有 queue tests / schema / payload / worker caller contract 維持不變。
  - 同步回寫 [`docs/plan/{STATUS.md,BACKLOG.md,README.md,backend-hotspot-decomposition.md}`](STATUS.md)，把「backend 主戰役完成但全域未完成」的 truth、`WORK-BE-D` closeout，以及下一個 active block `WORK-BE-E` 寫回 source-of-truth。
  - 驗收：pre-slice `bun run check`、post-slice targeted Rust / full check / build gates

- [x] **WORK-BE-E** — Command Facade Rustdoc And Dev Bridge Boundary
  - 讀先：
    `docs/plan/backend-hotspot-decomposition.md`
    `docs/architecture/desktop-command-surface.md`
    `docs/architecture/module-boundary-map.md`
    `docs/architecture/tech-stack.md`
  - 目標：處理後端 progress audit 暴露的下一個真 hotspot：`src-tauri/src/dev_ipc_bridge.rs` 超過 1000 行，且 `src-tauri/src/commands/intelligence/*` / `src-tauri/src/worker_bridge/intelligence/*` 仍有大量 command façade declaration-level rustdoc gaps。優先把 dev-only localhost bridge 的 payload DTO、router/dispatch table、command adapters 拆成 focused owners，並補齊 command / worker bridge 檔頭與 declaration comments。
  - 契約：維持現有 Tauri command names、devtools-bridge command strings、request/response payload shape、worker export surface、feature-gated + env-gated localhost-only 安全邊界，以及 `run_blocking_command` off-main-thread contract；不得把 dev automation mirror 擴寫成產品 remote-control API。
  - 2026-04-24：`src-tauri/src/dev_ipc_bridge.rs` 現在只保留 feature-gated listener startup / state handoff；command dispatch 與 desktop-layer updater/file-manager adapters 已抽到 [`src-tauri/src/dev_ipc_bridge/dispatch.rs`](../../src-tauri/src/dev_ipc_bridge/dispatch.rs)，並把 session round-trip / unknown command coverage 下沉到 [`src-tauri/src/dev_ipc_bridge/dispatch/tests.rs`](../../src-tauri/src/dev_ipc_bridge/dispatch/tests.rs)。
  - `dev_ipc_bridge/{config,router,payloads,dispatch}` 現在分別 owning env parsing、HTTP/CORS/error envelope、camelCase DTO、以及 command dispatch；command strings、payload shape、worker bridge implementation calls、updater/file-manager adapters 與 localhost-only feature+env gate 均未改 contract。父檔降到 `94` 行，dispatch owner 為 `764` 行。
  - 同步回寫 [`docs/plan/{STATUS.md,BACKLOG.md,README.md,backend-hotspot-decomposition.md}`](STATUS.md) 與 [`docs/architecture/desktop-command-surface.md`](../architecture/desktop-command-surface.md)，把 `WORK-BE-E` closeout、current line-count truth、以及 BACKLOG 暫無可提升 block 寫回 source-of-truth。
  - 驗收：`cargo test --manifest-path src-tauri/Cargo.toml --features devtools-bridge dispatch_command`、`bun run check`、`bun run build`

- [x] **WORK-IMPORT-SAFARI-A** — Safari Browser Direct Import Stop-Ship Completion
  - 讀先：
    `docs/features/archive.md`
    `docs/architecture/browser-support-and-adapter-playbook.md`
    `docs/architecture/desktop-command-surface.md`
    `TESTING.md`
    `docs/plan/m4-full-polish/release-readiness-runbook.md`
  - 目標：修掉 `/import` Browser Direct local DB path 仍呼叫 `inspect_takeout` / `import_takeout` 的錯誤，讓 Safari `History.db` 不再被丟進 Takeout parser，並把 Safari direct import 完成到 Chrome 同級的 preview-first workflow、dedupe、import batch、revert/restore、source evidence 與 validation contract。
  - 契約：保留既有 Takeout commands 不動；Browser Direct local database 只走 `inspect_browser_history` / `import_browser_history`；公開 validated Browser Direct path 只承諾 Google Chrome + macOS Safari；Safari 不偽造 Chrome-only Favicons / downloads / keyword-search sidecar；真機驗證與 docs 不記錄私人 URL。
  - 2026-04-24：新增 `BrowserHistoryImportRequest` 與 backend command/worker/dev-bridge surface `inspect_browser_history` / `import_browser_history`。`vault-core::takeout::browser_history` 現在會 staging snapshot selected DB、跑 `PRAGMA quick_check`、偵測 Safari / Chromium schema、串接 streamed parser、寫 canonical `urls` / `visits`、建立 `browser-history` import batch、保存 source evidence / capability snapshot / schema observation、刷新 search projection，且支援 re-import dedupe 與 batch revert/restore。
  - Safari parser 已補到 current schema 欄位：`load_successful`、`http_non_get`、`synthesized`、`redirect_source`、`redirect_destination`、`origin`、`generation`、`attributes`、`score` 會轉成 typed evidence / native preservation；缺欄位的舊 schema 仍 graceful degrade。對應 regression 覆蓋 generated fixture 與 `reference/browserexport/tests/databases/safari.sqlite`。
  - `/import` Browser Direct UI 現在按 method 分流：Takeout 繼續走 Takeout command，Browser Direct 走 browser-history command；detected list 只顯示 validated Chrome / Safari profile，Safari 缺 Full Disk Access 時保留 disabled guidance，不再藏起來或送錯 parser。三語 i18n 也補齊 Safari access guidance 與 unreadable state。
  - 同步回寫 [`docs/features/archive.md`](../features/archive.md)、[`docs/architecture/browser-support-and-adapter-playbook.md`](../architecture/browser-support-and-adapter-playbook.md)、[`docs/architecture/desktop-command-surface.md`](../architecture/desktop-command-surface.md)、[`docs/architecture/module-boundary-map.md`](../architecture/module-boundary-map.md)、[`docs/design/screens-and-nav.md`](../design/screens-and-nav.md)、[`TESTING.md`](../../TESTING.md)、[`docs/plan/{STATUS.md,BACKLOG.md,README.md,program/research-and-decisions.md}`](STATUS.md) 與 [`docs/plan/m4-full-polish/release-readiness-runbook.md`](m4-full-polish/release-readiness-runbook.md)。這是 user-directed Safari import stop-ship block，不覆寫目前已完成的 `WORK-BE-E` 後端軌道。
  - 真機驗證：目前 Codex shell 讀 `~/Library/Safari` 會被 macOS TCC 擋住，已驗證缺 Full Disk Access 時會走 access guidance；本進程未導入私人 Safari URL。授權 PathKeep / 執行進程 Full Disk Access 後，實機驗收 recipe 是用同一套 Browser Direct flow 跑 preview / import / re-import / revert / restore，且只記錄 aggregate counts 和時間範圍。
  - 驗收：`cargo test --manifest-path src-tauri/Cargo.toml -p browser-history-parser safari -- --nocapture`、`cargo test --manifest-path src-tauri/Cargo.toml -p vault-core browser_history -- --nocapture`、`bun run test:unit -- src/pages/trust-flows/import-flows.test.tsx`、`bun run check`、`bun run build`

- [x] **WORK-IMPORT-SAFARI-B** — Safari Full Disk Access Direct Settings Action
  - 目標：把 `/import` Browser Direct 裡 Safari 「需要權限」狀態從純 guidance 補成可直接前往 macOS Full Disk Access 設定的 action。
  - 契約：不可讀的 Safari profile 仍留在 detected list；不可用 profile 不再渲染成 disabled nested-button card；native launcher 只新增固定的 macOS Full Disk Access System Settings URL，不開放任意 custom scheme。
  - 2026-04-24：`ImportSelectStep` 將不可用 profile 改為非互動卡片，Safari 權限提示旁新增三語 `Open Full Disk Access` action；scan error 若指出 Full Disk Access 缺失，也會在錯誤 callout 顯示同一個 action；route owner 透過 `open_external_url` 開啟固定 `x-apple.systempreferences:com.apple.preference.security?Privacy_AllFiles`。
  - 同步回寫 [`docs/features/archive.md`](../features/archive.md)、[`docs/architecture/browser-support-and-adapter-playbook.md`](../architecture/browser-support-and-adapter-playbook.md)、[`docs/architecture/desktop-command-surface.md`](../architecture/desktop-command-surface.md) 與 [`docs/design/screens-and-nav.md`](../design/screens-and-nav.md)。

- [x] **WORK-IMPORT-ATLAS-A** — ChatGPT Atlas Browser Direct Import Completion
  - 讀先：
    `docs/features/archive.md`
    `docs/architecture/browser-support-and-adapter-playbook.md`
    `docs/architecture/desktop-command-surface.md`
    `TESTING.md`
    `docs/plan/m4-full-polish/release-readiness-runbook.md`
  - 目標：把 ChatGPT Atlas 導入完成到 Chrome 同級：discovery、Chromium parser reuse、Browser Direct preview/import、dedupe、batch revert/restore、source evidence、UI visibility、icon/i18n/support copy，以及 current archive validation。
  - 契約：Atlas 是 Chromium-family adapter，不新增 parser family / Tauri command / `BrowserHistoryImportRequest` 欄位；支援範圍只限 macOS `~/Library/Application Support/com.openai.atlas/browser-data/host/<profile>/History` 與 Chromium sidecars such as `Favicons`；不導入 Atlas workspace data、chats、tabs、bookmarks 或 suggestions。
  - 2026-04-24：`vault-core::chrome` 新增 `atlas` browser definition、macOS host root、`atlas:<raw-profile-dir>` profile identity 與 discovery regression；Browser Direct import regression 證明 Atlas 走 Chromium parser 並保留 `source_profiles.browser_product = ChatGPT Atlas`；backup ingest source-profile metadata 也改用 discovery product name，checkpoint restore fallback 補上 Atlas display name。
  - `stage_browser_history_source` 既有 SQLite Backup API 優先策略維持不變，並新增 WAL / sidecar copy regression，保護 online backup 失敗或 live DB locked 時 Chromium/Atlas WAL rows 不丟失。`/import` validated filter 現在顯示 ChatGPT Atlas profile，`browser-icons` 補上 Atlas glyph，onboarding support copy / i18n tests / Import route test 已同步。
  - 同步回寫 [`README.md`](../../README.md)、[`TESTING.md`](../../TESTING.md)、[`docs/features/archive.md`](../features/archive.md)、[`docs/architecture/browser-support-and-adapter-playbook.md`](../architecture/browser-support-and-adapter-playbook.md)、[`docs/architecture/desktop-command-surface.md`](../architecture/desktop-command-surface.md)、[`docs/design/screens-and-nav.md`](../design/screens-and-nav.md)、[`docs/plan/{STATUS.md,BACKLOG.md,README.md,program/research-and-decisions.md}`](STATUS.md) 與 [`docs/plan/m4-full-polish/release-readiness-runbook.md`](m4-full-polish/release-readiness-runbook.md)。
  - Current archive validation：透過 dev IPC bridge 用本機 Atlas profile 完成 preview / import / re-import / revert / restore。sanitized artifact 落在 [`artifacts/browser-support/2026-04-24-chatgpt-atlas/README.md`](../../artifacts/browser-support/2026-04-24-chatgpt-atlas/README.md)，只記 schema coverage、aggregate counts、time range、batch outcome 與 source-evidence counts；不記私人 URL / title / raw profile path。驗證後 Atlas import batch 已 restore，current archive 保持 visible。
  - 驗收：`cargo test --manifest-path src-tauri/Cargo.toml -p vault-core atlas -- --nocapture`、`cargo test --manifest-path src-tauri/Cargo.toml -p vault-core browser_history -- --nocapture`、`bun run test:unit -- src/pages/trust-flows/import-flows.test.tsx src/lib/browser-icons.test.tsx src/lib/i18n.test.ts`、`bun run check`、`bun run build`

- [x] **WORK-IMPORT-COMET-A** — Perplexity Comet Browser Direct Import Completion
  - 讀先：
    `docs/features/archive.md`
    `docs/architecture/browser-support-and-adapter-playbook.md`
    `docs/architecture/desktop-command-surface.md`
    `TESTING.md`
    `docs/plan/m4-full-polish/release-readiness-runbook.md`
  - 目標：把 Perplexity Comet 導入完成到 Chrome 同級：discovery、Chromium parser reuse、Browser Direct preview/import、dedupe、batch revert/restore、source evidence、UI visibility、icon/i18n/support copy，以及 current archive validation。
  - 契約：Comet 是 Chromium-family adapter，不新增 parser family / Tauri command / `BrowserHistoryImportRequest` 欄位；支援範圍只限 macOS `~/Library/Application Support/Comet/<profile>/History` 與 Chromium sidecars such as `Favicons`；不導入 Comet AI memory、Perplexity account / workspace data、chats、tabs、bookmarks 或 suggestions。
  - 2026-04-24：`vault-core::chrome` 新增 `comet` browser definition、macOS App Support root、`comet:<raw-profile-dir>` profile identity 與 discovery regression；Browser Direct import regression 證明 Comet 走 Chromium parser 並保留 `source_profiles.browser_product = Perplexity Comet`；backup ingest source-profile metadata 與 checkpoint restore fallback 也補上 Comet display name。
  - `/import` validated filter 現在顯示 Perplexity Comet profile，`browser-icons` 補上 Comet glyph，onboarding support copy / i18n tests / Import route test 已同步；Browser Direct 仍走既有 `inspect_browser_history` / `import_browser_history`，不把 Comet `History` 送進 Takeout parser。
  - 同步回寫 [`README.md`](../../README.md)、[`RELEASE.md`](../../RELEASE.md)、[`TESTING.md`](../../TESTING.md)、[`docs/features/archive.md`](../features/archive.md)、[`docs/architecture/browser-support-and-adapter-playbook.md`](../architecture/browser-support-and-adapter-playbook.md)、[`docs/architecture/desktop-command-surface.md`](../architecture/desktop-command-surface.md)、[`docs/design/screens-and-nav.md`](../design/screens-and-nav.md)、[`docs/reference-review.md`](../reference-review.md)、[`docs/plan/{STATUS.md,BACKLOG.md,README.md,program/research-and-decisions.md}`](STATUS.md) 與 [`docs/plan/m4-full-polish/release-readiness-runbook.md`](m4-full-polish/release-readiness-runbook.md)。
  - Current archive validation：透過 dev IPC bridge 用本機 Comet profile 完成 preview / import / re-import / revert / restore。sanitized artifact 落在 [`artifacts/browser-support/2026-04-24-perplexity-comet/README.md`](../../artifacts/browser-support/2026-04-24-perplexity-comet/README.md)，只記 schema coverage、aggregate counts、time range、batch outcome 與 source-evidence counts；不記私人 URL / title / raw profile path。驗證後 Comet 主 import batch 已 restore，current archive 保持 visible。
  - 驗收：`cargo test --manifest-path src-tauri/Cargo.toml -p vault-core comet -- --nocapture`、`cargo test --manifest-path src-tauri/Cargo.toml -p vault-core browser_history -- --nocapture`、`bun run test:unit -- src/pages/trust-flows/import-flows.test.tsx src/lib/browser-icons.test.tsx src/lib/i18n.test.ts`、`bun run check`、`bun run build`

- [x] **WORK-EXPLORER-FAVICON-A** — Time-Aware Favicon Domain Fallback
  - 讀先：
    `docs/features/recall.md`
    `docs/architecture/data-model.md`
    `docs/architecture/browser-support-and-adapter-playbook.md`
  - 目標：在 Explorer history row 缺失 exact page favicon 時，高性能、低成本地嘗試同網站可用 icon，同時維持主列表 row-only payload、favicon blob 去重與舊訪問紀錄的 icon 時間語義。
  - 契約：不新增 Tauri command、不把 favicon bytes 塞回 `query_history`、不做 unbounded favicon scan、不在 read path 改寫 canonical visit / favicon facts；domain fallback 只能由 indexed page / host / registrable-domain lookup 提供，且 host / domain fallback 不得使用晚於該 visit time 的 icon。
  - 2026-04-24：新增 migration `010_favicon_domain_fallback.sql`，在 `favicons` 保存 normalized `page_host` / `page_registrable_domain` 並建立 profile-aware / cross-profile lookup indexes；schema bootstrap 不同步掃描舊 favicon rows，新 ingest 直接寫入 metadata，favicon bytes 仍透過 `favicon_blobs` / `image_blob_hash` 去重。
  - Explorer lazy favicon hydration 現在傳入 `visitTime` 並把 cache key 擴成 `profileId + url + visitTime`。後端 lookup 先找 exact page icon，再按同 host / 同 registrable domain fallback；只要 request 帶 visit time，所有候選 icon 的 `last_updated_ms` 都必須早於或等於該 visit time，避免網站改 icon 後污染較早歷史紀錄。
  - 同步回寫 [`docs/features/recall.md`](../features/recall.md)、[`docs/architecture/data-model.md`](../architecture/data-model.md) 與 [`docs/plan/STATUS.md`](STATUS.md)。
  - 驗收：targeted Rust / Vitest slices、`bun run check`、`bun run build`

- [x] **WORK-IMPORT-PERF-A** — Browser Direct Import Bounded-Memory Finalization
  - 讀先：
    `docs/features/archive.md`
    `docs/architecture/data-model.md`
    `docs/plan/program/repo-baseline.md`
  - 目標：回應三個月真實資料導入後 app 崩潰、清資料後再導入卡死的 stop-ship 回報，審查 favicon fallback 與 import finalization 是否符合 1440 萬筆 baseline，並修掉導入主路徑的 unbounded memory / full rebuild 熱點。
  - 契約：導入 execute path 不得把 full source-native evidence 常駐在 parser return payload；導入成功後不得同步二次 stream 同一個 source DB；導入主路徑不得為了 keyword recall 重建整個 derived search projection。
  - 2026-04-24：`browser-history-parser` 的 shared `HistoryBatchConsumer` 新增 bounded `SourceEvidenceChunk` sink。Chromium / Safari parser 在 import consumer 選擇 `retain_source_evidence_in_report = false` 時，會把 typed evidence / native entities 隨 parser batch 流給 consumer；read-only `parse_history` API 仍保留完整 report，避免破壞既有 parser/debug 調用。
  - `vault-core::takeout::browser_history` 現在讓 Browser Direct archive consumer 直接持有 `DeferredSourceEvidenceBuilder`，source evidence 邊 parse 邊進 spool；preview range 也在寫 visits 時同步累計，不再 commit 後重新 `stream_browser_history` 一次。這把 Browser Direct execute path 從「canonical rows stream，但 cold evidence 全量常駐」修成真正 bounded-memory。
  - `derived/history-search.sqlite` finalization 新增 import-batch scoped refresh：Takeout / Browser Direct import 只 upsert 本次 batch 影響到的 URL-document FTS rows。`rebuild_search_projection` 仍保留給 rollback / restore / backup / repair 顯式維護路徑，但不再卡在 import foreground path。
  - 同步回寫 [`docs/features/archive.md`](../features/archive.md)、[`docs/architecture/data-model.md`](../architecture/data-model.md) 與 [`docs/plan/STATUS.md`](STATUS.md)。
  - 驗收：`cargo test --manifest-path src-tauri/Cargo.toml -p browser-history-parser --lib`、`cargo test --manifest-path src-tauri/Cargo.toml -p vault-core takeout::tests::browser_history --lib`

- [x] **WORK-UI-OPT-A** — Import Progress, i18n, And Dashboard Rhythm Repair
  - 讀先：
    `/Users/tim/Library/Mobile Documents/com~apple~CloudDocs/0-iCloud/Notes/core-v2/02_Projects/0 子項目/2026/4 chrome_history_backup/source_prompts/32 UI 优化.md`
    `docs/features/archive.md`
    `docs/design/screens-and-nav.md`
    `docs/design/ui-review-guardrails.md`
    `docs/design/ux-principles.md`
  - 目標：修復使用者實測回報的 UI blockers：大型單檔 / 單 profile 匯入時 progress overlay 長時間無變化、中文介面露出英文說明句、Dashboard Browsing Rhythm 的 `回到今年` 位置、匯入後 heatmap 需重啟才刷新，以及 activity mix 圖例顏色不可見。
  - 契約：不新增 Tauri command、不改 import command 必填 payload；`ImportProgressEvent` 只做 additive optional fields；Takeout / Browser Direct foreground import 必須回報真實 parser-batch record counters，未知總量時維持 indeterminate progress；Browser / Dashboard 可見 copy 必須走三語 i18n；Dashboard rhythm 必須跟隨 shell refresh token 重新讀取資料。
  - 2026-04-24：`ImportProgressEvent` 新增 `sourceLabel`、`processedRecords`、`totalRecords`、`importedRecords`、`duplicateRecords`、`skippedRecords`。Takeout payload consumer 與 Browser Direct archive consumer 現在會在 parser visit batch 後發 record-level progress；前端 import overlay 用預覽的 candidate count 補足 total，未知 total 則顯示遞增 record counter + indeterminate bar。raw backend notes 不再直接出現在中文 import preview，改成本地化 audit-note summary。
  - Dashboard rhythm 現在接收 shell `refreshKey` 並以 force read / cache bypass 重新讀取 discovery trend；匯入成功後也會清 Core Intelligence overview cache，再 refresh shell data。`回到今年` 已移到 year pager 左側，activity mix `video` / `ai` 等 category 不再引用不存在的 `--danger` token，並補齊 opaque fallback palette。
  - 同步回寫 [`docs/features/archive.md`](../features/archive.md)、[`docs/design/screens-and-nav.md`](../design/screens-and-nav.md)、[`docs/design/ui-review-guardrails.md`](../design/ui-review-guardrails.md) 與 [`docs/plan/STATUS.md`](STATUS.md)。
  - 驗收：targeted Rust progress tests、targeted Vitest import/dashboard/i18n tests、`bun run check`、`bun run build`、fresh desktop Computer Use truth pass。

- [x] **WORK-UI-OPT-B** — Dashboard Pager And Backup Progress Regression Repair
  - 讀先：
    `docs/features/archive.md`
    `docs/design/screens-and-nav.md`
    `docs/design/ux-principles.md`
  - 目標：修復 `WORK-UI-OPT-A` 後續實機回報的 regressions：Dashboard heatmap 年份 pager 被初始 refresh force path 折回只剩當前年份；onboarding / manual backup overlay 仍顯示 profile-level `1 / 2 · 50%`，且中文 UI 仍露出 raw backend / English progress copy。
  - 契約：`BackupProgressEvent` 只新增 optional record/source 欄位；manual backup 與 onboarding finalization 要和 import 一樣回報真實 processed/imported/duplicate/skipped record counters。未知 record total 時不得偽造 percent；Dashboard 初次載入不得因 `refreshToken` 初值就 bypass cache 並丟失 available-year continuity，只有實際 refresh token 變更才 force reread。
  - 2026-04-24：archive ingest consumer 現在支援 backup progress callback，manual backup 在每個 visit batch 後發出 `sourceLabel`、`processedRecords`、`importedRecords`、`duplicateRecords`、`skippedRecords`；shell busy overlay 會優先顯示本地化 record counter / stats，並在未知 total 時保持 indeterminate progress，不再把 profile count 當成 active write percent。
  - 實機回歸補修：Topbar / Audit 的 manual backup 觸發不再留下 unhandled rejection；Safari 在 staging 階段才遇到 Full Disk Access 權限失敗時會被記為 profile-level warning 並略過，Chrome / Comet 等可讀 profile 仍會成功 commit。shell 成功 notice 會用三語 copy 明確說 Safari 本次被略過，不再卡在 overlay 或靜默回滾整筆 run。
  - Dashboard `BrowsingRhythmCard` state owner 會保留已知 data years，初次 render 不再把 `refreshToken=0` 視為 force refresh；year pager 使用連續年份帶，`回到當前年份` 不依賴 backend 是否回傳今年。deterministic runtime jobs 從 active 轉 idle 時 shell 會再推一次 dashboard / rhythm refresh，避免 backup 完成後 heatmap 還卡在舊 cache。Dashboard zero-state / next-action 也把 backend English action 摘要映射成三語 UI copy。
  - 同步回寫 [`docs/features/archive.md`](../features/archive.md)、[`docs/design/screens-and-nav.md`](../design/screens-and-nav.md) 與 [`docs/design/ux-principles.md`](../design/ux-principles.md)。
  - 驗收：targeted Rust / Vitest regression tests、`bun run check`、`bun run build`、fresh desktop Computer Use truth pass。

- [x] **WORK-QA-GATE-A** — Restore Strict Checker Gates
  - 讀先：
    `docs/plan/program/quality-matrix.md`
    `TESTING.md`
    `docs/standards.md`
    `.github/workflows/ci.yml`
    `.github/workflows/mutation.yml`
    `package.json`
  - 目標：把 `bun run check` 恢復成真正 blocking 的 per-commit checker：base checks、100% JS/Rust coverage、browser build、browser-preview e2e、desktop-bridge truth gate、以及 lightweight desktop-contract JS mutation 都必須進入同一條本地與 CI gate。
  - 2026-04-27 closeout：full active-source JS coverage 與 full Rust source coverage 已補到 100%；`bun run check` 現在串起 `check:base`、`check:coverage`、browser build、browser-preview e2e、desktop-bridge truth gate、以及 `mutation:js:desktop-contract`。GitHub CI 改跑同一條 per-commit gate；scheduled/manual `Mutation` workflow 保留 full JS/Rust deep sweep。
  - Mutation cost decision：desktop-contract JS mutation 77 mutants，約 50 秒，已達 100% mutation score；full JS Stryker dry run 約 2m20s，21769 mutants，按 44m/32% 實測估算約 2-3 小時；Rust full cargo-mutants 有 5869 candidates，且 current copy-sandbox baseline 仍會因 repo-root Safari fixture path 缺失而失敗。使用者確認 full mutation 不作每次 commit hard gate，改由 `check:deep` / scheduled/manual workflow 承接。
  - Real bugs found during QA hardening：Search Keywords duplicate React keys；AI queue last-activity fallback ignored `startedAt`; profile switcher option Escape path was masked by document Escape; IPC bridge blank string rejection now falls back to generic desktop command error.
  - Future work：`BACKLOG.md` 新增 blocked `WORK-QA-GATE-B`，明確保留 future full JS/Rust mutation deep sweep、Rust cargo-mutants copy-sandbox fixture repair、survivor closeout 與窄範圍 equivalent evidence 流程。
  - 驗收：`bun run check` 通過；`bun run verify` 通過。

- [x] **WORK-UI-PROGRESS-A** — Global Task Progress And Topbar Cleanup
  - 讀先：
    `docs/features/archive.md`
    `docs/design/screens-and-nav.md`
    `docs/design/ui-review-guardrails.md`
    `docs/design/ux-principles.md`
    `docs/design/design-tokens.md`
    `docs/architecture/desktop-command-surface.md`
  - 目標：把 import / backup progress 從 route-local overlay 提升為 shell-owned global task panel，統一 progress UI / console log，移除 topbar search，並用 notification queue 取代 topbar notice banner。
  - 契約：不新增 Tauri command name 或 request payload；`pathkeep://import-progress` / `pathkeep://backup-progress` 只 additive 新增 structured `logEvents`；同一時間只允許一個 archive-write task；Jobs 是可找回進度的 canonical live surface。
  - 2026-04-27 closeout：新增 `src/app/shell-tasks.ts` shell task / notification helpers、shared `src/components/progress/task-progress.tsx` progress card / meter / console、ShellDataProvider import/backup global task actions、Jobs archive-write section、Import inline task card、sidebar compact archive task strip、topbar notification popover與 localStorage queue。backend import / backup progress event 仍保留 legacy fields，同時新增 structured `ProgressLogEvent` / `logEvents`，前端優先消費 structured events。
  - UI truth：topbar global search box / route submission tests 已移除；notification button 置於 ProfileSwitcher 左側，開啟後標記已讀並可逐條 dismiss；`ProfileSwitcher` 與 `Backup now` 維持最右兩個 controls。
  - 同步回寫 [`docs/features/archive.md`](../features/archive.md)、[`docs/design/screens-and-nav.md`](../design/screens-and-nav.md)、[`docs/architecture/desktop-command-surface.md`](../architecture/desktop-command-surface.md)、[`docs/plan/STATUS.md`](STATUS.md)、[`docs/plan/BACKLOG.md`](BACKLOG.md) 與 [`docs/plan/CHANGELOG.md`](CHANGELOG.md)。
  - 驗收：targeted Vitest / Rust progress tests、`bun run check`、fresh desktop Computer Use truth pass。

- [x] **WORK-INTEL-SCOPE-A** — Intelligence All-Time Scope And Progressive Loading
  - 讀先：
    `docs/features/intelligence.md`
    `docs/features/intelligence-current-state.md`
    `docs/design/screens-and-nav.md`
    `docs/design/ux-principles.md`
    `docs/plan/STATUS.md`
    `docs/plan/BACKLOG.md`
  - 目標：修復 Settings / Maintenance 頂部 sticky section nav hash link 只改 URL 不 scroll 的問題；為 `/intelligence` 增加 all-time scope preset；把 secondary grid 從整批 ready gate 改成 warm-cache progressive reveal；先寫清楚 deeper all-time preload/cache/invalidation 策略。
  - 契約：`Month` 仍是初始預設；`All time` deep link 使用 `?range=all`，不輸出 custom `start/end`；本 slice 不新增 Tauri command 或 backend payload shape；cold secondary load 仍走 overview batch，不能退回多 foreground IPC fan-out。
  - 2026-04-28 closeout：`TimeRangePreset`、route parsing/building、time selector與三語 i18n 已支援 `all`；route-level all-time 目前映射到 broad concrete `DateRange`，`Browsing Rhythm` 顯示層只渲染實際有資料的日期 span；secondary slots 會先顯示已 cached card，未 cached card 保持 card-level skeleton；Settings / Maintenance section nav click 與 initial hash route 都會 scroll+focus 對應 panel。
  - 同步回寫 [`docs/features/intelligence.md`](../features/intelligence.md)、[`docs/features/intelligence-current-state.md`](../features/intelligence-current-state.md)、[`docs/design/screens-and-nav.md`](../design/screens-and-nav.md)、[`docs/design/ux-principles.md`](../design/ux-principles.md)、[`docs/plan/intelligence-all-time-cache-invalidation.md`](intelligence-all-time-cache-invalidation.md)、[`docs/plan/STATUS.md`](STATUS.md)、[`docs/plan/BACKLOG.md`](BACKLOG.md) 與 [`docs/plan/CHANGELOG.md`](CHANGELOG.md)。
  - 驗收：targeted Vitest section-nav / route-state / time selector / secondary grid / browsing rhythm tests、`bun run check`。本輪已重啟 debug desktop app 嘗試 fresh native truth pass，但 Computer Use 對 Finder / PathKeep 均返回 macOS `Apple event error -10000`，`screencapture` 也無法從 display 產圖；可執行桌面驗收以 `bun run check` 內的 desktop bridge truth gate 為準。

- [x] **WORK-RELEASE-010-A** — Browser Support And Windows Scheduler Release Blockers
  - 讀先：
    `docs/features/archive.md`
    `docs/architecture/browser-support-and-adapter-playbook.md`
    `docs/architecture/desktop-command-surface.md`
    `TESTING.md`
    `docs/plan/m4-full-polish/release-readiness-runbook.md`
  - 目標：在 0.1.0 release 前，把 Chrome / Edge / Firefox 的 support 定義收斂成 backup + Browser Direct import；把 Windows scheduler 從 manual-review 升級為 app 可 preview / apply / status / remove 的 Task Scheduler support；同時保持 Atlas / Comet 既有 macOS scope 不擴張。
  - 契約：不新增 Tauri command name 或 `BrowserHistoryImportRequest` payload field；Firefox Browser Direct 走 `places.sqlite` + history-only parser；Edge 走 Chromium parser 但保留 Microsoft Edge / Edge Dev product metadata；Windows scheduler 使用 `schtasks`，Linux 仍維持 manual-review；所有新 user-visible copy 必須同步 `en` / `zh-CN` / `zh-TW`。
  - 2026-04-28 closeout：Firefox Browser Direct staging 現在支援 profile directory / direct `places.sqlite`、family detection、quick_check/schema mismatch、防誤送 Takeout、history-only import/re-import/revert/restore/source-evidence；Edge / Edge Dev 在 Browser Direct validated list 中保留 Chromium parser 但保存 `Microsoft Edge` / `Microsoft Edge Dev` product metadata；backup selection 以 readable history 為準，已選但不可讀的 profile 會進 skipped/degraded warning，仍讓同輪其他可讀 Chrome / Edge / Firefox profile 成功；Windows scheduler 已支援 generated XML artifact + `schtasks /Create` apply、`schtasks /Query /XML` status、`schtasks /Delete` remove 與 apply/remove audit。
  - 同步回寫 [`README.md`](../../README.md)、[`RELEASE.md`](../../RELEASE.md)、[`TESTING.md`](../../TESTING.md)、[`docs/features/archive.md`](../features/archive.md)、[`docs/architecture/browser-support-and-adapter-playbook.md`](../architecture/browser-support-and-adapter-playbook.md)、[`docs/architecture/desktop-command-surface.md`](../architecture/desktop-command-surface.md)、[`docs/design/screens-and-nav.md`](../design/screens-and-nav.md)、[`docs/plan/m1-solid-archive/schedule-security-and-storage.md`](m1-solid-archive/schedule-security-and-storage.md)、[`docs/plan/m4-full-polish/release-readiness-runbook.md`](m4-full-polish/release-readiness-runbook.md)、[`docs/plan/backend-hotspot-decomposition.md`](backend-hotspot-decomposition.md)、[`docs/plan/BACKLOG.md`](BACKLOG.md) 與 [`docs/plan/CHANGELOG.md`](CHANGELOG.md)。
  - 維護性 follow-up：`src-tauri/crates/vault-platform/src/scheduler.rs` 因 Windows Task Scheduler support 升到 `1261` 行，已按 `AGENTS.md` 在 [`BACKLOG.md`](BACKLOG.md) 新增 blocked `WORK-SCHED-MAINT-A`，等 Windows VM acceptance 後再做 scheduler module maintainability review。
  - 驗收：Firefox / Edge Browser Direct Rust + Vitest acceptance、backup readable-profile hardening tests、Windows scheduler apply/status/remove tests、Import / Schedule / onboarding / i18n Vitest slices、`bun run check`。

- [x] **WORK-RELEASE-011-A** — Judge Review Demo Trust Polish
  - 讀先：
    `docs/plan/STATUS.md`
    `docs/plan/BACKLOG.md`
    `docs/design/screens-and-nav.md`
    `docs/features/archive.md`
    `TESTING.md`
  - 目標：逐項驗證早期評審報告，排除使用者不同意的「明文預設 / 強化加密引導」建議後，只修會降低 demo 信任風險或狀態誤讀的 release polish 問題。
  - 契約：不改 archive encryption / plaintext default policy；不新增 Tauri command、IPC payload、browser support scope、或 backend ingest 行為；所有 user-visible copy 維持 `en` / `zh-CN` / `zh-TW` parity。
  - 2026-04-28 closeout：當前 `bun run check` 已證明評審提到的 Safari fixture / reference path failure 不再存在。Schedule copy 現在拆清 backup trigger cadence 與 installed-schedule health-check cadence；Jobs queue copy 不再把 queue active 說成 AI enabled；Explorer filter option / chip / recent-search label 不再外露 raw profile/browser tokens；Onboarding / Settings / Import profile selectors 第一層改顯示 browser/profile/history filename，Browser Direct source path 收進 selected-source detail；archive-write empty console copy 改成等待下一條 progress event 的誠實說明。
  - 同步回寫 [`docs/design/screens-and-nav.md`](../design/screens-and-nav.md)、[`docs/plan/STATUS.md`](STATUS.md) 與 [`docs/plan/CHANGELOG.md`](CHANGELOG.md)。
  - 驗收：pre-change `bun run check`、`bun run check:i18n`、targeted Vitest release polish slices、post-change `bun run check`。

- [x] **WORK-SCHED-REDESIGN-A** — Scheduled Backup Detection Audit And Design Gate
  - 讀先：
    `docs/features/archive.md`
    `docs/design/screens-and-nav.md`
    `docs/design/ux-principles.md`
    `docs/design/design-tokens.md`
    `docs/architecture/desktop-command-surface.md`
    `docs/plan/m1-solid-archive/schedule-security-and-storage.md`
    `TESTING.md`
  - 目標：先完成 scheduled backup detection audit 與 UI redesign spec，修復高優先級的 macOS legacy scheduler detection drift，然後產出 Schedule / Onboarding 設計稿並停在設計確認點。
  - 契約：Ticket B 的偵測修復不得改 UI / Onboarding；Ticket A 的 Phase 1/2 不得改 backup execution；Phase 3 UI 重建必須等設計稿確認後另行開始；保持 `com.yi-ting.pathkeep` clean-break namespace，不自動 migrate 或 remove legacy LaunchAgent。
  - 2026-04-29 closeout：已記錄 current-host scheduler truth：`dev.codex.browser-history-backup.backup` legacy LaunchAgent 存在且 loaded，`dev.codex.pathkeep.backup.plist` 存在但未 loaded，canonical `com.yi-ting.pathkeep.backup` 不存在。Ticket B Phase 1 審計落地為 [`scheduled_backup_audit.md`](scheduled_backup_audit.md)，Ticket A Phase 1 spec / Phase 2 design brief 落地為 [`scheduled_backup_redesign_spec.md`](scheduled_backup_redesign_spec.md)。高優先級修復只改 scheduler detection：macOS status 現在會把 known legacy LaunchAgent 顯示為 `legacy-install-detected`，不自動 migrate/remove。Phase 2 已用 browser-preview 擷取目前 Schedule / Onboarding schedule step 參考畫面，並停在 imagegen 設計稿確認點；Phase 3 UI / Onboarding 實作未開始。
  - 同步回寫 [`docs/features/archive.md`](../features/archive.md)、[`docs/architecture/desktop-command-surface.md`](../architecture/desktop-command-surface.md)、[`docs/plan/m1-solid-archive/schedule-security-and-storage.md`](m1-solid-archive/schedule-security-and-storage.md)、[`docs/plan/backend-hotspot-decomposition.md`](backend-hotspot-decomposition.md)、[`docs/plan/BACKLOG.md`](BACKLOG.md)、[`docs/plan/STATUS.md`](STATUS.md) 與 [`docs/plan/CHANGELOG.md`](CHANGELOG.md)。
  - 維護性 follow-up：`src-tauri/crates/vault-platform/src/scheduler.rs` 因最小 legacy detection bug fix 升到 `1411` 行，已在 [`BACKLOG.md`](BACKLOG.md) 將 `WORK-SCHED-MAINT-A` 升成 high-priority maintainability review；在該 review 完成前不得再往該檔新增業務邏輯。
  - 驗收結果：`cargo test --manifest-path src-tauri/Cargo.toml -p vault-platform scheduler -- --test-threads=1` 通過；`bun run check` 通過（base checks、100% JS/Rust coverage、build、browser-preview e2e、desktop-bridge truth gate、desktop-contract mutation）。

- [x] **WORK-SCHED-REDESIGN-B** — Scheduled Backup Settings UI And Onboarding Integration
  - 讀先：
    `docs/plan/scheduled_backup_redesign_spec.md`
    `docs/plan/scheduled_backup_audit.md`
    `docs/design/screens-and-nav.md`
    `docs/design/ux-principles.md`
    `docs/design/design-tokens.md`
    `docs/features/archive.md`
    `docs/architecture/desktop-command-surface.md`
    `TESTING.md`
  - 目標：依已確認的 Phase 2 版面方向重建 Scheduled Backup Settings 與 Onboarding schedule step；保留 PathKeep 原本美術風格、配色、panel/chip/button 語彙。
  - 契約：本 work block 只改 Ticket A 範圍；不得修改 scheduler detection/native scheduling/backup execution；路由維持 `/schedule`；Onboarding skip 只前進並提示設定位置，不 apply/remove schedule。
  - 2026-04-29 closeout：已依確認後的 Phase 2 版面方向完成 Schedule page 與 Onboarding schedule step，但保留 PathKeep 既有 dark/orange visual language。`/schedule` route 保持不變；左側欄項目移至 `SYSTEM`，三語改為 `Scheduled Backup Settings` / `定时备份设置` / `定時備份設定`。
  - UI truth：Schedule page 現在直接顯示平台、安裝狀態、legacy/error attention、目標間隔、目前設定、安裝/更新/移除 controls 與原 PME preview/manual/execute/verify panel；interval 改動會先顯示 explicit save/update action。Onboarding 新增 schedule install-or-skip step；skip 只進入 Ready 並提示 `System → Scheduled Backup Settings`，install path 只記錄 intent，真正 apply 仍在 finish 後使用既有 `apply_schedule` command。
  - 同步回寫 [`docs/features/archive.md`](../features/archive.md)、[`docs/architecture/desktop-command-surface.md`](../architecture/desktop-command-surface.md)、[`docs/design/screens-and-nav.md`](../design/screens-and-nav.md)、[`docs/plan/m1-solid-archive/schedule-security-and-storage.md`](m1-solid-archive/schedule-security-and-storage.md)、[`docs/plan/scheduled_backup_redesign_spec.md`](scheduled_backup_redesign_spec.md)、[`docs/plan/STATUS.md`](STATUS.md) 與 [`docs/plan/CHANGELOG.md`](CHANGELOG.md)。
  - 驗收結果：targeted Vitest / browser-preview e2e slices、`bun run check`、`bunx tauri build --debug --bundles app --no-sign`、fresh debug `.app` Computer Use truth pass。current-host truth pass 驗到 `legacy-install-detected` attention state 與 schedule controls；為避免改動使用者 LaunchAgents，未在真機點擊 install/remove/final finish，native apply/remove 行為由既有 Rust/platform tests 與 desktop bridge truth gate 覆蓋。

- [x] **WORK-SCHED-STATE-A** — Scheduled Backup Settings State-Machine Redesign
  - 讀先：
    `docs/plan/scheduled_backup_redesign_spec.md`
    `docs/plan/scheduled_backup_audit.md`
    `docs/design/screens-and-nav.md`
    `docs/design/ux-principles.md`
    `docs/design/design-tokens.md`
    `docs/features/archive.md`
    `docs/architecture/desktop-command-surface.md`
    `docs/plan/backend-hotspot-decomposition.md`
    `TESTING.md`
  - 目標：依使用者新插單，把 `/schedule` 重新改成狀態機驅動的系統設定頁：`CHECKING`、`NOT_INSTALLED`、`INSTALLED_OK`、`INSTALLED_WARN`、`INSTALLED_ERROR`；同時先拆 `vault-platform::scheduler`，避免繼續往超過 1400 行的巨檔加業務邏輯。
  - 契約：保留 `preview_schedule`、`schedule_status`、`apply_schedule`、`remove_schedule`，新增明確的 `repair_schedule`；不改 backup worker 的實際備份語義、不改 interval options、不靜默 migrate/remove legacy scheduler artifacts；瀏覽器清單在 `/schedule` 只讀，修改入口指向 `Settings > Browser Profiles`。
  - 2026-04-29 closeout：`vault-platform::scheduler` 已拆成 721 行 facade，平台 owner 下沉到 `scheduler/{macos,windows,linux,audit}.rs`；macOS status 現在回報 canonical loaded check、mismatch、permission/read failure、known legacy evidence 與 typed verification checks。`repair_schedule` 只在使用者明確點擊後移除 known pre-rename macOS LaunchAgent labels。
  - UI truth：`/schedule` 由 route-owned `useScheduleWorkflow` 管理初次偵測、手動重新偵測 timestamp、install/update/remove/repair/verify/copy-diagnostics progress/result 與 state transitions；Legacy warning 已併入 `INSTALLED_WARN`，installed-but-never-run 也會走 warning state。手動模式是 state-local step list，包含目的、折疊原因、命令/完整檔案內容、目錄提示、單步自動/驗證 controls、一鍵全部自動執行與「我已完成操作」重新偵測。
  - 同步回寫 [`docs/features/archive.md`](../features/archive.md)、[`docs/architecture/desktop-command-surface.md`](../architecture/desktop-command-surface.md)、[`docs/architecture/module-boundary-map.md`](../architecture/module-boundary-map.md)、[`docs/design/screens-and-nav.md`](../design/screens-and-nav.md)、[`docs/plan/m1-solid-archive/schedule-security-and-storage.md`](m1-solid-archive/schedule-security-and-storage.md)、[`docs/plan/scheduled_backup_redesign_spec.md`](scheduled_backup_redesign_spec.md)、[`docs/plan/backend-hotspot-decomposition.md`](backend-hotspot-decomposition.md)、[`docs/plan/BACKLOG.md`](BACKLOG.md)、[`docs/plan/STATUS.md`](STATUS.md) 與 [`docs/plan/CHANGELOG.md`](CHANGELOG.md)。
  - 驗收結果：`cargo test --manifest-path src-tauri/Cargo.toml -p vault-platform repair_schedule -- --test-threads=1`、targeted Schedule / workflow / command Vitest slices、`bun run test:e2e`、`bun run check` 全通過（100% JS/Rust coverage、browser-preview e2e、desktop-bridge truth gate、desktop-contract mutation）。fresh debug `.app` Computer Use truth pass 使用 repo bundle `src-tauri/target/debug/bundle/macos/PathKeep.app`，確認 current-host `INSTALLED_WARN` legacy state、`重新偵測` timestamp、手動修復步驟的 plist/command/open-path/單步驗證 controls；未點擊 repair/reinstall/remove，避免未確認地修改使用者 LaunchAgents。
  - 驗收備註：直接啟動 debug binary 沒有 bundle identity，Computer Use 會抓到 `/Applications/PathKeep.app` stale UI；本輪改用 repo debug `.app` bundle 驗證。`bunx tauri build --debug` 已產生可驗證的 `.app` bundle，但後續 DMG bundling 失敗，未作為 release gate。

- [x] **WORK-SCHED-CUSTOM-INTERVAL-A** — Scheduled Backup Minute-Level Custom Interval
  - 讀先：
    `docs/plan/scheduled_backup_redesign_spec.md`
    `docs/plan/scheduled_backup_audit.md`
    `docs/features/archive.md`
    `docs/design/screens-and-nav.md`
    `docs/plan/m1-solid-archive/schedule-security-and-storage.md`
    `TESTING.md`
  - 目標：依使用者 follow-up，把自動備份觸發間隔從固定 `6h / 12h / 24h / 72h` 擴成保留 presets 但支援使用者輸入自訂整數分鐘。
  - 契約：UI 顯示與輸入單位為分鐘；presets 仍保留 6 / 12 / 24 / 72 小時；設定仍透過既有 `dueAfterHours` schema persisted，但允許 fractional hours 代表 minute-level intervals；不改 backup worker 的備份執行語義，不做 silent legacy cleanup；Linux 仍維持 `OnCalendar + Persistent`，不得切成 `OnUnitActiveSec`。
  - 2026-04-29 closeout：Schedule 與 Onboarding 共用的 interval selector 已改成分鐘輸入，三語 i18n / aria / invalid state copy 已同步。Rust schedule/config/read model 從整數小時改為 `f64` 小時，worker 的 native schedule wake interval 會取 `min(due interval, health-check interval)`，避免 90 分鐘這類短於 6 小時的自訂值等不到下一次 wake。
  - 平台語義：macOS LaunchAgent 使用整數秒 `StartInterval`；Windows Task Scheduler XML 使用 ISO 8601 minute duration（例如 `PT90M` / `PT1H30M`）；Linux 仍使用 calendar timer，不能精確表示的分鐘間隔會選擇不晚於 due window 的安全 wake cadence，再由 `--due-only` guard 保證實際備份到期判斷。
  - 同步回寫 [`docs/features/archive.md`](../features/archive.md)、[`docs/design/screens-and-nav.md`](../design/screens-and-nav.md)、[`docs/plan/m1-solid-archive/schedule-security-and-storage.md`](m1-solid-archive/schedule-security-and-storage.md)、[`docs/plan/scheduled_backup_audit.md`](scheduled_backup_audit.md)、[`docs/plan/scheduled_backup_redesign_spec.md`](scheduled_backup_redesign_spec.md)、[`docs/plan/program/repo-baseline.md`](program/repo-baseline.md)、[`docs/plan/STATUS.md`](STATUS.md) 與 [`docs/plan/CHANGELOG.md`](CHANGELOG.md)。
  - 驗收結果：targeted interval / Schedule / Onboarding / worker / platform tests 與 `bun run check` 全通過（100% JS/Rust coverage、browser-preview e2e、desktop-bridge truth gate、desktop-contract mutation）。fresh debug `.app` Computer Use truth pass 使用 repo bundle `src-tauri/target/debug/bundle/macos/PathKeep.app`，實測 90 分鐘更新後 config `dueAfterHours = 1.5`、plist `StartInterval = 5400`、`launchctl` run interval `5400 seconds`；再用 UI 還原 6 小時並確認 config `6.0`、plist `21600`、`launchctl` run interval `21600 seconds`。同輪也驗證非法 `0` 分鐘 inline error、`驗證安裝`、`重新偵測` timestamp 與 `查看安裝細節`。
  - 驗收備註：未在真機點擊 `移除已安裝排程`，因它會刪除本機 LaunchAgent plist，屬於破壞性本機刪除；remove 行為由 Rust/platform tests 與 desktop bridge truth gate 覆蓋。

- [x] **WORK-RELEASE-AI-DEFER-A** — Defer Optional AI And Readable Content From v0.1.0
  - 讀先：
    `docs/architecture/decisions/009-default-desktop-optional-intelligence-shipping.md`
    `docs/architecture/tech-stack.md`
    `docs/features/intelligence.md`
    `docs/features/archive.md`
    `docs/architecture/data-model.md`
    `docs/design/screens-and-nav.md`
  - 目標：依使用者 0.1.0 release 指令，將實測不可用的 AI Assistant、embedding、semantic / hybrid search、vector sidecar、MCP / skill artifacts，以及網頁正文抓取先推到後續版本；UI 保留入口但禁用，不再假裝 v0.1.0 可用。
  - 契約：deterministic Core Intelligence、keyword Explorer、archive/import/backup/Audit/Jobs/Schedule/Settings 仍保持可用；`rig-core` 與 future-facing AI schema 暫留；直接 `lancedb` / `lance` / `datafusion` build dependency 從 v0.1.0 移除；readable-content fetch 不得 enqueue 或抓取網頁正文。
  - 2026-04-29 closeout：Assistant route 顯示 `Coming in v0.2` disabled state；Explorer Semantic / Hybrid mode 可見但 disabled，deep-link 也只顯示延期說明；Settings AI provider、Integrations MCP/skill artifacts、Dashboard AI quick actions、Jobs readable-content card、Settings derived readable-content plugin 都改成 disabled / future-release 文案。`readable-content-refetch` default config 與 runtime enabled check 也改為 disabled，既有 readable-content job 不再發出 network refetch。
  - 同步回寫 [`docs/architecture/decisions/009-default-desktop-optional-intelligence-shipping.md`](../architecture/decisions/009-default-desktop-optional-intelligence-shipping.md)、[`docs/architecture/tech-stack.md`](../architecture/tech-stack.md)、[`docs/architecture/data-model.md`](../architecture/data-model.md)、[`docs/features/intelligence.md`](../features/intelligence.md)、[`docs/features/archive.md`](../features/archive.md)、[`docs/features/intelligence-current-state.md`](../features/intelligence-current-state.md)、[`docs/plan/BACKLOG.md`](BACKLOG.md)、[`docs/plan/STATUS.md`](STATUS.md) 與 [`docs/plan/CHANGELOG.md`](CHANGELOG.md)。
  - 驗收結果：Cargo dependency graph verification confirmed no `lancedb` / `lance` / `arrow-array` / `arrow-schema` / `datafusion` / `tantivy` packages in the v0.1.0 build graph；`bun run check` 通過（base checks、100% JS/Rust coverage、build、browser-preview e2e、desktop-bridge truth gate、desktop-contract mutation）。fresh debug `.app` Computer Use truth pass 使用 repo bundle `src-tauri/target/debug/bundle/macos/PathKeep.app`，確認 Dashboard AI quick actions disabled、Assistant route 顯示 v0.2 延期狀態、Settings AI 服務 controls disabled；Explorer semantic / hybrid disabled 由 browser-preview e2e 覆蓋。

- [x] **WORK-WINDOWS-BACKUP-STAGING-A** — Windows Backup Staging Path Hotfix
  - 讀先：
    `docs/features/archive.md`
    `docs/architecture/browser-support-and-adapter-playbook.md`
    `docs/architecture/desktop-command-surface.md`
    `TESTING.md`
  - 目標：修復 Windows 0.1.0 包在首次 setup / manual backup 選到 Firefox profile 時，因 `firefox:<profile>` raw profile id 被直接放進 staging tempdir 名稱而失敗的 stop-ship 問題。
  - 契約：不改 profile id、selected profile、archive metadata、parser family、或 Browser Direct payload contract；只在檔案系統邊界使用可逆 Windows-safe path segment，確保 staging tempdir 與 raw-source checkpoint 目錄都不含 Windows 禁用字元。
  - 2026-04-30 closeout：`vault_core::utils` 新增 reversible filesystem segment encode/decode；backup staging tempdir prefix 與 raw-source checkpoint 目錄改用 encoded segment；snapshot restore fallback 會把新 encoded checkpoint 目錄 decode 回原始 profile id，舊 macOS/Linux raw-name checkpoint 仍可讀。
  - 同步回寫 [`docs/features/archive.md`](../features/archive.md) 與 [`docs/plan/CHANGELOG.md`](CHANGELOG.md)。
  - 驗收結果：targeted `vault-core` tests 通過：`path_segment_encoding_is_windows_safe_and_reversible`、`stage_profile_snapshot_copies_database_and_sidecars`、`checkpoint_profile_reconstruction_covers_missing_source_rows_and_artifact_edges`、`cargo test --manifest-path src-tauri/Cargo.toml -p vault-core chrome -- --test-threads=1`、`cargo test --manifest-path src-tauri/Cargo.toml -p vault-core backup_ -- --test-threads=1`。Windows 實機重跑仍需使用者用新 build 驗證。

- [x] **WORK-RELEASE-012-A** — Release Truth And Demo Gate Recovery
  - 讀先：
    `docs/plan/STATUS.md`
    `docs/plan/BACKLOG.md`
    `docs/features/intelligence.md`
    `docs/design/screens-and-nav.md`
    `TESTING.md`
    `README.md`
  - 目標：逐項落實最新 AI 評委報告中仍真實存在的 release-truth 問題：重新跑權威 checker、修復當前 `bun run check` 紅燈、確認 installed app / repo HEAD / source docs 的 truth 一致，並把 v0.1.0 的 AI / semantic deferred 訊息收乾淨。
  - 契約：不恢復 v0.1.0 optional AI / semantic / hybrid runtime；不新增 Tauri command、IPC payload、browser support scope、backend ingest 行為或 release packaging policy；不得降低 coverage threshold、不得把 active runtime 檔案用 broad exclude 踢出 `coverage:js`；所有 user-visible copy 維持 `en` / `zh-CN` / `zh-TW` parity。
  - 2026-04-30 closeout：v0.1.0 optional AI / semantic scope 的 copy 已收斂成 `Coming in v0.2`：Assistant、Core Intelligence semantic status、Settings AI provider、Integrations MCP / skill artifacts 與 Dashboard AI controls 不再引導使用者到 Settings 開啟 AI。Explorer `All time` 狀態的 START / END 欄位改成空值與 `All time` placeholder，避免 native date input 在 WKWebView 顯示出假日期。README 現在把 v0.1 scope 壓成 Archive + keyword recall + deterministic Core Intelligence，並補上 installation / uninstall truth。
  - 同步回寫 [`README.md`](../../README.md)、[`docs/features/intelligence.md`](../features/intelligence.md)、[`docs/features/intelligence-current-state.md`](../features/intelligence-current-state.md)、[`docs/design/screens-and-nav.md`](../design/screens-and-nav.md)、[`docs/plan/STATUS.md`](STATUS.md) 與 [`docs/plan/CHANGELOG.md`](CHANGELOG.md)。
  - 驗收結果：targeted Explorer / AI-provider Vitest slices、`bun run check:i18n`、`bun run coverage:js` 與 `bun run check` 全通過（100% JS/Rust coverage、browser build、browser-preview e2e、desktop-bridge truth gate、desktop-contract mutation）。`bun run check` 中的 Vite build 仍保留既有 `shared` chunk 約 `508.84 kB` 警告。fresh debug `.app` Computer Use truth pass 使用 repo bundle `src-tauri/target/debug/bundle/macos/PathKeep.app`，確認 Dashboard 真資料、Explorer keyword `github` 搜尋、Semantic / Hybrid disabled state、Assistant v0.2 deferred state、Import landing、Schedule status、Settings AI deferred copy 與 Integrations v0.2 copy。

- [x] **WORK-M14-A** — Lexical Recall V2 Primary Path
  - 讀先：
    `docs/features/recall.md`
    `docs/architecture/data-model.md`
    `docs/architecture/tech-stack.md`
    `docs/architecture/lexical-recall-v2.md`
    `docs/plan/m14-lexical-recall-v2/README.md`
    `TESTING.md`
  - 目標：不上 embedding，把 Explorer keyword recall 從 unicode61 prefix-only FTS 升級成 dependency-free normalization + SQLCipher-backed FTS5 trigram / CJK grams + relevance ranking 的 lexical recall v2。
  - 契約：不引入 SQLite loadable extension、`spellfix1`、Jieba、embedding、semantic/hybrid runtime 或 vector sidecar；不改 search derived DB 的 plaintext SQLCipher attach policy；regex mode 保持 manual post-filter；import finalization 仍只刷新 touched import batch 的 search projection，不把 full rebuild 拉回主線。
  - 2026-05-03 closeout：`history-search.sqlite` derived projection 升到 schema version 2，保留 raw URL/title/search terms，同步新增 normalized fields、compact text、CJK gram text、`history_search_terms` unicode61 prefix FTS 與 `history_search_trigram` FTS。Keyword query 現在共用 repo-owned analyzer，走 lowercase、compact punctuation/space-insensitive text、CJK 2/3-grams，再以 FTS candidate union + BM25 relevance 排序；regex mode 仍走 manual post-filter。
  - Supply-chain remediation：未經明確批准的 OpenCC / Unicode normalization 依賴已移除。繁簡中文與全形/半形 folding 不再列為 M14-A shipped behavior；必須等官方 OpenCC 或其他方案通過 supply-chain 審核與用戶明確授權。
  - 同步回寫 [`docs/architecture/lexical-recall-v2.md`](../architecture/lexical-recall-v2.md)、[`docs/features/recall.md`](../features/recall.md)、[`docs/architecture/data-model.md`](../architecture/data-model.md)、[`docs/architecture/tech-stack.md`](../architecture/tech-stack.md)、[`docs/plan/m14-lexical-recall-v2/README.md`](m14-lexical-recall-v2/README.md)、[`docs/plan/README.md`](README.md)、[`docs/plan/BACKLOG.md`](BACKLOG.md)、[`docs/plan/STATUS.md`](STATUS.md) 與 [`docs/plan/CHANGELOG.md`](CHANGELOG.md)。
  - 驗收結果：targeted `vault-core` search / lexical recall / archive tests、Rust fmt+clippy、Explorer / preview Vitest slices、`bun run coverage:js` 與 `bun run check` 通過。`WORK-M14-B` 已留在 BACKLOG，blocked on candidate-volume benchmark / dedicated fuzzy-recall window。

- [x] **WORK-M14-C** — Approved Chinese Normalization Supply-Chain Review
  - 讀先：
    `AGENTS.md`
    `docs/architecture/lexical-recall-v2.md`
    `docs/features/recall.md`
    `docs/architecture/tech-stack.md`
    `docs/plan/m14-lexical-recall-v2/README.md`
  - 目標：修正 M14-A remediation 後過度收窄的 normalization truth：Unicode Consortium / ICU4X 滿足 dependency trust gate，應直接恢復 NFKC / full-width folding；OpenCC 只允許官方 C/C++ toolchain 或 repo-owned audited implementation；fuzzy recall 的 `strsim` 因 RapidFuzz maintainer provenance 可作為 bounded rerank 候選。
  - 契約：不重新引入 rejected `ferrous-opencc`；不使用低信任 Rust OpenCC binding；不把 OpenCC 接進產品碼直到本地與 CI 的 CMake / C++ / header / link / packaging requirements 有 proof；`strsim` 只能用在 FTS/trigram bounded candidate set 之後，禁止 SQL full-scan edit distance。
  - 2026-05-03 closeout：`vault-core` analyzer 改為 ICU4X `icu_normalizer` NFKC → lowercase → compact / CJK gram pipeline，恢復 full-width / half-width compatibility recall；`icu_normalizer` 是 Unicode Consortium / ICU4X 維護，且已經由既有 URL/IDNA stack 進入 lockfile，本次只把它收成 direct `vault-core` dependency，並關閉不需要的 `utf16_iter` / `write16` feature。繁簡中文 folding 仍未 shipping，下一步由 `WORK-M14-D` 走官方 OpenCC toolchain proof。
  - 同步回寫 [`docs/architecture/lexical-recall-v2.md`](../architecture/lexical-recall-v2.md)、[`docs/features/recall.md`](../features/recall.md)、[`docs/architecture/tech-stack.md`](../architecture/tech-stack.md)、[`docs/plan/m14-lexical-recall-v2/README.md`](m14-lexical-recall-v2/README.md)、[`docs/plan/BACKLOG.md`](BACKLOG.md)、[`docs/plan/STATUS.md`](STATUS.md) 與 [`docs/plan/CHANGELOG.md`](CHANGELOG.md)。
  - 驗收結果：targeted `cargo test --manifest-path src-tauri/Cargo.toml -p vault-core search_lexical -- --test-threads=1` 與 `cargo test --manifest-path src-tauri/Cargo.toml -p vault-core lexical_recall -- --test-threads=1` 通過；`bun run check` 通過。

- [x] **WORK-M14-D** — Official OpenCC Toolchain And Script Folding
  - 讀先：
    `AGENTS.md`
    `docs/architecture/lexical-recall-v2.md`
    `docs/features/recall.md`
    `docs/architecture/tech-stack.md`
    `docs/plan/m14-lexical-recall-v2/README.md`
    `TESTING.md`
  - 目標：用官方 OpenCC C/C++ project / official assets 恢復繁簡中文 folding，但先把本地與 CI 的 CMake / C++ / header / link / packaging requirements 做成可驗證 contract。
  - 契約：不得使用 rejected `ferrous-opencc` 或低信任 Rust binding；不得依賴使用者本機 Homebrew dylib 才能跑；必須在產品碼接入前證明 CI toolchain、static/dynamic link 策略、release packaging、license/provenance、rollback path；如果官方 OpenCC path 讓 per-commit checker 脆弱，必須停在 ADR / toolchain fix，不得硬塞功能。
  - 2026-05-03 closeout：未使用 OpenCC native C++ library，也未重新引入任何低信任 Rust binding。產品碼改走 official OpenCC `ver.1.3.0` Apache-2.0 dictionary assets + repo-owned Rust converter：`t2s` 與 `tw2sp` 變體都進 index/query，解決 `設定` / `设定` / `设置` 互相召回。本機 probe 記錄 `cmake` / `pkg-config` 不在 `PATH`、Python/clang++ 可用，因此 native C++ link path 仍只保留為 future path，必須先證明 CI packages、link strategy、release packaging 與 rollback。
  - 同步回寫 [`docs/architecture/opencc-script-folding.md`](../architecture/opencc-script-folding.md)、[`docs/architecture/lexical-recall-v2.md`](../architecture/lexical-recall-v2.md)、[`docs/features/recall.md`](../features/recall.md)、[`docs/architecture/tech-stack.md`](../architecture/tech-stack.md)、[`docs/plan/m14-lexical-recall-v2/README.md`](m14-lexical-recall-v2/README.md)、[`docs/plan/README.md`](README.md)、[`docs/plan/BACKLOG.md`](BACKLOG.md)、[`docs/plan/STATUS.md`](STATUS.md) 與 [`docs/plan/CHANGELOG.md`](CHANGELOG.md)。
  - 驗收結果：targeted `cargo test --manifest-path src-tauri/Cargo.toml -p vault-core search_opencc -- --test-threads=1`、`search_lexical`、`lexical_recall` 通過；`bun run check` 通過。

- [x] **WORK-M14-B** — Bounded Fuzzy Recall And Query Expansion
  - 讀先：
    `docs/architecture/lexical-recall-v2.md`
    `docs/features/recall.md`
    `docs/architecture/data-model.md`
    `docs/plan/m14-lexical-recall-v2/README.md`
    `TESTING.md`
  - 目標：在 M14-A/C/D 的 FTS/trigram 候選之後，加入 bounded Rust-side fuzzy search / typo tolerance / alias expansion，而不是把 Levenshtein 變成 SQL full scan。
  - 契約：只能在 FTS/trigram 已產生 bounded candidate set 後使用 edit-distance rerank；不得啟用 SQLite loadable extension、`spellfix1`、Jieba、embedding、semantic/hybrid runtime 或 vector sidecar；alias dictionary 必須小型、可審查、可測試。
  - 2026-05-03 closeout：未新增 `strsim` 或任何新 third-party dependency；M14-B 用 repo-owned bounded edit distance 完成 Latin typo fallback。短別名 `gh` / `yt` / `pr` 在 query analyzer 內展開成 `github` / `youtube` / `pull request`。Fuzzy fallback 只在正常 FTS/trigram 結果為 0 時啟動，先用 trigram OR 查出最多 200 個 URL document / 400 個 visible visit，再在 Rust 內按 title > url > search terms > compact text 打分；Regex mode 不受影響。
  - 同步回寫 [`docs/plan/m14-lexical-recall-v2/fuzzy-candidate-benchmark.md`](m14-lexical-recall-v2/fuzzy-candidate-benchmark.md)、[`docs/architecture/lexical-recall-v2.md`](../architecture/lexical-recall-v2.md)、[`docs/features/recall.md`](../features/recall.md)、[`docs/plan/m14-lexical-recall-v2/README.md`](m14-lexical-recall-v2/README.md)、[`docs/plan/BACKLOG.md`](BACKLOG.md)、[`docs/plan/STATUS.md`](STATUS.md) 與 [`docs/plan/CHANGELOG.md`](CHANGELOG.md)。
  - 驗收結果：candidate-volume probe `search_projection::tests::fuzzy_trigram_candidate_probe_is_limited_before_rust_rerank`、targeted `search_lexical` 與 `lexical_recall` tests 通過；`bun run check` 通過。

- [x] **WORK-M14-E** — Project-Scoped Native Dependency Tooling
  - 讀先：
    `AGENTS.md`
    `docs/architecture/opencc-script-folding.md`
    `docs/architecture/lexical-recall-v2.md`
    `docs/architecture/tech-stack.md`
    `docs/plan/m14-lexical-recall-v2/README.md`
  - 目標：把未來 OpenCC / marisa / 其他 C/C++ 產品依賴的長期管理方式落成 repo-local contract，避免開發機或 CI 需要全局安裝 Homebrew / apt / winget native library 才能編譯產品。
  - 契約：產品碼仍不切到 native OpenCC；不得在 `build.rs` 裡下載或編譯任意 C/C++ source；vcpkg 只能作 project-scoped native dependency manager，不能繞過 supply-chain trust gate；Apple Silicon native OpenCC 仍 blocked until stock vcpkg port or audited overlay supports arm64 release targets。
  - 2026-05-03 closeout：新增 root `vcpkg.json` / `vcpkg-configuration.json`，pin Microsoft vcpkg registry baseline `522253caf47268c1724f486a035e927a42a90092`，並把 OpenCC native proof lane 放進 optional `opencc` feature。新增 `scripts/native-deps.mjs`，所有輸出寫到 ignored `var/native-deps`；新增 `.github/workflows/native-deps.yml`，用 Linux / Windows / Intel macOS proof workflow 驗證 project-scoped OpenCC install。`AGENTS.md` 與 architecture docs 現在明確禁止產品 native library 依賴全局 Homebrew / apt / winget / `pkg-config` 路徑。
  - 同步回寫 [`docs/architecture/native-dependency-management.md`](../architecture/native-dependency-management.md)、[`docs/architecture/opencc-script-folding.md`](../architecture/opencc-script-folding.md)、[`docs/architecture/lexical-recall-v2.md`](../architecture/lexical-recall-v2.md)、[`docs/architecture/tech-stack.md`](../architecture/tech-stack.md)、[`docs/plan/m14-lexical-recall-v2/README.md`](m14-lexical-recall-v2/README.md)、[`docs/plan/STATUS.md`](STATUS.md) 與 [`docs/plan/CHANGELOG.md`](CHANGELOG.md)。
  - 驗收結果：`bun run native-deps:doctor`、`node --check scripts/native-deps.mjs`、`bun run format:check`、`bun run check` 通過。未執行 `native-deps:install:opencc`，因為它是 slow/native proof lane 且產品目前不 link OpenCC C++；CI workflow 會在相關檔案 PR 上跑 project-scoped install proof。

- [x] **WORK-HISTORY-MAINT-A** — Archive History Read Surface Maintainability Review
  - 讀先：
    `src-tauri/crates/vault-core/src/archive/history.rs`
    `docs/architecture/lexical-recall-v2.md`
    `docs/architecture/module-boundary-map.md`
    `TESTING.md`
  - 目標：`WORK-M14-B` 後 `archive/history.rs` 到 `1229` 行，超過 1200 行 maintainability review threshold。專門審查 history read surface 是否要拆出 lexical SQL/fuzzy pagination/export/favicon hydration owners，而不是在 recall closeout 中順手重構。
  - 契約：審查階段先輸出 architecture map、職責邊界、拆分候選與測試保護，不直接改業務碼；不得在同一輪順手改排序、pagination、regex 或 favicon 行為；如果結論是不拆，必須寫明為什麼不拆比拆更好。
  - 2026-05-03 closeout：新增 [`docs/plan/history-read-surface-maintainability-review.md`](history-read-surface-maintainability-review.md)，確認 `history.rs` 目前同時承擔 public facade、baseline SQL recall、lexical/fuzzy recall、regex post-filter、pagination envelope、lazy favicon hydration、export rendering 與 row shaping。結論是 staged split 值得做，但不應一刀重寫；第一個 behavior-preserving code slice 只拆 pagination / favicon / export owners，lexical SQL 與 baseline SQL 後移。
  - 同步回寫 [`docs/architecture/module-boundary-map.md`](../architecture/module-boundary-map.md)、[`docs/plan/history-read-surface-maintainability-review.md`](history-read-surface-maintainability-review.md)、[`docs/plan/STATUS.md`](STATUS.md)、[`docs/plan/BACKLOG.md`](BACKLOG.md) 與 [`docs/plan/CHANGELOG.md`](CHANGELOG.md)。
  - 驗收結果：review-only block，未改 Rust product code；`bunx prettier --check` 與 `git diff --check` 通過。`bun run check` 仍以同日 `WORK-M14-E` closeout 的 green run 作為此審查基線。

- [x] **WORK-HISTORY-MAINT-B** — Archive History Read Surface Owner Extraction
  - 讀先：
    `docs/plan/history-read-surface-maintainability-review.md`
    `src-tauri/crates/vault-core/src/archive/history.rs`
    `docs/architecture/module-boundary-map.md`
    `TESTING.md`
  - 目標：依 `WORK-HISTORY-MAINT-A` 的審查結論，做第一個 behavior-preserving extraction：把 pagination helpers、lazy favicon hydration、export collection/rendering 拆到 `archive/history/` 子模組，讓 `history.rs` 回到 1200 行以下，並保留 public facade。
  - 契約：不得改 ranking、SQL filtering、regex behavior、fuzzy candidate limits、cursor encoding、export format、favicon fallback precedence 或 public `list_history` / `export_history` / `load_history_favicons` API；不得在同一 slice 重寫 lexical SQL 或 baseline SQL。
  - 2026-05-03 closeout：`archive/history.rs` 保留 public facade、mode dispatch、baseline/lexical/fuzzy/regex SQL 與 row shaping；pagination cursor/response helpers、lazy favicon hydration、export cursor-walk/rendering 已拆到 `archive/history/{pagination,favicons,export}.rs`。`history.rs` 從 `1229` 行降到 `729` 行，低於 1200 行 threshold；public API 與 SQL/ranking/cursor/export/favicon precedence contract 未改。
  - 同步回寫 [`docs/plan/history-read-surface-maintainability-review.md`](history-read-surface-maintainability-review.md)、[`docs/architecture/module-boundary-map.md`](../architecture/module-boundary-map.md)、[`docs/plan/STATUS.md`](STATUS.md)、[`docs/plan/BACKLOG.md`](BACKLOG.md) 與 [`docs/plan/CHANGELOG.md`](CHANGELOG.md)。
  - 驗收結果：targeted `cargo test --manifest-path src-tauri/Cargo.toml -p vault-core lexical_recall -- --test-threads=1`、`cargo test --manifest-path src-tauri/Cargo.toml -p vault-core archive::tests -- --test-threads=1`、Schedule Vitest regression slices、`cargo fmt --manifest-path src-tauri/Cargo.toml --all --check`、`git diff --check` 與 `bun run check` 通過。`bun run check` 中仍有既有 Vite `shared` chunk 超過 500 kB warning。

- [x] **WORK-RELEASE-WINDOWS-UNSIGNED-A** — Unsigned Windows Installer Release Path
  - 讀先：
    `README.md`
    `RELEASE.md`
    `TESTING.md`
    `docs/plan/program/quality-matrix.md`
    `.github/workflows/release.yml`
    `src-tauri/tauri.conf.json`
  - 目標：撤回 Windows release 必須先有 code signing 的錯誤 gate，讓 v0.2 Windows 可以透過 GitHub `Release` workflow 產出 unsigned MSI / NSIS installer。
  - 契約：不引入 Windows code-signing provider、不要求 PFX / Azure Trusted Signing / certificate thumbprint、不把 Windows release support 綁到 signing secret；unsigned release 必須明確告知 `Unknown Publisher` / SmartScreen prompt；如果需要 updater artifacts，仍由 updater minisign key 另行控制，不和 Windows installer code signing 混在一起。
  - 2026-05-04 closeout：Git history 已回到 `64a89c33` 乾淨基底後重做本修復；錯誤 signing-gate commit 不再作為本 work block 的基底。`Release` workflow manual dispatch 預設 `unsigned_preview=true`，Windows platform option 保留，unsigned path 透過 `--no-sign --config src-tauri/ci.unsigned.conf.json` 關閉 updater artifacts；`src-tauri/tauri.conf.json` 固定 Windows WebView2 `offlineInstaller` 並 pin WiX `upgradeCode`，避免 Windows host 缺 WebView2 runtime 時只能依賴網路 bootstrap。Updater fallback / manifest endpoint、browser-preview fallback URL 與 GitHub issue support links 全部改回 `t41372/PathKeep`。
  - 新增 `bun run release:check` 並納入 `check:base`，保護 PathKeep release URLs、unsigned Windows workflow、WebView2 offline installer、WiX upgrade code、support links，並防止 Windows signing gate / Azure signing script 被加回來。
  - 同步回寫 [`README.md`](../../README.md)、[`RELEASE.md`](../../RELEASE.md)、[`TESTING.md`](../../TESTING.md)、[`docs/plan/program/quality-matrix.md`](program/quality-matrix.md)、[`docs/plan/STATUS.md`](STATUS.md) 與 [`docs/plan/CHANGELOG.md`](CHANGELOG.md)。
  - 驗收結果：`ruby -e 'require "yaml"; YAML.load_file(".github/workflows/release.yml")'`、`jq empty src-tauri/tauri.conf.json package.json`、`node --check scripts/verify-release-config.mjs`、`bun run release:check`、`git diff --check` 與 `bun run check` 通過。

- [x] **WORK-WINDOWS-RUNTIME-HARDENING-A** — Windows Runtime Dependency And Shell Hardening
  - 讀先：
    `docs/features/archive.md`
    `docs/architecture/tech-stack.md`
    `src-tauri/crates/vault-core/src/git_audit.rs`
    `src-tauri/crates/vault-core/src/chrome/paths.rs`
    `src-tauri/crates/vault-core/src/remote/transfer.rs`
    `src-tauri/crates/vault-platform/src/scheduler/windows.rs`
    `TESTING.md`
  - 目標：深度審查 macOS-only 開發後 Windows 容易炸的 runtime surface，優先修復 browser import / backup 在 Windows 無 Git 時失敗、Unix-only test helpers、以及 Windows shell preview drift。
  - 契約：不得把使用者電腦全局 Git 當成資料操作前置條件；audit artifacts 必須先落 ordinary files；optional Git history 失敗只能降級成 warning；不新增 Cargo / npm / Bun / Tauri dependency。
  - 2026-05-06 closeout：`git_audit` 已拆成 durable audit directory 與 optional Git repo 兩層；browser backup、Takeout import/revert/restore、doctor repair、snapshot restore 與 retention prune 都會先寫 ordinary audit artifacts，Git missing / broken / policy failure 不再讓資料操作失敗。fresh install 仍預設 best-effort 啟用 optional Git history：有 Git 的機器保留本地 commit trail，沒有 Git 的機器只在 artifact 寫入後降級成 skipped warning。Windows remote backup preview 改用 `curl.exe` / `%ENV%` / Windows command escaping；Unix-only test helpers 已加 `cfg` guard 或 Windows `.cmd` fixture。
  - 同步回寫 [`docs/features/archive.md`](../features/archive.md)、[`docs/architecture/tech-stack.md`](../architecture/tech-stack.md)、[`docs/plan/STATUS.md`](STATUS.md) 與 [`docs/plan/CHANGELOG.md`](CHANGELOG.md)。
  - 驗收結果：targeted `vault-core` git-audit / remote / takeout / archive tests、`vault-worker` compile check、`git diff --check` 與 `bun run check` 通過（100% JS/Rust coverage、browser build、browser-preview e2e、desktop-bridge truth gate、desktop-contract mutation）。macOS host 上嘗試 `x86_64-pc-windows-msvc` Rust no-run cross-check，但被本機缺 Windows C SDK / MSVC OpenSSL build toolchain 阻擋（`windows.h` / `assert.h` / `openssl-sys` VC target），仍需要真 Windows runner 做 release-grade compile proof。

- [x] **WORK-RELEASE-WINDOWS-SMALL-A** — Windows Small Installer And Actions Runtime Refresh
  - 讀先：
    `RELEASE.md`
    `docs/plan/program/quality-matrix.md`
    `.github/workflows/release.yml`
    `.github/workflows/ci.yml`
    `.github/workflows/platform-native.yml`
    `.github/workflows/mutation.yml`
    `.github/workflows/native-deps.yml`
    `src-tauri/tauri.conf.json`
    `scripts/verify-release-config.mjs`
  - 目標：修正 Windows unsigned preview installer 因 WebView2 offline installer 膨脹到約 200MB 的 release policy drift，並消除 GitHub Actions Node.js 20 runtime deprecation warning。
  - 2026-05-07 closeout：Windows bundle WebView2 install mode 從 `offlineInstaller` 改為 `downloadBootstrapper`，讓常見 Windows 11 / current Windows 10 安裝包回到小體積；缺 WebView2 Runtime 的少數機器仍會在安裝時靜默下載 bootstrapper。Release docs 與 quality matrix 已同步更新，不再把 offline installer 當 preview release contract。
  - Actions refresh：`actions/checkout` 升到 `v6`、`actions/setup-node` 升到 `v6`、`actions/upload-artifact` 升到 `v7`、`actions/cache` 升到 `v5`；`arduino/setup-protoc@v3` 已移除，改由既有 `taiki-e/install-action@v2` 安裝 `protoc`。`bun run release:check` 現在會阻擋這些舊 action refs 回流。
  - 同步回寫 [`RELEASE.md`](../../RELEASE.md)、[`docs/plan/program/quality-matrix.md`](program/quality-matrix.md)、[`docs/plan/STATUS.md`](STATUS.md)、[`src-tauri/tauri.conf.json`](../../src-tauri/tauri.conf.json)、[`scripts/verify-release-config.mjs`](../../scripts/verify-release-config.mjs) 與 `.github/workflows/*`。
  - 驗收結果：`bun run release:check`、`bunx prettier --check`、`ruby -e 'require "yaml"; YAML.load_file(...)'` workflow syntax sweep、`jq empty src-tauri/tauri.conf.json package.json`、`node --check scripts/verify-release-config.mjs` 與 `git diff --check` 通過。

- [x] **WORK-WINDOWS-SCHED-CANONICAL-A** — Windows Scheduler Canonical XML Status Fix
  - 讀先：
    `docs/features/archive.md`
    `docs/architecture/tech-stack.md`
    `src-tauri/crates/vault-platform/src/scheduler/windows.rs`
    `TESTING.md`
  - 目標：修復 Windows 版 Schedule 頁在 `schtasks /Query /XML` 成功後仍把已安裝任務判成 `windows-task-mismatch` 的問題。
  - 2026-05-07 closeout：Windows Task Scheduler status verification 不再用整段 XML 去空白後相等比對，改成比對行為欄位：command、arguments、repetition interval、logon trigger、interactive token、least privilege 與 `StartWhenAvailable`。這避開 Task Scheduler 匯入後自動重排 XML、補 battery / idle / unified scheduling defaults，以及把 principal user canonicalize 成 SID 的 false warning，同時仍保留 command / arguments / interval / trigger drift 的 mismatch 偵測。
  - 同步回寫 [`docs/features/archive.md`](../features/archive.md)、[`src-tauri/crates/vault-platform/src/scheduler/windows.rs`](../../src-tauri/crates/vault-platform/src/scheduler/windows.rs)、[`src-tauri/crates/vault-platform/src/scheduler.rs`](../../src-tauri/crates/vault-platform/src/scheduler.rs) 與 [`docs/plan/CHANGELOG.md`](CHANGELOG.md)。
  - 驗收結果：targeted `vault-platform` scheduler tests 覆蓋 Task Scheduler canonical XML shape；`bun run check` 通過後提交。

- [x] **WORK-WINDOWS-TEST-BINARY-A** — Artifact-Only Windows Test Binary Action
  - 讀先：
    `RELEASE.md`
    `TESTING.md`
    `.github/workflows/release.yml`
    `.github/workflows/platform-native.yml`
  - 目標：給 Windows 測試機產出最新 unsigned Windows binary / installer，但不覆蓋既有 `v0.1.0` GitHub Release assets。
  - 2026-05-07 closeout：新增 GitHub `Windows Test Binary` manual workflow，在 `windows-latest` 上跑 focused release config guard，使用與 release preview 相同的 unsigned Tauri override build，並把 `pathkeep-desktop.exe`、MSI / NSIS installer、`SHA256SUMS.txt`、`WINDOWS-TEST-MANIFEST.json` 作為 14 天 workflow artifact 上傳。
  - 同步回寫 [`RELEASE.md`](../../RELEASE.md)、[`TESTING.md`](../../TESTING.md)、[`.github/workflows/windows-test-binary.yml`](../../.github/workflows/windows-test-binary.yml) 與 [`docs/plan/CHANGELOG.md`](CHANGELOG.md)。
  - 驗收結果：workflow YAML syntax、`bun run release:check`、`bunx prettier --check`、`git diff --check` 通過後提交並 dispatch Action。

- [x] **WORK-EXPLORER-ADVANCED-SEARCH-A** — Local Advanced Keyword Operators And Regex Dialect Guard
  - 讀先：
    `docs/features/recall.md`
    `docs/architecture/lexical-recall-v2.md`
    `docs/architecture/data-model.md`
    `src-tauri/crates/vault-core/src/archive/history.rs`
    `src/pages/explorer/hooks/use-explorer-url-state.ts`
  - 目標：修復 Explorer regex UI 會把 JavaScript-only look-around 誤判為可送後端的問題，並把「某個 domain 下不包含某個關鍵詞」落成 keyword mode 的正式語法，而不是要求使用者寫負向前瞻正則。
  - 2026-05-07 closeout：Regex mode 明確以 Rust `regex` dialect 為準，前端會先擋下 look-around、named capture / backreference 等 Rust 不支援的 pattern。Keyword mode 新增本地 Google-like operators：`site:`、leading `-`、quoted exact phrase、`OR`、`intitle:`、`inurl:`、`filetype:` / `ext:`、`after:`、`before:`；這些只讀 URL/title/search terms/visit time 與 rebuildable search projection，不引入網路或網頁正文依賴。`site:github.com -pathkeep` 與 Domain 欄 `github.com` + query `-pathkeep` 都會走後端 SQL-side constraints。
  - 同步回寫 [`docs/features/recall.md`](../features/recall.md)、[`docs/architecture/lexical-recall-v2.md`](../architecture/lexical-recall-v2.md) 與 [`docs/plan/CHANGELOG.md`](CHANGELOG.md)。
  - 驗收結果：targeted Explorer / browser-preview Vitest、`vault-core` `search_lexical` / `search_query` / advanced-history tests 與 canonical backup history regression 通過；完整 `bun run check` 另行作為提交前 gate。
