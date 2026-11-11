# Agent history contract tests

These tests were written before the core implementation. Per the task's explicit
instruction, execution took place in the final verification phase. Results and
platform limits are in `docs/architecture/research/agent-history/validation/`.

The CLI is the production entry point, with real files, compressed native logs,
encrypted storage, reopen operations and actual exports. No archive or parser is
mocked. All content is synthetic; native schemas are grounded in the pinned
sources in `docs/architecture/research/agent-history/`.

Failure contracts:

- Native IDs reused by another harness or another root must not merge archives.
- Compressed logs, all branches, unknown events and duplicate assistant response
  IDs must survive backup; lower DeepSeek generations remain raw evidence.
- A plan must not initialize an archive, and a write requires explicit execution.
- Source bytes and modification times must survive discovery, backup and export.
- A same-size rewrite with the original timestamp, truncation, deletion, or a
  partial final JSON record must not erase an older retained revision.
- Missing or escaping referenced files produce explicit partial coverage; text
  in a conversation does not grant access to unrelated files.
- Search must include the end of a conversation larger than 8 MiB, survive
  reopening, and find newly appended content without retaining replaced content.
- Pagination must cover each session/event exactly once with a stable cursor.
- Raw exports reproduce compressed or malformed source bytes exactly; normalized
  exports retain native payloads and Markdown remains a readable derivative.
- Wrong passwords, encrypted-state corruption and occupied output paths cannot
  silently produce successful results or destroy existing data.

Every CLI scenario writes `fixture.json` and `report.json` below
`artifacts/agent-history/<scenario>/`. The report includes expected/actual checks
and command results even when an assertion panics. Re-run with
`cargo test --manifest-path src-tauri/Cargo.toml -p agent-history --test cli_e2e`.
Crash injection, live writer races and platform-specific permission behavior are
separate integrity contracts; this CLI suite does not pretend to cover them.
