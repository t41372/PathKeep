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
> 2026-05-25 BROWSE-VIRT closeout：`WORK-FEEDBACK-0525-BROWSE-VIRT` 已完成並 append 到 `CHANGELOG.md`。viewport-driven day recycling (IntersectionObserver) + directional prefetch (RAF-sampled scroll direction with 4-frame hysteresis) + MAX_ACCUMULATED_PAGES 100 → 500 (originally 1 000；review §3 trade-off：incremental aggregator 未做，先把 cap 降到 500 避免 O(N) aggregation 在 ~25k entries 時毛刺) 都已 ship；sticky day header / cards-grid / sessions / a11y 契約都保留。residual follow-ups: LRU page eviction、real Chrome devtools FPS trace、Playwright e2e on 14M-row fixture、`docs/features/explorer-browse.md` 寫一份正式 feature spec、incremental aggregator（升回更大 cap 的前置條件）。
> 2026-05-25 import test harness planning note：使用者反映實際導入瀏覽記錄時觀察到疑似 duplication，並要求專門的 ingest robustness 測試基礎建設。經 ingest 代碼 audit（見 `docs/plan/program/import-dedup-audit.md`）確認：跨瀏覽器「視覺重複」是 per-source-profile 設計契約（不是 bug），但發現 6 個真實 bug：B1 URL upsert 倒退、B2 Firefox/Safari long-tail revisit 漏抓、B3 Takeout source_visit_id 綁路徑、B4 Takeout × local Chrome 必然雙倍、B5 takeout `stable_key_i64` 規模化碰撞、B6 Takeout 時間單位歧義。新增 `WORK-IMPORT-TEST-HARNESS-A` 作為**第一個 unblocked block**，內含 scaffold + Priority 1 scenario library；後續的 cross-source view-layer aggregation、bug fixes 都會依託這個 harness 寫 failing test。完整 scenario library 與驗收條件見 `docs/plan/program/import-test-harness-spec.md`。
> 2026-05-27 parser mutation hardening note：行為安全網補測把 `src-tauri/crates/browser-history-parser/src/chromium/mod.rs` 推到 1544 行。這次只補測試、不新增業務邏輯；但該檔已超過 1400 行硬限制，後續不得再往此檔新增 parser 業務邏輯，需用 dedicated 維護窗口做 owner split 審查。

> 2026-06-14 review-pipeline closeout：`docs/plan/program/review-pipeline.md` 全流程跑完，confirmed findings 已全部修復並各自過獨立 review（見 CHANGELOG `WORK-REVIEW-0614-FULL-PIPELINE-A`、報告 `docs/review/2026-06-14/`）。剩下的是 verifier 標為 non-blocking 的 polish，匯整成下方 `WORK-REVIEW-0614-FOLLOWUPS-A`（未阻塞，但非當前 focus）。被駁回 / 降級 / trade-off 的 findings 不重開。

- [ ] **WORK-REVIEW-0614-FOLLOWUPS-A** — 2026-06-14 review non-blocking follow-ups
  - 讀先：
    `docs/review/2026-06-14/phase-4/final-report.md`
    `docs/review/2026-06-14/phase-3/all-verdicts.json`
    `docs/plan/program/import-dedup-audit.md`
  - 目標：清掉 2026-06-14 review 確認修復後，獨立 reviewer 標記為 non-blocking 的尾巴項目。皆非阻塞、非回歸。
  - 範圍（逐項可獨立做）：
    - **per-URL visit summary 後端讀取**（X-VISITSUMMARY 的完整版）：新增 backend command 回傳單一 URL 的 first/last visit、visit_count、typed_count，串到 detail panel，恢復 recall.md 規格的 first/last/sparkline/title-versions。目前 detail panel 只誠實顯示單次 visit。
    - **F-LEGACY-CSS 殘留**：`src/pages/security/panels.tsx` 與 audit detail panel 仍帶 v0.2 `.panel` chrome；`PaperCardBody` 內仍用 legacy grid util（`intelligence-stack` / `intelligence-job-list`）。
    - **F-ROUTER 殘留**：`/onboarding` route subtree 仍無 ErrorBoundary（pre-existing，超出原 finding 範圍）。
    - **R-VISITIDS**：`totalVisitCount` 已持久化但 explainability panel 的 chip 仍從 capped array 推導；改讀 exact total。
    - **F-TOKENS micro**：light-mode `--ink-faint` 在 `--bg-page` / `--bg-hover` 上仍略低於 normal-text AA（已過 large-text 3:1，且遠優於修前）；如要再壓需小心 muted>faint 層級。`StatusCallout` CSS 仍用 `--warning-dim`/`--error-dim` 別名（resolve 正確，屬待清理 migration debt）。
    - **F-DASHBOARD micro**：`dashboardWeekRange(new Date())` / `isoWeek` 每 render 重算多次，可 memo；month preset 是 30-day query window 的近似。
    - **R-LOADVISITS micro**：scoped read 仍因 `?1 IS NULL OR ...` predicate 走 full table SCAN（CPU/IO，記憶體已 bounded）；可考慮拆兩條 query 走 index。
  - 契約：每項獨立、行為保留；動到 user-visible copy 要補三語；維持 100% coverage gate。
  - 驗收：所選項目修復 + 測試；`bun run check` 綠（或於 Linux CI gate 綠）。

- [ ] **WORK-MAINT-PARSER-CHROMIUM-A** — Split oversized Chromium parser owner
  - 讀先：
    `src-tauri/crates/browser-history-parser/src/chromium/mod.rs`
    `src-tauri/crates/browser-history-parser/src/types.rs`
    `src-tauri/crates/browser-history-parser/src/observation.rs`
    `docs/plan/program/repo-baseline.md`
  - 目標：先做審查階段，產出 Chromium parser 的職責地圖、streaming row owners、source-evidence/capability owners、fixture/test owner 邊界；確認覆蓋與 mutation evidence 後，再執行 behavior-preserving split。
  - 契約：maintainability-only；不改 Chromium parse/stream/capability/source-evidence semantics；不在審查階段改產品碼；拆分前後 `cargo test -p browser-history-parser` 與 Rust coverage/mutation parser slice 必須保持綠。
  - 驗收：`chromium/mod.rs` 降回合理 facade 尺寸，測試拆到 focused owner module 或 test helper；無 public parser contract 變動；CHANGELOG 記錄拆分邊界與驗證輸出。

- [x] **WORK-IMPORT-TEST-HARNESS-A** — Browser History Import Test Harness Foundation
  - 2026-05-25 closeout: audit + fixture crate + 12 e2e scenarios (9 contract, 3 `#[should_panic]` bug repros) + TODO for sub-ms Chrome collision. B5 scale test deferred to WORK-IMPORT-SCALE-TEST-A. See CHANGELOG for full details.
  - 讀先：
    `docs/plan/program/import-dedup-audit.md`
    `docs/plan/program/import-test-harness-spec.md`
    `docs/architecture/browser-support-and-adapter-playbook.md`
    `src-tauri/crates/vault-core/src/migrations/001_initial.sql`
    `src-tauri/crates/vault-core/src/migrations/002_archive_runtime_foundation.sql`
    `src-tauri/crates/vault-core/src/archive/ingest/writes.rs`
    `src-tauri/crates/vault-core/src/archive/ingest/mod.rs`
    `src-tauri/crates/vault-core/src/archive/ingest/parser.rs`
    `src-tauri/crates/vault-core/src/archive/mod.rs`
    `src-tauri/crates/browser-history-parser/src/chromium/mod.rs`
    `src-tauri/crates/browser-history-parser/src/firefox/mod.rs`
    `src-tauri/crates/browser-history-parser/src/safari/mod.rs`
    `src-tauri/crates/browser-history-parser/src/takeout/browser_history.rs`
    `src-tauri/crates/browser-history-parser/src/takeout/source.rs`
  - 目標：建立 `src-tauri/crates/browser-history-fixtures` crate，內含：(1) 真實 schema 的 Chromium History / Firefox places.sqlite / Safari History.db / Takeout JSON/JSONL/zip fixture generator；(2) Scenario DSL 與 deterministic seed；(3) 跑通 ingest pipeline 後讀回 canonical archive 的 assertion API；(4) Priority 1 scenarios（C1/C2/C3/T1/T2/X1）與 fixture round-trip self-validation；(5) 為 audit 列的 6 個 bug 各寫一個 failing `#[should_panic]` 測試並在 spec doc 加上 traceability。
  - 契約：
    - **絕對不讀取使用者真實瀏覽資料**。fixture 全部由 deterministic seed 程序化生成；URL / title 只用 checked-in public-domain corpus（Wikipedia article titles、`example.com` / `synthetic.test` 偽 hosts）。
    - 新 crate 進 Cargo workspace、納入 `bun run check`，所有現有 100% JS/Rust coverage gate 不放鬆。
    - 不修任何 product code bug —— harness 只負責 expose；fixes 由獨立 follow-up block 處理，merge 時把對應 scenario 從 `#[should_panic]` flip 成 `#[test]`。
    - 不新增 third-party dependency 除非經審核（目前計畫使用 `rusqlite` / `serde_json` / `chrono` / `rand` / `rand_chacha` / `tempfile` / `zip`，全部已在 workspace）。
    - 不在這個 block 內 cover view-layer cross-browser aggregation（另立 block）。
    - 生成 SQLite 必須通過真實 PathKeep parser 的 round-trip 測試（self-validation gate），否則 scenario 是無效保證。
    - 不在 STATUS.md 同時運行 paper redesign + harness 兩條軌道前需使用者授權（per AGENTS.md「計劃外大工作 → 進 BACKLOG.md，不直接做」）。
  - 驗收：
    - `browser-history-fixtures` crate builds clean、在 `bun run check` 通過。
    - `tests/fixture_roundtrip.rs` 全綠 —— 每個 generator output 都被真實 parser 正確讀回。
    - Priority 1 scenarios（C1/C2/C3/T1/T2/X1）實作完成，contract scenarios pass、bug scenarios `#[should_panic]` with doc comment 連到 audit bug ID。
    - `docs/plan/program/import-dedup-audit.md` 新增「Bugs with failing tests」章節，列出每個 bug 對應的 scenario function。
    - CHANGELOG 紀錄哪些 audit bugs 已有 failing tests、哪些尚待 follow-up。
    - 三語 i18n 不適用（test infra 內部 ID 用 ASCII）。

- [x] **WORK-IMPORT-TEST-REMAINING-A** — Import Test Harness Remaining Audit Items + Maintainability
  - 2026-05-25 closeout: all non-blocked audit items complete. Edge cases (E1-E6, C_SUB_MS, Empty DB×3, R1), cross-family baselines (F_C2, S_C2), Takeout coverage (ptoken, visitedAt, missing-time), and maintainability refactor (1274→641 lines via Takeout extraction + F2/S2 move) all shipped. R2/R3 and B5 remain blocked on infrastructure not yet built.
  - 讀先：
    `docs/plan/program/import-dedup-audit.md`
    `docs/plan/program/import-test-harness-spec.md`
  - 剩餘 blocked items now tracked individually：(1) R2/R3 crash rollback/batch revert — needs transaction-abort test infra；(2) B5 scale collision test — see WORK-IMPORT-SCALE-TEST-A。
  - 契約：不修 product code；maintainability refactor 不改 behavior。

- [!] **WORK-IMPORT-SCALE-TEST-A** — B5 Takeout `stable_key_i64` Collision At Scale [!blocked: needs million-record fixture infrastructure + benchmark tooling]
  - 讀先：
    `docs/plan/program/import-dedup-audit.md` (§B5)
    `docs/plan/program/import-test-harness-spec.md` (T4 scenario)
    `src-tauri/crates/browser-history-parser/src/takeout/browser_history.rs` (`stable_key_i64`)
    `src-tauri/crates/browser-history-fixtures/src/takeout/mod.rs`
  - 目標：驗證 B5 hash collision probability — 用 1M+ record Takeout fixture 觀察 `stable_key_i64` 的實際碰撞率，確認是否在 14.4M design ceiling 下需要更換 hash function。
  - 契約：不修 product code；只產出 benchmark + collision statistics。

- [x] **WORK-IMPORT-FIXTURE-SIDECARS-A** — Chromium Sidecar Tables Fixture Extension + End-to-End Scenarios
  - 2026-05-26 closeout: `ChromiumHistoryFixture` now writes parser-faithful
    `downloads` and `keyword_search_terms` tables plus a companion `Favicons`
    database (`favicons` / `favicon_bitmaps` / `icon_mapping`). Added
    fixture self-validation for downloads/search/favicon round-trip through the
    production parser, plus T6-T9 vault-core end-to-end scenarios for archive
    downloads, search terms, favicon blob dedup, and icon mapping. No product
    bugs confirmed.
  - 讀先：
    `docs/plan/program/import-dedup-audit.md` (§3 — "Downloads / search_terms / favicons all supported")
    `docs/plan/program/import-test-harness-spec.md`
    `src-tauri/crates/browser-history-fixtures/src/chromium/mod.rs` (current writer: urls + visits only)
    `src-tauri/crates/browser-history-parser/src/chromium/mod.rs` (lines 115+ — DOWNLOADS_SQL / SEARCH_TERMS_SQL / FAVICONS_SQL)
    `src-tauri/crates/vault-core/src/archive/ingest/writes.rs` (`insert_download`, `insert_search_term`, `insert_favicon`)
    `src-tauri/crates/vault-core/src/migrations/002_archive_runtime_foundation.sql` (downloads / keyword_search_terms / favicons / favicon_bitmaps schemas)
  - 觀察（2026-05-25）：現在的 `ChromiumHistoryFixture` 只能寫 `urls` + `visits` 兩張表。實際 Chrome `History` DB 還有 `downloads`, `keyword_search_terms`, `favicons`/`favicon_bitmaps`/`icon_mapping` 等表，parser 都有對應 SELECT 與 archive 寫入，但**端到端 scenario level 完全沒測過** —— CHANGELOG 早有記錄。實際使用者真的有下載歷史 / 搜尋詞 / favicon，這個 gap 真實存在。
  - 目標：(1) 在 `browser-history-fixtures/src/chromium/mod.rs` 加 `ChromiumDownloadRow` / `ChromiumKeywordSearchTermRow` / `ChromiumFaviconRow` + `ChromiumIconMappingRow` 三個（或四個）資料結構與對應的 `add_download` / `add_search_term` / `add_favicon` 方法；(2) 在 `SCHEMA_SQL` 補 real Chromium downloads / keyword_search_terms / favicons / favicon_bitmaps / icon_mapping 表結構（schema 要對齊真實 Chrome 145+ 版本，columns 取自 parser 的 SELECT 列表）；(3) 寫四個新 scenario：T6 `chromium_downloads_round_trip_to_archive_downloads_table`、T7 `chromium_keyword_search_terms_land_with_term_text_preserved`、T8 `chromium_favicons_link_to_canonical_url_rows_with_blob_dedup`、T9 `chromium_icon_mapping_resolves_url_to_favicon`；(4) 為新 fixture 表加 round-trip self-validation 測試到 `tests/fixture_roundtrip.rs`。
  - 契約：
    - 不修 product code；只擴展 fixture + 加 scenario。
    - **絕對不讀取使用者真實瀏覽 / 下載資料**。所有 fixture rows 由 deterministic seed 程序化生成，URL / filename / search term 只用 `example.com` / `synthetic.test` / public-domain corpus。
    - 三個（或四個）新 fixture data structures 不超過 800 行（含 schema、helper、unit test）。
    - 100% Rust coverage 維持；新 scenario 必須在 `cargo test -p vault-core` 與 `bun run check` 全綠。
    - Favicon blob bytes 使用 4-byte synthetic PNG header（`\x89PNG\r\n\x1a\n` + 1 byte filler），不從真實圖檔取材。
  - 驗收：
    - `ChromiumHistoryFixture` 至少支援 4 個新 add\_\* 方法 + 對應 SCHEMA_SQL 擴展。
    - 4 個新 scenario 全綠，分別 assert downloads / search_terms / favicons / icon_mapping 從 fixture 進 archive 後 column values 1:1 對應。
    - `tests/fixture_roundtrip.rs` 新增 self-validation 測試，確認 fixture writer 寫出的 SQLite DB 可被真實 parser 讀回。
    - audit doc §6 contract table 新增 T6-T9 rows + 對應 §3 Chromium downloads / search_terms / favicons 註腳更新。
    - CHANGELOG 紀錄哪些 sidecar tables 現在有 end-to-end scenario coverage。

- [x] **WORK-IMPORT-TEST-MINOR-A** — Minor Data-Integrity Contract Pins
  - 2026-05-26 closeout: Added E10-E14 focused edge-case scenarios for
    Chromium `visit_count` zero/nonzero preservation, dangling `from_visit`
    preservation, `visit_duration` value preservation, Safari synthesized
    source-evidence persistence, and Firefox `visit_type` passthrough. No
    product bugs confirmed; the duration test pins the current archive column
    name (`visit_duration_ms`) and verbatim value semantics.
  - 讀先：
    `docs/plan/program/import-dedup-audit.md`
    `src-tauri/crates/vault-core/src/archive/ingest/dedup_scenarios_edge_cases.rs` (where these will land)
    `src-tauri/crates/browser-history-parser/src/safari/mod.rs` (lines 585-605 — synthesized / load_successful / http_non_get context evidence)
  - 觀察（2026-05-25）：完成 35 個 dedup scenarios 之後剩下這些 narrow 的 contract pins，每個值都不大但加起來能補完 column-level 行為的測試覆蓋：
    1. **visit_count = 0 / visit_count = N round-trip** — Chrome 對 typed-but-never-visited URL 會寫 `visit_count = 0`，parser 應該照搬不做奇怪轉換。
    2. **`from_visit` referential integrity** — 如果 `from_visit` 指向不存在的 visit id（user 手動編輯 DB 或 parent visit 被刪），archive 怎麼存？current behavior 是 dangling reference 還是 0？
    3. **`visit_duration_micros` round-trip** — 顯式 assert duration 從 fixture 傳到 archive 的 `visit_duration_us` column 沒丟。
    4. **Safari `synthesized` context evidence** — audit §3 提到 Safari 的 synthesized flag 會 inflate visit_count，parser 把它記成 `safari.synthesized` ContextEvidence 但沒測過 round-trip。
    5. **Firefox `visit_type` enum mapping** — Firefox 的 visit_type 編碼跟 Chromium transition 不同，應該照搬到 archive 而不被 normalize。
  - 目標：每個 item 加一個 focused test 到 `dedup_scenarios_edge_cases.rs`（或在 baselines / takeout 各自模組裡），命名遵循 E-series（E10 / E11 / E12 / E13 / E14）。
  - 契約：不修 product code；每個 test < 80 lines；不擴展 fixture API（用現有 fields）；audit doc §6 同步更新。
  - 驗收：5 個新 test 全綠；`cargo test -p vault-core` + `bun run check`；audit doc §6 contract table 新增 5 rows；CHANGELOG 紀錄這批 pins。

- [x] **WORK-IMPORT-TEST-PARSER-ORDERING-A** — Visit-Before-URL Parser Ordering Contract
  - 2026-05-26 closeout: Added
    `chunk_consumer_skips_visits_when_url_batch_has_not_populated_the_map`,
    a direct `ArchiveChunkConsumer::visits` unit test that pins current
    visit-before-url behavior as silent skip: no canonical visit row, no
    `new_visits`, one skipped progress record, and no visit watermark marker
    advancement. No product bugs confirmed.
  - 讀先：
    `docs/plan/program/import-dedup-audit.md` (§4 — "Visit→URL ordering dependency" + §5.3)
    `src-tauri/crates/vault-core/src/archive/ingest/mod.rs` (lines 155-158 — `ArchiveChunkConsumer::visits` silently drops visit if url_id_map miss)
    `src-tauri/crates/vault-core/src/archive/ingest/chunk_consumer.rs` (if separate file)
  - 觀察：audit §4 明確指出 parser 必須先 emit `urls()` 再 emit `visits()`；任何後續 refactor 改動 batching order 都會造成 silent data loss。但這個契約完全在 parser 層，不容易從 e2e scenario 測 —— 需要寫一個 mock `ChunkConsumer` 或直接 call `ArchiveChunkConsumer::visits` 在沒有對應 url_id_map entry 時，verify 行為（silent skip vs error）。
  - 目標：在 vault-core 內加一個 unit test (不是 scenario) 直接驅動 `ArchiveChunkConsumer::visits` with empty url_id_map，assert visits are silently skipped (current behavior), 然後在 doc comment 連到 audit §4 警告任何未來 refactor 都要保留這個契約或顯式 fail-fast。
  - 契約：不修 product code；測試只 pin 現有行為（silent skip），不主張 fail-fast 行為。如果 reviewer 認為應該改成 fail-fast，那是另一個 design conversation。
  - 驗收：1 個 unit test 在 `dedup_scenarios_edge_cases.rs` 或 `writes.rs` 的 #[cfg(test)] module 全綠；audit doc §4 加 cross-reference 連到 test；CHANGELOG 紀錄這個 narrow contract pin。

- [x] **WORK-IMPORT-TEST-CONCURRENCY-A** — Multi-Profile Concurrent Ingest Safety
  - 2026-05-26 closeout: Added
    `same_profile_writer_waits_for_committed_watermark` in
    `archive::ingest::concurrency_tests`. The audit found no app-level ingest
    queue around backup/import commands; current same-profile serialization
    comes from the real SQLite writer lock acquired before watermark reads.
    The test holds one writer transaction open, proves a second same-profile
    writer cannot read the watermark before commit, then asserts it observes
    the committed cursor. No product bugs confirmed.
  - 讀先：
    `docs/plan/program/import-dedup-audit.md` (§4 — "Watermark race")
    `src-tauri/crates/vault-core/src/archive/ingest/mod.rs` (lines 411-437 — transaction + watermark save)
    `src-tauri/crates/vault-core/src/archive/mod.rs`
    `src-tauri/crates/vault-worker/src/archive_flows.rs`
  - 觀察：audit §4 指出 single-DB transaction 已經阻止 same-profile concurrent ingest，但 in-app queue serialization 與 backup vs Browser Direct cross-flow 沒測過。實際 production scenario：使用者點 manual backup 同時 schedule 觸發 auto backup，兩個 flow 都會試著 ingest 同一個 source_profile，race condition 可能讓 watermark 被踩或讓 same profile 同時被兩個 transaction 處理。
  - 目標：(1) Reading 現有 worker queue / archive flow code，確認 same-profile 的 serial guarantee 從哪裡來；(2) 寫一個 integration test 模擬兩個 import flow 對同一 profile，assert second flow 等到 first flow 完成才開始；(3) 如果發現 gap，建立 bug entry，但**不在這個 block 修**。
  - 契約：第一階段 audit-only（read + analysis），第二階段才寫測試；不修 product code；發現 bug 寫 BACKLOG entry 不直接 fix。
  - 驗收：audit doc 新增 §4.1 "concurrent ingest safety analysis" 子章節；至少 1 個 integration test 證明 same-profile concurrent flow 是 serialized；任何發現的真實 race condition 寫獨立 BACKLOG block。

- [x] **WORK-MAINT-IMPORT-EDGE-CASES-SPLIT-A** — Split oversized import edge-case scenario module
  - 2026-05-26 closeout: Split the 1250-line
    `dedup_scenarios_edge_cases.rs` into a 311-line shared helper harness plus
    five focused child modules: `chromium_contracts`, `empty_and_resilience`,
    `time_and_nullable`, `unicode_and_flags`, and `minor_data_integrity`. Test
    names and assertions were preserved; audit §4 / §6 links now point at the
    new owner files. No ingest behavior changed.
  - 2026-05-26 note: `src-tauri/crates/vault-core/src/archive/ingest/dedup_scenarios_edge_cases.rs`
    reached 1250 lines while adding E10-E14. Per AGENTS.md >1200-line
    rule, review the module boundary and split E-series / resilience / empty-DB
    scenario groups before adding more broad coverage to this file.
  - 讀先：
    `docs/plan/program/import-dedup-audit.md`
    `src-tauri/crates/vault-core/src/archive/ingest/dedup_scenarios_edge_cases.rs`
    `src-tauri/crates/vault-core/src/archive/ingest/mod.rs`
  - 契約：maintainability-only；不改 ingest behavior；先保留 existing test names unless
    Rust module visibility forces a local rename with doc traceability.
  - 驗收：edge-case tests split into focused owner modules, vault-core lib tests
    and `bun run check` stay green, and audit §6 links still resolve.

- [x] **WORK-MAINT-IMPORT-INGEST-FACADE-SPLIT-A** — Review oversized ingest orchestrator module
  - 2026-05-26 closeout: Reviewed the ingest owner map and split the embedded
    low-level regression suite into `archive/ingest/core_tests.rs`. The
    production facade dropped from 1249 to 637 lines while keeping
    `ArchiveChunkConsumer`, stream dispatch, watermark advancement, and
    source-evidence plan persistence adjacent in `mod.rs` to avoid widening
    hot-path visibility. `core_tests` owns the 7 low-level orchestration tests;
    browser scenario matrices remain in their existing focused modules.
  - 2026-05-26 note: `src-tauri/crates/vault-core/src/archive/ingest/mod.rs`
    reached 1247 lines while adding the parser-ordering contract pin. Per
    AGENTS.md >1200-line rule, review whether `ArchiveChunkConsumer`,
    source-evidence persistence, and test-only helpers should move behind
    focused owner modules before more ingest behavior is added here.
  - 讀先：
    `docs/plan/program/import-dedup-audit.md`
    `src-tauri/crates/vault-core/src/archive/ingest/mod.rs`
    `src-tauri/crates/vault-core/src/archive/ingest/writes.rs`
    `src-tauri/crates/vault-core/src/archive/ingest/parser.rs`
  - 契約：maintainability-only；不改 ingest semantics, watermark behavior,
    or public backup/import contracts.
  - 驗收：produce a responsibility map and, if still justified, split
    orchestrator/test owners so `cargo test -p vault-core --lib` and
    `bun run check` stay green.

- [~] **WORK-AI-V03-A** — Optional AI Runtime Re-Enablement [UNBLOCKED 2026-06-20 → 已重做為 AI-redesign-2026 並提升到 `STATUS.md`]
  - 2026-06-20 unblock note：v0.3 scope decision 已鎖定（見 `docs/plan/program/ai-redesign-2026/02-architecture-decisions.md` D1-D8），real provider acceptance 由本機 LM Studio（`http://localhost:1234/v1`，gemma-4-26b + qwen3-embedding-0.6b）提供。此 block 已被乾淨重做的 AI-redesign-2026 取代，拆成 `STATUS.md` 的 `WORK-AI-0-FOUNDATIONS` … 等執行序（見 `04-current-state-and-execution.md`）。原 LanceDB-禁用契約仍有效（向量層改用 `VectorIndex` trait + Turbovec/flat-scan，從不恢復 LanceDB）。
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
      cold) to a COUNT(\*) query_row that fires the closure on success.
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
  - 目標：把 `bun run check` 的 JS coverage threshold 從目前的 `lines:99 / functions:99 / branches:98 / statements:99` 重新拉回 100/100/100/100。
  - **2026-05-26 Codex review finding C5**：vitest.config.ts 目前的 99/99/98/99 是這個 work block 的 in-progress state，但 `docs/plan/program/quality-matrix.md`（Accepted）仍寫死 100/100/100/100。Codex 標這是 merge contract violation。使用者 2026-05-26 明確指示：本 block 才是正解，不要在 review fix bundle 裡偷偷降 quality-matrix，也不要硬拉到 100；把這保留為 release-style 大窗口任務。
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

- [!] **WORK-PERF-RUNTIME-LITE-A** — Intelligence Runtime Lite Read For Shell-Wide Sidebar Badges [!blocked: needs sidebar/intelligence contract design discussion]
  - 讀先：
    `docs/architecture/data-model.md`
    `docs/plan/program/quality-matrix.md`
    `src/app/shell-runtime-status.ts`
    `src/components/sidebar/background-status.tsx`
    `src/pages/intelligence/runtime-digest.tsx`
    `src-tauri/crates/vault-core/src/intelligence_runtime/`
    `src-tauri/crates/vault-core/src/models/core_intelligence/shared.rs`
  - 觀察（2026-05-21）：`load_intelligence_runtime` 在已導入 264k 列 archive 上每次 shell mount 耗時約 2.9s（dev IPC，加密 SQLite）。原因是回應內嵌每個 recent job 的 `dirtyDateKeys`，單 job 可達 ~400 個 date string；shell sidebar / dashboard badge 其實只需 `queue.{queued,running,failed,lastActivityAt}` 與 running job 的少量欄位，並不消費 `dirtyDateKeys` 或完整 modules table。
  - 目標：拆 `load_intelligence_runtime` 成兩條路徑：(1) `load_intelligence_runtime_digest`（lightweight，只回 queue counts、`lastActivityAt`、最 newest 一個 running job 的 `title`/`progressLabel`/`progressPercent`），(2) 既有的完整 `load_intelligence_runtime`（保留給 `/intelligence` route）。shell-runtime-status 改用 digest；intelligence runtime-digest panel 仍走完整 read。
  - 契約：必須保留 100% coverage gate；不得在 digest 路徑遮蓋真實 failure（必須照常 surface error）；digest 與完整 read 對相同 queue counts 須一致（用單一 SQL 視角共享 SELECT）。
  - 驗收：dashboard cold load 上 `runtime_digest` 響應 < 200ms（dev IPC、加密 archive、264k 列）；`bun run check`；新增 Rust + JS 測試覆蓋 digest path 並驗證跟完整 read 的 queue counts 一致。

- [!] **WORK-PERF-OG-QUEUE-A** — Background OG Image Refetch Queue [!blocked: needs FE progress contract design discussion]
  - 讀先：
    `docs/features/explorer-browse.md`
    `docs/architecture/data-model.md`
    `src-tauri/crates/vault-core/src/archive/history/og_images.rs`
    `src-tauri/crates/vault-core/src/archive/history/og_images_fetch.rs`
    `src-tauri/crates/vault-worker/src/archive_flows.rs`
    `src-tauri/src/commands/archive.rs`
    `src-tauri/src/dev_ipc_bridge/dispatch.rs`
    `src/pages/explorer/hooks/use-explorer-og-images.ts`
  - 觀察（2026-05-21）：`trigger_og_image_refetch` 是 fire-and-forget 從 FE 角度看是不阻塞 UI（commit cc93243 後 dev bridge 也包了 spawn_blocking），但 backend 端單次 batch 20 URL 仍會花 ~18s 把一條 dispatch 連線占住做網路+HTML 解析，背景成本不透明。
  - 目標：把 refetch 拆成 enqueue + worker：command 立即返回 `{ enqueued, alreadyQueued }`，新 `og_image_fetch_jobs` 表（或 reuse 既有 intelligence queue）紀錄 pending URL 狀態；新 worker thread 處理 fetch，per-host rate limit 與 retry policy 集中在 worker；FE 改成 long-poll 或 SSE 觀察進度（沿用 import progress event 模式），UI 上 og:image cell 顯示骨架直到 worker 寫回。
  - 契約：失敗仍要產 negative-cache row（既有合約不變）；UI 不得因 og:image 失敗轉成全頁錯誤；新 worker 要 honor user-agent / rate limit / timeout 既有設定；不得引入新依賴除非經審核。
  - 驗收：batch 100+ URL 時 UI 維持 60fps；`bun run check`；新增 worker + FE polling 測試；docs/features/explorer-browse.md 更新 fetch lifecycle 圖。

- [!] **WORK-PERF-VIRT-A** — Cards / List Virtualization For 14.4M-Row Goal [!blocked: needs measurement run on a 14M-row archive first to size the budget]
  - 讀先：
    `docs/architecture/data-model.md`
    `docs/design/ui-review-guardrails.md`
    `docs/features/explorer-browse.md`
    `src/components/explorer-paper/paper-contact-sheet.tsx`
    `src/components/explorer-paper/paper-list-row.tsx`
    `src/pages/explorer/hooks/use-explorer-infinite-pages.ts`
  - 觀察（2026-05-21）：目前 paper-contact-sheet 完整 render days/sessions/blocks。infinite scroll 每加載一頁 append 到 React tree。對 14.4M visit / 60 年中度使用者目標，單純依賴 infinite scroll 會在 1-2k DOM nodes 開始抖動；超過 5k 後 paint cost 不可接受。
  - 目標：把 cards/list 的 visit row render path 換成 virtualization（探討 `react-virtuoso` / `@tanstack/react-virtual` / 自寫 windowing），保持 sticky day-header、per-day insights、infinite scroll sentinel 全部可用。先做 spike measurement（1M / 5M / 14M row preview fixture）建立 budget，再實作。
  - 契約：必須評估依賴授權（依 AGENTS.md 紅線）；不得放棄既有 a11y/keyboard nav；不得拆掉 day header / 每日 insights / sessions grouping；列表/卡片切換時不得有 layout jank > 50ms；測試需含 jsdom + Playwright e2e。
  - 驗收：14M-row preview fixture 上 scroll 維持 60fps（Chrome devtools performance trace 證明）；DOM node 上限不超過 viewport × 3；`bun run check` 與新增 e2e；docs/features/explorer-browse.md 更新。

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

- [ ] **WORK-REVIEW-0612-FOLLOWUPS-A** — Deferred findings from the 2026-06-12 deep code review
  - 來源：`WORK-REVIEW-0612-HARDENING-A`（見 `CHANGELOG.md`）。當輪已修掉可驗證且範圍受控的安全/正確性 bug；下列是經獨立驗證為真、但屬「大型重構 / 需 Accepted-doc 決策 / 需實機視覺驗證 / 低優先」而**刻意延後**的項目。**不要**在沒有對應 slice 與覆蓋計畫下硬塞進別的 block。
  - **Intelligence 規模化效能（最高優先，需先做審查階段）**：以下都在 14.4M-visit 目標機上違反「禁止全量加載」。多數位於 >1000 行的 intelligence owner，必須先走大檔重構兩階段（審查→執行）：
    - 語意 index 全量 build 把整張 visits 表收進 `Vec<IndexedVisit>`（`ai/indexing.rs`，no-limit 分支），且在 v0.3 bail 之前就分配。
    - structural rebuild accumulator 的 `visit_ids: Vec<i64>` 與 evidence_json 的 `visitIds` 無上限（`intelligence/intelligence_structural_aggregates.rs`）。
    - query-family 分組 O(events × families) 且每次比較重新 tokenize（`intelligence/intelligence_structural_build.rs`）。
    - scoped-debug rebuild 的 `limit` 在全量 materialize + 分類之後才裁剪（`intelligence/intelligence_visit_records.rs`）。
    - 增量 / fallback daily-rollup 與 phase_three/phase_four read models 把無上限 `Vec` 收進記憶體（`intelligence/intelligence_daily_rollups.rs`、`phase_three.rs`、`phase_four.rs`）；另含增量 vs fallback 對 `domain_category` 的「first-seen vs most-frequent」不一致與雙 local-day 實作。
  - **Intelligence job lease / recovery 強化**：`recover_expired_intelligence_jobs` 無 attempt cap 地把過期 `running` job requeue（300s lease + 每 profile 才一次 heartbeat），可造成重跑 / 雙執行；enrichment job 在 claim 後失敗（`store_enrichment` 等）會被 `let _ =` 吞掉而永遠 `running`。修法：背景 heartbeat 續租或每 batch 進度、recovery 加 attempt cap、worker 端 on-Err `mark_intelligence_job_failed`。需與 lease 設計一起做並補測。
  - **Parser（需 Accepted-doc 決策 / 語義變更）**：Chromium `visit_duration`（µs）以 `_ms` 命名 verbatim 存（E12 accepted），但 `phase_three.rs:561` 把它當真 ms 聚合 → 1000× 膨脹，需走 trade-off 文檔；`PRAGMA quick_check`（無界）每次 Browser-Direct import/preview 全庫掃描（`takeout/browser_history/staging.rs`）；orphan visit 因 INNER JOIN 靜默丟棄（chromium/firefox/safari），需 warn 或 LEFT JOIN。
  - **History regex recall 無界**：`list_history_with_regex` 綁 `:pageLimit = -1` 後全量 materialize 再過濾（`archive/history.rs`）。修法需 bounded 候選掃描 + 三語誠實提示「已掃描前 N 筆，請縮小範圍」(en/zh-CN/zh-TW) 與 response contract 欄位。
  - **App-lock KDF 強度（需新增依賴 + 遷移）**：passcode 走 120k 輪 SHA-256，建議改 argon2id（RustCrypto，符合 trust gate 但需依賴授權）+ 提高最小熵 + 線上 attempt 鎖定；同時 rekey 前的 plaintext snapshot（`archive/maintenance.rs`）在 Plaintext→Encrypted 後仍留明文，需加密或誠實提示並清理；`UnlockAppSessionRequest`/`SetAppLockPasscodeRequest` 的 `Debug` 應 redact passcode。
  - **og:image SSRF 殘留**：目前 pre-flight 守住「page URL / og:image 直接指向私網」與最終 host；但 `reqwest` 仍會自動跟隨 redirect 到私網（中間 hop 仍會發 GET）。徹底封堵需自管 redirect-follow + 逐 hop 驗證（hermetic 覆蓋需可注入 resolver/transport）。
  - **Frontend（需實機視覺驗證 / 中等）**：~~Explorer sticky day-header 的 `top` offset 由 ResizeObserver→setState 驅動，filter chips 換行時有 1 幀延遲導致短暫脫離 sticky~~ — **已修（2026-06-13，見 CHANGELOG `WORK-REVIEW-0612-FOLLOWUPS-A`）**：改用 `--pk-toolbar-h` CSS custom property，由 `useLayoutEffect` 內的 ResizeObserver 直接寫入(無 React render 中介);剩餘 interactive chip-wrap/scroll pixel 驗收待在實機 app 補眼。其餘未做：`renderedTimeResults`→`groupEntriesByDay` 在每次 favicon/og cache mutation 對整個累積集 O(N log N) 重排+重建物件（建議把 icon hydration 與 grouped data 解耦）；`useBrowseDayInsightsCache.resolve` 在 render 期間發 IPC；IntersectionObserver `root` 假設 viewport 是 scroller 但實際是 `<main>`；link-previews rebuild 在 `refreshStats()` 完成前就顯示成功。
  - **Frontend lib（中等）**：`readRequestCache` / `overviewCache` 無界增長（建議 LRU）；`invokeCachedRead` 的 `force` 在有非 forced in-flight 時被忽略；`flattenDictionary` bare-alias 跨 namespace 葉鍵碰撞（目前無 live hit，建議加 catalog 測試斷言無碰撞，或移除 bare alias）。
  - **Build/tooling（低）**：desktop-bridge truth gate 用 `Math.random` 端口偏移 + `retries:2`，跨 run / 殘留進程可能撞埠造成偽紅（建議 OS ephemeral port）；native-deps cache 的 broad `restore-keys` 可能在 baseline bump 後重用舊樹；rusqlite `bundled-sqlcipher-vendored-openssl` 的 vendored C compile 與 native-dependency ADR 措辭需補一句一致性說明。
  - 契約：每一項落地時都要先確認屬於哪個 owner / slice、>1000 行檔案先走兩階段、保持 100% JS/Rust coverage 與 mutation gate；Accepted-doc 項目（visit_duration 單位、KDF/snapshot 安全模型）必須先產出 trade-off 文檔並徵得使用者同意。

- [ ] **WORK-AI-MAPREDUCE-A** — Sub-agent time-chunk map-reduce for whole-history analysis（需先做設計階段）
  - 來源：2026-06-24 使用者驗收。使用者要「讓 agent 在知道 what's ahead 的狀況下拿到完整數據」。當輪已做 **option A**（`run_code` 在沙箱內分頁聚合完整數據——rows 不進 context，只回 distilled 結果；見 CHANGELOG 的 sandbox-完整聚合 commit）。本 block 是 **option C**：當單一 context 裝不下、或需要「逐塊的 LLM 質性分析」（不只是 JS 統計聚合）時，用 sub-agent map-reduce 跨時間塊分析整段歷史。
  - 目標（先設計，再實作）：
    - 一個確定性 orchestrator（worker 端，非讓弱模型自己 spawn）把時間軸切成塊（依時間窗或資料量），每塊派一個**全新 context 的 sub-agent**，給它該塊的完整數據存取（search/run_code/intelligence_report 限定該塊範圍），各自回一個**結構化摘要**，主 agent 再 synthesize 成最終答案。對應「分析我這一年/整段歷史」這種單一 context 裝不下的質性問題。
    - 形狀參考 Claude 的 workflow/sub-agent map-reduce，但落在 PathKeep 的 local-first worker 上：要能 spawn 多個 `drive_agent_run`（或一個 map-reduce harness）、chunk 規劃、結果 schema + 聚合、跨 N 個 agent 的 token budget / 取消 / journaling、以及進度可觀測（使用者看得到 fan-out，呼應 [[WORK-AI-INDEX-OBS-A]] 的 observability 精神）。
    - 與 option A 的邊界要寫清楚：純統計聚合 → A（一個 run_code 沙箱內搞定，便宜）；需要每塊 LLM 質性判讀或資料超過單 context → C。
  - 讀先：
    `docs/architecture/ai-security-posture.md`
    `docs/features/intelligence-and-ai.md`（AI/assistant 規格）
    `src-tauri/crates/vault-core/src/ai/agent_harness.rs`（`drive_agent_run` 迴圈 / budget / journal / cancel）
    `src-tauri/crates/vault-worker/src/intelligence/chat.rs`（`run_agent_stream` / `ai_chat_send` 的 thread + runtime）
    `src-tauri/crates/vault-core/src/ai/agent_tools.rs`（工具表面，sub-agent 也要用）
  - 契約：本機跑 N×LLM 呼叫成本高，要 bound + 進度回報 + 可取消；前端流暢度硬指標（不阻塞主線程）；AI 可選、無 provider 安靜不出現；i18n×3；100% JS/Rust coverage + lethal tests；誠實態（部分失敗的塊要誠實標示，不靜默吞）；UI/UX 與文案交給 Opus 4.6。先產出 design 文檔（chunk 策略、result schema、budget 模型、failure/部分結果語義、UI）再動代碼。
  - 驗收：對「總結我這一整年的瀏覽」這類問題，orchestrator 正確切塊、各 sub-agent 跑完回結構化結果、主 agent 合成出涵蓋全段、誠實標示任何缺漏的答案；全程進度可見、可取消；`bun run check`；更新 `docs/features/` 與 `docs/architecture/`。

- [ ] **WORK-AI-CTX-BUDGET-A** — Context-budget hardening for the agent loop（從 ai-redesign 審查 defer）
  - 來源：2026-06-24 移除 8-step cap + 強制 final-synthesis turn 後的對抗式審查（H2 / M2 / M3 / L1）。當輪已修：64-step backstop、live-context gate（量「最後一輪 prompt」而非 cumulative）、無 usage 時 fallback 估算（`estimate_context_tokens`）、ceiling → 強制 tool-free synthesis turn。以下是已知殘留邊界，本 block 把它們做成長期最優解：
    - **H2 — synthesis turn 自身可能 overflow**：gate 量的是「剛跑完那輪」的 prompt（`last_prompt_tokens`），但 synthesis turn 還會把「該輪新 threaded 的 tool results」+ directive 再送一次。若最後一輪 `run_code` 回了接近 256KB 的結果且 context 已逼近 budget，這個「救援回合」反而可能撞 context-length（→ 誠實的 provider error / Failed，note 已先發，不是靜默）。要：gate 改用「含本輪新結果的投影大小」，或組 synthesis request 時丟掉/截斷最肥的那筆剛 threaded 的結果（模型本就被要求「用已收集到的證據」作答）。
    - **M3 — budget 是寫死的 110k，不知道 provider 真實 window**：`capabilities().max_context_tokens` 目前是 `None`。小 context 模型（如 32k）會在 budget gate 觸發前就被 provider 報 context-length 錯。要把 provider 回報的真實 window 接進來推導 budget（無回報時 fallback 110k），並把 `estimate_context_tokens` 換成真 tokenizer 估算（CJK 比例 char/token 與英文差很多，現在的 bytes/4 只是粗估）。
    - **M2 — synthesis turn 回空字串無 fallback**：模型若硬是在 tool-free turn 只吐一個（無效的）tool call、不吐 text，使用者會拿到 note + 證據 chips 但沒有 prose——等於那個 case 又「靜默」了。要：偵測 `accumulated.text` 為空時，串一條 deterministic 的最小摘要（localized），讓「never silent」契約對不聽話的模型也成立。
    - **L1 — synthesis directive 未進 journal**：trace replay 重建得到一段沒有可見成因的 assistant 答案。要把 directive（或一個「final-synthesis」marker）寫進 journal payload，維持「trace = 忠實重放」。
  - 讀先：
    `src-tauri/crates/vault-core/src/ai/agent_harness.rs`（`drive_agent_run` 迴圈、`run_final_synthesis_turn`、`estimate_context_tokens`、`DEFAULT_TOKEN_BUDGET` / `DEFAULT_MAX_ITERATIONS`）
    `src-tauri/crates/vault-core/src/ai/llm.rs`（`capabilities()` / `to_llm_usage` / streaming usage）
    `src-tauri/crates/vault-core/src/ai/traits.rs`（`LlmCapabilities::max_context_tokens`）
    `docs/architecture/ai-security-posture.md`
  - 契約：誠實態（寧可誠實 provider error 也不靜默吞；部分結果要標示）；前端流暢度不受影響；AI 可選；i18n×3（若加任何 user-facing 文案，交給 Opus 4.6）；100% JS/Rust coverage + lethal tests（含 no-usage、小 window、超大最後一筆結果、空答 fallback 的案例）；`bun run check`。
  - 驗收：構造「context 逼近 budget 時最後一輪回超大結果」的測試，synthesis turn 不再 overflow（或誠實降級且有測試證明）；小 window provider 走 graceful 路徑；no-usage provider 用真 tokenizer 估算；空答有 fallback 摘要；更新 `docs/architecture/`。

- [ ] **WORK-AI-INDEX-OBS-A** — Semantic Index Build Observability & Controllability（需先做設計階段）
  - 來源：2026-06-24 使用者驗收 v0.3.0。`93a80ff6` 已在 Settings → AI 服務 的 index-health box 加了「構建索引」按鈕，但使用者回報「這方面的 observability 和 controlability 似乎有些不足」。按鈕目前只 enqueue 一個背景 job、顯示一次性「已排入背景」確認，之後使用者**看不到**進度、**控制不了**正在跑的 build。本 block 要設計並實作這套觀測 + 控制面。
  - 讀先：
    `docs/features/intelligence-and-ai.md`（或對應的 AI/intelligence feature 規格）
    `docs/design/{ux-principles,screens-and-nav,ui-review-guardrails,design-tokens}.md`
    `docs/architecture/ai-security-posture.md`
    `src/pages/settings/ai-providers-section.tsx`（index-health box + 新的 `IndexBuildButton`）
    `src/pages/settings/ai-gpu-section.tsx`（既有 re-embed flow + 進度輪詢樣式）
    `src-tauri/crates/vault-worker/src/intelligence/ai_queue.rs`（`load_ai_queue` / `run_ai_queue_jobs` / `cancel_ai_job` / `replay_ai_job` / `build_ai_index_now` / `estimate_reembed_now`）
    `src-tauri/crates/vault-worker/src/intelligence/runtime.rs`（`load_intelligence_runtime_snapshot` / `retry_intelligence_job_now` / `cancel_intelligence_job_now`）
  - 目標（先設計，再實作）：
    - **Observability**：build 進行中要有即時、誠實的進度——目前階段（embedding / indexing / rollup…）、已處理 / 總數、吞吐（items/s）、估計剩餘時間、佇列深度（queued / running / failed），以及每個 job 的最後錯誤訊息。狀態要區分 idle / queued / running / paused / done / failed / stale，never 謊稱「已建好」當它還在排隊。資料來自既有 `load_ai_queue` + `load_intelligence_runtime_snapshot`；若粒度不足，補 per-job 進度回報（目前是 global-queue，見 ai-redesign review defer-set）。
    - **Controllability**：pause / resume / cancel 正在跑的 build；重試失敗的 job（`replay_ai_job`）；選擇 rebuild 範圍（incremental vs full vs clear-only，沿用 `buildAiIndex({fullRebuild, clearOnly, scope})`）。
    - **介面設計**：決定觀測面放哪——延伸 index-health box、強化 GPU section 的進度卡、或開一個專屬「索引狀態 / 構建」面板（傾向後者，因為這是長時間、可觀測、可控制的後台工作）。要有 skeleton / 漸進式載入、~100ms 內視覺回饋、動畫順、60fps；輪詢或事件流不得阻塞主線程。
  - 契約：i18n×3（en / zh-CN / zh-TW，含所有狀態 / 錯誤 / 空態 / aria-label）；AI/embedding 為**可選**，無 provider 時整個面安靜地不出現、不 nag；build 是顯式 action（按鈕），觀測是唯讀即時態，遵守 settings 全 auto-save 規則；100% JS/Rust coverage + lethal tests；前端流暢度硬指標；誠實態（queued≠built）。UI/UX 與文案交給 Opus 4.6 subagent。
  - 驗收：實機跑一次真實 build，全程看得到進度且數字誠實；pause/resume/cancel 與 retry 都實際生效；錯誤有可重試的呈現；`bun run check`；更新對應 `docs/features/` 與（若改了 schema/runtime）`docs/architecture/`。

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
