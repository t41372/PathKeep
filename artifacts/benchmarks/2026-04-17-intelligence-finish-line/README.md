# 2026-04-17 Core Intelligence Finish-Line Benchmarks

## Environment

- Host: primary macOS development machine
- Date: 2026-04-17
- Command root: `/Users/tim/LocalData/coding/2026/Lab/8_chrome_history_backup`
- Runtime shape:
  - deterministic / Core Intelligence only
  - no LLM provider
  - no embedding provider
  - benchmark harness: `src-tauri/crates/vault-core/examples/intelligence-benchmark.rs`

## Harness Contract

The benchmark example now supports two replay modes:

1. Synthetic seed
   - uses a disposable temp app root
   - seeds canonical archive rows, then runs full rebuild + scenario follow-up
2. Existing archive replay
   - uses `--app-root <PathKeep app root>`
   - intended for a disposable copy of a real app root
   - encrypted archives require `--session-key`

Synthetic command pattern:

```bash
cargo run -p vault-core --example intelligence-benchmark --manifest-path src-tauri/Cargo.toml -- \
  --visits <n> \
  --window-days 365 \
  --horizon-days <days> \
  --scenario <full|append-delta|visibility-regression-fallback|expired-lease-recovery> \
  --output <artifact>
```

Existing-archive replay pattern:

```bash
cargo run -p vault-core --example intelligence-benchmark --manifest-path src-tauri/Cargo.toml -- \
  --app-root "<disposable PathKeep app root copy>" \
  --session-key "<session key for encrypted archives>" \
  --window-days 365 \
  --scenario full \
  --output <artifact>
```

The example writes:

- source kind and replay command
- corpus stats
- baseline rebuild / query timings
- peak RSS snapshots
- scenario-specific follow-up timings and reports

## Captured Artifacts

### `low-ram-fallback-100k-60y.json`

Command:

```bash
cargo run -p vault-core --example intelligence-benchmark --manifest-path src-tauri/Cargo.toml -- \
  --visits 100000 \
  --window-days 365 \
  --horizon-days 21900 \
  --scenario visibility-regression-fallback \
  --output artifacts/benchmarks/2026-04-17-intelligence-finish-line/low-ram-fallback-100k-60y.json
```

Observed:

- baseline full rebuild: `48,982 ms`
- query surfaces: `65 ms`
- peak RSS: `142.19 MiB`
- visibility-regression fallback:
  - `visit-derive`: `1,570 ms`, `executionMode=fallback-full`, `dirtyVisitCount=24,976`
  - `daily-rollup`: `945 ms`, `executionMode=fallback-full`, `dirtyVisitCount=24,976`

### `expired-lease-recovery-100k-60y.json`

Command:

```bash
cargo run -p vault-core --example intelligence-benchmark --manifest-path src-tauri/Cargo.toml -- \
  --visits 100000 \
  --window-days 365 \
  --horizon-days 21900 \
  --scenario expired-lease-recovery \
  --output artifacts/benchmarks/2026-04-17-intelligence-finish-line/expired-lease-recovery-100k-60y.json
```

Observed:

- baseline full rebuild: `49,069 ms`
- query surfaces: `63 ms`
- peak RSS: `141.52 MiB`
- runtime expired-lease recovery: `1 ms`
- recovery outcome:
  - `expired-lease-cancelled` -> `cancelled`
  - `expired-lease-queued` -> `queued` with `"PathKeep recovered an expired intelligence lease."`

## Manual Replay Truth

The harness now rejects encrypted existing-archive replay unless a session key is supplied.

Local truth on this machine:

- a real PathKeep app root exists under `~/Library/Application Support/com.yi-ting.pathkeep`
- its `config.json` currently declares `archiveMode = "Encrypted"`
- no benchmark session key was available during this run

Observed command failure:

```bash
cargo run -p vault-core --example intelligence-benchmark --manifest-path src-tauri/Cargo.toml -- \
  --app-root "$HOME/Library/Application Support/com.yi-ting.pathkeep" \
  --window-days 365 \
  --scenario full
```

Result:

- `manual replay against an encrypted archive requires --session-key or a disposable plaintext copy`

This means the manual/real replay path is now implemented and documented, but **real-archive evidence is still open** on this host until a disposable plaintext copy or session key is provided.

## Still Open

- `10M` synthetic replay
- `14.4M` synthetic replay
- real-archive replay artifact from a disposable copy with a valid session key

`PG-RD-AI-011` should remain open until those missing evidence slices exist.
