# STATUS.md — 當前工作

> Agent 每次開工讀這個檔案。一次只做第一個 `[ ]` work block；不要把 `STATUS.md` 再拆回原子 task。

**當前 Milestone：M13 — Broad Reuse Audit Across Support / Trust / Workflow Surfaces**

---

## CURRENT FOCUS

> 這裡的單位是 **work block**，每個 block 的份量大約是半個 milestone。
> work block 內可以包含多個子任務、ADR、代碼變更與文檔同步，但只有整塊達成可驗收成果時才改成 `[x]`。
> `STATUS.md` 通常只維持 1-2 個 work blocks。commit 仍保持可 review，不要求「一個 work block = 一個 commit」。

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
  - 驗收：Schedule install/remove/update interval、current config view、legacy/error attention state、sidebar move/rename、Onboarding apply happy path、Onboarding skip hint path、三語 i18n parity、targeted Vitest、`bun run check`、current-host desktop truth pass。
  - 2026-04-29 closeout：已依確認後的 Phase 2 版面方向完成 Schedule page 與 Onboarding schedule step，但保留 PathKeep 既有 dark/orange visual language。`/schedule` route 保持不變；左側欄項目移至 `SYSTEM`，三語改為 `Scheduled Backup Settings` / `定时备份设置` / `定時備份設定`。
  - UI truth：Schedule page 現在直接顯示平台、安裝狀態、legacy/error attention、目標間隔、目前設定、安裝/更新/移除 controls 與原 PME preview/manual/execute/verify panel；interval 改動會先顯示 explicit save/update action。Onboarding 新增 schedule install-or-skip step；skip 只進入 Ready 並提示 `System → Scheduled Backup Settings`，install path 只記錄 intent，真正 apply 仍在 finish 後使用既有 `apply_schedule` command。
  - 同步回寫 [`docs/features/archive.md`](../features/archive.md)、[`docs/architecture/desktop-command-surface.md`](../architecture/desktop-command-surface.md)、[`docs/design/screens-and-nav.md`](../design/screens-and-nav.md)、[`docs/plan/m1-solid-archive/schedule-security-and-storage.md`](m1-solid-archive/schedule-security-and-storage.md)、[`docs/plan/scheduled_backup_redesign_spec.md`](scheduled_backup_redesign_spec.md)、[`docs/plan/STATUS.md`](STATUS.md) 與 [`docs/plan/CHANGELOG.md`](CHANGELOG.md)。
  - 驗收結果：targeted Vitest / browser-preview e2e slices、`bun run check`、`bunx tauri build --debug --bundles app --no-sign`、fresh debug `.app` Computer Use truth pass。current-host truth pass 驗到 `legacy-install-detected` attention state 與 schedule controls；為避免改動使用者 LaunchAgents，未在真機點擊 install/remove/final finish，native apply/remove 行為由既有 Rust/platform tests 與 desktop bridge truth gate 覆蓋。

> 2026-04-29 scheduled backup state-machine closeout：使用者插單的 `WORK-SCHED-STATE-A` 已完成並 append 到 `CHANGELOG.md`。BACKLOG 目前只有 blocked 的 `WORK-QA-GATE-B`，沒有可提升的未阻塞 work block。

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
  - 驗收：`docs/plan/scheduled_backup_audit.md`、`docs/plan/scheduled_backup_redesign_spec.md`、macOS legacy LaunchAgent high-priority fix、targeted scheduler / frontend tests、`bun run check`，以及 imagegen 設計稿輸出。
  - 驗收結果：`cargo test --manifest-path src-tauri/Cargo.toml -p vault-platform scheduler -- --test-threads=1` 通過；`bun run check` 通過（base checks、100% JS/Rust coverage、build、browser-preview e2e、desktop-bridge truth gate、desktop-contract mutation）。

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

> 2026-04-28 intelligence scope closeout：使用者插單的 `WORK-INTEL-SCOPE-A` 已完成並 append 到 `CHANGELOG.md`。source 現在修復 Settings / Maintenance sticky section nav 的 same-route hash click / initial hash scroll+focus；`/intelligence` 新增 `All time / 全部时间 / 全部時間` preset，首載仍維持 Month，deep link 使用 `?range=all`；secondary grid 改成 cache-aware progressive reveal，已 warm 的 card 不再等整批 secondary overview 完成才一起顯示；all-time preload/cache/invalidation follow-up 設計記在 [`docs/plan/intelligence-all-time-cache-invalidation.md`](intelligence-all-time-cache-invalidation.md)。

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

> 2026-04-27 UI progress closeout：使用者插單的 `WORK-UI-PROGRESS-A` 已完成並 append 到 `CHANGELOG.md`。source 現在把 Import / Backup archive-write progress 下沉到 shell-owned global task store：Import route 只顯示 handoff card，Jobs 是 canonical live progress / bounded console surface，sidebar footer 顯示 compact active archive task，topbar 則改成 persistent notification queue。頂部 global search 已移除；右側順序固定為 optional lock、notifications、ProfileSwitcher、Backup now。

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

> 2026-04-27 QA gate closeout：`WORK-QA-GATE-A` 已完成並 append 到 `CHANGELOG.md`。`bun run check` 與 `bun run verify` 已回綠；`BACKLOG.md` 新增 blocked `WORK-QA-GATE-B`，專門保留未來 full JS/Rust mutation deep sweep 與 survivor closeout。

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
> 2026-04-22 backend closeout：`WORK-BE-A` 已完成。這輪把 import boundary 真正收進 bounded-memory / streamed contract，並完成 `intelligence_runtime` 與 `intelligence/mod.rs` 的第三輪 giant-file 拆分；最新 execution slice 又把 structural rebuild internals 拆成 `intelligence_structural_{state,build,aggregates,persist,stream,stage}.rs`，讓 `intelligence/mod.rs` 再降到 `5561` 行，且所有新文件都回到 `600` 行硬限制內。下一輪 backend active current-focus 轉到 `WORK-BE-B`，專門收剩餘 query/read-model helper clusters，以及 `vault-worker/src/intelligence.rs` / `ai.rs` 的 follow-through。
> 2026-04-23 M13-B closeout：`WORK-M13-B` 已完成 shell runtime owner、Security workflow owner、Dashboard fallback owner、Browsing Rhythm state owner、以及 Import workflow follow-through；最後的 legacy `PathRow` 候選經 repo search 確認已無 active component / consumer，實際 owner 是 `ReviewPathActionRow`。`BACKLOG.md` 目前沒有可提升的未阻塞 work block。
> 2026-04-23 backend progress audit：live tree scan 證明後端主戰場已大幅拆完，但不能宣稱整個 backend「屎山優化完成」。production Rust 仍有 `src-tauri/src/dev_ipc_bridge.rs` (`1141` 行) 超過 1000 行、`host_artifacts.rs` / `browser-history-parser::chromium` / `vault-platform::scheduler` 等 800-1000 行 follow-up 候選，且 command / worker-bridge intelligence façade 還沒有達到原本 declaration-level rustdoc 標準。這輪先完成 `WORK-BE-D`，把 `vault-core/src/ai_queue.rs` 內嵌 regression suite 下沉到 `ai_queue/tests.rs`，runtime module 降到 `768` 行；下一個 active current-focus 轉到 `WORK-BE-E`。
> 2026-04-24 backend command-mirror closeout：`WORK-BE-E` 已完成。dev-only bridge 現在由 `dev_ipc_bridge/{config,router,payloads,dispatch}` 分別 owning env parsing、HTTP/CORS/error envelope、camelCase DTO、以及 command dispatch；parent `src-tauri/src/dev_ipc_bridge.rs` 降到 `94` 行，dispatch owner 為 `764` 行，command strings / payload shape / worker export surface / localhost-only feature+env gate 均維持不變。`BACKLOG.md` 目前沒有可提升的未阻塞 work block。
> 2026-04-24 Safari Browser Direct stop-ship closeout：使用者明確指出 `/import` 的 Browser Direct Safari `History.db` 被送進 Takeout parser，這輪插單已以 `WORK-IMPORT-SAFARI-A` 收口並 append 到 `CHANGELOG.md`。新 truth 是 Browser Direct local DB 改走 `inspect_browser_history` / `import_browser_history`，Safari `History.db` 支援 preview / execute / re-import dedupe / import batch revert+restore / source-evidence + capability snapshot；Takeout command surface 保持不變。這是 user-directed import stop-ship block，不覆寫已完成的 `WORK-BE-E` 後端 command-mirror closeout。
> 2026-04-24 ChatGPT Atlas Browser Direct closeout：使用者要求把 ChatGPT Atlas 導入提高到 Chrome 完成度。這輪插單已以 `WORK-IMPORT-ATLAS-A` 收口：Atlas 現在是 Chromium-family adapter，macOS discovery root 為 `com.openai.atlas/browser-data/host/<profile>`，Browser Direct 走既有 `inspect_browser_history` / `import_browser_history`，UI / icon / i18n / public support truth 已補齊。current archive 驗證用本機 Atlas profile 完成 preview / import / re-import dedupe / revert / restore，最終 Atlas batch 已 restore 並保持可見；validation artifact 只記 schema / aggregate counts / time range，不記私人 URL / title。
> 2026-04-24 Perplexity Comet Browser Direct closeout：使用者要求把 Perplexity Comet 導入提高到 Chrome 完成度。這輪插單已以 `WORK-IMPORT-COMET-A` 收口：Comet 現在是 Chromium-family adapter，macOS discovery root 為 `~/Library/Application Support/Comet/<profile>`，Browser Direct 走既有 `inspect_browser_history` / `import_browser_history`，UI / icon / i18n / public support truth 已補齊。current archive 驗證用本機 Comet profile 完成 preview / import / re-import dedupe / revert / restore，最終 Comet 主 import batch 已 restore 並保持可見；validation artifact 只記 schema / aggregate counts / time range，不記私人 URL / title。
> 2026-04-24 Explorer favicon fallback closeout：使用者臨時插單的 `WORK-EXPLORER-FAVICON-A` 已完成。Explorer 主 `query_history` payload 仍不含 favicon bytes；visible-row lazy hydration 現在會帶 visit time，後端會先找不晚於該 visit 的 exact page icon，再依序嘗試同 host / 同 registrable domain fallback，並以 `favicons.page_host` / `page_registrable_domain` 索引與 `last_updated_ms <= visit_time` 保護大 archive 查詢與舊訪問紀錄的 icon 時間語義。這是 read-time fallback，不改寫 canonical visit / favicon facts，也不新增 parser family 或 Tauri command；schema migration 只加欄位 / 索引，新 ingest 寫 metadata，不在 archive open / bootstrap 時同步掃描舊 favicon rows。
> 2026-04-24 Import performance stop-ship closeout：使用者回報三個月資料導入後 app 崩潰且清資料再導入會卡死。這輪插單以 `WORK-IMPORT-PERF-A` 收口：Browser Direct execute path 不再把 full `TypedEvidenceBatch` / `native_entities` 留在 `StreamedHistory`，而是 parser batch 直接流入 `DeferredSourceEvidenceBuilder` spool；execute 完成後也不再二次 stream source DB 只為重算 preview range。Takeout / Browser Direct import finalization 改為只刷新本次 import batch 影響到的 URL-document FTS projection，不再在導入主路徑同步重建整個 `derived/history-search.sqlite`。

- [x] **WORK-IMPORT-COMET-A** — Perplexity Comet Browser Direct Import Completion
  - 讀先：
    `docs/features/archive.md`
    `docs/architecture/browser-support-and-adapter-playbook.md`
    `docs/architecture/desktop-command-surface.md`
    `TESTING.md`
    `docs/plan/m4-full-polish/release-readiness-runbook.md`
  - 目標：把 Perplexity Comet 當成 Chromium-family browser adapter 接入 discovery、backup metadata、Browser Direct import、Import route validated list、icon/i18n/support copy，並用 current archive 做完整 live validation。
  - 契約：不新增 parser family、不新增 Tauri command、不新增 `BrowserHistoryImportRequest` 欄位；Comet 只支援 macOS `~/Library/Application Support/Comet/<profile>/History` 與 Chromium sidecars such as `Favicons`；不得導入 Comet AI memory、Perplexity account / workspace data、chats、tabs、bookmarks 或 suggestions。
  - 2026-04-24 closeout：`vault-core::chrome` 新增 `comet` browser definition 與 macOS App Support root；Browser Direct / backup source-profile metadata 保留 `Perplexity Comet` product；Import route validated filter、`browser-icons`、onboarding support copy、i18n tests 與 Import route tests 已同步。
  - current archive validation：本機 Comet profile dry-run preview `587` candidates，首次 import `587` / duplicate `0`，re-import imported `0` / duplicate `587`，revert 後 batch visible `0`，restore 後 visible `587`；`source_profiles.browser_product = Perplexity Comet`，source-evidence batches / native entities 存在，import-batch audit artifact 存在。最終 live archive state 保持 Comet 主 import batch restored / visible。
  - 驗收：targeted Rust / Vitest slices、`bun run check`、`bun run build`

- [x] **WORK-IMPORT-ATLAS-A** — ChatGPT Atlas Browser Direct Import Completion
  - 讀先：
    `docs/features/archive.md`
    `docs/architecture/browser-support-and-adapter-playbook.md`
    `docs/architecture/desktop-command-surface.md`
    `TESTING.md`
    `docs/plan/m4-full-polish/release-readiness-runbook.md`
  - 目標：把 ChatGPT Atlas 當成 Chromium-family browser adapter 接入 discovery、backup metadata、Browser Direct import、Import route validated list、icon/i18n/support copy，並用 current archive 做完整 live validation。
  - 契約：不新增 parser family、不新增 Tauri command、不新增 `BrowserHistoryImportRequest` 欄位；Atlas 只支援 macOS `~/Library/Application Support/com.openai.atlas/browser-data/host/<profile>/History` 與 Chromium sidecars such as `Favicons`；不得導入 workspace data、chats、tabs、bookmarks 或 suggestions。
  - 2026-04-24 closeout：`vault-core::chrome` 新增 `atlas` browser definition 與 macOS host root；Browser Direct / backup source-profile metadata 保留 `ChatGPT Atlas` product；SQLite staging 補上 WAL/sidecar regression；Import route validated filter、`browser-icons`、onboarding support copy、i18n tests 與 Import route tests 已同步。
  - current archive validation：本機 Atlas profile dry-run preview `63` candidates，首次 import `63` / duplicate `0`，re-import imported `0` / duplicate `63`，revert 後 batch visible `0`，restore 後 visible `63`；`source_profiles.browser_product = ChatGPT Atlas`，source-evidence batches / native entities 存在，import-batch audit artifact 存在。最終 live archive state 保持 Atlas batch restored / visible。
  - 驗收：targeted Rust / Vitest slices、`bun run check`、`bun run build`

- [x] **WORK-BE-E** — Command Facade Rustdoc And Dev Bridge Boundary
  - 讀先：
    `docs/plan/backend-hotspot-decomposition.md`
    `docs/architecture/desktop-command-surface.md`
    `docs/architecture/module-boundary-map.md`
    `docs/architecture/tech-stack.md`
  - 目標：處理後端 progress audit 暴露的下一個真 hotspot：`src-tauri/src/dev_ipc_bridge.rs` 超過 1000 行，且 `src-tauri/src/commands/intelligence/*` / `src-tauri/src/worker_bridge/intelligence/*` 仍有大量 command façade declaration-level rustdoc gaps。優先把 dev-only localhost bridge 的 payload DTO、router/dispatch table、command adapters 拆成 focused owners，並補齊 command / worker bridge 檔頭與 declaration comments。
  - 契約：維持現有 Tauri command names、devtools-bridge command strings、request/response payload shape、worker export surface、feature-gated + env-gated localhost-only 安全邊界，以及 `run_blocking_command` off-main-thread contract；不得把 dev automation mirror 擴寫成產品 remote-control API。
  - 2026-04-23 progress：dev-only bridge payload DTO 已拆到 `src-tauri/src/dev_ipc_bridge/payloads.rs`，保留 camelCase JSON shape 與所有 command string；router / CORS / health / HTTP error envelope 已拆到 `dev_ipc_bridge/router.rs`，env parsing 已拆到 `dev_ipc_bridge/config.rs`，`src-tauri/src/dev_ipc_bridge.rs` 目前降到 `851` 行。`src-tauri/src/commands/intelligence/*` / `src-tauri/src/worker_bridge/intelligence/*` 也已補上檔頭與 declaration-level rustdoc。下一步仍需處理 dev bridge dispatch owner 邊界。
  - 2026-04-24 closeout：dispatch table / adapters 已抽到 `src-tauri/src/dev_ipc_bridge/dispatch.rs`，並把 session round-trip / unknown command regression 下沉到 `dispatch/tests.rs`。父檔現在只負責 feature-gated listener startup 與 state handoff；router 只呼叫 dispatch owner；payload DTO、command strings、updater/file-manager adapters 與 worker bridge implementation calls 均未改 contract。
  - 驗收：relevant targeted Rust regressions、`bun run check && bun run build`

- [x] **WORK-BE-C** — Remaining Backend Hotspot Decomposition Beyond Core Intelligence Parent
  - 讀先：
    `docs/plan/backend-hotspot-decomposition.md`
    `docs/architecture/data-model.md`
    `docs/architecture/module-boundary-map.md`
    `docs/architecture/desktop-command-surface.md`
    `docs/architecture/tech-stack.md`
  - 目標：把 backend 軌道剩餘的 giant-file 從 `core intelligence parent` 之外繼續往外拆，優先處理 `models/core_intelligence.rs`、`remote.rs`、`intelligence/site_dictionary.rs`，並把 `intelligence/mod.rs` 仍內嵌的 regression suite / support types 繼續下沉到 focused owners。
  - 契約：維持現有 Tauri command、worker CLI、serde payload、Core Intelligence DTO shape、visit-taxonomy classification semantics、remote bundle manifest/upload/verify contract、以及 `IntelligenceRuntimeSnapshot` 的 off-main-thread background task 邊界穩定；不得因為 giant-file 清理而重開已接受的 `/intelligence` route / payload grammar。
  - 2026-04-22 backend hot-spot reset：`WORK-BE-B` 已完成。最新 execution slice 把 `intelligence/mod.rs` 的 residual shared helpers 拆成 `intelligence_{shared,visit_records,visit_derive,daily_rollup_state,daily_rollups,core_persist}.rs`，讓 visit-derived stage、daily-rollup stage、shared date/query heuristics、與 scoped full-rebuild persistence 都有獨立 owner。`intelligence/mod.rs` 現在降到 `2583` 行，只剩 exported surface、core record types、batch cursors、常數與 regression suite；worker façade (`124` 行) 與 `ai.rs` (`199` 行) 也都已完成 follow-through。
  - 2026-04-23 visit-taxonomy slice：原 `deterministic` module 已改名並拆成 `visit_taxonomy/{mod,types,url,text,rules,classification,tests}.rs`，保留 `crate::visit_taxonomy::*` façade 與既有 taxonomy / URL / tokenization semantics；最大新 owner 是 `rules.rs` (`535` 行)。下一刀優先 `intelligence/site_dictionary.rs`，再接 `models/core_intelligence.rs`、`remote.rs` 與 `intelligence/mod.rs` regression suite。
  - 2026-04-23 site-dictionary slice：`intelligence/site_dictionary.rs` 已拆成 `site_dictionary/{mod,types,overrides,search_rules,classification,tests}.rs`，維持 search rule / override schema、Settings payload、visit classification 與 search-query extraction semantics；最大新 owner 是 `search_rules.rs` (`436` 行)。下一刀改為 `models/core_intelligence.rs`，再接 `remote.rs` 與 `intelligence/mod.rs` regression suite。
  - 2026-04-23 Core Intelligence DTO slice：`models/core_intelligence.rs` 已拆成 `core_intelligence/{mod,shared,requests,reads,analytics,overview,exports,tests}.rs`，維持所有 serde field/tag/alias shape 與 `vault_core::*` re-export surface；最大新 owner 是 `analytics.rs` (`376` 行)。下一刀改為 `remote.rs`，再接 `intelligence/mod.rs` regression suite。
  - 2026-04-23 remote-backup slice：`remote.rs` 已拆成 `remote/{mod,bundle,manifest,transfer,verify,tests}.rs`，維持 `preview/run/verify` public façade、bundle manifest、curl upload 與 restore-verification DTO contract；bundle build / verify 也改成 chunked SHA + zip streaming，避免大 SQLite payload 被整檔載入記憶體。下一刀改為 `intelligence/mod.rs` 內嵌 regression suite / support-type 下沉。
  - 2026-04-23 regression-suite closeout：`intelligence/mod.rs` 內嵌 regression suite 已下沉到 `intelligence/tests/{schema_overview,stage_rebuild,structural_incremental,batch_equivalence,fixtures}.rs`，parent module 降到 `418` 行，只剩 module map、public façade、core records、batch cursors 與 constants；最大新 test owner 是 `stage_rebuild.rs` (`601` 行)。`WORK-BE-C` 後端支援檔拆分範圍已完成。
  - 驗收：relevant targeted Rust regressions、`bun run check && bun run build`

- [x] **WORK-M13-B** — Shared Support / Workflow Composition Extraction
  - 讀先：
    `docs/plan/m13-broad-reuse-audit/README.md`
    `docs/design/screens-and-nav.md`
    `docs/design/ux-principles.md`
    `docs/plan/e2e-workflow-tests.md`
  - 目標：根據 `WORK-M13-A` 的 inventory，把至少一輪高價值的 support / trust / workflow composition 抽離，優先處理 Jobs plugin/module summary、workflow follow-through 與剩餘 support summary drift。
  - 契約：只抽明確跨 consumer 重複且能降低 drift 的 grammar；不得為了抽象而重開 M6–M12 已收斂的 route / payload / review / support-action contract。
  - 2026-04-21 progress：shared runtime-boundary card grammar 已落到 `src/components/review/runtime-boundary-card.tsx`，Jobs runtime health / plugin / module summary 與 Settings derived runtime review 是第一批 consumer；Jobs route shell 也因此降到 `1000` 行以下。下一輪優先處理 shell-data owner split、Security / Import workflow follow-through、Dashboard fallback owner 與 `Browsing Rhythm` layering smell。
  - 2026-04-22 Import workflow slice：`/import` 現在已把 `new import wizard -> grouped scan report -> recent imports / selected batch / doctor repair` 的閱讀順序落地，並直接吃 backend 新增的 `will-import / known-but-ignored / needs-review / parse-error` file classification、detected locale 與 preview time range；Takeout UI 不再只把檔案全塞進一個雜亂 preview list，而是能說清楚目前 shipping 的 Chrome-first scope。
  - 2026-04-23 shell runtime owner slice：`src/app/shell-data.tsx` 已把 shared AI queue / Core Intelligence runtime refresh、in-flight dedupe、active/idle polling cadence 下沉到 `src/app/shell-runtime-status.ts`；`useShellData()` 對外 shape 不變，Jobs / Sidebar / digest 仍走 shell shared runtime source。下一步改看 Security workflow follow-through、Dashboard fallback owner 與 `Browsing Rhythm` layering smell。
  - 2026-04-23 Security workflow owner slice：`src/pages/security/index.tsx` 已把 posture load、unlock/keyring、lock 與 rekey mutation state machine 下沉到 `src/pages/security/use-security-workflow.ts`；route shell 現在只保留 fallback、deep-link focus、path-copy feedback 與 panels composition。下一步改看 Dashboard fallback owner 與 `Browsing Rhythm` layering smell。
  - 2026-04-23 Dashboard fallback owner slice：Dashboard 的 bootstrap error path 已把 Security status probe 下沉到 `src/pages/dashboard/route-fallback-access.ts`，fallback resolver / renderer / archive-access probe 現在同屬 Dashboard fallback owner；route shell 不再直接知道 Security DTO 的 fallback fields。下一步改看 `Browsing Rhythm` layering smell。
  - 2026-04-23 Browsing Rhythm state owner slice：shared calendar card 已把 discovery-trend load、selected-year / selected-day state、range summary 與 lazy day-preview 下沉到 `src/components/intelligence/browsing-rhythm-card-state.ts`；`BrowsingRhythmCard` 現在回到 Dashboard / `/intelligence` 共用 render shell。下一步只剩 legacy `PathRow` retirement 候選。
  - 2026-04-23 PathRow retirement audit：`src/components/ui.tsx` 已無 `PathRow` export，repo 內也沒有 active `PathRow` consumer；M12 推出的 `ReviewPathActionRow` 已是 path/copy/open grammar 的唯一實作 owner，因此這一項收口為 stale-planning cleanup 而不是新增代碼。
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
  - 驗收：targeted Rust progress tests、targeted Vitest import/dashboard/i18n tests、`bun run check`、`bun run build`、fresh desktop Computer Use truth pass。

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
