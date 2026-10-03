# Test Plan

The old test plan in this file (May 2026) was a module-by-module log of a unit-test and coverage pass over the previous frontend, written for the 100% coverage and mutation-testing gates. Both gates are gone and the frontend it described has been replaced, so the log no longer describes anything in the tree. It is still in git history (`git log -- TEST_PLAN.md`).

How we test now is in [TESTING.md](./TESTING.md); the gate itself is defined in [docs/plan/program/quality-matrix.md](./docs/plan/program/quality-matrix.md).

## What to cover next

Write E2E scenarios in `tests/e2e/` for the flows that matter most, hardest first:

- First run to first backup against a real Chrome profile, then a second backup that dedupes.
- Import (Browser Direct and Takeout): preview, execute, re-import dedupe, revert, restore.
- Encrypted archive: set a passphrase, restart, unlock, and fail cleanly on a wrong passphrase.
- Search and Explorer over a large archive: results appear promptly and the UI stays responsive while loading.
- Scheduled backup attempts and their outcomes in Jobs.

Rust behavior keeps its `cargo test` suites; add a Rust test when a failure mode can only be reached below the UI, such as a crash window in an archive write.
