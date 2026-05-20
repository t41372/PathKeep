# BACKLOG — 後續工作塊佇列

> 這裡只放 **half-milestone 粒度**的 work blocks。  
> `STATUS.md` 清空或完成當前 block 後，才從這裡把下一個未被阻塞的 block 搬上去。

---

## 工作塊佇列（按順序）

> 2026-04-18 update：使用者已明確把第二台主機 benchmark parity 從當前計劃移除。current-host desktop truth audit 之後，新增一個 follow-up block，但目前仍屬 blocked。
> 2026-04-19 note：使用者直接插單的 `WORK-UI-D`（dashboard yearly rhythm + Intelligence IA cleanup）已從當輪暫時拉進 `STATUS.md` 完成並 append 到 `CHANGELOG.md`；BACKLOG 仍維持只有 blocked 的 `WORK-CI-N`。
> 2026-04-19 M6 follow-up：`WORK-M6-A` 已完成，並把下一輪 `WORK-M7-A — Cross-App Reuse Audit And Insight Entity Consolidation` 正式立項。依照 work-block 流程，`WORK-M7-A` 已從 BACKLOG 提升到 `STATUS.md` 作為當前 active block；BACKLOG 目前因此仍只剩 blocked 的 `WORK-CI-N`。
> 2026-04-19 M7 follow-up：`WORK-M7-A` 已完成，並把下一輪 `WORK-M8-A — Aggregate Entity Identity And Context Reuse` 正式立項、提升到 `STATUS.md`。BACKLOG 目前仍只剩 blocked 的 `WORK-CI-N`；M8 的 active plan 則改在 `STATUS.md` 與 `docs/plan/m8-aggregate-entity-identity/README.md` 維護。
> 2026-04-19 M8 follow-up：`WORK-M8-A` 已完成，並把下一輪 `WORK-M9-A — Remaining Reuse Inventory And Single-Source Map` 與 `WORK-M9-B — Shared Digest / CTA / Evidence Composition Extraction` 正式立項、提升到 `STATUS.md`。BACKLOG 目前再次只剩 blocked 的 `WORK-CI-N`；M9 的 active plan 改在 `STATUS.md` 與 `docs/plan/m9-cross-app-reuse/README.md` 維護。
> 2026-04-19 M9 follow-up：`WORK-M9-A` 與 `WORK-M9-B` 已完成，並把下一輪 `WORK-M10-A — Shared Review Rows And Workbench Surface Reuse` 與 `WORK-M10-B — Intelligence Route And Desktop Glue Decomposition` 正式立項、提升到 `STATUS.md`。BACKLOG 目前再次只剩 blocked 的 `WORK-CI-N`；M10 的 active plan 改在 `STATUS.md` 與 `docs/plan/m10-workbench-reuse/README.md` 維護。
> 2026-04-19 M10 follow-up：`WORK-M10-A` 與 `WORK-M10-B` 已完成，並把下一輪 `WORK-M11-A — App-Wide Reuse Inventory And Single-Source Map` 與 `WORK-M11-B — Shared Review / PME / Diagnostics Surface Extraction` 正式立項、提升到 `STATUS.md`。BACKLOG 目前再次只剩 blocked 的 `WORK-CI-N`；M11 的 active plan 改在 `STATUS.md` 與 `docs/plan/m11-app-wide-reuse/README.md` 維護。
> 2026-04-19 M11 follow-up：`WORK-M11-A` 與 `WORK-M11-B` 已完成，並把下一輪 `WORK-M12-A — Shared Support Actions And Diagnostics Inventory` 與 `WORK-M12-B — Support Action / Diagnostics Primitive Extraction` 正式立項、提升到 `STATUS.md`。BACKLOG 目前再次只剩 blocked 的 `WORK-CI-N`；M12 的 active plan 改在 `STATUS.md` 與 `docs/plan/m12-support-actions-and-diagnostics/README.md` 維護。
> 2026-04-19 M12 follow-up：`WORK-M12-A` 與 `WORK-M12-B` 已完成，並把下一輪 `WORK-M13-A — Broad Reuse Inventory Across Support / Trust / Workflow Surfaces` 與 `WORK-M13-B — Shared Support / Workflow Composition Extraction` 正式立項、提升到 `STATUS.md`。BACKLOG 目前再次只剩 blocked 的 `WORK-CI-N`；M13 的 active plan 改在 `STATUS.md` 與 `docs/plan/m13-broad-reuse-audit/README.md` 維護。
> 2026-04-20 stop-ship note：使用者臨時插單 `WORK-PERF-A`，先把 `/intelligence` large-archive 凍結與 revisit 卡頓當成 stop-ship blocker 處理。M13 broad reuse audit 沒有取消，只是暫時讓位給 performance hot-path recovery；closeout 後仍回到 `STATUS.md` 繼續。
> 2026-04-20 archive/import stop-ship closeout：使用者又插單 `WORK-PERF-B`，要求先修 Onboarding 初始化 / 手動備份 / Takeout scan-import 的 UI freeze。該 block 已獨立完成並 append 到 `CHANGELOG.md`；latest truth 是 archive/import 長任務的 Tauri facade 已改成 off-main-thread `async + spawn_blocking`，M13 broad reuse audit 則繼續留在 `STATUS.md`。
> 2026-04-20 search activity note：使用者又插單 `WORK-CI-R`，要求先修 Search Activity keyword truth、shared keyword browser 與 search-engine domain deep-dive。這個 block 已獨立完成並 append 到 `CHANGELOG.md`，不折回 M13 reuse scope；BACKLOG 目前仍只保留 blocked 的 `WORK-CI-N`。
> 2026-04-20 desktop truth-pass closeout：`WORK-CI-N` 已完成並 append 到 `CHANGELOG.md`。這輪先修掉前端 shipped blocker（remote glyph font、glyph a11y、Settings 分組 i18n），再補 shared entity CTA 的 HashRouter grammar，最後透過重打 current-host release `.app` 解掉 stale frontend drift。最新 Computer Use 驗到 build label `6412ad59+`、Chrome `Yi-Ting` re-import + `000000` 加密（未寫入鑰匙圈）、`rememberDatabaseKeyInKeyring: false` config truth、`/intelligence` 無 raw glyph ids、以及 domain deep-dive → Explorer evidence CTA 正常落到 `#/explorer?...`。BACKLOG 現在再次只剩 `STATUS.md` 上的 M13 active blocks，沒有額外 pending 的 desktop-truth follow-up。
> 2026-04-21 backend track note：使用者明確要求新增平行的後端 hotspot 拆分軌道。`WORK-BE-A` 已直接進入 `STATUS.md` 與 `docs/plan/backend-hotspot-decomposition.md`；這不是取消 `WORK-M13-B`，而是把 frontend reuse 與 backend decomposition 分開推進。
> 2026-04-23 backend follow-up note：`WORK-BE-C` 已完成 `visit_taxonomy` rename/split、`intelligence/site_dictionary` owner split、`models/core_intelligence` DTO-family split、`remote` bundle/upload/verify owner split、以及 `intelligence/mod.rs` 內嵌 regression suite 下沉。backend 軌道這個 active block 已收口；`STATUS.md` 的下一個未完成 block 回到 `WORK-M13-B`。`AGENTS.md` 若仍 dirty，視為使用者自有未提交改動，不納入 backend commits。
> 2026-04-23 M13-B closeout note：`WORK-M13-B` 已完成 shell runtime、Security workflow、Import workflow、Dashboard fallback、Browsing Rhythm state owner，以及 legacy `PathRow` stale-planning cleanup。BACKLOG 目前沒有可提升的未阻塞 work block；下一輪 work block 需要從新的使用者指令或正式 planning pass 建立。
> 2026-04-23 backend progress-audit note：使用者要求深度審查「後端拆分 / 屎山優化是否完成」後，live scan 確認 `WORK-BE-A/B/C` 只代表主要 giant-file 戰役完成，不代表整個 backend 完成。`WORK-BE-D` 已補掉 `vault-core::ai_queue` 內嵌 regression suite 造成的 1000 行越線；下一個 unblocked backend follow-up 已直接提升到 `STATUS.md` 的 `WORK-BE-E`，聚焦 `dev_ipc_bridge.rs`、command façade rustdoc、worker bridge rustdoc。
> 2026-04-24 backend command-mirror closeout note：`WORK-BE-E` 已完成，dev-only bridge 已拆成 focused `config` / `router` / `payloads` / `dispatch` owners，command / payload / worker export / localhost-only safety contract 維持不變。BACKLOG 目前沒有可提升的未阻塞 work block；下一輪 work block 需要新的使用者指令或正式 planning pass 建立。
> 2026-04-24 Safari Browser Direct closeout note：使用者插單的 `WORK-IMPORT-SAFARI-A` 已完成並 append 到 `CHANGELOG.md`。這輪修的是 `/import` Browser Direct local DB path，讓 Safari `History.db` 不再走 Takeout parser；當時 Browser Direct validated promise 仍只限 Chrome / Safari，後續 Atlas / Comet promotion 以 [browser-support-and-adapter-playbook.md](../architecture/browser-support-and-adapter-playbook.md) 與下方 closeout note 為準。Firefox / other browser adapter 不因這次修復升級公開承諾。BACKLOG 目前仍沒有可提升的未阻塞 work block。
> 2026-04-24 ChatGPT Atlas Browser Direct closeout note：使用者插單的 `WORK-IMPORT-ATLAS-A` 已完成並 append 到 `CHANGELOG.md`。Atlas 現在以 Chromium-family adapter 進入 macOS Browser Direct / backup support truth，current archive 已用本機 Atlas profile 完成 preview / import / re-import / revert / restore 並保持 restored / visible。BACKLOG 目前仍沒有可提升的未阻塞 work block。
> 2026-04-24 Perplexity Comet Browser Direct closeout note：使用者插單的 `WORK-IMPORT-COMET-A` 已完成並 append 到 `CHANGELOG.md`。Comet 現在以 Chromium-family adapter 進入 macOS Browser Direct / backup support truth，current archive 已用本機 Comet profile 完成 preview / import / re-import / revert / restore，主 import batch 已 restored / visible。BACKLOG 目前仍沒有可提升的未阻塞 work block。
> 2026-04-27 UI progress closeout note：使用者插單的 `WORK-UI-PROGRESS-A` 已完成並 append 到 `CHANGELOG.md`。Import / Backup archive-write progress 現在由 shell-owned global task store、Jobs live console、sidebar compact strip 與 topbar notification queue 承接；topbar global search 已移除。BACKLOG 目前仍只有 blocked 的 `WORK-QA-GATE-B`，沒有可提升的未阻塞 work block。
> 2026-04-28 intelligence scope closeout note：使用者插單的 `WORK-INTEL-SCOPE-A` 已完成並 append 到 `CHANGELOG.md`。`/intelligence` 現在有非預設的 all-time preset、cache-aware progressive secondary reveal，Settings / Maintenance hash section nav 也恢復 scroll+focus。deeper all-time cache/preload/invalidation 已記成 design note，不直接升成未阻塞 work block；BACKLOG 目前仍只有 blocked 的 `WORK-QA-GATE-B`。
> 2026-04-28 release blocker follow-up note：`WORK-RELEASE-010-A` 的 Windows scheduler support 讓 `src-tauri/crates/vault-platform/src/scheduler.rs` 升到 `1261` 行，超過 1200 行維護性 review threshold。依 `AGENTS.md` 規則，已新增 blocked follow-up `WORK-SCHED-MAINT-A`，等 release blocker 實機驗收後用專門窗口審查是否拆分；不要在 0.1.0 blocker closeout 中順手重構 scheduler。
> 2026-04-29 scheduled-backup audit follow-up：`WORK-SCHED-REDESIGN-A` 只允許修高優先級 macOS legacy detection bug，修後 `scheduler.rs` 已到 `1411` 行並超過 1400 行硬限制。本次改動是既有巨檔內的最小 bug fix；`WORK-SCHED-MAINT-A` 升級為 high-priority maintainability review，必須在 scheduled-backup design gate / release acceptance 後專門處理，後續不得再往該檔新增業務邏輯。
> 2026-04-29 scheduled-backup state-machine closeout note：使用者直接插單的 `WORK-SCHED-STATE-A` 已把 scheduler maintainability follow-up 和 `/schedule` state-machine redesign 合併收口。`vault-platform::scheduler` 現在是 721 行 facade，平台 owner 分別在 `scheduler/{macos,windows,linux,audit}.rs`；原 blocked `WORK-SCHED-MAINT-A` 已完成並從 BACKLOG 移除。BACKLOG 目前仍只有 blocked 的 `WORK-QA-GATE-B`。
> 2026-05-03 M14 follow-up note：`WORK-M14-A` 已完成 deterministic lexical recall v2 primary path；未經批准的 OpenCC / Unicode normalization 依賴已移除。後續使用者確認 Unicode Consortium / ICU4X 符合 dependency trust gate、官方 OpenCC 可走但必須先證明 CMake/C++/CI toolchain、`strsim` 因 RapidFuzz maintainer provenance 可批准。`WORK-M14-C` 已完成 NFKC / full-width folding；`WORK-M14-D` 已完成 official OpenCC dictionary asset + repo-owned Rust converter path；`WORK-M14-B` 已完成 bounded alias/fuzzy recall，未新增 third-party dependency；`WORK-M14-E` 已完成 project-scoped vcpkg native dependency tooling，future C/C++ product dependencies 不得依賴全局 Homebrew / apt / winget / `pkg-config` 路徑。
> 2026-05-03 history maintainability note：使用者以「繼續開展工作」授權打開 dedicated backend maintainability window。`WORK-HISTORY-MAINT-A` review 已完成並從 BACKLOG 移除；`WORK-HISTORY-MAINT-B` 已完成第一個 behavior-preserving extraction slice，把 history pagination / favicon / export owners 拆到 `archive/history/` 子模組。BACKLOG 目前只剩 blocked work blocks，沒有可提升的未阻塞 current-focus block。
> 2026-05-07 archive test-suite maintainability note：Explorer advanced-search 插單補測時，`src-tauri/crates/vault-core/src/archive/tests.rs` 已達 3272 行。本次只追加 regression coverage，沒有新增業務邏輯；依 `AGENTS.md` 巨檔規則，新增 high-priority follow-up `WORK-ARCHIVE-TEST-MAINT-A`，必須用 dedicated 維護窗口審查拆分測試 owner，後續不要繼續把 archive 新測試集中塞進該檔。
> 2026-05-10 v0.2.0 planning repair note：v0.2.0 發佈範圍正式收斂為 M14 Lexical Recall V2、advanced keyword syntax、Windows unsigned installer / scheduler preview、release/security hardening，以及既有 archive / deterministic Core Intelligence。原先未完成的 v0.2 AI / semantic / MCP / readable-content blocker 已全部移到 v0.3.0；`STATUS.md` 只保留 v0.2 release closeout，不能再把 AI / readable-content 當成 v0.2 ship blocker。

- [!] **WORK-AI-V03-A** — Optional AI Runtime Re-Enablement [!blocked: v0.3 scope decision, real provider acceptance, release-size evidence]
  - 讀先：
    `docs/architecture/decisions/009-default-desktop-optional-intelligence-shipping.md`
    `docs/architecture/tech-stack.md`
    `docs/features/intelligence.md`
    `docs/architecture/data-model.md`
    `docs/plan/program/research-and-decisions.md`
  - 目標：重新評估並實作 v0.3 optional AI：Assistant、provider probes、embedding/index jobs、semantic / hybrid search、MCP / skill artifacts、以及 vector sidecar storage。
  - 契約：不得直接恢復 v0.1.0 移除的 LanceDB dependency；必須先補 runtime truth、provider / App Lock / queue acceptance、packaging / release-size / supply-chain evidence、以及 vector-store sidecar trade-off。UI 必須在可用前保持 `Coming in v0.3` disabled state。
  - 驗收：real desktop provider smoke、semantic search + assistant evidence trace、queue cancel/replay、MCP / skill manual review、release-size audit、`bun run check`，以及 updated ADR / tech-stack / feature docs。

- [!] **WORK-READABLE-CONTENT-V03-A** — Readable Webpage Body Fetch Roadmap [!blocked: privacy model, network policy, failure UX, real-site acceptance]
  - 讀先：
    `docs/features/archive.md`
    `docs/features/intelligence.md`
    `docs/architecture/data-model.md`
    `docs/design/screens-and-nav.md`
    `docs/plan/program/research-and-decisions.md`
  - 目標：把 `readable-content-refetch` 從 v0.2.0 disabled roadmap surface 做成真正可用的 network-backed derived runtime。
  - 契約：不得在 backup/import critical path 內同步 refetch；不得宣稱可抓取登入頁、PDF、JSON、redirect boundary 或 rate-limited 內容；必須有 explicit privacy/network boundary、queue retry/cancel、failure taxonomy、人話 UI、storage accounting、clear/rebuild 行為，以及 real-site acceptance evidence。
  - 驗收：Settings / Jobs / Maintenance disabled-to-enabled flow、network boundary copy、real HTML/PDF/redirect/rate-limit fixtures、blob storage cleanup、`bun run check`，以及 archive/intelligence/data-model docs 回寫。

- [ ] **WORK-V03-COVERAGE-RESIDUAL** — Restore 100% JS coverage gate after orphan sweep
  - 讀先：
    `vitest.config.ts`
    `src/app/shell.tsx`
    `src/app/shell-helpers.ts`
    `src/pages/dashboard/index.tsx`
    `src/pages/explorer/index.tsx`
    `src/components/explorer-paper/*`
  - 目標：把 `bun run check` 的 JS coverage threshold 從本輪 dev-machine bring-up 暫降的 `lines:99 / functions:98 / branches:98 / statements:98` 重新拉回 100/100/100/100。當前 global 為 99.07% lines / 98.83% statements / 98.12% branches / 98.8% functions（2026-05-20）。
  - 殘餘清單（each file < 100% 在 vitest 報表內可定位）：
    - `src/app/shell.tsx` 71.64% — 殘餘 lines 是 useMemo / palette select / lock handler 等內聯 callback；多數需要 augment 既有 `src/app/index-tests/` 整合測試補手勢。
    - `src/pages/explorer/index.tsx` 73.68% — 殘餘 lines 583-643, 747-748 是 paper search panel + paper view + legacy compat 條件分支的內聯 handler。Phase 4 (`WORK-V03-PAPER-REDESIGN-A` legacy retire) 退役 `?layout=legacy` 後，數字會自然回升。
    - `src/pages/dashboard/index.tsx` 82.35% — 殘餘 lines 327-329, 335, 342 是 `formatSpan(t)` 內部 years/months/days 條件分支。可以把 helper 抽到 sibling `dashboard-helpers.ts` 然後直接單測（同 `shell-helpers.ts` pattern）。
    - `src/components/explorer-paper/*` — 多檔 1-2 line gaps (branch threshold 主要拖累)。多數是 props default 分支與細部 prop conditional。可逐檔 augment 既有 test 收口。
    - `src/components/shell/pk-topbar.tsx` line 86 — `typeof navigator === 'undefined'` defensive guard，jsdom 永遠有 navigator。可考慮把此判斷下沉到一個獨立 helper 然後用 module mock 觸發。
    - `src/lib/backend-client/annotations.ts` 97.36% / `src/lib/backend-client/index-references.ts` 96% / `src/lib/i18n/.../date-helpers.ts` 97.56% — 各自 1-2 line gap。
  - 契約：拉回時要長期最優解 — 不允許再用 exclusion 蓋住 active runtime code；只能寫實質測試或把 shim 提取成可單測的 sibling module。`vitest.config.ts` 的 inline comment 已記錄當前殘餘來源，重新 100% 後刪掉那段 comment。
  - 驗收：`bun run coverage:js` 全 metric ≥100%；vitest.config.ts threshold 恢復成 `100/100/100/100`；該 inline comment 移除。`STATUS.md` 與 `CHANGELOG.md` 同步寫回。

- [!] **WORK-SCHEDULE-PAGE-MAINT-A** — Schedule Page Maintainability Review [!blocked: schedule a dedicated frontend maintainability window]
  - 讀先：
    `docs/plan/program/repo-baseline.md`
    `src/pages/schedule/index.tsx`
    `src/pages/schedule/schedule-ui-state.ts`
  - 目標：`src/pages/schedule/index.tsx` 已達 1296 行（> 1200 行 review threshold），因為持續新增狀態面板元件（encrypted-no-keyring warning, Linux manual callout 等）。需審查是否將 state panel components 拆成獨立 owner files（如 `not-installed-state.tsx`, `installed-ok-state.tsx` 等），減少單檔行數並明確職責邊界。
  - 契約：第一階段只產出架構地圖、拆分方案與測試覆蓋確認，不改產品碼；第二階段保持行為等價，不降低 `bun run check` gate。
  - 驗收：`bun run check`，以及 `repo-baseline.md` / BACKLOG 回寫。

- [!] **WORK-ARCHIVE-TEST-MAINT-A** — Archive Rust Test Suite Owner Split [!blocked: schedule a dedicated archive test-suite maintainability window]
  - 讀先：
    `docs/plan/program/repo-baseline.md`
    `docs/plan/program/quality-matrix.md`
    `src-tauri/crates/vault-core/src/archive/tests.rs`
    `src-tauri/crates/vault-core/src/archive/`
  - 目標：對 `vault-core::archive` Rust regression suite 做審查階段，建立測試 owner map、fixture/helper 邊界與拆分方案，再把 `tests.rs` 拆成 pagination / backup pipeline / recall search / import rollback / maintenance 等 focused modules。
  - 契約：第一階段只產出架構地圖、職責清單、拆分方案與 coverage gate 確認，不改產品碼；第二階段保持行為等價，不降低 `coverage:rust` 100% 與 `bun run check` gate。
  - 驗收：`cargo test --manifest-path src-tauri/Cargo.toml -p vault-core archive -- --test-threads=1`、`bun run coverage:rust`、`bun run check`，以及 `repo-baseline.md` / 本 BACKLOG 的 giant-file follow-up 回寫。

- [!] **WORK-QA-GATE-B** — Full Mutation Deep Sweep And Survivor Closeout [!blocked: schedule a dedicated multi-hour mutation hardening window]
  - 讀先：
    `docs/plan/program/quality-matrix.md`
    `TESTING.md`
    `.github/workflows/mutation.yml`
    `package.json`
    `docs/plan/qa-gate-handoff-2026-04-27.md`
  - 目標：在 `WORK-QA-GATE-A` 已恢復 per-commit `bun run check` 後，單獨重啟 full frontend Stryker 與 whole-workspace Rust cargo-mutants，補足 surviving mutants 或寫下 narrow equivalent / inapplicable evidence。
  - 契約：不得把 full mutation 重新塞回每次 commit 的 `bun run check`，除非先有新的成本 benchmark 與使用者明確確認；不得用 broad excludes 偽裝通過；每個 survivor 都要落到補測、產品碼修正、或窄範圍註解/排除。
  - 已知起點（2026-04-27）：`bun run mutation:js:full --dryRunOnly` 約 2m20s，full JS Stryker 21769 mutants，按 44m/32% 實測估算約 2-3 小時；`cargo mutants --manifest-path src-tauri/Cargo.toml --workspace --list --json` 顯示 5869 Rust candidates，且 current copy-sandbox baseline 會因 repo-root Safari reference fixture path 缺失失敗。
  - 執行順序：先確認 `bun run check` 綠；再跑 `bun run mutation:js:full` 並從 top survivor/timeouts files 補測；Rust 先修 cargo-mutants copy-sandbox fixture contract，再用 shards 跑 `bun run mutation:rust:full` 或等價 `cargo mutants --shard n/m`，合併 survivor 清單後逐項處理。
  - 驗收：`bun run check`、`bun run mutation:js:full`、`bun run mutation:rust:full`、更新 `docs/plan/program/quality-matrix.md` / `TESTING.md` / `CHANGELOG.md` 的耗時、survivor closeout 與任何 narrow equivalent evidence。

---

## 依賴關係圖

```
WORK-M0-A ──┐
WORK-M0-B ──┴── WORK-M1-A → WORK-M1-B → WORK-M2-A → WORK-M2-B → WORK-M3-A → WORK-M3-B → WORK-QC-A → WORK-QC-B → WORK-M4-A → WORK-M4-B → WORK-M4-C / WORK-M4-D / WORK-M4-E / WORK-M4-F / WORK-M4-G / WORK-M4-H → WORK-QC-D → WORK-M4-J → WORK-M4-I → WORK-M4-K → WORK-M4-L → WORK-M5-A → WORK-M5-B
                     └──────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────→ WORK-QC-C → WORK-M1-C → WORK-M1-D
WORK-QC-T → WORK-CI-B / WORK-CI-F
WORK-CI-F → WORK-CI-H
WORK-CI-B → WORK-CI-C
WORK-CI-H → WORK-CI-I
WORK-M5-C → WORK-M6-A → WORK-M7-A → WORK-M8-A → WORK-M9-A → WORK-M9-B → WORK-M10-A → WORK-M10-B → WORK-M11-A → WORK-M11-B → WORK-M12-A → WORK-M12-B → WORK-M13-A → WORK-M13-B
WORK-RELEASE-020-A → WORK-AI-V03-A / WORK-READABLE-CONTENT-V03-A
```

---

## 維護規則

1. `STATUS.md` 清空時，從這裡頂部取最多 2 個未被阻塞的 work blocks
2. 新工作一律先放進這裡，除非使用者明確要求立刻調整 `STATUS.md`
3. 如果某個 block 需要再拆，只在 milestone 文檔的 WBS 裡拆，不把 `STATUS.md` / `BACKLOG.md` 再退回原子 task
4. 依賴解除後，直接更新行尾的 `[!blocked: ...]`
