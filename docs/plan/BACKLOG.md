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

- [x] **WORK-V03-PAPER-REMAINING-ROUTES** — Paper restyle for /schedule, /security, /maintenance, /jobs, /integrations, /onboarding, /lock
  - 2026-05-20 closeout: all seven sibling routes use the paper outer wrapper convention (`mx-auto max-w-[1080px] flex flex-col pt-7`) wrapped around existing data shapes. Inner sub-section / step component / state-panel chrome still v0.2 per route — that's a deeper Phase 3-shaped sweep covered by `WORK-SCHEDULE-PAGE-MAINT-A` (schedule), follow-up onboarding step paper sweep, and a future jobs panel paper sweep. Bottom-up commits: 8f8319c integrations / 6a568f2 security+maintenance / c213619 lock / 1852628 onboarding / 6b6599f jobs / e5d0ff6 schedule.

- [x] **WORK-V03-SETTINGS-SECTIONS-PAPER** — Paper restyle inside each Settings sub-section
  - 2026-05-20 closeout: all 11 Settings sub-sections (general / platform / derived-state / ai-providers / app-lock / profile-selection / updater / retention / remote-backup-section / remote-backup-preferences + the already-paper appearance / link-previews from Phase 1.1) now use paper-form-primitives (Field / Toggle / SegmentedControl) + PaperCard. PaperCard accepts an optional `id` prop so `document.getElementById('settings-X')` queries + hash-link scrolling work as before. Commits: 3e8347f (Phase 1.1) / eaf59ba / 4a4f9e9 / acab346 / 690fff7 / 833d3d9.

- [x] **WORK-V03-PAPER-REMAINING-ROUTES-INNER** — Paper restyle sibling-route inner panels (deferred from Phase 2)
  - 2026-05-20 closeout (commits cc33a92 / d6ac788 / eb0172f):
    - **Security panels** (cc33a92) — SecurityStatusPanel / SecurityUnlockPanel /
      SecurityRekeyPanel outer chrome now uses PaperCard + PaperCardHeader +
      PaperCardBody. PaperCardBadge surfaces the "session active / needs unlock"
      and "preview before execute" subtitles. Warning rows (password loss,
      rekey-preview warnings, custom localizedWarnings) all use StatusCallout
      instead of the bespoke `.warning-box` chrome.
    - **Onboarding steps** (d6ac788) — StorageStep / ScheduleStep /
      BrowserDetectionStep / ReadyStep wrap their sub-panels in PaperCard with
      PaperCardBadge subtitles. SecurityStep keeps its bespoke radio-card mode
      picker (Encrypted / Plaintext) since it's a designed editorial pattern.
      Five `.ob-info-box` and `.warning-box` rows across the four steps now
      use StatusCallout primitives (info / warning toned).
    - **Jobs panels** (eb0172f) — overview hero / queue summary / runtime
      summary cards in `jobs/index.tsx`, plus JobPanel + RuntimeJobPanel from
      `jobs/job-panels.tsx`, plus the 4 focus cards + 2 runtime-health
      summary cards from `jobs/runtime-health-section.tsx`, all use
      PaperCard. Updated three `.closest('.panel')` selectors in
      jobs-runtime.test.tsx to `.closest('section, .panel')` so they find
      the new PaperCard `<section>` root.
    - Schedule state panels (not-installed / installed-ok / installed-warn)
      are explicitly **deferred** — they sit inside the 1297-line
      schedule/index.tsx and belong to the larger `WORK-SCHEDULE-PAGE-MAINT-A`
      maintainability sweep that needs to extract the state-panel components
      out of the route before reskinning. Welcome / ready hero blocks stay
      editorial by design.
    - 1654/1654 unit tests pass; `bun run check:base` clean.

- [x] **WORK-V03-E2E-PAPER-MIGRATION** — Migrate Playwright e2e suites off the v0.2 dashboard surface
  - 2026-05-20 closeout (commit 7a1543c, prior commit 5067cfa for the shared
    `completePreviewOnboarding` helper): - `tests/e2e/shell.spec.ts` now passes 5/5. The previously-failing four
    tests fell into two buckets: - **Surface-still-exists, selector drift** — "walks remote backup
    settings and Maintenance PME": swapped `.panel`-class CSS locators
    for `getByTestId('settings-remote')` in both Settings + Maintenance
    scopes since the PaperCard root forwards `navItem.id` as testid. - **Surface concept retired by Phase 4** — three tests asserted v0.2
    ExplorerQueryFiltersPanel chrome (profile combobox in body, regex
    toggle, debounced keyword input, inline alert copy, jsonl export
    button, AdvancedSearchHelp hover card with `site:github.com -pathkeep`
    and `manual OR youtube` example tokens), and "Hybrid coming in v0.3"
    / "Assistant is coming in v0.3" deferred copy that paper now
    replaces with the real composer. Two tests were trimmed to keep
    their still-relevant assertions (audit ledger walk; intelligence +
    assistant testids); the third — `keeps shared profile scope, regex
recall, and export guardrails aligned` — was deleted entirely with
    a doc-comment block explaining where each retired surface moved. - Run command:
    `PLAYWRIGHT_CHROMIUM_EXECUTABLE_PATH=/usr/bin/google-chrome bun run
test:e2e -- tests/e2e/shell.spec.ts` → 5 passed (13.3s).
  - Note：`playwright.config.ts` 已支援 Ubuntu 26.04 via `PLAYWRIGHT_CHROMIUM_EXECUTABLE_PATH` env var（系統 Chrome at `/usr/bin/google-chrome`）— upstream playwright supportedOSes table 還沒對 26.04 提供 chrome-headless-shell binary。

- [x] **WORK-V03-LEGACY-RETIRE** — Retire `?layout=legacy` and the v0.2 panel branches
  - 2026-05-20 closeout: `?layout=legacy` is gone — explorer/index.tsx ditches the
    `paperLayout`/`paperSearchSurface` ternary and mounts PaperExplorerView /
    PaperSearchPanel / PaperDetailPanelMount unconditionally (753 → 593 lines).
    Deleted `src/pages/explorer/{timeline-bar,query-filters-panel,advanced-search-help}.tsx`
    - `src/pages/explorer/panels/results-panel.tsx` and their tests; dropped the
      `.advanced-search-help` block from `explorer.css`. Paper components
      (PaperContactFrame / PaperListRow / PaperDomainStack / PaperDetailPanel /
      PaperSearchResult) now run URL/title text through `sanitizeExplorerDisplayText`
      so the privacy invariant the v0.2 ExplorerResultsPanel guarded is preserved;
      `privacy-redaction.test.tsx` retargeted onto PaperListRow + PaperSearchResult.
      Trimmed legacy-chrome tests across `index.test.tsx`,
      `lock-and-explorer-shell.test.tsx` (explicit-page-jumps test gone — paper has
      no paginator), `explorer-grouped-views.test.tsx`, and rewrote
      `explorer-controls.test.tsx` to keep only the three paper-neutral
      survivors (shell-gate matrix, adjacent-page prefetch, semantic runtime
      actions). 1611/1611 unit tests pass. Commits: f3b2fd2 (paper sanitize) /
      d19cea3 (privacy test retarget) / afb7e94 (legacy retire).

- [x] **WORK-V03-DASHBOARD-REAL-DATA** — Wire dashboard heatmap + threads to real backend
  - 2026-05-20 closeout: dashboard surfaces are now backed by real data on
    every card. - **Active Threads** (commit 03fcb86): calls
    `coreIntelligenceApi.getPathFlows(last-30-days, profileId, 3, 10)` and
    renders the top three recurring 3-step paths with arrow-chained domain
    chips + `{count} occurrences` mono badges. Honest loading / empty /
    error states; row clicks forward `flowId` to the route's `onOpenThread`. - **Year Heatmap** (commit 897b816): replaces the empty placeholder with
    a paper-aesthetic 7×N grid that fetches
    `coreIntelligenceApi.getDiscoveryTrend(last-365-days, profileId, 'day')`
    and buckets visit counts into 0-4 quartile levels. Streak badge surfaces
    the longest consecutive-day run. Day clicks deep-link into Explorer's
    Browse contact-sheet centred on the date. - **Backend design decision**: the BACKLOG entry called for a new
    `get_daily_rollups(range)` Tauri command, but `get_discovery_trend` with
    `granularity='day'` already returns `{ dateKey, totalVisits }` rows from
    `daily_summary_rollups` — the exact shape the heatmap consumes. Adding
    a parallel `get_daily_rollups` command would have introduced a fourth
    daily-aggregation path through `vault-core → vault-worker → tauri →
core-intelligence/api`, all returning the same data. Reusing the existing
    endpoint keeps the surface lean, reuses the primary-overview cache, and
    eliminates a maintenance edge. The new YearHeatmap is layered above the
    same backend boundary the discovery-trend route already uses. - **Helpers extraction**: `formatSpan`, `compactNumber`, `humanizeBytes`,
    `sumStorageBytes`, plus new `dashboardThreadsRange` /
    `dashboardHeatmapRange` / `isoDateOnly` live in
    `src/pages/dashboard/dashboard-helpers.ts` with 15 unit cases — matches
    the `shell-helpers.ts` pattern the BACKLOG called for. - 1648/1648 unit tests pass. New surface tests: 5 (active-threads-card) +
    5 (year-heatmap-card) + 8 (heatmap helpers) + 4 (YearHeatmap render) + 15
    (dashboard-helpers). - No fake / fabricated dashboard data reintroduced — every card preserves
    the Trust & Transparency invariant.

- [ ] **WORK-V03-RUST-COVERAGE-RESIDUAL** — Restore full-scope Rust coverage gate
  - 2026-05-21 進度 (commit f986455 + this session):
    - **`package.json` `check:coverage` 已恢復為 `coverage:rust` (full scope)** —
      codex review flagged that running `coverage:rust:quality` was an
      unauthorised gate weakening vs. docs/plan/program/quality-matrix.md.
      The script now calls the full gate and surfaces residual failures
      honestly. `bun run check` will fail until the residual closes.
    - **annotations.rs**: 4 條 defensive 分支已關閉 (set_notes empty URL,
      replace_tags empty URL, replace_tags >MAX_TAGS_PER_URL, search
      empty-query fallthrough to list_annotations).
    - **og_images.rs:730**: row mapper closure now exercised by changing
      the existing eviction test from `surviving_a == None` (closure
      cold) to a COUNT(*) query_row that fires the closure on success.
    - **og_images_fetch.rs**: 11 → 7 lines closed (utf8 lossy decode
      fallback, absolutize_url base-parse failure, plus the
      `read_capped_bytes<R: Read>` extraction for Io / TooLarge / Ok
      paths — production `read_response_body` now delegates to the
      generic helper which is directly unit-testable).
  - 2026-05-20 進度 (commit a436304):
    - **全部 22 個 uncovered functions 已關閉**: vault-worker::annotations × 5,
      vault-worker::archive_flows og:image entries × 5 (load_history_og_images,
      mark_og_images_shown, og_image_storage_stats, clear_og_image_cache,
      run_og_image_cleanup, refetch_og_images),
      worker_bridge::annotations::\*\_impl × 5,
      worker_bridge::archive::\*\_impl og:image shims × 5,
      vault-core::og_images_fetch::http_status_from_error.
    - **dispatch.rs 154-188** og:image + annotations 命令分配臂 covered by
      extending `dispatch_command_decodes_all_browser_mirror_command_payloads`.
    - **og_images_fetch 設計修正**: 移除 fetch_og_image_for / 測試用 helper 的 ~70
      行重複，提取 `fetch_og_image_for_pipeline(client, page_url, upgrade_image_url: bool)`
      讓 mockito 測試直接走 production pipeline。新增 4 個 mockito tests:
      HTML body > MAX_HTML_BYTES, og:image URL unreachable, image endpoint 404,
      direct fetch_og_image_for against unresolvable host。
    - `node scripts/verify-rust-coverage.mjs coverage/rust.lcov.info full` 現在
      report "Uncovered Rust source functions: (empty)"。
  - 剩餘殘餘（uncovered LINES，functions 已 0 — 2026-05-21 update）：
    - `src-tauri/crates/vault-core/src/archive/history/og_images_fetch.rs:192,193,241,242,245,246,269` —
      7 行: HTML body Io error (192-193), image body TooLarge mid-stream
      (241-242), image body Io error (245-246), Selector::parse failure
      (269 — unreachable since selector strings are static literals).
      192-246 require a partial-body / mid-stream-close fixture mockito
      can't deliver natively.
    - `src-tauri/crates/vault-worker/src/archive_flows.rs:214,224,225,226,411,421,428,431,442,444,447,454,477` —
      13 行: try_refetch_due_og_images backup-flow path (214: fetch_enabled
      false, 224-226: due-urls success path with non-empty list), worker
      pool internals (411 mutex poison, 421 rate-limit sleep, 428 fetch
      success branch, 431 sender disconnect, 442 successful counter
      bump, 444-447 persist error, 454 propagate persist error, 477
      host_throttle mutex poison). Needs either an integration test that
      stands up backup flow + mockito worker, or a refactor that
      decomposes the worker loop into testable units.
  - 契約：長期最優解，不允許再用 exclusion 蓋住 active runtime；只能寫實質 integration test。
    `coverage:rust` 恢復為 full（不再用 quality 限制）後，`package.json` 的 `check:coverage`
    換回 `coverage:rust`，並刪掉這個 backlog entry。
  - 驗收：`node scripts/verify-rust-coverage.mjs coverage/rust.lcov.info full` 報告 0 uncovered lines
    且 0 uncovered functions、`check:coverage` 用回 `coverage:rust`。

- [!] **WORK-V03-CODEX-REVIEW-FOLLOWUP** — Codex review (2026-05-21) closeout for feat/v0.3-redesign-2 [!blocked: Rust + JS coverage residuals still open]
  - 讀先：
    `src/app/shell.tsx`
    `src/lib/paper-preferences.ts`
    `src/components/explorer-paper/paper-contact-sheet.tsx`
    `src/pages/explorer/hooks/use-explorer-og-images.ts`
    `package.json` (check:coverage)
    `vitest.config.ts`
  - 2026-05-21 codex review captured the following findings against the branch:
    - ✅ #1 Blocking: shell.tsx palette query contract — palette was sending
      `{ search, limit, offset }` and reading `response.rows` instead of the
      real `{ q, limit, sort }` / `response.items`. Tests were mocking the
      wrong shape, hiding the bug. Fixed in commit a187ee0: shell now calls
      `backend.queryHistory` with the typed contract and maps HistoryEntry
      items. shell.test.tsx mocks the typed backend.
    - ✅ #5 Medium: i18n raw English copy — char/chars counter, "Remove tag",
      "Calendar" dialog aria, "now" / "first" year-rail captions, dashboard
      morning/afternoon/evening greetings are all in the three-language
      catalog now. 2803 keys × 3 locales, parity 100%.
    - ✅ #6 Medium: theme dual state — applyPaperPreferences now dispatches
      `pathkeep.paperPreferencesChanged` and both shell.tsx + Settings
      Appearance subscribe. Theme toggle button in topbar updates Settings
      radio without re-mount. Visually verified with playwright (light →
      dark via topbar → Settings radio shifts to "Darkroom · dark").
    - ✅ #2 Blocking: paper Explorer pagination — PaperContactSheet now
      renders an optional footer with Newer/Older buttons + page-size
      selector + "Page X of Y · N rows" summary. PaperExplorerView accepts
      a `pagination` descriptor; Explorer route threads
      handlePreviousHistoryPage / handleNextHistoryPage /
      setHistoryPageSize from useExplorerUrlState. 1440 M-row goal now
      addressable via cursor/pagination. Visually verified.
    - ✅ #3 High: og:image fetch trigger — use-explorer-og-images now
      enqueues `triggerOgImageRefetch(batch)` for non-`ok` rows after
      loadHistoryOgImages resolves. Bounded at 20 per render, deduped
      per cache epoch, .catch swallows rate-limit / fetch-disabled
      rejections.
    - 🟡 #4 Blocking/Process: 100% coverage gate restoration —
      `check:coverage` script is restored to `coverage:rust` (full), not
      the quality slice. The full gate still fails until the
      WORK-V03-RUST-COVERAGE-RESIDUAL line residual closes (7 lines in
      og_images_fetch defensive arms + 13 lines in archive_flows worker
      pool). vitest threshold remains at 99/99/98/99 pending
      WORK-V03-COVERAGE-RESIDUAL; raising it to 100/100/100/100 would
      also require closing the JS jsdom-defensive guards.
  - 契約：codex 把 #4 標為 merge blocker。在 line residual 關閉或得到 user 明確
    授權的 deviation 之前，這個 follow-up block 維持 blocked。不要私自降回
    quality slice。

- [ ] **WORK-V03-COVERAGE-RESIDUAL** — Restore 100% JS coverage gate after orphan sweep
  - 讀先：
    `vitest.config.ts`
    `src/app/shell.tsx`
    `src/app/shell-helpers.ts`
    `src/pages/dashboard/index.tsx`
    `src/components/explorer-paper/*`
  - 目標：把 `bun run check` 的 JS coverage threshold 從目前的 `lines:99 / functions:98 / branches:98 / statements:99` 重新拉回 100/100/100/100。
  - 2026-05-20 進度（commit 63ddf37 / 5b3720c / b4a2872 / b7c58ab）：
    - **dashboard/index.tsx** 從 82.35% lines → 98.42% lines。新增 dashboard-helpers.ts (15 unit cases) + 3 個 route-shell callback tests（On This Day open-entry、jumpToDate target-label button、insights badge、All threads badge）。剩餘殘餘 ~2 lines 是 useMemoGreeting 的 evening (hour ≥ 18) 分支與 footer，需要 Date mock 才能測。
    - **paper-preferences.ts** 從 88.88% statements / 72.22% branches → 100/83。新增 9 unit cases；剩下 branch 缺口是 `typeof window === 'undefined'` SSR guards（jsdom 永遠 truthy，無法觸發）。
    - **shell.tsx** 從 70.88% → 81.01% statements。新增一個 palette debounce test 觸發 invokeCommand('query_history')；handlePaletteSelect (lines 191-196)、handleManageSources (line 156)、response 沒 rows 的 fallback (line 173) 仍未覆蓋。後續要在 popover 開啟後點 manage-sources、在 palette UI 上選結果。
    - **dashboard-helpers.ts** + **dashboard cards** 100% lines；殘餘是 branch-only 細節（兩個 in-flight cancelled 路徑、language === 'en' 三元 fallback）。
    - threshold 已從 99/98/98/98 提升到 99/98/98/99 鎖定 statement 改善；global 為 99.28 lines / 98.96 functions / 98.12 branches / 99.02 statements。
  - 殘餘清單（each file < 100% 在 vitest 報表內可定位，更新自當前 coverage:js 報告）：
    - `src/app/shell.tsx` 81.01% — handlePaletteSelect (lines 191-196) + handleManageSources (line 156) + response no-rows fallback (line 173)。需要 popover 開啟 + manage-sources click + palette CommandItem select 的整合測試。
    - `src/pages/dashboard/index.tsx` 86.2% statements / 67.74% branches — 殘餘是 `useMemoGreeting` evening hour ≥ 18 分支 + 三個 FooterEpigraph / DashboardYearHeatmapCard 內聯 arrows。需要 Date mock + 直接驅動三張卡片 callbacks。
    - `src/components/explorer-paper/*` — 多檔 1-2 line gaps (branch threshold 主要拖累)：paper-contact-sheet (89.83/84.9), paper-detail-panel (96.59/93.25), paper-domain-stack (100/81.48), paper-intelligence-view (100/62.5), paper-search-result (90/84), paper-list-row (100/93.75), paper-contact-frame (100/95.83), paper-import-method-card (100/92.3), paper-year-rail (100/94.11), paper-audit-view (100/77.77), paper-assistant-view (100/80)。多數是 props default 分支與細部 prop conditional。
    - `src/components/shell/pk-topbar.tsx` line 86 — `typeof navigator === 'undefined'` defensive guard，jsdom 永遠有 navigator。可考慮把此判斷下沉到一個獨立 helper 然後用 module mock 觸發。
    - `src/components/shell/pk-status-bar.tsx` 93.75% lines — small uncovered chunk around line 188。
    - `src/components/shell/pk-search-palette.tsx` 98.21% statements / 85.71% branches — palette `paletteHintFullSearch` Cmd+Enter shortcut + multi-result render path。
    - `src/components/explorer-paper/paper-calendar-popover.tsx` 100% statements / 87.5% branches — month-jump callbacks。
    - `src/lib/backend-client/annotations.ts` 97.36% / `src/lib/i18n/.../date-helpers.ts` 97.56% — 各自 1-2 line gap。
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
