# 2026-04-12 Intelligence Rewrite Benchmarks

## Environment

- Host: primary macOS development machine
- Date: 2026-04-12
- Command root: `/Users/tim/LocalData/coding/2026/Lab/8_chrome_history_backup`
- Runtime shape:
  - plaintext temp archive
  - no LLM provider
  - no embedding provider
  - deterministic rebuild only

## Harness

- Source: `src-tauri/crates/vault-core/examples/intelligence-benchmark.rs`
- Entry command pattern:

```bash
cargo run -p vault-core --example intelligence-benchmark --manifest-path src-tauri/Cargo.toml -- --visits <n> --window-days 365 --horizon-days <days> --output <artifact>
```

## Captured Artifacts

- `100k.json`
  - command:

```bash
cargo run -p vault-core --example intelligence-benchmark --manifest-path src-tauri/Cargo.toml -- --visits 100000 --window-days 365 --output artifacts/benchmarks/2026-04-12-intelligence-rewrite/100k.json
```

- result:
  - corpus: `100,000` visits over a 2-year horizon
  - `runInsightsMs = 40104`
  - `loadInsightsMs = 591`

- `100k-60y.json`
  - command:

```bash
cargo run -p vault-core --example intelligence-benchmark --manifest-path src-tauri/Cargo.toml -- --visits 100000 --window-days 365 --horizon-days 21900 --output artifacts/benchmarks/2026-04-12-intelligence-rewrite/100k-60y.json
```

- result:
  - corpus: `100,000` visits over a 60-year horizon
  - current 365-day analysis window touched `1,460` visits
  - `runInsightsMs = 1032`
  - `loadInsightsMs = 91`

- `1m-60y.json`
  - command:

```bash
cargo run -p vault-core --example intelligence-benchmark --manifest-path src-tauri/Cargo.toml -- --visits 1000000 --window-days 365 --horizon-days 21900 --output artifacts/benchmarks/2026-04-12-intelligence-rewrite/1m-60y.json
```

- result:
  - corpus: `1,000,000` visits over a 60-year horizon
  - current 365-day analysis window touched `16,425` visits
  - `runInsightsMs = 13052`
  - `loadInsightsMs = 921`

## Current Reading

- The rewrite materially improved deterministic rebuild shape for long-horizon archives where the active analysis window is bounded.
- The current bottleneck is still window materialization: runtime cost scales with the active window, not total archive size alone.
- These artifacts do not yet sign off 10M scale, queue recovery RSS, or native desktop shell responsiveness under the same corpus.
