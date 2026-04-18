# 2026-04-17 Core Intelligence Signoff

## Scope

- Work block: `WORK-CI-B`
- Research item: `PG-RD-AI-011`
- Host: primary macOS development machine (`18` CPU cores, `64 GiB` RAM, local workspace)
- Purpose: turn the previous signoff attempts into replayable, final synthetic evidence for `WORK-CI-B`, then reduce the remaining blocker to the real encrypted-app-root replay only

## Code changes exercised before these runs

- live queued enrichment execution moved out of legacy `vault-core::insights` and into `enrichment.rs`
- benchmark harness now supports `--persist-app-root` so large synthetic corpora can be replayed without reseeding every scenario
- benchmark harness now also supports `--skip-baseline-rebuild` for existing `--app-root` follow-up scenarios, and it refuses that flag unless the replay target already has a Core Intelligence read model
- `daily-rollup` full/fallback rebuilds now aggregate in SQLite instead of hydrating every derived visit row into Rust
- `visit-derive` fallback persistence now uses a prepared statement inside one transaction
- structural rebuild no longer clones the full `tail_visits` buffer before session/trail assignment
- `build_sessions` no longer does an `O(sessions × visits)` second-pass rescan; session aggregates are accumulated during the first pass
- structural rebuild now streams tail visits in batches, range-clears affected memberships instead of collecting giant `visit_id` lists, and rebuilds `source_effectiveness` from batched `search_trails` reads rather than a whole-profile reload
- archive / intelligence SQLite connections now use larger cache windows plus best-effort mmap/WAL tuning suited for multi-million-row rebuilds
- archive migration `007_visible_profile_time_index.sql` adds the visible profile-time/id lookup index used by large profile scans
- Core Intelligence schema migration `batch-read-indexes` adds `visit_derived_facts(profile_id, visit_id)`, `search_events(profile_id, visit_id)`, and `search_trails(profile_id, first_visit_ms, trail_id)` indexes
- visit-derived-facts and structural tail batch readers now start from `archive.visits` ordered by `source_profile_id + visit_time_ms + id`, which removes the repeated temp-sort behavior exposed by `EXPLAIN QUERY PLAN`
- benchmark artifacts now expose `stageMs`, and `stageTimingsMs` now sum across all profiles instead of only reflecting the first profile

## Final synthetic evidence

### Smoke `2k / 60y`

Command:

```bash
cargo run -p vault-core --example intelligence-benchmark --manifest-path src-tauri/Cargo.toml -- \
  --visits 2000 \
  --window-days 365 \
  --horizon-days 21900 \
  --scenario full \
  --persist-app-root "$HOME/Library/Caches/pathkeep-benchmarks/2k-signoff" \
  --output artifacts/benchmarks/2026-04-17-intelligence-signoff/full-2k-smoke-signoff.json
```

Observed:

- baseline full rebuild: `158 ms`
- query surfaces: `11 ms`
- stage totals: `visit-derive=47 ms`, `daily-rollup=39 ms`, `structural-rebuild=60 ms`, `total=146 ms`
- peak RSS: about `22.8 MiB`

### Debug `1M / 60y`

Command:

```bash
cargo run -p vault-core --example intelligence-benchmark --manifest-path src-tauri/Cargo.toml -- \
  --visits 1000000 \
  --window-days 365 \
  --horizon-days 21900 \
  --scenario full \
  --persist-app-root "$HOME/Library/Caches/pathkeep-benchmarks/1m-signoff" \
  --output artifacts/benchmarks/2026-04-17-intelligence-signoff/full-1m-60y-signoff.json
```

Observed:

- baseline full rebuild: `106,503 ms`
- query surfaces: `375 ms`
- stage totals: `visit-derive=30,135 ms`, `daily-rollup=3,761 ms`, `structural-rebuild=71,322 ms`, `total=105,218 ms`
- peak RSS: about `780.7 MiB`
- corpus: `1,000,000` visits, archive about `520.0 MiB`, search terms about `166,667`

Conclusion:

- the corrected stage totals now match the whole-job wall clock closely, which makes this artifact honest enough to compare against `10M`
- structural rebuild is still the dominant stage, but the `1M` run is no longer ambiguous about where the time goes

### Release `10M / 60y` rebuild replay on durable root

The durable synthetic root was created with the same `10M / 60y` parameters under:

```bash
$HOME/Library/Caches/pathkeep-benchmarks/10m-signoff
```

The kept artifact is the rebuild-only replay against that existing root:

Command:

```bash
cargo run --release -p vault-core --example intelligence-benchmark --manifest-path src-tauri/Cargo.toml -- \
  --window-days 365 \
  --scenario full \
  --app-root "$HOME/Library/Caches/pathkeep-benchmarks/10m-signoff" \
  --output artifacts/benchmarks/2026-04-17-intelligence-signoff/full-10m-60y-signoff.json
```

Observed:

- baseline full rebuild: `2,078,480 ms`
- query surfaces: `1,250 ms`
- stage totals: `visit-derive=469,343 ms`, `daily-rollup=19,351 ms`, `structural-rebuild=1,579,679 ms`, `total=2,068,373 ms`
- peak RSS: about `1.44 GiB`
- corpus: `10,000,000` visits, archive about `5.17 GiB`, intelligence about `5.97 GiB`, search terms about `1,666,667`

Conclusion:

- the old “release `10M` never emits a final artifact” blocker is gone on this host
- structural rebuild is still overwhelmingly the dominant stage, but it is now measured honestly with a completed artifact instead of inferred from aborted runs

### Release `10M / 60y` expired-lease recovery replay

Command:

```bash
cargo run --release -p vault-core --example intelligence-benchmark --manifest-path src-tauri/Cargo.toml -- \
  --window-days 365 \
  --scenario expired-lease-recovery \
  --app-root "$HOME/Library/Caches/pathkeep-benchmarks/10m-signoff" \
  --skip-baseline-rebuild \
  --output artifacts/benchmarks/2026-04-17-intelligence-signoff/expired-lease-recovery-10m-signoff.json
```

Observed:

- baseline rebuild skipped as intended
- query surfaces: `1,237 ms`
- peak RSS: about `598.4 MiB`
- recovered jobs:
  - `expired-lease-queued` -> `queued` with `PathKeep recovered an expired intelligence lease.`
  - `expired-lease-cancelled` -> `cancelled` with `cancelled from UI`
- runtime recovery time: `0 ms`

Conclusion:

- the durable `10m-signoff` root is now good enough to support follow-up replay without reseeding
- expired-lease recovery behavior is no longer hypothetical benchmark plumbing; it now has a replayable `10M` artifact

## Real-archive replay status

The current real app root is encrypted:

- `~/Library/Application Support/com.yi-ting.pathkeep/config.json` reports `"archiveMode": "Encrypted"`
- the disposable replay target is already copied to `~/Library/Caches/pathkeep-benchmarks/real-replay-copy`
- CLI keychain retrieval via `security find-generic-password -s com.yi-ting.pathkeep -a database-key -w` still does not return a usable key on this host
- no user-supplied session key was available during this run

So the real-archive replay slice is still blocked on one of:

1. the archive session key
2. a local source for that key that can be used against the already-prepared disposable replay target

## Current truth

`PG-RD-AI-011` remains open.

What changed today is the quality of the blocker:

- synthetic `2k / 1m / 10m / queue-recovery` evidence is now complete
- corrected stage totals show that structural rebuild is the real remaining runtime heavyweight, not daily-rollup or the old RSS cliff
- the only finish-line artifact still missing for `WORK-CI-B` is the session-key-backed real replay against the disposable encrypted app-root copy
