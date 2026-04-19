# Notes

## What was observed

- Fresh desktop startup still opened on a generic dashboard archive-read failure instead of a clear unlock / onboarding state.
- The Security route loaded the real encrypted/locked archive state and exposed the unlock controls.
- Entering `000000` and pressing `解鎖` triggered a long-lived busy overlay that did not settle into success or a readable failure during the observed window.

## What the sample captured

- Repeated runtime polling work while the archive remained locked:
  - `load_ai_queue_status`
  - `load_intelligence_runtime`
- Main-thread time also showed keychain lookup work under `vault_platform::keyring::*`.

## What this means

- The current host still has a real desktop truth problem before import or Core Intelligence deep-dive work can be trusted.
- This artifact is good enough to prove pre-unlock background churn exists, but not good enough to sign off the post-import performance story.
- A later fresh `bun run desktop:dev` restart still rendered the old generic dashboard copy and no compact build SHA label, so this host also appears to have stale WebView / bundle drift on top of the locked-archive truth bug.
