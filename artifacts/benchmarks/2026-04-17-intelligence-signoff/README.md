# 2026-04-17 Core Intelligence Signoff Attempts

## Scope

- Work block: `WORK-CI-B`
- Research item: `PG-RD-AI-011`
- Host: primary macOS development machine (`18` CPU cores, `64 GiB` RAM, local workspace)
- Purpose: push beyond the existing `100k / 1m` artifacts and find the next real finish-line blockers for large-host Core Intelligence rebuilds

## Code changes exercised before these runs

- live queued enrichment execution moved out of legacy `vault-core::insights` and into `enrichment.rs`
- benchmark harness now supports `--persist-app-root` so large synthetic corpora can be replayed without reseeding every scenario
- `daily-rollup` full/fallback rebuilds now aggregate in SQLite instead of hydrating every derived visit row into Rust
- `visit-derive` fallback persistence now uses a prepared statement inside one transaction
- structural rebuild no longer clones the full `tail_visits` buffer before session/trail assignment
- `build_sessions` no longer does an `O(sessions × visits)` second-pass rescan; session aggregates are accumulated during the first pass

## Completed smoke evidence

Command:

```bash
cargo run -p vault-core --example intelligence-benchmark --manifest-path src-tauri/Cargo.toml -- \
  --visits 2000 \
  --window-days 365 \
  --horizon-days 21900 \
  --scenario full \
  --persist-app-root /tmp/pathkeep-benchmark-smoke \
  --output /tmp/pathkeep-benchmark-smoke.json
```

Observed:

- replay command recorded the persisted synthetic root correctly
- baseline full rebuild: `270 ms`
- query surfaces: `9 ms`
- peak RSS: about `22.8 MiB`

This smoke was enough to verify the new persisted-root harness path, but it is **not** large-host signoff evidence.

## Large-host attempts

### Debug `10M / 60y` full rebuild before runtime cleanup

Command:

```bash
cargo run -p vault-core --example intelligence-benchmark --manifest-path src-tauri/Cargo.toml -- \
  --visits 10000000 \
  --window-days 365 \
  --horizon-days 21900 \
  --scenario full \
  --output artifacts/benchmarks/2026-04-17-intelligence-signoff/full-10m-60y.json
```

Observed before aborting:

- process was still running after about `43m`
- sample hotspot: `execute_daily_rollup_stage -> build_daily_rollups_for_profile_in_batches -> load_profile_derived_visit_batch`
- temp synthetic app root reached roughly `archive=5.0 GiB`, `derived=2.0 GiB`
- no JSON artifact was emitted before the run was stopped

Conclusion:

- the pre-fix daily-rollup full path was still too expensive to use as honest `10M` signoff evidence

### Debug `10M / 60y` after daily-rollup / visit-derive / structural-memory cleanup

Command:

```bash
cargo run -p vault-core --example intelligence-benchmark --manifest-path src-tauri/Cargo.toml -- \
  --visits 10000000 \
  --window-days 365 \
  --horizon-days 21900 \
  --scenario full \
  --persist-app-root /tmp/pathkeep-benchmark-10m-signoff \
  --output artifacts/benchmarks/2026-04-17-intelligence-signoff/full-10m-60y.json
```

Observed before aborting:

- the hot path moved out of `daily-rollup`; sample first showed `execute_visit_derive_stage`
- later sample showed `execute_structural_stage -> build_sessions`
- structural-stage RSS was about `3.0 GiB`, down from the earlier `~4.8 GiB` range seen before removing the extra `tail_visits.clone()`
- no JSON artifact was emitted before the run was stopped

Conclusion:

- the runtime cleanup materially improved the shape of the `10M` run
- the next blocker moved to structural rebuild CPU/memory, not daily-rollup fallback

### Release `10M / 60y` with persisted synthetic root

Command:

```bash
cargo run --release -p vault-core --example intelligence-benchmark --manifest-path src-tauri/Cargo.toml -- \
  --visits 10000000 \
  --window-days 365 \
  --horizon-days 21900 \
  --scenario full \
  --persist-app-root /tmp/pathkeep-benchmark-10m-signoff \
  --output artifacts/benchmarks/2026-04-17-intelligence-signoff/full-10m-60y.json
```

Observed before termination:

- process elapsed went past `30m`
- `ps`-reported RSS moved between about `2.0 GiB` and `4.7 GiB`
- `sample` reported physical footprint around `2.1 GiB` with peak around `2.6 GiB` during one structural-stage snapshot
- persisted synthetic root stabilized around `archive=5.0 GiB`, `derived=2.7 GiB`
- the run exited without emitting the JSON artifact

Conclusion:

- even after the runtime cleanup, `10M` signoff is still not closed on this host
- `14.4M` and large-host queue-recovery should remain open until structural full rebuild cost is reduced further or the run completes end-to-end with a real artifact

## Real-archive replay status

The current real app root is encrypted:

- `~/Library/Application Support/com.yi-ting.pathkeep/config.json` reports `"archiveMode": "Encrypted"`
- CLI keychain retrieval via `security find-generic-password -s com.yi-ting.pathkeep -a database-key -w` returns exit code `128`
- no disposable plaintext copy or user-supplied session key was available during this run

So the real-archive replay slice is still blocked on one of:

1. a disposable plaintext app-root copy
2. the archive session key

## Current truth

`PG-RD-AI-011` remains open.

What changed today is not the final closeout; it is the quality of the blocker:

- daily-rollup fallback is no longer the first obvious `10M` bottleneck
- queued enrichment execution no longer depends on legacy `insights` ownership
- persisted synthetic roots are now available for repeatable large-host follow-up runs
- the remaining large-host blocker is structural full rebuild cost plus real-archive unlockability, not lack of harness capability
