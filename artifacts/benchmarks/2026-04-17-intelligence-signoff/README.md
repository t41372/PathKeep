# 2026-04-17 Core Intelligence Signoff Attempts

## Scope

- Work block: `WORK-CI-B`
- Research item: `PG-RD-AI-011`
- Host: primary macOS development machine (`18` CPU cores, `64 GiB` RAM, local workspace)
- Purpose: push beyond the existing `100k / 1m` artifacts and find the next real finish-line blockers for large-host Core Intelligence rebuilds

## Code changes exercised before these runs

- live queued enrichment execution moved out of legacy `vault-core::insights` and into `enrichment.rs`
- benchmark harness now supports `--persist-app-root` so large synthetic corpora can be replayed without reseeding every scenario
- benchmark harness now also supports `--skip-baseline-rebuild` for existing `--app-root` follow-up scenarios, and it refuses that flag unless the replay target already has a Core Intelligence read model
- `daily-rollup` full/fallback rebuilds now aggregate in SQLite instead of hydrating every derived visit row into Rust
- `visit-derive` fallback persistence now uses a prepared statement inside one transaction
- structural rebuild no longer clones the full `tail_visits` buffer before session/trail assignment
- `build_sessions` no longer does an `O(sessions × visits)` second-pass rescan; session aggregates are accumulated during the first pass
- structural rebuild now streams tail visits in batches, range-clears affected memberships instead of collecting giant `visit_id` lists, and rebuilds `source_effectiveness` from batched `search_trails` reads rather than a whole-profile reload

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

### Debug `1M / 60y` full rebuild after structural streaming

Command:

```bash
cargo run -p vault-core --example intelligence-benchmark --manifest-path src-tauri/Cargo.toml -- \
  --visits 1000000 \
  --window-days 365 \
  --horizon-days 21900 \
  --scenario full \
  --output artifacts/benchmarks/2026-04-17-intelligence-signoff/full-1m-60y-post-streaming.json
```

Observed:

- baseline full rebuild: `383,083 ms`
- query surfaces: `403 ms`
- peak RSS: about `118 MiB`
- corpus: `1,000,000` visits, archive about `522.5 MiB`, URL chars about `42.3M`

Conclusion:

- structural streaming keeps the `1M` debug run memory-flat enough to emit a replayable artifact
- the remaining finish-line risk is no longer obvious `1M` RSS blow-up; it is higher-horizon `10M+` wall-clock completion

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

- process elapsed went past `31m`
- `ps`-reported RSS stayed around `440 MiB` instead of the earlier multi-GiB structural spike
- persisted synthetic root stabilized around `archive=5.0 GiB`, `derived=2.2–2.7 GiB`
- the run still did **not** emit `full-10m-60y-post-streaming.json`
- local `sample` could not inspect the process without elevated permissions, so there is no fresh stack snapshot for this run

Conclusion:

- structural streaming appears to have removed the earlier `10M` RSS cliff
- `10M` signoff is still not closed on this host because wall-clock completion remains open even after the memory shape improved
- `14.4M` and large-host queue-recovery should remain open until the release run actually emits a final artifact and the follow-up replay path can reuse that completed root

## Real-archive replay status

The current real app root is encrypted:

- `~/Library/Application Support/com.yi-ting.pathkeep/config.json` reports `"archiveMode": "Encrypted"`
- CLI keychain retrieval via `security find-generic-password -s com.yi-ting.pathkeep -a database-key -w` returns exit code `128`
- no user-supplied session key was available during this run

So the real-archive replay slice is still blocked on one of:

1. the archive session key
2. a user-approved disposable replay target if the real app root should not be exercised directly

## Current truth

`PG-RD-AI-011` remains open.

What changed today is not the final closeout; it is the quality of the blocker:

- daily-rollup fallback is no longer the first obvious `10M` bottleneck
- queued enrichment execution no longer depends on legacy `insights` ownership
- persisted synthetic roots plus `--app-root --skip-baseline-rebuild` are now enough to support follow-up recovery/query-validation runs once a completed large-host root exists
- the remaining large-host blocker is now wall-clock structural completion plus real-archive unlockability, not the earlier structural RSS cliff
