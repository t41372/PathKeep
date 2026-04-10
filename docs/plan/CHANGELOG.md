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
