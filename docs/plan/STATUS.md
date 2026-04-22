# STATUS.md — 當前工作

> Agent 每次開工讀這個檔案。一次只做第一個 `[ ]` work block；不要把 `STATUS.md` 再拆回原子 task。

**當前 Milestone：M13 — Broad Reuse Audit Across Support / Trust / Workflow Surfaces**

---

## CURRENT FOCUS

> 這裡的單位是 **work block**，每個 block 的份量大約是半個 milestone。
> work block 內可以包含多個子任務、ADR、代碼變更與文檔同步，但只有整塊達成可驗收成果時才改成 `[x]`。
> `STATUS.md` 通常只維持 1-2 個 work blocks。commit 仍保持可 review，不要求「一個 work block = 一個 commit」。
> 2026-04-18 planning note：使用者已明確把第二台主機 benchmark parity 從當前計劃移除；current-host `14.4M / 60y` signoff 仍是目前的 stop point。其後這輪 desktop truth audit 已落地 source-level repair 與第一輪 Computer Use / profiling evidence，但 full real-data pass 仍卡在 current-host locked-archive bootstrap / unlock drift，因此 `STATUS.md` 目前仍暫無新的 active current-focus work block。
> 2026-04-18 UI polish closeout：使用者臨時插單的 Explorer / Intelligence polish 已完成並 append 到 `CHANGELOG.md`。source 現在有 topbar 全局上一頁 / 下一頁、Explorer 分頁列的當前頁 / 總頁數與每頁筆數控制、sticky detail rail，以及 `/intelligence` 的注意力重排（移除頂部 archive-wide / Settings 大橫幅、縮小 runtime digest、提升 habits、refind 改半寬、activity mix 補分類示例、browsing rhythm 改成可點日格 + 當日 digest、低價值空卡降到 secondary grid）。`STATUS.md` 仍暫無新的 active current-focus block；`BACKLOG.md` 頂部仍只有 blocked 的 `WORK-CI-N`。
> 2026-04-18 UI follow-up note：上一輪 `/intelligence` polish 又被使用者實機 review 打回一輪。source 現在已把 `Browsing Rhythm` 還原成週內 × 小時熱力圖，並在同一卡片補上近期實際日期 chooser + 當天 digest；`Stable Sources`、`Search Effectiveness`、`Discovery Trend`、`Breadth Index` 與 `Habits` 也都改成更誠實的人話說明與可讀排版。這一輪後續修補也明確改成 desktop-only truth gate：browser preview 不再算 `/intelligence` 驗收替身；low-signal 的 `Stable Sources` / `Friction` / `Reopened Investigations` / `Path Flows` 現在會直接讓位或隱藏，而 Explorer 的 timeline summary 也會同步顯示當前頁 / 總頁數，避免只剩 loaded count。
> 2026-04-19 calendar heatmap note：使用者已明確推翻上一輪「週內 × 小時」主圖。source 現在改成真實日期的 GitHub 式 `Browsing Rhythm` 日曆熱力圖，點某一天後才顯示當天 digest / top sites / 24 小時分布；`Search Activity` 與 `Activity Mix` 也回到 half-width 同列，且 Intelligence 卡片統一接上 capped body + internal scroll。`docs/design/intelligence-rhythm-calendar-heatmap-tradeoff.md`、`ui-review-guardrails.md`、`screens-and-nav.md`、`intelligence-current-state.md` 與 `core-intelligence-ultimate-design.md` 現在都已同步這個新 truth。
> 2026-04-19 performance decoupling closeout：`/intelligence` 現在已改成 staged overview load：先批次載入 runtime digest / digest summary / 首屏 cards，再在 first paint / idle 後補 secondary grid；`Browsing Rhythm` 初次進頁不再自動抓同日 detail。shell 也已把 sidebar / Dashboard / intelligence digest 的 queue/runtime 輪詢合併成單一 shared source，import/onboarding/backup overlay 則正式支援 `phase/current/total/percent/logLines` progress stream。current-host live desktop 已用 Computer Use 驗到 dashboard ↔ intelligence 切換與 backup 後 background rebuild 不再把 shell 直接凍住；剩餘 full onboarding re-import truth pass 若要清資料，仍需使用者另外確認 destructive reset。
> 2026-04-20 search activity closeout：使用者臨時插單的 `WORK-CI-R` 已完成並 append 到 `CHANGELOG.md`。source 現在會把 URL-like / hostname-like navigational noise 從 `Top Concepts` / `Search Keywords` surface 排除，`Top Concepts` 改成 ranked horizontal bar chart，overview 與 domain route 共用 bounded `Search Keywords` browser，而 search-engine domain deep-dive 也會在 compact scope strip 下額外顯示 domain-scoped keyword history。`STATUS.md` 仍暫無新的 active current-focus block；`BACKLOG.md` 頂部仍只有 blocked 的 `WORK-CI-N`。
> 2026-04-20 desktop truth-pass rerun：使用者已清 app root 並明確要求重跑 real-data import / encryption / desktop acceptance。這輪已用 Computer Use 完成 Chrome `Yi-Ting` onboarding、`000000` 加密（未寫入鑰匙圈）、首次備份與手動解鎖；Dashboard、`/intelligence`、domain deep dive、Explorer grouped session / trail、Jobs、Audit、Schedule、Assistant disabled state 與 Settings external outputs 也都已重新抽查。但 current-host desktop 仍對 `/settings` 與 `/intelligence` 吃到 stale frontend module：持續顯示 `CORE` 分組與 `bar_chart` / `auto_stories` icon token，即使 source-level fix 已由 targeted Vitest + browser preview 證實。`WORK-CI-N` 因此仍保持 blocked，但 blocker 已從 destructive reset 改成 host/runtime stale-frontend drift。
> 2026-04-20 desktop truth-pass closeout：`WORK-CI-N` 已完成。這輪後續先透過重打 current-host release `.app` 解掉 stale bundle drift，再在最新 bundle 上重跑 Chrome `Yi-Ting` onboarding / re-import / `000000` 加密（未寫入鑰匙圈）。最新 live desktop 現在顯示 `6412ad59+` build label、`config.json` 記錄 `rememberDatabaseKeyInKeyring: false`、`/intelligence` 不再外露 raw glyph ids，且 domain deep-dive `打開網域證據` 已能正確進入 `#/explorer?...`。current focus 因此回到原本的 `WORK-M13-A` / `WORK-M13-B`。
> 2026-04-18 release-bundle note：這一輪最後是靠重打 current-host release `.app` / 直接啟動 `src-tauri/target/release/pathkeep-desktop` 才完成桌面真機抽查。Computer Use 已確認 `/explorer` 的頂部頁碼摘要與 `/intelligence` 的新 habits copy 確實進入 live desktop；但這台 host 上的 CUA 對直接啟動的 release binary 仍偶發 `noWindowsAvailable`，所以底部分頁列與較下方 intelligence sections 的 signoff 主要仍靠 regression tests + 當前可見區桌面證據，而不是假裝整頁滾到底都人工驗過。
> 2026-04-19 M6 closeout：`WORK-M6-A` 已完成。`day` 與 `domain` 現在都已升格成 first-class shared insights entity：新增 `/intelligence/day/:date`、保留但正式升格 `/intelligence/domain/:domain`、shared href grammar、`Insight Access` strip，以及 Dashboard / Intelligence / Explorer 的 route-first entry。下一輪 active current-focus 改成 `WORK-M7-A`，用來全面盤點其餘仍然 consumer-local 的 intelligence entity reuse。
> 2026-04-19 M7 closeout：`WORK-M7-A` 已完成。repo 現在正式有 generic `InsightEntityTarget` / href contract、shared entity CTA chrome、以及 `/intelligence/query-family/:familyId`、`/intelligence/refind/:canonicalUrl`、`/intelligence/session/:sessionId`、`/intelligence/trail/:trailId` 四條 first-class shared insights route；`reopened investigation`、`habit/stable source/friction/multi-browser diff`、`compare set` 與 Settings external-output chips 也已收斂到 shared destination。下一輪 active current-focus 改成 `WORK-M8-A`，專門處理 path-flow stable identity、compare-set full detail、context focus 與更多 reusable entity IDs。
> 2026-04-19 M8 closeout：`WORK-M8-A` 已完成。repo 現在正式有 `/intelligence/compare-set/:compareSetId`、shared `focusType` / `focusId` query grammar、typed `path flow` identity、以及 trusted external-output payload 的 structured entity targets；`public snapshot` 仍維持 redacted。下一輪 active current-focus 改成 `WORK-M9-A` / `WORK-M9-B`，全面盤點剩餘 consumer-local composition 與 shared extraction 機會。
> 2026-04-19 M9 closeout：`WORK-M9-A` 與 `WORK-M9-B` 已完成。repo 現在正式有 shared route-level metric strip、`query-family-card`、compare-set page list、structured target label，以及 inline-end section-meta header chrome；`證據與新鮮度` badge 不再佔整行或吃滿整個 card header。下一輪 active current-focus 改成 `WORK-M10-A` / `WORK-M10-B`，專門處理仍未抽出的 workbench/review rows 與 route / desktop glue decomposition。
> 2026-04-19 M10 closeout：`WORK-M10-A` 與 `WORK-M10-B` 已完成。repo 現在正式有 shared `refind` workbench shell、Explorer session/trail shared group-card/member-row primitive、Settings external-output/local-host shared review chrome，以及 split 的 promoted routes / Core Intelligence API / Tauri command + worker-bridge intelligence facade；public route / payload contract 維持不變。下一輪 active current-focus 改成 `WORK-M11-A` / `WORK-M11-B`，從 app-wide reuse / review grammar 角度盤點剩餘 mixed helper、dev mirror 與 diagnostics surface。
> 2026-04-19 M11-A closeout：`WORK-M11-A` 已完成。repo 現在已有 app-wide review grammar single-source map、[`docs/design/app-wide-review-grammar-tradeoff.md`](../design/app-wide-review-grammar-tradeoff.md)、`PG-RD-UX-012`、以及 M12 seed 計劃；`src/lib/intelligence.ts` / dev IPC mirror / worker pass-through 的後續邊界也已定案。active current-focus 現在只剩 `WORK-M11-B`，專門把 neutral review primitive 抽到 Settings / Schedule / Audit / Jobs。
> 2026-04-19 M11-B closeout：`WORK-M11-B` 已完成。repo 現在正式有 app-wide neutral review primitive（`review-surface`、`PmeTabBar`、`GeneratedArtifactViewer`、`VerifyCheckList`），而 Settings / Schedule / Audit / Jobs 也都已接上 shared review grammar；`src/lib/intelligence.ts` 只剩 compatibility barrel，dev mirror / worker pass-through follow-up 則改由 M12 parity inventory 追蹤。依照工作流，下一輪 active current-focus 已切到 `WORK-M12-A` / `WORK-M12-B`。
> 2026-04-19 M12 closeout：`WORK-M12-A` 與 `WORK-M12-B` 已完成。repo 現在正式有 app-wide shared support-action / clipboard grammar：`src/components/review/` 追加了 shared clipboard helper 與 `ReviewPathActionRow`，而 Settings general diagnostics / App Lock、Audit manifest / artifact review、Import selected-batch audit path、Schedule detected-file / audit quick jump、Security / Lock path rows，以及 Explorer export path 都已接回同一個 canonical owner。Jobs plugin / module summary rows與 dev bridge / worker parity follow-up 則已明確改記 `TODO: M13`。依照工作流，下一輪 active current-focus 已切到 `WORK-M13-A` / `WORK-M13-B`。
> 2026-04-20 performance stop-ship closeout：使用者明確要求先停下 M13 reuse audit，優先修復 `/intelligence` 在三個月真實資料上的 UI 凍結與 route revisit 卡頓。這輪插單的 `WORK-PERF-A` 已完成：Core Intelligence overview 讀路徑現在同一批只重用一條 intelligence connection 與一份 runtime snapshot；前端則補上 scope-keyed warm cache、in-flight dedupe、stale-while-revalidate、以及 Search Activity hidden tabs 的 idle prewarm。M13 A/B 保留 pending，等這輪驗收完成後再繼續。
> 2026-04-20 archive/import stop-ship closeout：使用者再度插單 `WORK-PERF-B`，要求先修 Onboarding 初始化 / 手動備份 / Takeout scan-import 會把整個桌面 UI 卡死的問題。source 現在已把 `initialize_archive`、`run_backup_now`、`inspect_takeout`、`import_takeout` 改成 off-main-thread `async + spawn_blocking` facade，Import route 也補上 explicit paint-first yield；同時新增 shell-data 與 Import route regressions，確保 busy overlay 在 promise 未完成前就已經可見，且進度文案不再等任務結束後才一次補播。M13 A/B 繼續維持 active current-focus。
> 2026-04-21 M13 inventory closeout：`WORK-M13-A` 已完成。`docs/plan/m13-broad-reuse-audit/README.md` 現在正式記錄 app-wide single-source map、extraction priority 與 remaining hotspot；`PG-RD-UX-016` 也把 runtime-boundary review grammar 收斂成 `src/components/review/runtime-boundary-card.tsx` 的 canonical owner。這輪同步落地的第一個 code slice 讓 Jobs runtime health / plugin / module summary 與 Settings derived runtime review 共用同一套 runtime-boundary card shell，但 `WORK-M13-B` 仍保持 active，後續 focus 改成 shell-data、Security / Import workflow follow-through、Dashboard fallback owner 與 `Browsing Rhythm` layering。
> 2026-04-21 backend track note：使用者明確要求並行開啟後端 hotspot 拆分，不等 `WORK-M13-B` front-end reuse 收束。這輪新增 `WORK-BE-A` 作為 user-directed parallel block；frontend reuse 與 backend decomposition 分開推進，彼此都不得覆寫對方未提交中的工作樹。

- [ ] **WORK-BE-A** — Backend Hotspot Decomposition And Import Boundary Split
  - 讀先：
    `docs/plan/backend-hotspot-decomposition.md`
    `docs/architecture/data-model.md`
    `docs/architecture/module-boundary-map.md`
    `docs/architecture/desktop-command-surface.md`
    `docs/architecture/tech-stack.md`
  - 目標：把 2026-04-21 backend 架構審查轉成可執行的拆分軌道，先處理 `takeout` / parser / archive ingest 這條大數據量風險最高的 import boundary，再往 intelligence runtime 與 core intelligence hotspot 推進。
  - 契約：維持現有 Tauri command、worker CLI、serde payload、audit artifact 與 canonical schema 語義穩定；不得把 frontend M13 reuse block 的未提交改動捲進來；所有新建或整段重寫的 backend 模塊都必須帶完整 file header 與 declaration-level doc comments。
  - 2026-04-22 progress：第一階段後端審查已完成並落在 `docs/plan/backend-hotspot-decomposition.md`；第一個 execution slice 已把 `src-tauri/crates/vault-core/src/takeout.rs` 拆成 `takeout/{inspect,import_flow,batches,tests}.rs`，保留 import command / batch audit contract 不變。第二個 execution slice 已把 `src-tauri/crates/vault-core/src/archive/mod.rs` 的 canonical ingest boundary 下沉到 `archive/ingest/{mod,parser,writes}.rs`，並讓 `archive::maintenance` 透過明確的 preview helper 使用 parser/watermark contract，而不是偷用內部狀態。第三、四個 execution slice 把 Takeout import / review 熱路徑繼續收斂：`takeout/import_flow.rs` 現在委派 `takeout/payload_import.rs` 直接消費 parser report，`takeout/batch_review.rs` 承接 recent-batch read、preview 與 audit-artifact repair，而 `takeout/batches.rs` 回到 write-side revert/restore ownership。第五個 execution slice 又把 non-dry-run execute path 的雙重解析拿掉：`import_takeout` 不再先跑完整 `inspect_takeout`，而是單次掃描檔案並在寫 canonical rows 的同時累積 batch metadata，最後再用 persisted `preview_import_batch` 回填 review payload。第六個 execution slice 再把 `archive/mod.rs` 的 backup orchestration 與 manifest/snapshot support helper 下沉到 `archive/{backup,run_support,artifacts}.rs`，讓 parent module 直接降到 `406` 行，同時保留 backup/restore/takeout 的既有 contract。第七個 execution slice 又把 backup ingest 與 Takeout import 的 source-evidence plan 從整份 `ParsedHistory` 收斂成 `typed_evidence + native_entities`，先拿掉 canonical commit 之後那份最熱的 duplicate retention。第八個 execution slice 把 Chromium live-backup path 改成 chunked canonical ingest；第九個 execution slice 再把 Firefox / Safari live-backup path 也切到同一條 streamed parser contract。第十個 execution slice 進一步把 deferred cold source-evidence payload 改成可 spill 到 `staging/source-evidence-spool/` 的 bounded-memory contract，並把 `snapshot_restore` preview 改成直接讀 checkpoint row counts，同時修掉 multi-profile backup 下 checkpoint preview 會誤用第一個 `profile_scope` 的 Safari/Firefox 歸屬漂移。第十一個 execution slice 再把 `browser-history-parser::takeout` 從單個 `728` 行 giant-file 拆成 focused submodules，並新增 payload-level streaming contract，讓 `vault-core::takeout::payload_import` 可以在 BrowserHistory payload 仍在解析時就開始寫 canonical URL/visit rows。第十二個 execution slice 再把 `inspect_takeout` 改成直接消費 payload-level streamed preview：dry-run preview 現在不再先物化完整 payload report，也不再在 inspection path 上累積 `typed_evidence/native_entities`。第十三個 execution slice 再把 Takeout import 剩下的 source-native evidence retention 改成 streamed chunk contract：`browser-history-parser::takeout` 現在可以把 source evidence 分段交給 consumer，而 `vault-core::takeout::payload_import` 會透過新的 `archive::source_evidence_builder` 邊接收邊 spill 到 `staging/source-evidence-spool/`，所以單一 payload import 不再需要先保留一整份 native-evidence batch 才能進 post-commit cold write。第十四個 execution slice 再把 `src-tauri/crates/vault-core/src/intelligence_runtime.rs` 拆成 `intelligence_runtime/{mod,enqueue,claims,job_control,recovery,snapshot,tests_queue,tests_runtime}.rs`，把 queue writes、claim/recovery、job-state control、runtime snapshot read model 與 regression suite owner 分開，同時保留既有 `load_intelligence_runtime` / `retry_intelligence_job` / `cancel_intelligence_job` / worker claim flow 契約不變。第十五個 execution slice 再把 `intelligence/mod.rs` 的 overview / summary / domain / export read models 抽成 `intelligence_{overview,summary,domain,outputs}.rs`，把 `/intelligence` staged overview、digest/stable-source/search-effectiveness、domain/deep-dive/discovery/on-this-day、以及 embed/widget/public snapshot payload 從 rebuild giant-file 中拆開；`mod.rs` 也因此先從 `11043` 行降到 `9761` 行，同時保留既有 query surface、overview contract、worker/runtime semantics 不變。第十六個 execution slice 又把 `intelligence/mod.rs` 的 refind/detail/explain surfaces 抽成 `intelligence_{refind,explain,explain_helpers}.rs`，讓 refind page detail、`explain_entity`、以及 explanation-only helper loaders 不再和 rebuild logic 混在一起；`mod.rs` 因此再降到 `8848` 行。第十七個 execution slice 再把 schema/bootstrap 與 rebuild orchestration 從 `intelligence/mod.rs` 分別抽成 `intelligence_{schema,schema_sql,rebuild}.rs`，把 migration/bootstrap、derived-state clear、public rebuild entrypoints、legacy scoped fallback、以及 runtime-ready update ownership 從 giant-file 本體拿出來，讓 `mod.rs` 再降到 `7703` 行。現在 `archive/takeout import`、`intelligence_runtime`，以及 `intelligence/mod.rs` 的 read/explain/schema/rebuild ownership 都已完成第三輪結構拆分；剩餘 giant-file 風險更集中在 structural rebuild internals 與 query/read-model helpers，之後再視情況跟進 `vault-worker/src/intelligence.rs` 的 passthrough/orchestration 拆分。
  - 2026-04-22 Takeout truth follow-up：使用者用真實 Google Takeout 驗出「現有導入其實不能用」後，這一輪又把 Takeout boundary 往 product truth 再收一層：`browser-history-parser::takeout` 現在改成 locale-aware path dispatch，只把 dedicated Chrome history payload（`BrowserHistory.json`、`History.json`、`Verlauf.json`）視為正式 importable；typed-url / session companion 改成 source-evidence-only，Chrome 相關 `My Activity` 改成 `needs-review`，不再因為 path 裡含 `history` / `browser` 就硬吃。`vault-core::takeout::{inspect,import_flow}` 也同步升格成 additive file classification / detected locale / preview time-range contract，讓 `/import` 能誠實解釋「哪些會導入、哪些故意略過、哪些需要人工複核」。
  - 驗收：relevant targeted Rust regressions、`bun run check && bun run build`

- [ ] **WORK-M13-B** — Shared Support / Workflow Composition Extraction
  - 讀先：
    `docs/plan/m13-broad-reuse-audit/README.md`
    `docs/design/screens-and-nav.md`
    `docs/design/ux-principles.md`
    `docs/plan/e2e-workflow-tests.md`
  - 目標：根據 `WORK-M13-A` 的 inventory，把至少一輪高價值的 support / trust / workflow composition 抽離，優先處理 Jobs plugin/module summary、workflow follow-through 與剩餘 support summary drift。
  - 契約：只抽明確跨 consumer 重複且能降低 drift 的 grammar；不得為了抽象而重開 M6–M12 已收斂的 route / payload / review / support-action contract。
  - 2026-04-21 progress：shared runtime-boundary card grammar 已落到 `src/components/review/runtime-boundary-card.tsx`，Jobs runtime health / plugin / module summary 與 Settings derived runtime review 是第一批 consumer；Jobs route shell 也因此降到 `1000` 行以下。下一輪優先處理 shell-data owner split、Security / Import workflow follow-through、Dashboard fallback owner 與 `Browsing Rhythm` layering smell。
  - 2026-04-22 Import workflow slice：`/import` 現在已把 `new import wizard -> grouped scan report -> recent imports / selected batch / doctor repair` 的閱讀順序落地，並直接吃 backend 新增的 `will-import / known-but-ignored / needs-review / parse-error` file classification、detected locale 與 preview time range；Takeout UI 不再只把檔案全塞進一個雜亂 preview list，而是能說清楚目前 shipping 的 Chrome-first scope。
  - 驗收：`bun run check && bun run build`

- [x] **WORK-UI-D** — Dashboard Rhythm Merge And Intelligence IA Cleanup
  - 讀先：
    `docs/design/screens-and-nav.md`
    `docs/features/intelligence.md`
    `docs/features/intelligence-current-state.md`
    `docs/features/core-intelligence-ultimate-design.md`
    `docs/architecture/data-model.md`
  - 目標：把 `feat/dashboard-heatmap` 的有用 contract 收回目前分支，同時維持已 accepted 的真實日期 `Browsing Rhythm`、staged `/intelligence` load、shared runtime polling，並把 `On This Day` 從 `/intelligence` 移回 Dashboard-only。
  - 契約：Dashboard 的 `Browsing Rhythm` 必須固定以 calendar year 呈現，year switcher 只來自 `getDiscoveryTrend(..., 'day').availableYears`；`On This Day` 不再出現在 `/intelligence`；storage analytics 的 top-level summary 先固定成 `core history` / `other data`。
  - 驗收：`bun run check && bun run build`

- [x] **WORK-QC-L** — Intelligence Recovery And Desktop Truth Gate
  - 讀先：
    `docs/plan/e2e-workflow-tests.md`
    `docs/features/intelligence.md`
    `docs/features/deterministic-intelligence.md`
    `docs/design/screens-and-nav.md`
    `docs/architecture/desktop-command-surface.md`
  - 目標：把 deterministic insights、Settings / Insights copy、desktop-bridge e2e 與 CI 驗收重新收斂成真的可用 surface，而不是 preview fixture / placeholder completion。
  - 契約：backup / import 後 deterministic rebuild 必須自動排入並留下可 review 的 runtime trace；`On This Day` 只能回看過去年份；主產品 UI 不得外露 `m4-v1` / `m5b-v1` 這類內部里程碑版本字串；desktop bridge 必須驗到 live Rust flow，而不是只停在 health / build-info smoke。
  - 驗收：`bun run build`、targeted Rust / Vitest regression tests、`test:e2e:desktop-bridge:truth` 能在有權限的 host 上穩定跑完；source docs 與 plan tracking 同步回寫真實邊界。

- [x] **WORK-QC-N** — Backend Rustdoc Sweep And Module Decomposition
  - 讀先：
    `docs/architecture/data-model.md`
    `docs/architecture/module-boundary-map.md`
    `docs/architecture/desktop-command-surface.md`
    `docs/architecture/tech-stack.md`
    `docs/features/archive.md`
    `docs/features/intelligence.md`
    `docs/features/deterministic-intelligence.md`
  - 目標：把 Rust backend 補成 self-explanatory map。所有 runtime Rust 檔案都要有清楚檔頭與符號級 doc comments，並在補文檔時同步拆掉 `vault-worker`、`vault-core::archive`、`vault-core::{chrome, ai, insights}` 等現有 hotspot 的責任混寫。
  - 契約：維持現有 Tauri command、CLI command、serde payload 與 top-level re-export 穩定；任何行為修正都必須附對應測試與 source-doc 更新。
  - 驗收：`bun run check && bun run build`

> 2026-04-17 priority note：Core Intelligence reset 的後續工作已經不適合再靠 pre-reset M3/M4/M5 文檔或舊 `WORK-QC-*` 名稱猜進度。若使用者明確要求「繼續前端」或「繼續後端」的 Core Intelligence 工作，先讀 `docs/plan/core-intelligence-progress.md` 與 `docs/plan/core-intelligence-handoff.md`，再選對應的 `WORK-CI-*` block。

- [x] **WORK-CI-C** — Core Intelligence Legacy Cleanup And Long-Horizon Signoff
  - 讀先：
    `docs/plan/core-intelligence-progress.md`
    `docs/plan/core-intelligence-handoff.md`
    `docs/plan/program/research-and-decisions.md`
    `docs/architecture/data-model.md`
  - 目標：在 `WORK-CI-B` 已完成之後，把 remaining legacy `vault-core::insights` 責任、`14.4M+` / longer-horizon benchmark、額外 runtime complexity / resume strategy，以及 backend finish-line 收口後仍殘留的長期 signoff scope 收口成 accepted truth。
  - 契約：所有刪舊與性能收口都要以 current Core Intelligence contract 為中心，不可再為 legacy snapshot-first path 補 compatibility 層。
  - 驗收：source docs、benchmark artifact、cleanup diff、以及對應 quality / manual recipe 都存在。

> 2026-04-18 closeout：`WORK-CI-C` 已完成。current-host `14.4M / 60y` signoff 與 expired-lease recovery artifact 已落在 `artifacts/benchmarks/2026-04-18-intelligence-long-horizon-signoff/`；legacy `vault-core::insights` 也已正式退場。`BACKLOG.md` 目前沒有新的未阻塞 block，因此 `STATUS.md` 暫無新的 active current-focus 項目。

> 2026-04-18 closeout：`WORK-M5-C` 已完成。`/intelligence` 與 `/intelligence/domain/:domain` 現在會透過 typed section envelope 顯示 generated-at、scope/window、module ownership、source tables、enrichment flag、以及 stale / disabled / degraded reason；mutation controls 仍明確留在 Settings / Jobs。當時規劃的第二台主機 benchmark parity follow-up 已在後續由使用者明確移出當前計劃。

> 2026-04-18 external host closeout：`WORK-CI-I` 已完成。Settings external outputs 現在除了 manual review / copy-export baseline，也能 preview / build / verify 第一個 trusted local host `browser-snippet-v1`，固定產出 `app_root/integrations/core-intelligence/browser-snippet-v1/{index.html,bundle.json}`。目前 stop point 維持在 current-host `14.4M / 60y` signoff，`STATUS.md` 暫無新的 active current-focus 項目。

- [x] **WORK-CI-K** — Core Intelligence App Truth Repairs
  - 讀先：
    `docs/plan/core-intelligence-progress.md`
    `docs/plan/core-intelligence-handoff.md`
    `docs/features/intelligence-current-state.md`
    `docs/features/core-intelligence-ultimate-design.md`
    `docs/design/screens-and-nav.md`
  - 目標：把 2026-04-18 實機驗證抓到的 shipped blockers 收口：`/intelligence` section-envelope crash、`daily-rollup` fallback uniqueness bug、encrypted onboarding 無 keychain regression，以及 queue / copy / privacy / route error truth drift。
  - 契約：section metadata 再壞也只能 degraded 顯示、不得把整頁炸成 React 預設錯誤頁；`domain_daily_rollups` 維持一天 / 一 profile / 一 registrable domain 一列；加密 onboarding 在不儲存鑰匙圈的情境下必須能走完；Explorer / explainability / onboarding / dashboard 不能外露未處理的 raw callback URL、token、email 或明顯半成品文案。
  - 驗收：targeted Rust / Vitest regressions、`bun run check && bun run build`；browser preview `/intelligence` truth pass；手動桌面驗證若仍撞上 stale bundled assets，要在 source docs 誠實記錄 host-specific noise，而不是把 source 修復誤記成未完成。

- [x] **WORK-CI-L** — Core Intelligence Desktop Truth Repair
  - 讀先：
    `docs/plan/core-intelligence-progress.md`
    `docs/plan/core-intelligence-handoff.md`
    `docs/features/intelligence-current-state.md`
    `docs/features/core-intelligence-ultimate-design.md`
    `docs/design/screens-and-nav.md`
  - 目標：把 2026-04-18 後續實機驗證抓到的前端 shipped-truth drift 再收一輪：archive-wide callout / activity-mix copy、external-output CTA、Explorer 可見 URL redaction、domain deep-dive decoded path、以及 `/intelligence` runtime digest 的 data dependency。
  - 契約：不新增 Tauri command、不改 Core Intelligence schema / payload-provider contract；`/intelligence` digest 只看 Core Intelligence runtime truth，不再主動讀 AI queue；Explorer 任何可見 UI 都不能再直接外露 callback URL、token、auth code 或 email-like 字串。
  - 驗收：targeted Vitest regressions、`bun run check && bun run build`；fresh desktop app manual pass 若仍顯示 raw key / 舊 CTA / 舊 queue 行為，必須把 current-host stale WebView / bundle cache noise 寫回 source docs，而不是把 source 修補誤記成未完成。

> 2026-04-18 desktop truth repair closeout：`WORK-CI-L` 已完成。source 現在已固定 archive-wide callout copy、`category_community` label、external-output CTA、Explorer URL redaction、domain deep-dive decoded path，且 `/intelligence` digest 只讀 `load_intelligence_runtime`。planning truth 也已回寫：原始 deterministic Core Intelligence P1–P4 scope 已完成，只剩 `browser-snippet-v1` 之外的 external host integration。這台主機的 fresh Tauri dev app 若仍顯示 raw `intelligence.*` key、舊 CTA 文案或舊 queue 行為，應先視為 current-host WebView / stale bundle cache noise。

- [x] **WORK-CI-M** — Desktop Truth Audit And Locked-Archive Bootstrap Repair
  - 讀先：
    `docs/plan/core-intelligence-progress.md`
    `docs/plan/core-intelligence-handoff.md`
    `docs/features/core-intelligence-ultimate-design.md`
    `docs/features/intelligence-current-state.md`
    `docs/plan/e2e-workflow-tests.md`
    `docs/plan/m4-full-polish/large-archive-performance-runbook.md`
  - 目標：針對 current-host desktop app 做一次真的 locked-archive startup / Security unlock / cross-route truth audit，而不是只靠 source docs 宣稱 P1-P4 已完成；同時把 audit 途中攔住全局 shell 的 bootstrap / error-shaping 問題先止血。
  - 契約：不新增 Tauri command；優先修 transport error shaping、Dashboard fallback、worker snapshot 的 best-effort degradation，並把 current-host 真實觀察、未完成 audit 範圍與 perf artifact 誠實寫回 source docs。
  - 驗收：targeted Vitest / Rust regressions、`artifacts/perf/2026-04-18-desktop-truth-audit/` evidence、以及 planning/source docs 對 current-host blocker 與後續 follow-up scope 的同步回寫。

> 2026-04-18 desktop audit note：`WORK-CI-M` 已完成 source-level repair 與第一輪真機盤點。source 現在補上 Tauri transport detection、raw invoke error shaping、Dashboard `securityStatus()` fallback、以及 worker app snapshot 的 best-effort browser-discovery/runtime-diagnostics degradation；同時留下 [`docs/plan/core-intelligence-desktop-truth-audit.md`](core-intelligence-desktop-truth-audit.md) 與 [`artifacts/perf/2026-04-18-desktop-truth-audit/`](../../artifacts/perf/2026-04-18-desktop-truth-audit/)。但 current-host live app 仍存在 locked-archive bootstrap / unlock drift：Dashboard fresh boot 依然顯示 generic `無法讀取封存`、Security route 雖可讀到真實 encrypted+locked 狀態，`000000` unlock flow 也未在觀察窗口內 settle。因此 full import / `/intelligence` / Explorer session-trail / domain deep-dive real-data pass 已移到後續 follow-up，而不是在這輪 audit 內假裝完成。

- [x] **WORK-CI-O** — Locked-Archive Shell Truth Follow-Up And Build Revision Diagnostics
  - 讀先：
    `docs/plan/core-intelligence-desktop-truth-audit.md`
    `docs/plan/core-intelligence-progress.md`
    `docs/plan/core-intelligence-handoff.md`
    `docs/features/intelligence-current-state.md`
    `docs/design/screens-and-nav.md`
  - 目標：把 `WORK-CI-M` 暴露出的兩條 source-level follow-up 再收一輪：讓 locked encrypted archive 的 shell bootstrap 至少能退化成可用 snapshot / fail-fast unlock path，並把 compact build diagnostics（`version · short-sha[+]`）補回 app chrome，方便 current-host 真機審計時辨認到底跑的是哪個 build。
  - 契約：不新增 Tauri command；延續既有 `securityStatus()` / build-info contract。locked archive 時 sidebar 不得再主動輪詢 background runtime；若 current-host fresh relaunch 仍顯示 generic dashboard copy 或不帶 SHA 的 shell chrome，必須把它誠實記成 stale WebView / bundle cache drift，而不是把 source 修補誤記成沒做。
  - 驗收：targeted Rust / Vitest regressions、fresh `bun run desktop:dev` Computer Use relaunch note、`bun run check && bun run build`

> 2026-04-18 locked-archive follow-up note：`WORK-CI-O` 已完成 source-level修補。worker `app_snapshot` 現在對已初始化但未解鎖的 encrypted archive 會回傳 usable locked snapshot，Security unlock flow 也會先驗 candidate key 是否真的解鎖，再決定要不要進 full shell refresh；sidebar 背景工作 strip 在 archive 未解鎖時不再輪詢 runtime，shell / onboarding / lock / diagnostics 也已補回 compact `version · short-sha[+]` build label。但這台主機 fresh `bun run desktop:dev` 重啟後仍顯示舊的 generic dashboard copy 與不帶 SHA 的 shell chrome，同時 worker log 已明確打出 encrypted-archive key warnings；這應先視為 current-host stale WebView / bundle cache drift，`WORK-CI-N` 仍保持 blocked，等待 host-side cache noise 或 reset 決策被解掉。

---

> 2026-04-10 unblock：使用者已對 `ADR-006` 明確 sign off，`WORK-M5-A` 因此從 proposal / blocked 轉為 active。M4 closeout 仍維持完成，但 2026-04-10 也補修了 onboarding archive-mode IPC 契約與 insights refresh queue regression。

> 2026-04-10 closeout：`WORK-M5-A` 已完成，deterministic foundation / taxonomy、first-party-only enrichment runtime、dual built-in plugin defaults，以及 Settings / Insights queue review / retry / cancel surface 現在都已回寫到 source docs 與實作。

> 2026-04-10 backend size closeout：使用者臨時插單的 `WORK-QC-E` 已完成。macOS release executable 透過 native keyring backend slim-down + release strip/LTO，從 `190M` 降到 `104M`；更深一層的 optional intelligence build-boundary 問題已誠實回收到 `BACKLOG.md` 的 `WORK-QC-F`。

> 2026-04-10 packaging closeout：使用者已明確 sign off 保留 default desktop build 內建 optional AI / MCP / semantic runtime；`WORK-QC-F` 因此以 [ADR-009](../architecture/decisions/009-default-desktop-optional-intelligence-shipping.md) 與 `artifacts/release/2026-04-11-size-audit/` 的 refreshed evidence 正式收口。當前 truth 是：web payload 仍低於 `1 MB`，而 unsigned macOS executable 約 `104 MiB`，這個重量現在屬 accepted trade-off，而不是 active blocker。

> 2026-04-10 platform quality closeout：使用者臨時插單的 `WORK-QC-G` 已完成。`vault-platform` 已拆成 keyring / scheduler / launcher / host capability / discovery 子模組，`bun run check` 現在固定納入 `check:platform`，會在對應 host 上跑 native keyring / scheduler / launcher / discovery / biometric smoke；updater 也已收回 typed desktop command surface，不再讓前端直接調 plugin guest API。

> 2026-04-10 testing closeout：使用者臨時插單的 `WORK-QC-H` 已完成。repo 現在有 feature-gated `desktop:dev:bridge` / `test:e2e:desktop-bridge` local dev loop，能在 macOS 上把前端跑進 Chrome 並透過 localhost 命中真實 Rust desktop command façade；`browser-preview`、`browser-desktop-bridge`、`tauri` 三種 runtime 邊界也已回寫到 quality / architecture docs。

> 2026-04-10 code-review sweep closeout：`WORK-QC-I` 與 `WORK-QC-J` 已完成。remote backup verify 現在補上 detached manifest checksum + zip entry-set drift detection、App Lock / rekey / import recoverability gaps 已回補、Insights scoped stale-state 與 Explorer drilldown 保 scope、derived rebuild / bridge updater / release size audit provenance 也都已用 regression tests 與 source docs 收口。

> 2026-04-11 frontend maintainability closeout：`WORK-QC-K` 已完成。活躍前端 `src/` surface 現在補上 file header 與 declaration-level doc comments，把 shell IA、PME / trust grammar、i18n contract、shared profile scope、design token / typography policy 直接寫回代碼；同時也抽出 `src/pages/settings/helpers.ts`、補齊對應 tests、刪除 stale `src/lib/i18n/messages.ts` duplicate，並補記新的 transitive `RUSTSEC-2026-0097` allowlist rationale 讓 `bun run check` 重新回綠。

> 2026-04-12 intelligence recovery closeout：`WORK-QC-L` 已完成。Jobs / Insights 現在會用真實 queue / enrichment / deterministic runtime 誠實呈現 backlog、needs-review、content-fetch 失敗原因與 analysis snapshot，不再把 deferred work 誤報成整條功能失敗；browser-desktop-bridge truth gate 也已修補 multi-process fixture drift、cold-start cache 與 stale port 問題，`bun run test:e2e:desktop-bridge:truth` 在這台主機上已連續兩次跑綠，後續 hosted-runner platform-native truth 只保留在 manual workflow，不再燒每次 push / PR 的 mainline CI 分鐘。

> 2026-04-14 source-evidence architecture closeout：使用者明確 sign off 後，`WORK-QC-S` 已完成。repo 現在以 [ADR-011](../architecture/decisions/011-source-evidence-archive-and-capability-contract.md) 與 `docs/dev/` guides 正式凍結多瀏覽器 schema / evidence 保存 contract；archive plane 進一步明確成 hot canonical + cold source-evidence split，`browser-history-parser` 會輸出 schema observation / capability snapshot / typed evidence / native entities，remote bundle 也已把 `archive/source-evidence.sqlite` 納入 restore-ready contract。

> 2026-04-17 external output closeout：`WORK-CI-H` 已完成。Settings 現在正式承接 `embed cards`、`widget snapshot`、`public snapshot` 的 manual review / copy-export surface，會沿用 shared profile scope 與 local time window，並誠實標示 trusted-only / public-redacted 邊界；`/intelligence` 則改成指向 Settings 的 CTA，不再把 payload provider 誤包裝成完整 host integration。

> 做完了？→ 把完成的 work block append 到 [CHANGELOG.md](CHANGELOG.md)，同步 source docs，然後再從 [BACKLOG.md](BACKLOG.md) 補下一個 block。
