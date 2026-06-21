# STATUS.md — 當前工作

> Agent 每次開工讀這個檔案。一次只做第一個 `[ ]` work block；不要把 `STATUS.md` 再拆回原子 task。

**當前 Milestone：M17 — AI Integration（AI-redesign-2026 實作）**
（M16 v0.3 Paper Redesign 的 route-sweep 收尾為 carryover，見下方 `WORK-V03-PAPER-REDESIGN-A`，不阻擋 AI 工作）

---

## CURRENT FOCUS

> 這裡的單位是 **work block**，每個 block 的份量大約是半個 milestone。
> work block 內可以包含多個子任務、ADR、代碼變更與文檔同步，但只有整塊達成可驗收成果時才改成 `[x]`。
> `STATUS.md` 通常只維持 1-2 個 work blocks。commit 仍保持可 review，不要求「一個 work block = 一個 commit」。

- [x] **WORK-AI-0-FOUNDATIONS** — AI traits / model-agnostic config / storage planes / secrets（無模型呼叫）
  - 讀先（**第一份必讀**）：
    `docs/plan/program/ai-redesign-2026/04-current-state-and-execution.md`（現實對齊 + 執行序 + review 協議 + file anchors）
    `docs/plan/program/ai-redesign-2026/02-architecture-decisions.md`（§0 D1-D8、§A storage planes、§B、§C、§I 供應鏈 ledger）
    `docs/plan/program/ai-redesign-2026/03-implementation-plan.md`（M-AI0）
    `src-tauri/crates/vault-core/src/ai.rs` / `ai/provider.rs`（rig 用法、D4 硬編 dim 在 `provider.rs:445-528`）
    `src-tauri/crates/vault-core/src/models/intelligence.rs:250-341`（`AiProviderConfig`/`AiSettings`，已 model-agnostic 形狀）
    `src-tauri/crates/vault-core/src/config.rs:26-135`（paths + `ensure_paths`）
    `src-tauri/crates/vault-core/src/intelligence/incremental.rs`（watermark = embedding fingerprint 範本）
    `docs/plan/program/quality-matrix.md`（權威 gate；100% JS/Rust coverage、無 exclusion）
  - 目標：立穩 AI 重做的所有後端邊界，**不做任何新模型呼叫**，關 AI 時 app 行為完全不變；既有 12 命令 + `AppSnapshot` AI 欄位契約不破。
  - work（逐項皆需 100% 覆蓋 + 過 review pipeline）：
    1. 在 `vault-core` 定義 `LlmProvider` / `EmbeddingProvider` / `VectorIndex` traits（純 trait + 型別，rig 型別只能出現在 adapter 內）。
    2. **修 D4 違反**：`provider.rs` 的硬編 1536/768 dim fallback 改成**讀實際回傳向量長度**（02 §C.3 鐵律 a）；dim/pooling/normalized/instruction 改成 per-provider 描述符（runtime 偵測）。
    3. API key in-memory 處理改 `secrecy::SecretString`（drop 清零 / log redact / 不序列化進 trace/sidecar）；沿用既有 keyring + `api_key_saved`。
    4. 新 storage planes：`config.rs` 的 `ProjectPaths` 加 `vectors_dir`（`derived/vectors/`）、`agent_database_path`（`derived/agent.sqlite`）、`models_dir`（`<root>/models/`）；`ensure_paths` 建立；更新 path-override 測試。
    5. embedding fingerprint 結構：`hash(provider, model_id, effective_dim, output_dtype, normalized, pooling, instruction_template, version)`，鏡像 intelligence checkpoint/watermark；先落型別 + 持久化 header，stale 偵測接口（尚不觸發重嵌）。
  - 契約：rig/secrecy 用 `name.workspace = true`（rig **不 hard-pin**）；`secrecy` 加入後 `deny:rust` / `audit:rust` 須綠（MIT/Apache 過 license）；新 `.rs` 全進 100% 覆蓋。**現有 AI/semantic 代碼全是 placeholder（使用者 2026-06-20），可自由重做——不需保留舊 AI 命令/型別/snapshot 欄位形狀**；只需不破壞非-AI 功能與非-AI preview fixture；改動 `src/main.tsx`/`bridge.ts` 需對齊既有 generic IPC contract（本 block 不需碰）。
  - 驗收：`bun run check:base` 綠（Rust fmt/clippy/test + supply-chain + i18n + typecheck）；`coverage:rust` 對新 `.rs` 100%；關 AI 時 deterministic intelligence + FTS5 行為不變；走完 review pipeline（find → verify → fix）。
  - 2026-06-20 closeout：交付於 `feat/ai-redesign-2026`（commit `c417f36a`）。實作 subagent + 4 獨立 finder + 1 對抗 verifier 的 review pipeline 跑完。修掉的 confirmed findings：Gemini `None`-dim 走 rig 仍硬編 768 的 D4 殘留（HIGH，對照 rig 0.34 源碼驗證 → 新增 `resolve_embed_request_dim`，Gemini 無顯式 dim 直接 bail）；`l2_normalize` denormal 飽和 + 非有限 norm 測試（殺 is_finite mutant）；fingerprint hash 文檔修正 + collision-lock + golden-vector 測試；`from_descriptor` 全欄位斷言；移除過廣的 `secrecy::self` re-export；`models/` 補進 `EXPORT_EXCLUSIONS_DOC`。**Carryover → W-AI-4**：descriptor 的 dtype/normalized 改 per-adapter；兩個 `EmbeddingProvider` 用 `enum AnyEmbeddingProvider`（非 `Box<dyn>`）。**Carryover → W-AI-1**：`LlmChatRequest/Response` 補 tool defs / structured-output / `tool_call_id` / `usage`（皆 additive）。Gate：clippy(-D warnings)/test(732 pass，僅已知 macOS `/dev/shm` 失敗)/verify-rust-coverage full（AI 檔 100%）/deny/audit 全綠。

- [ ] **WORK-AI-1-LLM-STREAMING** — 串流式 external LLM transport（`LlmProvider` rig adapter + Tauri 串流事件 + LLM functions 降級）
  - 讀先：`docs/plan/program/ai-redesign-2026/04-current-state-and-execution.md`（§1 placeholder 自由重做、§3 W-AI-1 行、§4 LM Studio、§5 review、§7 IPC/streaming anchors）；`02-architecture-decisions.md` §B；`src-tauri/crates/vault-core/src/ai/traits.rs`（W-AI-0 trait + carryover）、`ai/provider.rs`（`run_llm_agent` rig 用法）；`src/lib/ipc/import-progress.ts`（`listen` 範本）、`src-tauri/src/commands/import.rs:31`（`emit` 範本）、`dev_ipc_bridge/dispatch.rs`（off-thread hop）。
  - 目標：實作 `LlmProvider`（單一 `RigLlmProvider` struct 內部分流 openai/anthropic/gemini）的 `chat` + `chat_stream`；`chat_stream` 把 `LlmStreamChunk`（token / reasoning / tool-call）經 `pathkeep://ai-stream` Tauri event 串到前端；capability/connection probe 補 streaming；第一批 LLM functions（topic/query summary 等）無 provider 時退 deterministic。**現有 assistant 命令/型別是 placeholder，可自由重設計成串流式**（不保留舊 job-polling 形狀）。對 LM Studio（gemma-4-26b）跑真機 e2e。
  - 契約：rig 型別只在 adapter 內；`LlmChatRequest/Response` 依 W-AI-0 carryover 補 tool/usage 欄位（additive）；real 網路路徑 `#[cfg(not(any(test, coverage)))]` + 確定性 stub `#[cfg(any(test, coverage))]`（沿用 W-AI-0 pattern，保 100% 覆蓋）；不破壞非-AI 功能。
  - 驗收：對 LM Studio 真機 streaming chat（token + reasoning 可見）跑通；無 provider 時 functions 退 deterministic；`bun run check:base` + `coverage:rust` 綠；走完 review pipeline。

- [ ] **WORK-V03-PAPER-REDESIGN-A** — Paper + Archival Frontend Rebuild (foundation shipped, route sweep pending) — *carryover，非當前 focus*
  - 讀先：
    `docs/design/handoff/README.md` (handoff index)
    `docs/design/handoff/paper-redesign/README.md` (cover sheet from design tool)
    `docs/design/handoff/paper-redesign/project/pk-tokens.css` (visual rule book — 3,978 lines)
    `docs/design/handoff/paper-redesign/project/PathKeep Redesign.html` (entry composition)
    `docs/design/handoff/paper-redesign/project/pk-components.jsx` (PKSidebar / PKStatusBar / PKDetailPanel / PKSearchPalette / PKHeatmap)
    `docs/design/handoff/paper-redesign/project/pk-views.jsx` (HomeView / Dashboard editorial layout — already shipped)
    `docs/design/handoff/paper-redesign/project/pk-contactsheet.jsx` (Browse: contact sheet, day sticky, sessions, domain stacks)
    `docs/design/handoff/paper-redesign/project/pk-browse-nav.jsx` (CalendarPopover / DayNavControl / YearRail / archive density)
    `docs/design/handoff/paper-redesign/project/pk-search.jsx` (3-mode search hero + day-grouped results)
    `docs/design/handoff/paper-redesign/project/pk-intelligence.jsx` (KPIs / topics / domains / sessions / refind)
    `docs/design/handoff/paper-redesign/project/pk-assistant.jsx` (chat + evidence panel)
    `docs/design/handoff/paper-redesign/project/pk-import.jsx` (method picker + wizard stepper)
    `docs/design/handoff/paper-redesign/project/pk-audit.jsx` (manifest chain + runs + storage + snapshots)
    `docs/design/design-tokens.md` (will be rewritten)
    `docs/design/screens-and-nav.md` (will be rewritten)
    `docs/design/ux-principles.md` (will be rewritten)
    `src/styles/tokens.css`
    `src/components/shell/`
  - 目標：把 v0.2 brutalist 前端徹底替換為 "Paper + Archival" 美學（cream 紙感、Newsreader serif + JetBrains Mono、3 px 圓角、paper noise、darkroom vignette），全部接入既有 Rust/Tauri 2 後端。同時新增 per-URL notes + tags 後端能力與 AI Assistant / 語意搜尋的 provider-gated 真實接入。
  - 契約：
    - 全部 user-visible copy 在 commit 時三語齊全（en / zh-CN / zh-TW），`html[lang]` 必須跟著 runtime locale 走（typography-and-font-fallback ADR）。
    - 字體預設使用 bundled Newsreader + JetBrains Mono Latin subsets，Settings 提供 "system fonts only" 切換；CJK 永遠 fall back 到系統字體。
    - 100% JS / Rust coverage 與 mutation gate 不放鬆；既有 quality-matrix.md 仍是權威。
    - 此 redesign 已獲使用者授權 override 之前 Accepted design docs（design-tokens / screens-and-nav / ux-principles / brutalist radius / typography memory）。
    - 後端只追加 url_annotations + url_tags table，現有 schema / commands 不破壞；migration 011 forward-only。
  - 進度（2026-05-22 update — Browse UX second pass closed out）：
    - ✅ **Year-rail removed**：`paper-year-rail*` files + `paper-contact-sheet-year-rail` testid + `yearRail*` i18n / `PaperExplorerCopy` keys all deleted. Day-nav pill + calendar popover are now the authoritative jump UI; global `::-webkit-scrollbar` bumped 6 → 12 px with 32 px min-thumb so the viewport scrollbar carries the fast-scroll role (`d36bf87`).
    - ✅ **Browse infinite scroll**：`dateFiltered` no longer flips `infiniteDisabled`; both card and list mode infinite-scroll under `?date=YYYY-MM-DD` too. Search surface / grouped views still use pagination.
    - ✅ **Day insights enriched**：collapsible "More details" disclosure adds first/last visit, peak hour, longest session, top-3 revisited URLs. `aggregateDayInsights` exposes the new fields; `localHourOf` now handles both ms-precision and second-precision `visitTime` so peak hour no longer spuriously lands on hour 0 (`8cf5c6f`).
    - ✅ **List-row icon precedence**：`favicon → og:image (square crop) → swatch`. Pure render-time fallback over data the og:image hook already buffered (`feb6e6f`).
    - ✅ **Detail panel slide-in**：`paper-detail-slide-in` keyframes + `paper-detail-backdrop-in` fade added; panel + scrim use `motion-safe:animate-[...]`. 220 ms cubic-bezier, GPU translate3d; respects `prefers-reduced-motion`.
    - ✅ **12 h clock default + Settings toggle**：every non-chart time stamp now reads as `3:14 PM` / `上午 3:14`; `pathkeep.clockFormat` persists choice, `CLOCK_FORMAT_EVENT` flushes through live routes. Sparkline axes stay 24 h.
    - ✅ **Day-nav pill localized**：`dow / monthDay / year` formats with the active i18n language instead of hard-coded `en-US`.
    - ✅ **Browse view-mode persisted**：`pathkeep.explorerViewMode` localStorage; defaults to cards, survives reload.
    - ✅ **Backup progress non-blocking**：new `BackgroundProgress` strip renders above the status bar gated by `BusyOverlayState.background = true`; archive unlock + truly-blocking actions keep the modal.
    - ✅ **og:image fetcher headers**：real desktop Chrome User-Agent + `Accept-Language: en-US,en;q=0.9,zh;q=0.6`, follows up to 8 redirects (was 1), 15 s / 10 s timeouts. Privacy posture unchanged (no Referer / cookies / fingerprinting headers).
    - ✅ **Authoritative gate**：full `bun run check` (base + 100 % JS / Rust coverage + desktop-contract mutation + e2e + desktop-bridge truth + release-check) green. The carryover Rust coverage residuals in `og_images_fetch.rs`, `archive_flows.rs`, and `read_models.rs` are closed via `da007f6` — collapsed the dead `Selector::parse(Err)` and `None` `match` arms, extracted `BodyPhase` / `fetch_status_for_body_error` and `drain_one_worker_url` / `record_refetch_outcome` / `finalize_refetch_run` / `lock_or_recover` helpers with focused unit tests, and added a TcpListener-backed test that aborts a chunked image body mid-chunk so the body Io error path fires from a real reqwest read. Coverage scope reports 100 % across 31,762 instrumented Rust lines / 1,501 functions with no exclusions.

  - 進度（2026-06-14 update — 全面審查流水線 + confirmed-finding 修復）：
    - ✅ 跑完 `docs/plan/program/review-pipeline.md` 的 4 階段流水線（13 審查 sub-agent → 144 findings → 對抗性驗證 5 sub-agent → 13 confirmed + 8 trade-off，50% 存活率）。產出在 `docs/review/2026-06-14/`，最終報告 `phase-4/final-report.md`。
    - ✅ 全部 confirmed finding 已修復，每個 fix 都配一個獨立 strict-review sub-agent（fresh context）覆核並通過：R-REGEX（regex recall 不再全表載入記憶體，改 50k bounded streaming window）、R-ASYNC（37 個 Core Intelligence 命令改 off-main-thread async）、R-VISITIDS（refind visit_ids 改 50-entry reservoir + exact total）、R-LOADVISITS（SQL LIMIT 下推）、R-HASH（takeout key 改 SHA-256）、F-TOKENS（ink ramp 過 WCAG AA + design-tokens.md 對齊）、F-DASHBOARD（This Week 顯示真實週數據 + Active Threads deep-link）、F-JOBS（AI job handler 補 catch）、F-ROUTER（9 路由補 ErrorBoundary）、F-IMPORT（onboarding 改用 backend-client）、F-LEGACY-CSS（Assistant/Audit 轉 paper）、X-VISITSUMMARY（detail panel first/last visit 誠實化）。詳見 CHANGELOG `WORK-REVIEW-0614-FULL-PIPELINE-A`。
    - 🟡 Trade-off（Longevity）：R-HASH 改 hash 會讓跨版本 takeout 去重斷層（pre-fix 與 post-fix 的 key 不相容），已在 `import-dedup-audit.md` §B5 與代碼註解記錄；pre-1.0 可接受。
    - 🟡 驗證：`tsc -b` clean；vitest 2219/2219；i18n parity 100%；`cargo check --workspace --tests` clean；`cargo test --workspace --lib` 777 passed（唯一 fail 是已知 macOS `/dev/shm` migration 測試，非回歸）。完整 `bun run check`（rust coverage + e2e + mutation）留待 Linux CI gate。非阻塞 polish 與 deferred backend per-URL visit summary 記在 BACKLOG `WORK-REVIEW-0614-FOLLOWUPS-A`。

  - 進度（2026-05-25 update — feedback-2026-05-25 §3.2 + §3.3 B closeout）：
    - ✅ **Search shell + zero-result hijack (§3.2)**：`/search` route now mounts ExplorerPage directly instead of `Navigate replace="/explorer?surface=search"`. Loss of route handle after the redirect was silently flipping the sidebar's active tab back to Browse and the topbar title to "歷史記錄". ExplorerPage detects `pathname === '/search'` and treats it as the search surface alongside the existing `?surface=search` param. The full-screen `EmptyState` / `ErrorState` / regex `StatusCallout` hijacks no longer fire on the search surface — they render as an in-place `aboveResultsCallout` slot inside PaperSearchPanel below the hero so the composer never unmounts (the user who typed a misspelt query can keep editing). PaperSearchView's existing "memory is patient" italic-serif copy now actually surfaces. New `PaperSearchPanelAboveResultsCallout` prop + 4 explorer-route tests + 2 paper-search-panel tests cover the new behaviour. Router-structure test updated for the lazy `/search` route.
    - ✅ **Advanced syntax hover hint (§3.3 B)**：new `PaperAdvancedSearchHelp` popover renders a `?` chip next to the mode toggle in PaperSearchHero. Reuses the existing `explorer.advancedSearchHelp*` i18n keys for `en` / `zh-CN` / `zh-TW` (kept after the v0.2 legacy retire). Examples: `site:github.com -pathkeep`, `"release notes"`, `manual OR youtube`, `intitle:manual inurl:docs`, `filetype:pdf after:2026-05-01 before:2026-05-07`. Hover (mouse) and focus (keyboard) both open the popover; `aria-describedby` + `role="tooltip"` keep it accessible. New paper-search-hero test covers visibility lifecycle; paper-explorer-copy parity guard updated to walk the nested `advancedSyntaxHelp` bag separately.
    - 🟡 **Remaining feedback-2026-05-25 items** (each warrants its own dedicated work block, see BACKLOG):
      - `WORK-FEEDBACK-0525-DAY-INSIGHTS` — bug §3.1 day insights chart only aggregates loaded cards.
      - `WORK-FEEDBACK-0525-TAG-NOTE-SEARCH` — bug §3.3 A search must support tag/note dimensions.
      - `WORK-FEEDBACK-0525-S3-REMOVE` — feature §2.2 sweep S3 cloud backup out of FE/BE/tests/docs.
      - `WORK-FEEDBACK-0525-BROWSE-VIRT` — perf §1.1 + §1.2 sliding-window DOM recycling + directional prefetch.
    - 🟡 **Tests / gate**：`bunx tsc --noEmit` clean. 752/752 tests across `src/app` + `src/components/explorer-paper` + `src/pages/explorer` + `src/app/index-tests/router-structure.test.tsx` pass. Full `bun run check` deferred to next session along with the remaining feedback work blocks above.

  - 進度（2026-05-25 update — feedback-2026-05-25 §2.1 closeout: full app data Export/Import shipped）：
    - ✅ **New backend module** `vault-core/src/migration.rs` (700+ lines, 8 unit tests). Packs the entire project tree (config, archive databases via `sqlcipher_export`, derived projections, audit ledger, raw snapshots, intelligence + semantic sidecars) into a single `.pathkeep-bundle` zip. Unpacks the bundle back with per-file sha256 verification, manifest tamper detection, and forward-migration of the archive schema. Pack/unpack stream through a 64 KiB buffer so peak memory stays bounded regardless of archive size — the hot path for 14M-row archives.
    - ✅ **Bundle format v1** (`pathkeep-export-manifest.json` + `.sha256` sidecar + per-file entries). Manifest carries `formatVersion`, `appVersion`, `archiveSchemaVersion`, `archiveMode`, `exportedAt`, `exporterHostname`, and `files[{path,sha256,sizeBytes}]`. New `archive::max_schema_version()` exposed so `preview_import` can refuse bundles produced by _newer_ PathKeep builds the local binary can't migrate forward.
    - ✅ **Explicit include / exclude lists**. Included subtrees: `derived/`, `audit/`, `raw-snapshots/`, `sidecars/`. Excluded: `vault.hold`, `stronghold-salt.txt` (App Lock secrets stay on source), `logs/`, `diagnostics/`, `schedule/` (platform-specific scheduler artifacts), `staging/`, `quarantine/`, `exports/`. Surfaced to the UI via `EXPORT_EXCLUSIONS_DOC` so the import-preview panel tells the user what _won't_ migrate.
    - ✅ **Atomic-ish apply**. Apply renames every overwritable target to a sibling `.bak-<timestamp>` directory before moving staged subtrees into place, so a user who imported the wrong bundle can recover. Returns `preservedPreviousAsBak: true` whenever the previous tree was preserved.
    - ✅ **Three new Tauri commands** (off-main-thread via `run_blocking_command`):
      - `export_app_data(targetPath)` → `ExportedBundle`
      - `preview_app_data_import(bundlePath)` → `ImportPreview` (read-only; safe to call repeatedly)
      - `apply_app_data_import(bundlePath, { confirmOverwrite })` → `ImportResult`
        Wired through `vault-worker::migration`, `worker_bridge::migration`, `commands::migration`, `dev_ipc_bridge` dispatch arms + payload envelopes, and registered in the top-level `tauri::generate_handler!`.
    - ✅ **Settings → Data migration section** (`pages/settings/data-migration-section.tsx`). Mounted between Cloud backup and Link previews. Export action: native save dialog (`@tauri-apps/plugin-dialog`) → backend call → success banner with bundle path + size + file count. Import action: native open dialog → backend preview → inline PME panel showing exported-at / hostname / source app version / archive schema (current vs. migrate-forward count) / archive mode / payload size / overwrite warning / exclusion list → Confirm executes apply, errors stay in-place so the panel doesn't unmount mid-retry.
    - ✅ **i18n parity 100%**. 35 new keys × 3 locales (`en` / `zh-CN` / `zh-TW`) added to `settings-remote-and-outputs.ts`. Catalog now 2903 keys per locale. New SettingsSectionKey `'migration'` with anchor id `settings-migration`.
    - ✅ **Tests**: 8 Rust unit tests (`export_and_reimport_roundtrip_preserves_manifest_and_files`, `preview_rejects_bundle_with_newer_archive_schema`, `preview_rejects_bundle_with_unknown_format_version`, `preview_rejects_tampered_manifest`, `preview_detects_will_overwrite_when_target_already_initialized`, `apply_import_refuses_overwrite_without_explicit_confirm`, `apply_import_preserves_previous_tree_as_bak`, `max_schema_version_matches_latest_migration`) + 9 frontend tests (`data-migration-section.test.tsx` covers idle render, export happy path / dialog cancel / error, import preview happy path / error, apply happy path / error retains preview, cancel returns to idle). 1902/1902 JS unit + 575 Rust unit tests pass.
    - 🟡 **Followup not in this slice**: re-keying archive with a _different_ key on import (current flow preserves source key, user can rekey afterwards via existing Settings → Security); progress events streaming for very large bundles (current UI shows in-flight spinner state, no incremental bytes-written). Both are recorded for the next pass but not user-blocking.

  - 進度（2026-05-25 update — feedback-2026-05-25 §2.2 closeout: S3 cloud backup retired）：
    - ✅ **Backend deletion**: `vault-core/src/remote/` directory (bundle.rs / manifest.rs / mod.rs / tests.rs / transfer.rs / verify.rs) deleted; `vault-core/src/models/remote.rs` (`RemoteBackupConfig` / `S3CredentialInput` / `RemoteBackupPreview` / `RemoteBackupResult` / `RemoteBackupVerification`) deleted. `AppConfig` no longer carries `remote_backup`; `BackupReport` no longer carries `remote_backup: Option<RemoteBackupResult>`. `vault-core::archive::backup`/`maintenance` updated to drop the field.
    - ✅ **Worker deletion**: `vault-worker::archive_flows` lost `preview_remote_backup_bundle` / `upload_remote_backup_bundle` / `verify_remote_backup_bundle` plus the post-backup S3 upload limb (was injecting `Remote backup is enabled, but S3 credentials are not stored…` warnings); `vault-worker::security` lost `store_s3_credentials` / `clear_s3_credentials`; `vault-worker::cli` lost the `remote-backup` CLI verb; `vault-worker::context` no longer hydrates `remote_backup.credentials_saved`.
    - ✅ **Platform deletion**: `vault-platform::keyring` lost `keyring_get_s3_credentials` / `keyring_set_s3_credentials` / `keyring_clear_s3_credentials` / `s3_credentials_saved` and the `KEYRING_S3_USER` slot. `native_host` integration test drops the S3 roundtrip assertions.
    - ✅ **Tauri façade deletion**: `src-tauri/src/commands/{remote.rs}` and `worker_bridge/remote.rs` removed; `commands/security.rs` lost `store_s3_credentials` / `clear_s3_credentials`; `tauri::generate_handler!` shed five command names; `dev_ipc_bridge` dispatch arms + `CredentialsPayload` envelope + dispatch coverage tests all removed.
    - ✅ **Frontend deletion**: `src/lib/backend-client/remote.ts` and `src/lib/types/remote.ts` deleted; `backend-client/index.ts` + `backend.ts` lost the five `previewRemoteBackup` / `runRemoteBackup` / `verifyRemoteBackup` / `storeS3Credentials` / `clearS3Credentials` methods; browser-preview `backend-preview-{state,support,workflow-commands,shell-commands}.ts` purged of `s3Credentials`, `lastRemoteBundlePath`, `previewRemoteBackupFixture`, `verifyRemoteBackupFixture`, `remoteBundlePath` / `remoteObjectKey` / `remoteUploadUrl` helpers; `normalizeMockConfig` signature simplified.
    - ✅ **Settings UI deletion**: `pages/settings/remote-backup-section.{tsx,test.tsx}`, `remote-backup-preferences-section.{tsx,test.tsx}`, `use-settings-remote-state.{ts,test.tsx}` deleted; `SettingsSectionKey` lost the `'remote'` member and `settings-remote` anchor id; Maintenance route no longer mounts the remote PME card; Settings nav arrays trimmed across `settings/index.tsx`, `maintenance/index.tsx`, `section-nav-items.{ts,test.ts}`, `section-nav.test.tsx`, and `intelligence-surfaces/settings-core-sections.test.tsx` (the former `walks the maintenance remote backup PME and derived-state controls` test became `walks the maintenance derived-state controls`).
    - ✅ **Tests cleanup**: ~96 lines of `remoteBackup: null,` / `remoteBackup: {…}` / `store_s3_credentials` / `previewRemoteBackup` / `verifyRemoteBackup` assertions stripped across shell-data / app-index / backend-client / preview-smoke / preview-workflows / tauri-passthrough / intelligence-surfaces tests. 1890 / 1890 unit tests pass; 64 / 64 vault-worker tests pass.
    - ✅ **i18n cleanup**: 59 unused `remoteBackup*` / `s3Compatible` / `previewRemoteBackup` / `bucketLabel` / `manualBoundary*` / etc. keys × 3 locales removed from `settings-remote-and-outputs.ts`. Catalog now sits at 2844 keys per locale (was 2903); parity stays 100%.
    - ✅ **Docs sweep**: `docs/features/archive.md` § 遠端備份 rewritten as 整機資料遷移（Export / Import）pointing at the new bundle. `docs/architecture/data-model.md` Remote-backup-bundle-contract section rewritten as Data-migration-bundle-contract. `docs/architecture/desktop-command-surface.md` row swapped from `preview_remote_backup / run_remote_backup / verify_remote_backup` to `export_app_data / preview_app_data_import / apply_app_data_import`. `docs/design/screens-and-nav.md` Maintenance / Settings rows + remote-backup PME paragraph rewritten. `docs/design/ui-review-guardrails.md`, `docs/design/app-wide-review-grammar-tradeoff.md`, `docs/features/intelligence-current-state.md`, `docs/vision-and-requirements.md` all updated. Historical `docs/plan/m4-full-polish/` references left as historical record. `WORK-FEEDBACK-0525-S3-REMOVE` block removed from BACKLOG.
    - 🟡 **Verification**: `bunx tsc -b` clean across the whole workspace; `cargo test --workspace --lib` 64/64 green; `bunx vitest run` 1890/1890 green; `bun scripts/i18n-progress.ts` reports 0 missing / 0 raw-English. Full `bun run check` (Rust coverage gate + e2e + mutation + release-check) deferred to the next session along with the remaining feedback work blocks (`WORK-FEEDBACK-0525-DAY-INSIGHTS`, `…-TAG-NOTE-SEARCH`, `…-BROWSE-VIRT`).

  - 進度（2026-05-25 update — feedback-2026-05-25 §3.1 closeout: day insights chart now reflects the full day, not the loaded subset）：
    - ✅ **New Rust aggregator** `vault-core/src/archive/history/day_insights.rs` (7 unit tests). One SQL pass over `archive.visits JOIN urls JOIN source_profiles` for the requested local-calendar day, then a linear walk that yields the same `BrowseDayInsights` shape the frontend was previously building from scroll-loaded cards: `hourBuckets[24]`, `totalPages` / `typedCount` / `linkCount` / `searchCount`, `topDomains`, `topUrls`, `topSearchQueries`, `sessionCount`, `distinctDomains`, `firstVisitMs`, `lastVisitMs`, `peakHour`, `longestSessionMs`. Excludes reverted visits (`visits.reverted_at IS NULL`) and respects the optional `profile_id` filter.
    - ✅ **Session-walk parity**: mirrors the frontend's 30-minute `SESSION_GAP_MS` constant so `sessionCount` and `longestSessionMs` match exactly what the contact sheet draws as visible session boundaries.
    - ✅ **Search-query extraction parity**: same `SEARCH_QUERY_PARAMS_BY_HOST` host → param map the frontend uses (Google / Bing / DuckDuckGo / Kagi / Startpage / Ecosia / Brave / Baidu / Yandex / Yahoo / so.com / Sogou), with the 120-char paste cap and host normalisation pinned by `extract_search_query_understands_known_engines_only`.
    - ✅ **Local-day boundaries**: `local_day_bounds_ms` resolves the `YYYY-MM-DD` to UTC ms via `chrono::Local`, including a DST fallback for spring-forward midnight gaps. Day grouping matches the rest of the Browse surface (`local_datetime_from_millis` semantics).
    - ✅ **New Tauri command** `get_browse_day_insights({date, profileId})` registered through `vault-worker::browse_day_insights`, `worker_bridge::browse_day_insights_impl`, `commands::archive::get_browse_day_insights`, the dev-IPC dispatch arm + `BrowseDayInsightsPayload` envelope, and the top-level `tauri::generate_handler!`. Frontend client exposes it on `backend.getBrowseDayInsights`.
    - ✅ **Frontend hook** `useBrowseDayInsightsCache({profileId, refreshKey})` in `src/pages/explorer/hooks/use-browse-day-insights-cache.ts` (5 React tests). Single state holder keyed by `(refreshKey, profileId)` token — fresh Map gets minted inline whenever the token rotates (canonical "derived state from props" pattern, no ref-during-render lint violations). `resolve(date)` returns cached insights or null + triggers a backend fetch on first miss. In-flight dedup so two adjacent re-renders against the same date fire one backend call. Errors land in the cache as a sentinel so the hook never retries on the same scroll tick and never bubbles into the UI.
    - ✅ **PaperContactSheet integration**: new `resolveDayInsights?: (date) => DayInsights | null` prop on PaperContactSheet + PaperExplorerView. When supplied, the per-day strip prefers the backend aggregate; when it returns null (cache miss or backend error), the existing client-side `aggregateDayInsights(day)` keeps rendering so the panel never blinks empty between scroll-into-view and the backend reply landing. Wired into `pages/explorer/index.tsx` via `useBrowseDayInsightsCache({ profileId, refreshKey })`.
    - ✅ **Tests**: 7 new Rust unit tests + 1 search-query test in `day_insights::tests` (`empty_day_returns_zero_metrics_with_padded_hour_buckets`, `aggregates_transitions_domains_urls_and_search_queries`, `session_walk_splits_on_gap_greater_than_thirty_minutes`, `hour_buckets_track_local_hour_and_peak_hour_picks_the_busiest`, `profile_filter_only_aggregates_the_requested_profile`, `reverted_visits_are_excluded_from_the_aggregate`, `invalid_date_string_returns_an_error`, `extract_search_query_understands_known_engines_only`). 5 React hook tests. 2 contact-sheet tests (`resolveDayInsights` override beats client aggregator; `resolveDayInsights → null` falls back). 1897/1897 unit tests pass; 64/64 vault-worker tests pass.
    - 🟡 **Verification**: `bunx tsc -b` clean; `cargo build --workspace` + `cargo test --workspace --lib` green; full vitest run green; `bunx eslint --max-warnings 0` clean on every touched file. Full `bun run check` (coverage gate + e2e + mutation) deferred to the next session along with the remaining feedback work blocks (`…-TAG-NOTE-SEARCH`, `…-BROWSE-VIRT`).

  - 進度（2026-05-25 update — feedback-2026-05-25 §3.3 A closeout: tag:/note: search operators wired through）：
    - ✅ **Parser extension** (`vault-core::archive::search_query`). `ParsedHistorySearchQuery` gains `required_tags` / `excluded_tags` / `required_notes` / `excluded_notes` fields. Two new match arms in `parse_history_search_query` recognise `tag:foo` / `note:"bar baz"` (and their negated forms `-tag:foo` / `-note:bar`). `normalized_tag_filter` / `normalized_note_filter` lowercase + trim the operand so users can type either `tag:Rust` or `tag:rust` and still match the stored tag.
    - ✅ **SQL filter clauses** added to LIST_HISTORY_SQL + COUNT_HISTORY_SQL (in `archive/mod.rs`) AND LIST_HISTORY_LEXICAL_SQL + LIST_HISTORY_FUZZY_CANDIDATES_SQL + COUNT_HISTORY_LEXICAL_SQL (in `archive/history.rs`). Pattern mirrors the existing `history_required_sites` / `history_excluded_terms` advanced-filter approach: four new temp tables (`history_required_tags`, `history_excluded_tags`, `history_required_notes`, `history_excluded_notes`) get populated by `prepare_advanced_search_filters` and queried via `NOT EXISTS (...) AND NOT EXISTS (...)`. Tag join uses `LOWER(url_tags.tag) = advanced_filter.value` for case-insensitive exact match; note join uses `LOWER(url_annotations.notes) LIKE '%' || advanced_filter.value || '%'` for case-insensitive substring match. Performance: when no tag/note operators are in the query, the temp tables are empty and the `NOT EXISTS` clauses short-circuit — pre-existing query paths stay untouched.
    - ✅ **Advanced-syntax help popover** picks up two new entries (`tag:rust -tag:archived` and `note:"design doc"`) plus their three-locale explanatory copy keys (`advancedSearchHelpTag` / `advancedSearchHelpNote`). `PaperAdvancedSearchHelpCopy` gains the matching `tag` / `note` fields; `buildPaperSearchViewCopy` threads them through; the i18n catalog stays parity-100% (2844 → 2846 keys per locale).
    - ✅ **Tests**: 2 new parser unit tests (`parses_tag_and_note_operators_with_normalization_and_negation`, `empty_tag_and_note_operands_are_dropped`) + 1 new integration test against the canonical archive (`history_keyword_query_supports_tag_and_note_operators_against_annotations`) that seeds two extra URLs via `annotations::replace_tags` / `annotations::set_notes`, then asserts `tag:rust` finds the Rust-tagged URL, `note:"design doc"` finds the URL with the matching note substring, and `-tag:rust` excludes the Rust-tagged URL without dropping un-tagged rows. 209/209 archive tests pass; 64/64 vault-worker tests pass; 1897/1897 JS unit tests pass.
    - 🟡 **Optional follow-up not in this slice**: `+tag` / `+note` filter chips in PaperSearchHero (would let users build tag filters without typing the `tag:` operator). The operator-via-typing path is enough to close §3.3 A as a functional gap; the chip UX is a nice-to-have that's deferred to whenever the broader filter-chip wiring for `+ date` / `+ source` / `+ domain` (still inert today) gets implemented. Recorded as a Browse-search polish follow-up — not user-blocking.

  - 進度（2026-05-25 update — feedback-2026-05-25 §3.3 A polish: `+ Tag` / `+ Note` chips wired end-to-end）：
    - ✅ **New helper** `src/pages/explorer/paper-search-filters.ts` — pure functions: `tokenizeQuery` (whitespace + ASCII/smart quotes + escape), `parseActiveSearchFilters` (projects known operator tokens — `tag`/`note`/`site`/`filetype`/`ext`/`after`/`before`/`intitle`/`title`/`inurl`/`url` — into `ActiveSearchFilter` chips with negation + display value), `appendOperator(query, 'tag')` → `"prev tag:"`, `removeFilterToken(query, tokenIndex)` → strips that exact token even when the user typed duplicates. 18 unit tests pin tokeniser edges, alias collapse, empty-operand skip, and quoted-phrase preservation across remove.
    - ✅ **Hero** (`PaperSearchHero`) gains `addFilterTag` / `addFilterNote` copy fields and `onAddTagFilter` / `onAddNoteFilter` optional handlers — chips render disabled when no handler is wired, matching the existing inert chip pattern. data-testid: `paper-search-add-tag`, `paper-search-add-note`.
    - ✅ **View** (`PaperSearchView`) threads the two new handlers AND a new `inputRef` prop through to the hero, so the panel can place caret right after `tag:` / `note:` once it appends the operator.
    - ✅ **Panel** (`PaperSearchPanel`) is the wiring owner: parses the current `query` into `activeFilters` via `parseActiveSearchFilters`, builds `handleAddTagFilter` / `handleAddNoteFilter` that call `appendOperator` + `onQueryChange` + focus the input at the end via `requestAnimationFrame` + `setSelectionRange`, and `handleRemoveFilter` resolves the chip id back to a tokenIndex and calls `removeFilterToken`. Other inert chips (Date / Source / Domain / Visit count) intentionally stay unwired — they need their own broader pass and are outside §3.3 A scope.
    - ✅ **i18n** (`paperSearchView.heroAddFilterTag` / `heroAddFilterNote` × en / zh-CN / zh-TW). Catalog stays 100% parity (2843 → 2845 keys per locale).
    - ✅ **Tests**: 18 new helper tests + 2 new hero tests (annotation chips render with copy / disabled when handler omitted) + 3 new panel integration tests (+Tag appends `tag:` with leading space, +Note appends on empty query without leading space, active `tag:`/`note:` operators surface as removable chips and × strips the exact token). 1928/1928 vitest pass; `bunx tsc -b` clean; i18n parity check 100%.
    - ✅ **Real bundle smoke**: validated the export/import fix against the user's notebook bundle at `/mnt/st500_share/feedbacks/pathkeep-export-2026-05-25.pathkeep` (538 MB zip, 977 manifest entries, 2.65 GB internal payload, archive schema v13 = current, plaintext mode). One-shot Rust example ran `preview_import` end-to-end, manifest validated, no migrations needed; example file removed afterwards.
    - 🟡 **Verification**: `bunx tsc -b` clean; full vitest run green; i18n parity 100%. Lint surfaces 36 errors but all in pre-existing files (`use-route-history-nav.*`, `link-previews-section.test.tsx`, `paper-form-primitives.test.tsx`) carried over from earlier commits on this branch — none from §3.3 A polish. Full `bun run check` (coverage + e2e + mutation) deferred along with `BROWSE-VIRT`.

  - 進度（2026-05-25 update — feedback-2026-05-25 §1.1 + §1.2 BROWSE-VIRT closeout: viewport-driven day recycling + directional prefetch）：
    - ✅ **Spike measurement first** (`browse-virt-spike-2026-05-25.md`). Quantified the un-virtualised baseline before implementing: list mode = 6.25 nodes / row, cards mode = 14.25 nodes / row, 100-page cap = 31 k (list) / 71 k (cards) DOM nodes. The 71 k figure is the regime where Chrome's compositor + style recalc go non-linear on the 4-core / 8 GB target — matches the "scrolls froze" feedback. Spike doc derives the implementation targets (window ≈ 400 list / 580 cards mounted nodes; cache cap 50 k entries ≈ 30 MB; directional prefetch +2 down / +1 up; MAX_ACCUMULATED_PAGES 100 → 1 000).
    - ✅ **New `useViewportMount` hook** (`src/pages/explorer/hooks/use-viewport-mount.ts`). IntersectionObserver-driven render gating; `initialInView=true` so first paint renders everything (tests + SSR keep their behaviour); `rootMargin` defaults to one screen above and below; captures `measuredHeight` before recycling so the placeholder preserves scroll position. No-ops gracefully when IntersectionObserver is unavailable.
    - ✅ **New `useScrollDirection` hook** (`src/pages/explorer/hooks/use-scroll-direction.ts`). RAF-deduped `window.scrollY` sampler with hysteresis. Defaults: 4-px delta threshold, 4-frame hysteresis (≈64 ms at 60 fps). Allocation-free hot path.
    - ✅ **`PaperContactSheet` refactor**. The `days.map` block now renders each day through a new `PaperDayBlock` wrapper that owns the viewport-mount hook. When `inView` the wrapper renders the existing day chrome (PaperDayHeader, PaperDayInsights, sessions, cards / list rows) verbatim. When out of view the wrapper holds `min-height` equal to the last measured render; `data-virt-state` attribute flips between `mounted` / `recycled` for tests. Two opt-out props (`disableVirtualization`, `virtualizationRootMargin`) keep the spike harness honest.
    - ✅ **`useExplorerInfinitePages`**: gains a `scrollDirection?` option. When direction is `'down'` (the dominant Browse pattern — moving back through older history) the hook warms one extra page in the background (`target + 2`). `'up'` / `'idle'` fall back to the original single-page background prefetch. MAX_ACCUMULATED_PAGES raised 100 → 1 000 — DOM cost is no longer linear with page count post-virt, so the cap is now memory-driven (50 000 entries ≈ 30 MB of HistoryEntry objects).
    - ✅ **Route wire-up** (`src/pages/explorer/index.tsx`): the Browse route now calls `useScrollDirection()` and passes the signal into `useExplorerInfinitePages`. The directional prefetch is live on the shipping Browse surface.
    - 🟦 **Why not `@tanstack/react-virtual`**: it positions virtual items with `transform: translateY()`, which per CSS spec breaks `position: sticky` semantics inside the transformed parent. The Browse day separator MUST stay pinned (`feedback-explorer-sticky-day-header`), so a transform-based virtualiser would either need a custom sticky-overlay layer or have to abandon CSS sticky. The BACKLOG spec listed "自寫" (custom) as a valid alternative alongside react-virtuoso / react-virtual, so the IO-gated approach was taken — preserves every existing CSS contract (sticky day header, cards grid auto-fill, document scroll, target-banner anchoring). Dependency was added and removed in the same session; lockfile reflects no net-new dep.
    - ✅ **Tests**: 2 new virt-integration tests (recycle / remount via mocked IntersectionObserver, `disableVirtualization` flag honoured), 5 new `useViewportMount` tests, 5 new `useScrollDirection` tests (idle / down / up / sub-threshold hysteresis reset / cleanup), 2 new `useExplorerInfinitePages` tests (`'down'` warms page+2, `'idle'` does not). 1947/1947 vitest pass; `bunx tsc -b` clean; i18n parity 100%.
    - 🟡 **Residual follow-ups (NOT user-blocking)**: LRU page eviction at the 50 k-entry cap (currently hard cap; LRU only matters once users regularly hit it); real Chrome devtools FPS trace on the populated archive + Playwright e2e with a 14 M-row preview fixture (both need real-desktop session time); `docs/features/explorer-browse.md` (BACKLOG referenced it as future doc but it doesn't exist yet — spike doc + STATUS cover the v0.3 truth until a Browse feature spec is written).
    - 🟡 **Verification**: `bunx tsc -b` clean; full vitest run green (1947 / 1947, +19 over previous baseline); i18n parity 100%. Full `bun run check` (coverage + e2e + mutation) deferred to the next combined deep-gate pass per user direction.

  - 進度（2026-05-19）：
    - ✅ **Foundation shipped**：Tailwind v4 + shadcn primitives + cn helper + paper tokens.css + fonts.css (bundled Newsreader / JetBrains Mono) + paper.css (noise / vignette / animations) + tailwind.css (@theme 對應 paper tokens 與 shadcn 變數)；@/ path alias 接入 tsconfig + vite。
    - ✅ **Shell shipped**：`src/components/shell/` 新增 PKBrandMark / PKGlyph / PKSidebar / PKTopbar / PKStatusBar / PKSearchPalette；`src/app/shell.tsx` 已重寫為新 shell；i18n shell namespace 新增 paper-redesign 鍵 (findAPage / archiving / sources* / palette* / epigraph1..6) 在三語齊備。
    - ✅ **Dashboard shipped**：`/` 路由已實作 paper-redesign landing page — HeroBand + greeting + 4-stat strip + On This Day card + This Week card + YearHeatmap + Active Threads + Archive card + epigraph footer；接入 `useShellData()` 與 `coreIntelligenceApi.getOnThisDay`，deep-link 進 Explorer / Intelligence。
    - ✅ **Settings Appearance section shipped**：theme / font / density / paper texture persisted prefs in place（`appearance-section.tsx` + `paper-preferences.ts`）。
    - ✅ **Design handoff preserved in-repo**：`docs/design/handoff/paper-redesign/` 收藏完整 design package（HTML / pk-tokens.css / 11 個 JSX）為 source-of-truth，搭配 `docs/design/handoff/README.md` 導讀。`/tmp/pathkeep-design/` 不再是必要依賴。
    - ✅ **Routes 全部完成 paper opt-in 掛載**（每個 route 在 `?layout=paper` 下都接到對應的 paper view，現有 v0.2 surface 仍保留）：
      - **`/explorer` Browse**：`?layout=paper` 渲染 PaperExplorerView — contact sheet + day-sticky toolbar + DayNavControl + CalendarPopover + YearRail + DomainStack + ContactFrame + ListRow，接入 `useExplorerUrlState` / `useExplorerData` / `useExplorerFavicons`，detail panel 寫入 annotations backend。
      - **`/explorer?surface=search`**：PaperSearchView — literary hero + 3-mode toggle + day-grouped results + "see in context" jump-to-Browse；queryInput / mode / regex 共用 explorer URL state。
      - **`/intelligence?layout=paper`**：PaperIntelligenceView — KPI strip + domain rank + refind shelf；接 `primaryOverview.topSites` / `refindPages` 與 dashboard stats。
      - **`/assistant?layout=paper`**：PaperAssistantView — literary greeting + 3-prompt cards + chat composer + citation evidence；adapts AssistantConversationMessage[] → PaperAssistantMessageDescriptor[]，reuse existing handleSend / sending flow。
      - **`/import?layout=paper`**：PaperImportPanel — literary intro + 3 method cards + 5-step stepper above the v0.2 workflow body。
      - **`/audit?layout=paper`**：PaperAuditPanel — manifest chain block strip mapping `snapshot.recentRuns`，current run highlighted。
      - **`/settings?layout=paper`**：PaperSettingsHeader — eyebrow / serif title / italic subtitle + paper jump-nav anchors；既有 sections 在 paper tokens 下自動繼承樣式。
      - **PKDetailPanel slide-over**：title + url + actions + Notes textarea + Tags + Look further — `useDesktopAnnotations` 在 desktop transport 上寫進 annotations backend，browser-preview 仍走 localStorage。
      - **剩餘 follow-up**：section-panel 級別的 paper restyle（Settings 各 section 仍 v0.2 視覺）、`/schedule` `/security` `/maintenance` `/jobs` `/integrations` `/onboarding` `/lock` 的 paper card grid 重排、把 `?layout=paper` 翻成預設。
    - ✅ **Backend annotations**：migration 011_notes_tags.sql shipped；`vault-core::annotations` (get / set_notes / replace_tags / list / search) + 9 sibling tests；`vault-worker::annotations` thin layer；5 個 Tauri commands (`get_url_annotation` / `set_url_notes` / `replace_url_tags` / `list_url_annotations` / `search_url_annotations`)；typed front-end client + `useDesktopAnnotations` hook 接入 Explorer 路由；feature spec 在 `docs/features/annotations.md`。
    - ⏳ **Docs sweep**：annotations feature spec 已上 (`docs/features/annotations.md`)；design-tokens.md / screens-and-nav.md / ux-principles.md / ui-review-guardrails.md / typography-and-font-fallback.md / data-model.md 仍要按新方向重寫；intelligence.md + recall.md 移除 v0.3-coming 標記。
    - ⏳ **Memory**：feedback_brutalist_radius.md / project_v0_3_redesign.md / feedback_typography_policy.md 改成記錄 brutalist → paper 轉向。
    - ✅ **Tests / quality**：3 個 stale v0.2 topbar tests 已重寫，full unit suite 1485/1485 pass；Rust 454 + 33 pass；mutation / e2e gate 仍待 `bun run check` 全套執行。
  - 驗收（block 結束時必須全部達標）：
    - 設計圖中每個畫面在 light + dark 下都與設計檔高度一致；Settings / Schedule / Security / Maintenance / Jobs / Integrations / Onboarding / AppLock 也用相同視覺語言補完。
    - 每個 route 接入真實後端（不再有 v0.3-coming disabled UI）；AI / semantic search 在 provider 未配置時 inline 提示 "Configure AI provider → Settings"。
    - Notes / Tags 從 detail panel 寫入後端 annotations，重新打開仍可讀；FTS 索引可以搜尋。
    - 三語 i18n parity 100%；`html[lang]` 與 locale 同步；字體切換在 Settings 真實生效。
    - `bun run check` + `bun run verify` 全綠（100% JS / Rust coverage + mutation gate + desktop bridge truth gate）。
    - design-tokens / screens-and-nav / ux-principles / ui-review-guardrails / typography-and-font-fallback / data-model / annotations feature spec / intelligence / recall / STATUS / CHANGELOG / BACKLOG / research-and-decisions 全部反映新方向。
    - 截圖：每個 route 在 light + dark 都產出，附在 release artifacts。

- [x] **WORK-V03-OG-IMAGE-A** — Card-mode og:image cache for paper Browse
  - 讀先：
    `.claude/plans/indexed-giggling-ullman.md` (full plan + policy decisions)
    `docs/features/og-images.md` (feature spec — newly written)
    `docs/architecture/data-model.md` §`og_images` paragraph (storage contract)
    `src-tauri/crates/vault-core/src/migrations/012_og_images.sql`
    `src-tauri/crates/vault-core/src/archive/history/og_images.rs` + `og_images_fetch.rs`
  - 目標：讓 paper Browse 卡片模式渲染每個 page 的真正 og:image，per-URL key、content-hash dedup、opt-out 預設開啟，使用者可在 Settings 看到 cache 大小並清空。
  - 契約：
    - 讀路徑 exact-page-URL only，**不做 host fallback**（GitHub / Medium 同 host 不同 page 的社交卡不同）。
    - 寫路徑 content-hash dedup（`sha256_hex(bytes)`）；identical bytes 共用一個 `og_image_blobs` row。
    - Fetch 是 HTTPS-only、無 Referer、靜態 UA、2 MiB 上限、1 個 redirect、12 s timeout；http:// page URL 直接 `parse_error` 不發網路。
    - 三語 i18n key 在 commit 時齊全；Settings toggle 走 `saveConfig` 透過 shell-data context。
    - og:image 快取是 derived，**不進 backup export**；restore 後從空表 lazy 重建。
    - 100% JS / Rust coverage 維持；新增 mockito 測試 +13 og_images storage 測試 +15 fetch 測試 +7 hook 測試 +4 settings 測試 +2 contact frame 測試 +3 list-row 測試。
  - 進度：六個 atomic commit（C1–C6）已全部 ship。
  - 後續 backlog：見 `docs/features/og-images.md` §6（blocklist UI、eviction picker UI、per-host rate limit、daily schedule cleanup tick、negative-cache TTL re-fetch）。

- [x] **WORK-RELEASE-020-A** — v0.2.0 Planning Repair, Security Refresh, And Publication
  - 讀先：
    `README.md`
    `RELEASE.md`
    `docs/plan/BACKLOG.md`
    `docs/plan/CHANGELOG.md`
    `docs/plan/program/quality-matrix.md`
    `docs/features/intelligence.md`
    `docs/features/intelligence-current-state.md`
    `docs/features/archive.md`
    `docs/architecture/tech-stack.md`
    `docs/design/screens-and-nav.md`
    `.github/workflows/release.yml`
  - 目標：把 v0.2.0 發佈 truth 收斂到已完成內容，先處理 Dependabot alerts，再修復 milestone / backlog / status / source docs 的 v0.2 / v0.3 out-of-sync，最後 bump、驗證、tag、發佈 v0.2.0。
  - v0.2.0 發佈範圍：Lexical Recall V2、advanced keyword syntax、Windows unsigned installer / scheduler preview、release/security hardening、既有 archive / deterministic Core Intelligence。
  - 移出 v0.2.0 的 blocker：AI Assistant、embedding、semantic / hybrid search、MCP / skill artifacts、vector sidecar、readable webpage body fetch。這些全部搬到 `BACKLOG.md` 的 `WORK-AI-V03-A` / `WORK-READABLE-CONTENT-V03-A`，作為 v0.3.0 blocker 管理。
  - 契約：不可假裝 AI / readable-content 已可用；user-visible copy 必須同步 `en` / `zh-CN` / `zh-TW`；release 前必須處理 Dependabot alerts、跑 `bun run check` 與 `bun run verify`；release notes 必須包含本次 release 相關的真實 app 截圖。
  - 驗收：
    - GitHub Dependabot alerts #13 / #15 (`openssl`) 與 #14 (`tauri`) 已更新到 patched dependency versions；GitHub alert state 以 dependency graph rescan 為準。
    - `README.md`、feature / architecture / design docs、`BACKLOG.md`、`STATUS.md`、`CHANGELOG.md` 對 v0.2.0 / v0.3.0 scope 一致。
    - app 內 disabled AI / readable-content copy 改為 v0.3 roadmap，且三語 i18n parity 維持 100%。
    - `bun run check`、`bun run verify` 通過；release screenshot assets 由當前 app 產生並嵌入 GitHub release note。
  - 2026-05-09 closeout：v0.2.0 發佈 scope 收斂到已完成的 local-first archive、Lexical Recall V2 / advanced keyword syntax、deterministic Core Intelligence、Windows unsigned installer / scheduler preview 與 release/security hardening；未完成的 AI Assistant、embedding、semantic / hybrid search、MCP / skill artifacts、vector sidecar、readable webpage body fetch 全部移入 `BACKLOG.md` 的 v0.3.0 blocker blocks。
  - 發佈準備：版本已 bump 到 `0.2.0`；preview fixtures、backend deferred notes、Jobs / Assistant / Settings / Integrations / Explorer copy 與三語 i18n 已同步 v0.2.0 / v0.3 truth；release notes 與真實 app 截圖已產生於 `artifacts/release/v0.2.0/`。
  - 驗證結果：`bun run check` 與 `bun run verify` 通過，包含 100% JS/Rust coverage、browser-preview E2E、desktop-bridge truth gate、desktop-contract mutation gate、Rust supply-chain audit、release config guard 與 debug desktop build rehearsal。

> `BACKLOG.md` 目前的前兩個 blocked blocks 是 v0.3.0 AI / readable-content scope；maintenance / deep mutation hardening 不屬於 v0.2.0 release blocker，除非使用者另外排 dedicated window。

- [x] **WORK-PREVIEW-SHOWCASE-A** — Vercel Browser Preview Synthetic Dataset
  - 讀先：
    `docs/plan/STATUS.md`
    `docs/plan/BACKLOG.md`
    `docs/plan/CHANGELOG.md`
    `docs/plan/program/quality-matrix.md`
    `docs/features/archive.md`
    `docs/features/intelligence.md`
    `docs/features/intelligence-current-state.md`
    `docs/design/ux-principles.md`
    `docs/design/screens-and-nav.md`
    `docs/design/ui-review-guardrails.md`
    `docs/design/design-tokens.md`
    `src/lib/backend-preview-fixtures.ts`
    `src/lib/backend-preview-state.ts`
    `src/lib/backend-preview-shell-commands.ts`
    `src/lib/backend-preview-intelligence-commands.ts`
    `src/lib/backend-preview-search.ts`
  - 目標：讓 Vercel 靜態 browser preview 預設使用 synthetic showcase data，讓訪客能看到有資料時的 Dashboard / Explorer / deterministic Intelligence 形態。
  - 契約：不得把真實 archive、raw browser history、URL、title、profile name 或 secret 寫進 repo / bundle；本地真實資料只允許用 read-only aggregate shape 作參考。Tauri / desktop runtime 不得接入 showcase fixture；browser preview 必須繼續誠實標示 fixture boundary，不得冒充 desktop truth。
  - 驗收：Vercel build path 可明確啟用 showcase dataset；local default browser-preview tests 不被迫改走 showcase；targeted preview tests、`bun run build` 與 `bun run check` 通過。
  - 2026-05-10 closeout：新增 browser-preview showcase dataset，以 synthetic public-domain rows 和 modeled aggregate totals 呈現 dataful Dashboard / Explorer / deterministic Core Intelligence；Vercel 透過 `vercel.json` build command 明確使用 `PATHKEEP_BROWSER_PREVIEW_DATASET=showcase`，local default 仍是 setup fixture。
  - 隔離邊界：showcase fixtures 只在 browser preview bundle 使用；Tauri / desktop `isTauri()` path 不讀取或接入 showcase data。本地真實 archive 只透過 read-only aggregate shape script 參考總量、活躍時段、來源族群與月份分佈，未寫入 raw URLs、titles、search terms、profile paths 或 secrets。
  - 驗證結果：targeted preview / showcase tests、`PATHKEEP_BROWSER_PREVIEW_DATASET=showcase bun run build`、Playwright static preview smoke（Dashboard / Explorer / Intelligence）與完整 `bun run check` 通過；`bun run check` 包含 100% JS/Rust coverage、browser-preview E2E、desktop-bridge truth gate 與 desktop-contract mutation gate。
