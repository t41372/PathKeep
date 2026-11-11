# Agent history verification artifacts

The `agent-history` crate's real CLI E2E writes synthetic fixture descriptions,
command outcomes and assertions here. Crash scenarios also retain their
checkpoint and recovery reports. Files are ignored except this guide.

Run the crate through the workspace gate, or run its focused suite:

```sh
cargo test --manifest-path src-tauri/Cargo.toml -p agent-history
```

`scripts/agent-history-verify.py` additionally supports a synthetic scale probe
and explicitly requested local-sample validation. Local samples, archives and
exports stay in a private temporary directory and are removed afterward. Its
report contains aggregate counts and outcomes, never private conversations,
native paths, titles, session identifiers, or content hashes.

An artifact proves only the checks and platform it records. A passing synthetic
fixture is not proof of complete cloud history or an untested native version.
