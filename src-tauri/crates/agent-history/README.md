# Agent history core

This Rust crate preserves local agent conversation evidence in an independent
encrypted archive. It supplies source discovery, retained revisions, native-event
projections, bounded queries, lexical search and exports. It does not launch an
agent, call a model, fetch cloud history, modify native records or integrate the
PathKeep UI.

Start with the [capability matrix](../../../docs/features/agent-history.md),
[research index](../../../docs/architecture/research/agent-history/README.md) and
[maintainer notes](../../../docs/architecture/research/agent-history/implementation-notes.md).
The archive is separate from both browser history and PathKeep's own assistant
store. These APIs are synchronous: embedding applications must call them from a
blocking/background worker.

## CLI

From the repository root, this development helper invokes the real CLI:

```sh
ah() {
  cargo run --manifest-path src-tauri/Cargo.toml \
    -p agent-history --bin pathkeep-agent-history -- "$@"
}

ah help --locale zh-TW
ah plan --source "pi=$HOME/.pi/agent" --source "codex=$HOME/.codex"
```

`plan` reads an inventory without creating an archive or opening conversation
bodies. Its `sources` list is a bounded sample, not a frozen execution manifest;
`total_files`, `total_bytes`, `truncated` and `issues` describe the inventory.
Execution rediscovers the explicitly selected roots and revalidates files.
Omitting `--source` uses known local defaults. Repeat `--source harness=path` for
custom profiles; it replaces the default set for that invocation.

Harness identifiers are `claude-code`, `codex`, `pi`, `deepseek`, `continue`,
`cline`, and `roo-code`. Editor global-storage layouts and custom task roots
should be supplied explicitly; an adapter name does not imply that every editor
profile or alternate storage generation can be discovered automatically.

Commands opening an archive require `--password-stdin`. Pipe the password from
your chosen local secret-input tool; it is never a command-line argument. In the
examples below, **`password-input` is a placeholder for that tool**, not a command
provided by PathKeep. One trailing LF or CRLF is removed from stdin.

```sh
password-input | ah init --archive /private/path/agent-archive --password-stdin

password-input | ah backup --archive /private/path/agent-archive --password-stdin \
  --source "pi=$HOME/.pi/agent" --source "codex=$HOME/.codex" --execute

password-input | ah list --archive /private/path/agent-archive --password-stdin \
  --harness pi --sort updated --limit 100

password-input | ah sources --archive /private/path/agent-archive --password-stdin \
  --harness codex --kind transcript --limit 100

password-input | ah events --archive /private/path/agent-archive --password-stdin \
  --session ARCHIVE_SESSION_ID --limit 100

password-input | ah revisions --archive /private/path/agent-archive --password-stdin \
  --source-id SOURCE_ID --limit 100

password-input | ah reparse --archive /private/path/agent-archive --password-stdin \
  --source-id SOURCE_ID --execute

password-input | ah search --archive /private/path/agent-archive --password-stdin \
  --query "database migration" --limit 20

password-input | ah export --archive /private/path/agent-archive --password-stdin \
  --session ARCHIVE_SESSION_ID --format jsonl --output /private/path/events.jsonl --execute

password-input | ah raw-export --archive /private/path/agent-archive --password-stdin \
  --revision REVISION_ID --output /private/path/original-capture.bin --execute

password-input | ah verify --archive /private/path/agent-archive --password-stdin
password-input | ah sync-search --archive /private/path/agent-archive --password-stdin
password-input | ah sync-search --archive /private/path/agent-archive --password-stdin --rebuild
```

Use IDs returned by `list`／`sources`／`revisions`; native harness UUIDs are not archive
session IDs. `list`, `sources`, `events` and `revisions` accept the returned `next_cursor` as
`--after`. A stale cursor is rejected rather than silently mixing publications.
`list` also supports `--project` with an exact recorded path and `--sort created`.
Page limits are 1–500; search limits are 1–100.

`sources` lists every retained artifact, including attachments, metadata and
unparseable transcripts after their live files disappear. Unlike the bounded
preview, its pages cover the complete retained inventory. Use its source IDs and
`latest_revision_id` with `revisions` and `raw-export` for recovery.

`reparse` creates a new interpretation from the source's most recently retained
native revision, even after the live source has disappeared. It reads only
archived bytes and durable attachment bindings, does not accept `--source`, and
does not fetch newly referenced files. Missing bindings produce diagnostics. Use
it after a parser change; raw revision bytes remain unchanged.

Output is JSON. Errors have stable codes and localized messages (`en`, `zh-CN`,
`zh-TW`). A successfully returned backup report can still have `status: partial`;
consume its status and diagnostics, not only the process exit code. Write commands
for backup/reparse/export require `--execute`; initialization and explicit index repair
are separate commands. Export refuses occupied destinations and destinations
inside source/archive roots.

## What the data means

| Result            | Guarantee and boundary                                                                                                                                             |
| ----------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------ |
| Raw revision      | Exact captured bytes for ordinary files, including compression and malformed tails. Historical revisions survive source deletion.                                  |
| SQLite evidence   | A committed read-transaction snapshot in `pathkeep-sqlite-evidence-v1`, with schema and typed rows. It is not a native SQLite/WAL byte copy or a restore database. |
| Native event      | A source ordinal, interpreted fields and untouched parsed JSON payload. Unknown fields remain available. It is not necessarily one distinct human message.         |
| Normalized export | JSONL or Markdown containing session provenance and native events. Branches, mirrored records and compaction evidence are not replayed into one conversation.      |
| Search result     | Literal, case-insensitive token AND search of projected text, with exact harness/project filters. Top-k only; no search continuation cursor.                       |
| Integrity report  | Checks retained bytes and database integrity, plus search freshness. Intact bytes do not imply complete normalization or all remote attachments captured.          |

`SearchPage.total` counts matching native events, not distinct sessions. CJK
characters are individual tokens; there is no phrase, regex, fuzzy or semantic
search. An index behind canonical publication returns `IndexBehind`; ordinary
search does not secretly rebuild it. Use `sync-search` to synchronize it.
Use `sync-search --rebuild` to explicitly recreate a damaged derived index from
canonical projections; ordinary synchronization does not mask corruption.

All export reports set `native_resume_ready: false`. Markdown includes literal
text and native JSON evidence; it is not an executable document or a rendered
native agent transcript. Raw exports are plaintext at the caller's chosen writer
or destination; storage at rest remains encrypted.

## Library boundary

| Module         | Public entry points / responsibility                                                                                                            |
| -------------- | ----------------------------------------------------------------------------------------------------------------------------------------------- |
| `capture`      | `default_sources`, `preview`, `source_identity`, `Archive::backup`, `Archive::reparse_source`; live capture or retained-evidence interpretation |
| `adapters`     | `Adapter`, `adapter(Harness)`; source-specific discovery and interpretation, no source writes                                                   |
| `archive`      | `Archive::create` / `open`, retained revision and projection operations, run reports                                                            |
| `query`        | `list_sources`, `list_sessions`, `session`, `events`, `revisions`; bounded stable views                                                         |
| `search`       | `search`, `synchronize_search`, `rebuild_search`; encrypted derived full-text index                                                             |
| `export`       | `write_raw`, `write_normalized`; stream to a caller-owned writer                                                                                |
| `sqlite`       | `capture_sqlite_with_options`; bounded coherent logical evidence                                                                                |
| `verification` | `Archive::verify`; retained evidence audit                                                                                                      |

Most integrations should use `Archive::backup` rather than assembling the
low-level begin/append/publish protocol themselves. A writer can receive
provisional bytes before an error; publish a destination only after successful
completion. The CLI already stages exports privately and publishes without
overwriting existing files. Keep full error/coverage states when wrapping the
library in a typed backend client.

## Tests and verification

Failure scenarios and tests preceded production implementation; execution took
place in the final verification phase. Core tests passed 220/220 on macOS and
Linux; `bun run check` passed with 135 desktop E2E cases. The Linux release CLI
also passed a 14.4M-event synthetic run with enforced 4 CPU / 8 GiB limits.
See the [verification record](../../../docs/architecture/research/agent-history/validation/README.md)
for measurements, exact scope, real local samples and platform gaps.

```sh
cargo test --manifest-path src-tauri/Cargo.toml -p agent-history
cargo test --manifest-path src-tauri/Cargo.toml -p agent-history --test cli_e2e
bun run check
```

The CLI E2E uses real files, compressed logs, encryption, reopen and exports;
fixtures contain synthetic content in researched native formats. It writes
repeatable evidence under `artifacts/agent-history/`. See
[tests/README.md](tests/README.md) for failure contracts. Private local verification
uses isolated copies and must never commit real conversations or credentials.
