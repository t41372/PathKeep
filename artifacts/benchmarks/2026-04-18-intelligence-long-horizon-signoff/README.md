# 2026-04-18 Intelligence Long-Horizon Signoff

This folder is the replayable artifact bundle for `WORK-CI-C`.

Accepted closeout truth:

- Core Intelligence now signs off on the current primary macOS development host with replayable `14.4M / 60y` evidence.
- The repo no longer ships legacy `vault-core::insights`, `InsightStatus`, or snapshot-era deterministic module ids as living contracts.
- `alternate-host` evidence is explicitly deferred; if we want it later, it should be tracked as a new backlog item instead of reopening `WORK-CI-C`.

## Host

- Host: `tims-MacBook-Pro.local`
- OS: `macOS 26.4.1 (25E253)`
- Role: current primary development host and signoff host for `WORK-CI-C`

## Durable App Root

- Path: `~/Library/Caches/pathkeep-benchmarks/14_4m-signoff`
- Total size after signoff: `18G`
- `archive/history-vault.sqlite`: `7.6G`
- `derived/history-intelligence.sqlite`: `8.6G`
- `derived/history-search.sqlite`: `1.8G`

This root was first created by the full synthetic replay with `--persist-app-root`, then reused by the expired-lease recovery replay with `--skip-baseline-rebuild`.

## Artifacts

- `full-14_4m-60y-signoff.json`
- `expired-lease-recovery-14_4m-signoff.json`

The numbers below are copied from those JSON payloads and should match them exactly.

## Canonical Replay Commands

### Full `14.4M / 60y` replay

```bash
cargo run --release -p vault-core --example intelligence-benchmark --manifest-path src-tauri/Cargo.toml -- --scenario full --window-days 365 --visits 14400000 --horizon-days 21900 --persist-app-root '/Users/tim/Library/Caches/pathkeep-benchmarks/14_4m-signoff' --output 'artifacts/benchmarks/2026-04-18-intelligence-long-horizon-signoff/full-14_4m-60y-signoff.json'
```

### Expired-lease recovery replay

Canonical command stored in the JSON artifact:

```bash
cargo run --release -p vault-core --example intelligence-benchmark --manifest-path src-tauri/Cargo.toml -- --scenario expired-lease-recovery --window-days 365 --app-root '/Users/tim/Library/Caches/pathkeep-benchmarks/14_4m-signoff' --skip-baseline-rebuild --output 'artifacts/benchmarks/2026-04-18-intelligence-long-horizon-signoff/expired-lease-recovery-14_4m-signoff.json'
```

Actual execution on this host reused the already-built release example binary after the `cargo run` wrapper stalled inside release build-script processes:

```bash
src-tauri/target/release/examples/intelligence-benchmark --window-days 365 --scenario expired-lease-recovery --app-root '/Users/tim/Library/Caches/pathkeep-benchmarks/14_4m-signoff' --skip-baseline-rebuild --output 'artifacts/benchmarks/2026-04-18-intelligence-long-horizon-signoff/expired-lease-recovery-14_4m-signoff.json'
```

The artifact still records the canonical `cargo run` replay shape shown above.

## Results

### `full-14_4m-60y-signoff.json`

- Corpus: `14,400,000` visits, `2,400,000` search terms, `8,125,448,192` archive bytes
- Baseline rebuild wall clock: `4,758,160 ms`
- Query surfaces: `8,969 ms`
- Stage timings:
  - `visit-derive`: `588,302 ms`
  - `daily-rollup`: `27,320 ms`
  - `structural-rebuild`: `4,124,819 ms`
  - total stage time: `4,740,441 ms`
- Peak RSS after queries: `1,738.96875 MiB`
- Surface totals:
  - sessions: `8,738`
  - search trails: `39,420`
  - query families: `26`
  - digest total visits: `239,204`
  - digest total searches: `39,420`
  - digest refind pages: `5,220`

### `expired-lease-recovery-14_4m-signoff.json`

- Corpus: `14,400,000` visits, `2,400,000` search terms
- Archive bytes: `8,125,448,192`
- Intelligence bytes: `9,273,229,312`
- `baselineRebuildSkipped`: `true`
- Query surfaces: `2,013 ms`
- Peak RSS after queries: `598.59375 MiB`
- Runtime recovery time: `1 ms`
- Recovery behavior:
  - the intentionally cancelled lease stayed cancelled with reason `cancelled from UI`
  - the expired queued lease was recovered with last error `PathKeep recovered an expired intelligence lease.`

## Closeout Reading

- `docs/plan/program/research-and-decisions.md` (`PG-RD-AI-011`)
- `docs/plan/core-intelligence-progress.md`
- `docs/plan/core-intelligence-handoff.md`
- `docs/features/intelligence-current-state.md`

These docs now all align on the same closeout truth: `WORK-CI-C` is finished, current-host `14.4M` evidence exists, legacy `insights` is retired, and alternate-host evidence is intentionally deferred.
