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

---

## Remaining Work After This Audit

1. **Fix the shell bootstrap drift for locked archives**
   - current host still needs to actually render the repaired shell bundle so dashboard / sidebar / topbar agree with Security route truth
2. **Fix the Security unlock settle path**
   - re-run the live desktop flow now that source fails fast on bad keys; `000000` must either unlock successfully or fail explicitly
3. **Re-run the full requested all-app audit after unlock works**
   - import Chrome `yi-ting`
   - run `/intelligence`, domain deep dive, Explorer session/trail, Settings external outputs, Jobs, Audit, Schedule, Assistant
4. **Finish the profiling bundle after unlock**
   - route-load + interaction samples after real data import

Until those are done, the original “P1-P4 complete” statement remains true for **source-level implementation**, but not yet for **desktop truth on this host**.
