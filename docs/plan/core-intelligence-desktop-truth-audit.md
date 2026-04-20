# Core Intelligence Desktop Truth Audit

> **Status:** First-pass audit complete, follow-up still required  
> **Date:** 2026-04-18  
> **Scope:** current-source desktop app + Computer Use + non-browser profiling

---

## Why This File Exists

`core-intelligence-progress.md` and `core-intelligence-handoff.md` correctly describe the **source-level** state: the original deterministic Core Intelligence P1-P4 scope is implemented, and only alternate external hosts remain deferred.

This file answers a different question:

> What actually happens in the real desktop app on the current host when a user opens PathKeep, sees a locked archive, tries to unlock it, and attempts to continue into the rest of the product?

That truth is currently more complicated than the source-only completion story.

---

## Environment

- Host: current macOS desktop session
- App runtime: `bun run desktop:dev`
- Interaction method: Computer Use only
- Archive state at audit start:
  - app root exists at `~/Library/Application Support/com.yi-ting.pathkeep`
  - archive is encrypted
  - archive session key is not unlocked
  - keychain persistence is disabled in config
- Requested unlock password for audit: `000000`

Profiling artifacts for this pass live under:

- [`artifacts/perf/2026-04-18-desktop-truth-audit/`](../../artifacts/perf/2026-04-18-desktop-truth-audit)

---

## Completion Matrix

| Surface / contract                                                | Design / planning status                                           | Current source status                                                                                                                                           | Computer Use truth on current host                                                                                                            | Verdict                                             |
| ----------------------------------------------------------------- | ------------------------------------------------------------------ | --------------------------------------------------------------------------------------------------------------------------------------------------------------- | --------------------------------------------------------------------------------------------------------------------------------------------- | --------------------------------------------------- |
| Core Intelligence deterministic P1-P4 backend/query surface       | Planned complete                                                   | Implemented and still test-backed                                                                                                                               | Not fully re-verified end-to-end because locked-archive bootstrap blocked the full pass                                                       | **Done in source; full desktop pass still pending** |
| Dashboard locked/uninitialized entry grammar                      | Expected truthful state split                                      | Source now has transport error shaping + dashboard fallback via `securityStatus()`                                                                              | Current host still opens on the old generic `無法讀取封存` state with no CTA                                                                  | **Source fixed, real app still drifting**           |
| Shell bootstrap snapshot for locked archive                       | Expected to stay usable enough to orient / repair                  | Source now degrades `app_snapshot` into a usable locked snapshot instead of failing the whole shell                                                             | Fresh-restarted current host still falls back to “not initialized/plaintext” even though worker logs show encrypted-archive key errors        | **Source fixed, real app still drifting**           |
| Security route locked-archive review                              | Expected truthful and actionable                                   | Works in source                                                                                                                                                 | Computer Use verified the route loads real encrypted/locked status, archive path, keychain availability, and unlock controls                  | **Verified**                                        |
| Security unlock action (`000000`, no keychain)                    | Expected to unlock or fail clearly                                 | Source now validates the candidate key via `securityStatus()` before doing a full shell refresh, so wrong keys can fail fast instead of sitting under a spinner | Current host still needs a new real-app pass because the stale shell bundle never exposed the repaired unlock path after restart              | **Source fixed; live pass still pending**           |
| Explorer locked-archive state                                     | Expected honest locked/degraded state                              | Route exists                                                                                                                                                    | Current host still shows a generic load failure instead of an explicit unlock path                                                            | **Broken in real app**                              |
| Intelligence locked-archive runtime polling                       | Expected honest degraded state without noisy background churn      | Source sidebar chrome now stops polling background-work runtime until the archive is unlocked                                                                   | Profiling bundle still captured pre-fix locked-state churn, and the stale desktop shell on this host still needs a fresh live pass            | **Source fixed; live pass still pending**           |
| Compact build diagnostics                                         | Expected to expose support-friendly build identity                 | Source sidebar / lock / onboarding chrome now show `version · short-sha` and append `+` for dirty worktrees                                                     | Fresh-restarted current host still rendered only `v0.1.0`, which matches the same stale-bundle drift seen in locked-archive shell copy        | **Source fixed, real app still drifting**           |
| Schedule route                                                    | Out of Core Intelligence scope, but requested in the all-app audit | Implemented                                                                                                                                                     | Computer Use verified the route remains navigable and shows preview/manual/execute/verify content even while shell snapshot truth is drifting | **Verified**                                        |
| Import / Audit / Jobs / Settings / full `/intelligence` deep pass | Requested in this audit                                            | Implemented in source                                                                                                                                           | Not completed end-to-end because the audit could not reach a stable unlocked archive session first                                            | **Blocked by unlock/bootstrap truth drift**         |

---

## Computer Use Findings

### 1. Dashboard still opens on the wrong repair state

Observed on fresh `bun run desktop:dev` startup:

- sidebar footer says `尚未設定封存 / 未加密 / 0 B`
- top-right primary action stays disabled as `請先完成設定`
- main content shows:
  - `無法讀取封存`
  - `PathKeep 無法載入最新的歸檔狀態。`

That is inconsistent with the real archive state verified from the Security route:

- archive is initialized
- archive is encrypted
- archive is locked
- the correct next action is to unlock it

### 2. Security route has the correct truth, but shell chrome does not

Computer Use could navigate to `#/security` and verify:

- encrypted / locked status
- archive path
- stronghold path
- keychain availability
- unlock form
- rekey preview controls

So the app **does** know the real archive state. The shell bootstrap / dashboard entry is what drifts.

### 3. Unlock attempt did not settle

Using Computer Use:

1. open Security
2. enter `000000`
3. press `解鎖`

Observed result:

- a busy overlay remained visible
- the route did not settle into either:
  - unlocked success, or
  - a clear “wrong password / unlock failed” error

This means the current host still has a real shipped blocker in the locked-archive flow, independent of the original Core Intelligence feature-completeness story.

### 4. Fresh restart still showed stale shell UI after the source fixes landed

After the source-level follow-up landed, PathKeep was fully restarted again through `bun run desktop:dev`.

Computer Use still observed:

- old generic Dashboard copy
- old locked-archive sidebar/footer grammar
- no compact `version · short-sha` build label in the visible shell chrome

At the same time, the desktop terminal still logged encrypted-archive key warnings from the current worker build.

The most likely explanation is still host-specific stale WebView / bundle cache drift, not missing source changes.

---

## Profiling Notes

First-pass sample artifact:

- [`unlock-hang-sample.txt`](../../artifacts/perf/2026-04-18-desktop-truth-audit/unlock-hang-sample.txt)

Key observations from that sample:

- the app was not pegged on one giant CPU hot loop
- repeated work showed up around:
  - `pathkeep_desktop::commands::intelligence::load_ai_queue_status`
  - `pathkeep_desktop::commands::intelligence::load_intelligence_runtime`
  - keychain lookups inside `vault_platform::keyring::*`
- on the current host, locked-archive startup still allows runtime polling pressure to continue while the user is trying to recover the session

This does **not** yet replace a full post-unlock profiling pass. It only proves there is already avoidable background churn before the archive is even usable.

---

## What Landed In Source During This Audit

- `src/lib/runtime.ts`
  - better Tauri runtime detection for `tauri:` protocol / injected internals
- `src/lib/ipc/bridge.ts`
  - wraps raw Tauri string rejections into `Error` objects so shell routes can keep actionable refusal messages
- `src/pages/dashboard/index.tsx`
  - adds `securityStatus()` fallback so a failed shell bootstrap can still distinguish:
    - uninitialized archive
    - encrypted but locked archive
    - real generic failure
- `src-tauri/crates/vault-worker/src/app.rs`
  - browser discovery / runtime diagnostics now degrade instead of taking down the whole shell snapshot
  - locked encrypted archives now return a usable shell snapshot with `archiveStatus.warning` instead of failing the whole bootstrap
- `src/pages/security/index.tsx`
  - validates a candidate archive key through `securityStatus()` before doing a full shell refresh, so wrong keys can fail fast
- `src/components/sidebar/background-status.tsx`
  - stops polling background-work runtime while the archive is still locked and routes the compact CTA to Security instead
- `src/lib/build-info.ts`
  - formats compact `version · short-sha[+]` labels for sidebar / onboarding / lock / diagnostics chrome
- regression coverage:
  - runtime / bridge tests
  - dashboard fallback tests
  - worker app snapshot degradation test
  - locked-archive unlock-path / sidebar build-label regressions

These fixes are real and test-backed even though the current host still shows live desktop drift that requires another pass.

### 2026-04-20 follow-up: query-family + Security truth repair did land in source, but current-host Tauri WebView is still drifting

This follow-up re-ran `bun run desktop:dev`, unlocked the archive with `000000`, and used Computer Use to revisit:

- `#/intelligence/domain/google.com`
- the shared `query-family` deep link reached from `打開搜尋演化`
- `#/security` in both unlocked and locked states

What landed in source during this follow-up:

- `src/pages/intelligence/promoted-entity-routes/query-family-route.tsx`
  - route-level fallback now uses already-shipping intelligence keys (`queryFamilyRouteTitle`, `queryFamilyQueriesTitle`, `searchQueriesEngineFilter`) instead of depending on freshly added copy keys
  - the route now formats query-family dates itself and passes them through the existing `QueryFamilyCard` footer path, so live desktop no longer depends on the card module updating before ISO timestamps disappear
- `src/pages/security/index.tsx`
  - known locked-archive warning sentences are now mapped into front-end-owned localized copy instead of being rendered as raw backend English
- targeted regression coverage now includes:
  - query-family route translation/date assertions in `src/pages/intelligence-surfaces.test.tsx`
  - locked-archive warning localization in `src/pages/trust-flows.test.tsx`

Verification truth from this host:

- `bun run test:unit:product-flows` passed
- `bun run check` passed
- `bun run build` passed
- `curl http://127.0.0.1:1420/src/pages/intelligence/promoted-entity-routes/query-family-route.tsx` showed the new route-local fallback labels/date formatting
- `curl http://127.0.0.1:1420/src/pages/security/index.tsx` showed the new warning-localization code
- **but** the current-host Tauri desktop window still rendered:
  - raw `INTELLIGENCE.*` / `intelligence.*` copy on the `query-family` route
  - raw ISO timestamps on the `query-family` route
  - raw `database key is required for encrypted archives` on the Security page

At this point the most honest interpretation is:

- source-level repair is real and test-backed
- the current host is still serving stale route/security modules inside the Tauri WebView even after a clean `bun run desktop:dev` restart
- current-host desktop truth for these specific surfaces is therefore **not signed off yet**, but the blocker is now host/runtime drift rather than an unverified source fix

---

### 2026-04-20 follow-up: the real-data import / unlock pass completed, but current-host desktop still serves stale Settings / Intelligence UI

This follow-up started from a cleared app root after the user explicitly asked for a fresh pass. Using Computer Use against the desktop app:

- onboarding selected Chrome `Yi-Ting` (`~/Library/Application Support/Google/Chrome/Default`)
- storage stayed at `~/Library/Application Support/com.yi-ting.pathkeep`
- archive encryption was enabled with `000000`
- keychain persistence stayed off during onboarding, and the later Security unlock typed `000000` manually instead of saving it
- first backup finished as run `#1`

Live-desktop behavior that is now verified on this host:

- onboarding + first backup completed successfully
- the archive entered `encrypted / locked` state on relaunch instead of falling back to a fake plaintext / uninitialized shell
- Security unlocked the archive with the manually entered password and continued to show `Save to Keychain` as an optional action, which is consistent with “password not stored”
- Dashboard then loaded real archive totals (`64,696` visits, `35,170` URLs, `244` downloads)
- `/intelligence` loaded real archive-wide data, and `/intelligence/domain/google.com?range=month` rendered live counts + top paths
- Explorer grouped `session` and `trail` views both loaded real data
- Jobs, Audit, Schedule preview, Assistant disabled state, and Settings external-output surfaces all remained navigable after the import

Current-host drift that is still blocking full signoff:

- shell chrome still reports build label `dc410477` even in the newly launched desktop sessions from the current working tree
- `/settings` still renders the old English group dividers (`CORE`, `DATA & UPDATES`, etc.) instead of the localized strings that now exist in source/tests
- `/intelligence` still exposes raw icon ids (`bar_chart`, `auto_stories`, `sync`) in the desktop accessibility tree, even though the source-level glyph contract now hides decorative icons
- clicking Explorer’s `Open session insights` CTA through Computer Use landed on a malformed `tauri://localhost/intelligence/...#/...` URL and dropped back to Dashboard instead of opening the promoted entity route

Source-level confirmation gathered during this follow-up:

- `bunx vitest run src/index-html.test.ts src/App.helpers.test.tsx src/pages/intelligence-surfaces.test.tsx` passed
- `bun run build` passed
- browser-preview `/intelligence` no longer exposes raw glyph ids in its accessibility snapshot
- the Settings zh-TW group-divider regression is now locked in `src/pages/intelligence-surfaces.test.tsx`

The most honest interpretation after this rerun is:

- the destructive-reset blocker is gone
- the real-data import / unlock pass is now substantially complete
- the remaining blocker is current-host desktop stale-frontend drift, not missing source fixes

## Remaining Work After This Audit

### 2026-04-20 closeout: current-host stale bundle drift is resolved and the desktop truth pass is signed off

The stale-frontend blocker turned out to be a host artifact issue, not a missing source fix. `Computer Use` kept attaching to the old `dc410477` release bundle even when the new debug binary had already embedded `6412ad59`. Rebuilding the current-host release app with `bunx tauri build --bundles app --no-sign` and reopening that `.app` finally put the live desktop on the same code that source/tests were already exercising.

Using `Computer Use` on that rebuilt release app:

- live desktop chrome now reports `v0.1.0 · 6412ad59+`
- `/intelligence` no longer leaks raw glyph ids into the accessibility tree
- `/intelligence/domain/google.com?range=month` now renders `打開網域證據` as `tauri://localhost#/explorer?...`, and the CTA opens Explorer instead of dropping back to Dashboard
- the rebuilt bundle unexpectedly recreated an empty app root, so the real-data pass was rerun on the new bundle itself:
  - onboarding selected Chrome `Yi-Ting`
  - storage stayed at `~/Library/Application Support/com.yi-ting.pathkeep`
  - archive encryption used `000000`
  - the keychain checkbox stayed off during onboarding
  - the resulting `config.json` now records `rememberDatabaseKeyInKeyring: false`
  - the first backup completed as run `#1`
  - Dashboard now shows `64,498` visits and `35,110` URLs for this rerun
- Explorer grouped `trail` view still loads correctly from the domain-scoped evidence CTA on the rebuilt bundle

Source-level and host-level evidence both line up now:

- `bunx vitest run src/components/intelligence/entity-actions.test.tsx src/pages/explorer/panels/privacy-redaction.test.tsx src/index-html.test.ts src/App.helpers.test.tsx src/pages/intelligence-surfaces.test.tsx` passed
- `bun run check` passed
- `bun run build` passed

Current honest status:

- the stale-bundle blocker is gone
- the malformed entity/evidence CTA grammar is fixed
- the requested Chrome `Yi-Ting` import + encrypted archive bootstrap truth pass is complete on this host

## Remaining Work After This Audit

1. **Optional future follow-up**
   - collect a fresh post-unlock profiling bundle only if another performance regression investigation needs it

The original “P1-P4 complete” statement is now true for both **source-level implementation** and **desktop truth on this host**.
