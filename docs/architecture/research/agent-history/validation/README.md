# Verification — 2026-11-11

These are aggregate results; no private conversation bytes, native session IDs,
titles, paths or per-file hashes are committed.

- [Gate](gate.json): macOS core tests 220/220, strict Clippy, six verifier tests,
  and `bun run check` including 135 desktop E2E cases. The gate ran against the
  shared working tree with its pre-existing uncommitted work preserved.
- [Local native samples](macos-native-samples.json): seven files from Claude Code,
  Codex, pi and DeepSeek. Every raw export matched its source, the originals were
  unchanged, reopening was idempotent, and temporary copies were removed. This
  sample deliberately lacks some parent records; coverage correctly remains partial.
- [macOS 100k](macos-100k.json): debug CLI, one synthetic Pi transcript; all 100,000
  events are searchable and the tail matches once. Timings include startup/unlock.
- [Linux 14.4M summary](linux-14m-summary.json) and [full report](linux-14m.json):
  release CLI with enforced 4 CPU / 8 GiB limits, exact full-corpus counts, raw
  byte verification, reopening and archive integrity. Linux core tests also passed
  220/220. This is not a Windows or many-session discovery benchmark, and the CPU
  clock was not calibrated to 3 GHz.
- [Compute lifecycle](modal-lifecycle.json): every task sandbox stopped; no source
  snapshot, installed skill or repo Modal config was changed. The first attempt
  lacked GNU time. The second was stopped because scalar resource requests allow
  bursting. The successful attempt used creation-time CPU `(4,4)` and memory
  `(8192,8192)` tuples, plus a 90-minute hard timeout. The temporary runner changed
  only those two arguments; Linux test processes still ran as the unprivileged user.

Reproduce locally after building the CLI:

```sh
cargo build --manifest-path src-tauri/Cargo.toml -p agent-history --bin pathkeep-agent-history
python3 scripts/agent-history-verify.py --synthetic-records 100000
python3 scripts/agent-history-verify.py --local-samples
```

For large-scale validation, use `--synthetic-records 14400000` with a release
binary, sufficient disk, GNU time and an explicit resource/time bound. Never run
`--local-samples` remotely. Native sample selection is repeatable as a procedure,
but selects currently eligible files; private fixture identities are intentionally
not retained. Synthetic fixtures are deterministic by count and runner version.
