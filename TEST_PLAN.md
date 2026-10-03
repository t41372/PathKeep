# Test Plan

The old test plan in this file (May 2026) was a module-by-module log of a unit-test and coverage pass over the previous frontend, written for the 100% coverage and mutation-testing gates. Both gates are gone and the frontend it described has been replaced, so the log no longer describes anything in the tree. It is still in git history (`git log -- TEST_PLAN.md`).

How we test now is in [TESTING.md](./TESTING.md); the gate itself is defined in [docs/plan/program/quality-matrix.md](./docs/plan/program/quality-matrix.md).

## What to cover next

Covered in `tests/e2e/` today: first run with an encrypted archive, first and incremental backups (exact counts), a paused source catching up, full-text / regex / browser-filtered search, star and note, top sites, app lock, and delete all data.

Next, hardest first:

- Takeout import: preview, execute, re-import dedupe, revert, restore.
- Encrypted archive across a restart: unlock with the password, fail cleanly on a wrong one; change the password and decrypt (rekey) from Settings.
- Restore from a safety copy, and Move to another computer (export, import into a fresh profile).
- Automatic backup once the scheduler has a debug-build sandbox: install, status, remove.
- Search over a large archive (millions of visits): results appear promptly and the UI stays responsive while loading.
- Ask with a local provider, citing real visits.

Rust behavior keeps its `cargo test` suites; add a Rust test when a failure mode can only be reached below the UI, such as a crash window in an archive write.
