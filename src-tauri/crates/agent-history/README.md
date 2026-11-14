# Agent history core

This Rust crate preserves local agent conversation evidence in an independent
archive with optional encryption. It supplies source discovery, retained revisions, native-event
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

Encrypted archives require `--password-stdin`. Unencrypted archives are created with
`init --no-encryption` and reject passwords on later commands. Pipe the password from
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
| SQLite evidence   | A committed read-transaction snapshot in `pathkeep-sqlite-evidence-v2`, with schema and typed rows. It is not a native SQLite/WAL byte copy or a restore database. |
| Native event      | A source ordinal, interpreted fields and untouched parsed JSON payload. Unknown fields remain available. It is not necessarily one distinct human message.         |
| Normalized export | JSONL or Markdown containing session provenance and native events. Codex inherited segments form the current line; native branch/mirror payloads remain evidence.  |
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
or destination; storage at rest follows the archive mode. Database spill files remain encrypted in both modes.

## Library boundary

| Module         | Public entry points / responsibility                                                                                                            |
| -------------- | ----------------------------------------------------------------------------------------------------------------------------------------------- |
| `capture`      | `default_sources`, `preview`, `source_identity`, `Archive::backup`, `Archive::reparse_source`; live capture or retained-evidence interpretation |
| `adapters`     | `Adapter`, `adapter(Harness)`; source-specific discovery and interpretation, no source writes                                                   |
| `archive`      | `Archive::create` / `open`, retained revision and projection operations, run reports                                                            |
| `query`        | `list_sources`, `list_sessions`, `session`, `events`, `revisions`; bounded stable views                                                         |
| `search`       | `search`, `synchronize_search`, `rebuild_search`; derived full-text index in the archive encryption mode                                        |
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
place in the final verification phase. The earlier format passed 220/220 on macOS and
Linux on 2026-11-11; `bun run check` passed with 135 desktop E2E cases. The Linux release CLI
also passed a 14.4M-event synthetic run with enforced 4 CPU / 8 GiB limits.
See the [verification record](../../../docs/architecture/research/agent-history/validation/README.md)
for measurements, exact scope, real local samples and platform gaps.

```sh
cargo test --manifest-path src-tauri/Cargo.toml -p agent-history
cargo test --manifest-path src-tauri/Cargo.toml -p agent-history --test cli_e2e
CARGO_PROFILE_DEV_DEBUG=0 CARGO_PROFILE_TEST_DEBUG=0 TZ=Pacific/Kiritimati cargo clippy --manifest-path src-tauri/Cargo.toml -p agent-history --all-targets --all-features -- -D warnings
cargo fmt --manifest-path src-tauri/Cargo.toml -p agent-history -- --check
```

The CLI E2E uses real files, compressed logs, encryption, reopen and exports;
fixtures contain synthetic content in researched native formats. It writes
repeatable evidence under `artifacts/agent-history/`. See
[tests/README.md](tests/README.md) for failure contracts. Private local verification
uses isolated copies and must never commit real conversations or credentials.

## Evidence format work block (2026-11-13)

This unreleased format changes in place. The fix-round rules below supersede the
first implementation’s origin-mark and concurrency assumptions. There is no migration or compatibility
path for the previous schema. The broader core/feature documents describe the
previous format; this section and the implementation notes record this block's
amendments. The next read-model block still replaces the per-event projection.

### Plan and maintainer review

The planning pass checked the design, observable contract, native-format notes,
new CLI scenarios and existing capture/publication APIs before format code.
The sequence was release blob measurement; maintainer review of identity, schema,
source relations, retention and crash recovery; implementation; crate-only tests,
strict Clippy and rustfmt. No desktop build, dependency addition or commit belongs
to this block. Only synthetic data was used.

The initial parent-ID contradiction was reported and resolved by the user before
this implementation. Parent IDs are now looked up from native evidence at read
time. The maintainer recommended keeping reproducible source membership in the
hot publication rather than duplicating it across databases. Its objections were
addressed: expired state A→B→A gets fresh manifests; prefix links are flattened;
both Codex boundaries are validated; metadata-less files get source-keyed
conversations; hot metadata and relations publish together; a captured unreadable DeepSeek
generation keeps a reachable empty view, rather than substituting an older line. A second
review caught source reassignment hiding a surviving conversation, ambiguous
lineage borrowing a stranger's bytes, and a git path canonicalization race. The
implementation reselects surviving sources atomically, rejects ambiguous ancestor
identities, and opens the original git pathname through directory descriptors.

### Blob measurement and decision

Run `examples/blob_measurement.rs` in release mode using this workspace target.
It generates JSONL with synthetic message/tool-shaped repetition and distinct
content, compresses each slice separately at zstd level 3, and uses vault-store's
encrypted Turso databases and encrypted temporary I/O. Writes are 32 separate
committed primary-key inserts. Reads use 12 reopened stores and eight subsequent
reads per store; decompression and password KDF are outside the measured I/O.
On macOS, each cold trial sweeps only the synthetic files with F_NOCACHE before
opening Turso. This is not a machine-wide cache purge or a power-on disk test.

| Uncompressed slice | Stored bytes | First write MiB/s | Subsequent writes MiB/s | Cold read median / max ms | Warm read median / p95 ms |
| ------------------ | -----------: | ----------------: | ----------------------: | ------------------------: | ------------------------: |
| 1 MiB              |      337,783 |             174.8 |                   222.8 |             0.543 / 0.670 |             0.061 / 0.532 |
| 4 MiB              |    1,358,842 |             249.7 |                   280.4 |             1.869 / 2.132 |             0.224 / 1.843 |

Throughput counts stored compressed bytes. Choose the single `SLICE_BYTES =
4 * 1024 * 1024` constant: warm reads are far below tens of milliseconds, so the
specified fallback to 1 MiB is unnecessary. Release artifacts for this crate were
removed with `cargo clean --release -p agent-history` after measurement.

### Observable behavior

`list` returns conversations with `sources`; its IDs are content-derived hashes.
Use `--put-away exclude|only|include`, `--trash exclude|only`, and `--starred`.
`mark --conversation ID --star true --put-away true --title TEXT --execute`
changes canonical user state. `--trash true|false` sets/clears a soft deletion;
a preview omits `--execute` and changes no state. Trash hides spawned descendants from
lists/search and totals; forks stay top-level. Put-away affects default lists,
while search marks hits. Parent and fork links have separate native/resolved fields.
`events --session ID` addresses a conversation and returns source/ordinal pairs.
`folders [--path PATH]` returns changed folder observations, newest first.
`backup --no-git` observes folder existence without reading git files. `plan`
names the fixed git read allowlist and its cap; it does not perform those reads.
`verify.slices` reports stored slice count and compressed bytes.

Ordinary appended transcripts retain every revision. A proven byte prefix keeps
its own digest/length and directly addresses its successor's manifest, without a
successor chain. Manifest byte offsets let an already-open reader continue across
retargeting, even with different producer slice boundaries. Export clips the last
slice to the requested revision. State keeps old revision records with
`bytes_retained=false`; only the latest verified snapshot keeps bytes. Documents
and write-once artifacts retain changed revisions whole. Native .zst/.zstd
transports are stored without another compression layer. Codex thread_history
SQLite databases are excluded; v2 SQLite evidence writes valid UTF-8 TEXT as JSON
strings and preserves other text bytes with their storage class and hex.

### Schema inventory

The full column definitions are in `src/archive/schema.rs`. Primary keys below
also account for SQLite/Turso's implicit key indexes; INTEGER PRIMARY KEY uses
the rowid. Both migrated databases have vault-store's `__pk_migrations(version)`;
the scratch role has no agent-history tables. No SQL views remain.

Evidence database (`archive/evidence.db`):

- `ah_sources` (PK id): physical provenance, retention kind, latest revision,
  last sighting and missing-since cache.
- `ah_revisions` (PK id): digest/length/time/format, publication state,
  bytes-retained flag, direct manifest target, source codec and staging counters.
- `ah_chunks` (PK hash): stored blob and codec.
- `ah_revision_chunks` (PK revision_id, ordinal): hash, length and byte offset.
- `ah_chunk_garbage` (PK hash): bounded reclamation candidates.
- `ah_user_state` (PK conversation_id): star, put-away, trash time and title.
- `ah_user_state_pending` (PK conversation_id): recoverable changed-row generation,
  removed only after its hot mirror commits.
- `ah_state_generation` (PK id=1): canonical user-state generation.
- `ah_source_observations` (PK run_id, source_id): seen/missing and observation time.
- `ah_folder_observations` (PK id): path, existence, time and sanitized git facts.
- `ah_parse_reports` (PK revision_id, parser_version): interpretation evidence.
- `ah_backup_runs` (PK id): durable run status, times and report.
- `ah_reference_bindings` (PK id): parent revision/native reference to target revision.
- `ah_reference_observations` (PK id): changed capture outcomes by parent revision
  and reference key; a null reason means kept, otherwise only the reason class.

Evidence explicit indexes:

- `ah_revisions_source_state(source_id,state,id)`;
  `ah_revisions_digest(source_id,digest,state)`;
  `ah_revisions_state(state,id)`; `ah_revisions_manifest(manifest_revision_id)`.
- `ah_sources_location(harness,root,relative_path)`;
  `ah_sources_head(latest_revision_id)`; `ah_sources_harness_id(harness,id)`;
  `ah_sources_kind_id(kind,id)`; `ah_sources_harness_kind_id(harness,kind,id)`.
- UNIQUE `ah_revision_chunks_offset(revision_id,byte_offset)`;
  `ah_revision_chunks_hash(chunk_hash)`.
- `ah_source_observations_source(source_id,run_id)`;
  `ah_folder_observations_path(path,id DESC)`;
  `ah_backup_runs_status(status,id)`;
  `ah_reference_bindings_lookup(parent_revision,reference_key,id DESC)`.
- `ah_reference_observations_lookup(parent_revision,reference_key,id DESC)`.

Projection database (`archive/catalog.db`):

- `ah_sessions` (PK id=source id): per-source published metadata, parser/revision
  token, conversation key, retained locator, native links, generation,
  copy relation, current-source flag, first-record digest, native filename time,
  generation group, current-only query policy, native parent/fork evidence,
  raw-head freshness, attachment-count generation and current total.
- `ah_events` (PK id): source/publication/ordinal, native fields, text and payload.
- `ah_lineage_validation` (PK revision_id,end_ordinal,byte_len): verified boundary
  with physical own-file ordinal and decoded prefix digest; matching lengths
  alone cannot select an ancestor.
- `ah_event_references` (PK projection_id,ordinal,reference_key): reference
  occurrences per native record, excluding lineage links.
- `ah_reference_outcomes` (PK parent_revision,reference_key): disposable mirror
  of the latest canonical capture outcome.
- `ah_attachment_generations` (PK id): staged, ready or retired counter batches.
- `ah_attachment_counts` (PK generation,ordinal): cumulative unavailable-reference
  counts at record boundaries, allowing bounded prefix queries.
- `ah_projection_gaps`: parser diagnostic positions by publication, used to
  distinguish inherited-prefix failures from failures in an abandoned tail.
- `ah_user_state` (PK conversation_id), `ah_state_generation` (PK id=1): disposable
  copies of canonical user edits, applied generation and hot visibility counter.
- `ah_meta` (PK key): publication/parser counters.
- `ah_coverage` (PK source_id): source/root coverage and whether a failure affects
  conversation normalization; attachment-only capture failures do not.
- `ah_outbox` (PK seq): publication work for search.
- `ah_projection_work` (PK session_id): unpublished attempt token.
- `ah_projection_retired` (PK session_id,projection_id): bounded cleanup queue.

Projection explicit indexes:

- `ah_sessions_copy(copy_of_source_id)`;
  `ah_sessions_order(conversation_id,generation DESC,updated_at DESC,source_time DESC,id DESC)`;
  `ah_sessions_group(harness,source_root,generation_group)`;
  `ah_sessions_conversation(conversation_id)`;
  `ah_sessions_rollout(harness,rollout_native_id,source_root)`;
  `ah_sessions_lineage(harness,lineage_base_native_id)`;
  `ah_sessions_parent(harness,parent_session_id)`;
  `ah_sessions_native(harness,native_id,source_root,relative_path)`.
- UNIQUE `ah_events_session_projection(session_id,projection_id,ordinal)`;
  `ah_coverage_complete(complete)`; `ah_outbox_session(session_id,seq)`.
- `ah_projection_gaps_lookup(projection_id,ordinal)`.
- `ah_sessions_updated(conversation_current,updated_at DESC,conversation_id DESC)`;
  `ah_sessions_created(conversation_current,created_at DESC,conversation_id DESC)`;
  `ah_sessions_harness_updated(conversation_current,harness,updated_at DESC,conversation_id DESC)`;
  `ah_sessions_harness_created(conversation_current,harness,created_at DESC,conversation_id DESC)`;
  `ah_sessions_project_updated(conversation_current,project_path,updated_at DESC,conversation_id DESC)`;
  `ah_sessions_project_created(conversation_current,project_path,created_at DESC,conversation_id DESC)`.

Identifiers:

- Source/root IDs hash the domain, harness and canonical root (plus relative path
  for a source), each with a big-endian u64 length prefix.
- Conversation IDs hash the similarly length-prefixed conversation domain,
  harness, native ID and origin mark. Claude Code/Pi origin: SHA-256 of the first record’s
  exact bytes. Claude native keys use the filename or parent id plus path below
  subagents. Codex and DeepSeek use the native ID alone;
  rollout/generation timestamps are not stable. Continue/Cline/Roo use the source
  ID as the mark because a normal save rewrites the whole document. Without a native ID the key is solely
  the source ID. Cross-conversation references never guess a foreign hash.
- Slice hashes address uncompressed bytes. Whole-revision digests address exact
  exported bytes, including the native compressed representation.
- Revision/run/binding/folder/event IDs and outbox sequence are local INTEGER PKs.
  Projection IDs come from the persisted next_projection_id counter. These are
  handles, never conversation identities. Ordinals/offsets come from source order.
  Reference keys retain the existing hash of kind/target/boundaries; parser
  versions cover adapter semantics and byte budgets.

### Decisions, rejected alternatives and remaining boundaries

- Retain 4 MiB zstd slices rather than 1 MiB: measured reads meet the stated
  threshold and larger slices retain more compression context. Hash original
  bytes rather than encoded blobs, so verification and deduplication are codec-independent.
- Direct prefix manifests and byte offsets replace retained tail copies and
  chained redirects. A transactional garbage-candidate queue replaces corpus-wide
  scans. Old and already-open readers still authenticate their own bytes.
- Expired state rows remain records; returning A→B→A gets a new verified revision
  rather than reusing an expired handle with no bytes. Documents retain every
  changed revision; no record-level document deduplication is introduced.
- Rebuildable source/conversation facts stay in the hot publication, not a second
  canonical mapping table. Irreplaceable observations and user edits stay in
  evidence. A pending-row state mirror provides indexed filters without
  cross-database joins, whole-table copies per mark or read/publication locks.
  Hot row updates and their visibility counter commit together; search retries
  if user state changes across its filter/hit reads. Cache recovery precedes
  exposing an opened Archive. Content scans precede the short publication commit.
- Full decoded bytes determine prefix copies rather than normalized event JSON,
  so malformed/unprojected bytes cannot merge divergent sources. The latest last-record time chooses branches; a tie uses
  native filename time. DeepSeek generations rank first. Byte-equivalent copies
  use a deterministic representative; content digests never choose a branch. Capture time never chooses a line.
- Foreign parent/fork/continuation links remain native evidence. Read-time lookup
  prefers the same root/layout and refuses ambiguous ancestor identities rather
  than choosing whichever matching length came first. Both ordinal and byte
  boundaries and their prefix digests are cached against immutable revisions, instead of
  rescanning a long prefix on each page. Unknown parent links do not lower Claude
  subagent completeness. Pi fork paths remain provenance, not invented parent IDs.
- The per-source event projection stays. Events/exports concatenate resolved
  inherited segments, retaining physical addresses. Search still indexes native
  projected records, including mirrors/branches/copies; this block does not build
  usage deduplication or the later read model.
- No-encryption mode has an explicit marker and no envelope, rather than an empty
  password pretending to be encryption. Persistent databases/search follow that
  mode; a fresh process-only key protects spills. Trash remains a reversible flag,
  not a claim that plaintext bytes have disappeared.
- Folder evidence is stored only on change rather than copied each run. Root
  access failures stay unknown rather than becoming missing sightings. Git uses
  the fixed bounded reader and a small config parser, never git execution,
  config includes, index/objects/hooks or a worktree crawl. Only structured cwd
  authorizes this exception; URL userinfo is removed before persistence.
- Direct indexed SQL replaces computed SQL views after Turso rejected the nested
  computed parent alias. The workaround stays local; no dependency patch or
  upstream contact was needed. The normal capture path withholds an unfinished first JSONL
  record; valid rewritten JSON documents need no line end. Failed decoders do
  not publish readable events; a trusted low-level producer can still publish its own opaque native
  interpretation, using the raw first-record digest if transport decoding fails.
- The schema changes in place and legacy projection/report wrappers are removed,
  rather than preserving an unreleased compatibility protocol. No new dependency,
  archive role, frontend or desktop integration is introduced.

Current validation uses synthetic native fixtures on this macOS host. It does not
revalidate Windows/Linux, real home histories, a 14.4M corpus, power-loss recovery
on physical storage, every cache-replay/retention crash point, git-directory race
scheduling, or multi-source performance at target-machine limits. The existing
process-exit capture/publication cases and new retention readers/reassignment
cases exercise real databases, but green tests are not proof of those unrun cases.
The pre-block cross-platform/14.4M numbers above remain historical evidence only.

First implementation round (historical): 259 tests passed across 31 result groups (including
zero-test doc results); the supplied CLI contract suites and strict Clippy passed.
The final gate uses CARGO_PROFILE_DEV_DEBUG=0, CARGO_PROFILE_TEST_DEBUG=0 and
TZ=Pacific/Kiritimati, followed by crate rustfmt --check. No tests were ignored.

## One fix round: WORK-AH-EVIDENCE-FORMAT

The amended contract is implemented without changing any assertion in the new
contract suites or either reviewer-strengthened replacement test. This round
changes no Cargo files, dependencies, other crates, app code or remote resources.
The original 4 MiB measurement remains the storage decision; it was not rerun.

| Finding                        | Implementation                                                                                                                                                                                                                                                                                                                                                                                                              | Verification                                                                                                                                             |
| ------------------------------ | --------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | -------------------------------------------------------------------------------------------------------------------------------------------------------- |
| 1. Retention reclassification  | `archive/sources.rs` retains any existing non-state policy after publication; a state policy may strengthen, never weaken retained bytes                                                                                                                                                                                                                                                                                    | `a_reclassified_source_keeps_every_revision_it_already_had`                                                                                              |
| 2. Native identity             | Each adapter declares its own key beside its evidence: Claude/Pi first-record digest, Codex/DeepSeek no mark, documents source mark. ID extraction remains independent of the projection budget                                                                                                                                                                                                                             | `codex_threads`, `a_rewritten_document_stays_one_conversation_with_its_user_state`, `a_larger_parser_budget_does_not_rename_a_known_native_conversation` |
| 3. Unfinished first record     | `capture/readiness.rs` gates JSONL publication, not low-level publication; bytes remain retained. Document adapters opt out                                                                                                                                                                                                                                                                                                 | `a_transcript_caught_mid_write_has_no_conversation_until_its_first_record_is_whole`                                                                      |
| 4. Fork versus child           | Adapter metadata and source relations carry separate native fork evidence; `query/parents.rs` resolves both links. Only parent links enter trash recursion                                                                                                                                                                                                                                                                  | `trashing_a_thread_hides_the_threads_it_spawned_and_leaves_its_fork`, copied-beginning case                                                              |
| 5. Current source              | `archive/selection.rs` and indexed re-election use generation, last-record time, then native filename time                                                                                                                                                                                                                                                                                                                  | chain/latest-record/filename-tie cases in `codex_threads`                                                                                                |
| 6. Reachability                | Retirement reselects remaining copies atomically. `archive/generations.rs` publishes an empty incomplete view only after an unreadable newer generation is captured, preserving prior native summary and canonical user state                                                                                                                                                                                               | DeepSeek stability case; `retiring_the_current_source_selects_a_remaining_copy_and_preserves_state`                                                      |
| 7. Reads during backup         | `query/line.rs`/`events.rs` resolve and page one hot snapshot, with a line-local publication fingerprint. Reads hold no publication/state mutex. Pending state mirrors never delete the active cache. Trash closure is one deduplicated recursive SQL query; memberships use bounded batches. Expiry has a typed reader error and verify skips it                                                                           | unrelated-file cursor and republished-base cursor cases, held-publication-owner test, membership bound test, expired-reader/export tests                 |
| 8. Git observations            | Resolve the session folder alias first; fixed git reads retain descriptor-relative no-follow traversal. Report `git_unreadable`, including invalid UTF-8. Do not read unused gitdir backlinks                                                                                                                                                                                                                               | amended git tests; `invalid_git_text_is_reported_as_unreadable`                                                                                          |
| 9. Work proportional to change | `archive/chunks.rs` attaches existing addresses before compression; missing data is compressed outside the writer and existence is rechecked atomically. Codec is recorded once per revision. Comparisons settle whole digests/aligned slices without body reads; only unmatched spans are read. Appends reuse existing copy proofs; equivalent copies are grouped by stored digest. Marks mirror pending changed rows only | prefix-sharing/retention suites, parser/reparse suites and existing real-store integrity tests                                                           |
| 10. Shape and ownership        | Split file authority/git reads, native rendering/validation, root normalization, line resolution and source membership by job. No src line exceeds 100 characters. Coverage/search use an adapter-declared current-only policy, not a tool name                                                                                                                                                                             | rustfmt, strict Clippy, grep; full crate suite                                                                                                           |

The maintainer reviewed both the plan and implementation. Its mirror-failure
objection led to the durable pending-row queue, so a failed change A cannot be
skipped when a later mark B advances the applied generation. Its final review
caught mixed search state and ambiguous same-thread rollout prefixes; both were
first reproduced, then fixed with a hot visibility counter and cached prefix
hashes respectively. No publication lock is held while Tantivy searches.

Further regressions written before the corresponding fixes cover a base reparse
during normalized export, retained corruption swallowed by the first-record gate,
large membership parameter lists, parser-budget identity changes, and raw export
losing the existing retention-expiry classification. The shared nonblocking file
opener already rejects FIFO inputs; inspection confirmed no extra FIFO workaround
was needed.

Two deliberate differences from the sketches: event pages use one committed
snapshot rather than optimistic retries; slice existence and manifest attachment
share one transaction, so reclamation cannot remove an observed existing slice
between those steps. Pending state rows are acknowledged only after mirroring;
normal marks do not rebuild the full table. Source integrity hashing and full raw
verification remain required by the preservation contract; compression and source
comparison avoid redoing already-settled work.

Removed code: the capture coordinator's no-metadata else branch was unreachable
after preparation supplies fallback metadata. The prefix cycle check was also
unreachable: sharing is called only for a strictly shorter verified prefix, and
a selected manifest's target is at least as long as that selected revision.

Coverage remains local and synthetic. No Windows/Linux runtime, target-machine
14.4M/multi-source benchmark, physical power loss, exhaustive cache/retention
fault-injection matrix, or adversarial directory-swap scheduling is claimed.

The fix round also rejects malformed native lineage IDs as filesystem authority:
Codex history_base and DeepSeek parentSession are lookup-only IDs; Pi alone
advertises lineage paths. Parent/fork provenance does not require another file's
bytes. Invalid UTF-8 git files report git_unreadable. These cases have dedicated
synthetic regressions. Retention expiry keeps its typed NotFound classification
through raw export and is excluded from verify's damage count.

No current-format assertion or fixture data was weakened. The three permitted
existing-test semantic changes and all new regressions are named in tests/README.md.
Only fixture borrowing and rustfmt layout changed in supplied contract files.

Resume-2 validation (historical): 283 tests passed in this clone, strict Clippy passed, and
all src lines are at most 100 characters. The final verification repeats the
three requested crate gates and the exact grep check after documentation updates.
The implementation remains uncommitted. No item from the ten-finding list is
left unresolved; platform and fault-injection coverage limits above still apply.

## Second fix round (resume-3)

Codex continuation boundaries now preserve both native meanings. The byte offset
ends line k of the named base's own decoded file; its whole-history ordinal must
equal that base's inherited ordinal plus k. The cached physical ordinal bounds
that source's records while recursive reads include its inherited prefix.
`sources.base_end_ordinal` remains the native whole-history number. The chain and
single-step cases in `codex_threads` and `conversation_identity` pin this rule.

Completeness is resolved from the current line when read. A valid chain becomes
complete as soon as its last missing base is captured, without reparsing its
child. Parser reports describe only their own records; the obsolete static
lineage diagnostic and unused adapter method were removed. List, search coverage,
verify normalization coverage and normalized export use the same chain rules.
Root failures and unprojected failed transcripts still lower global coverage.
Recorded parser-gap positions prevent an abandoned malformed tail from lowering
an otherwise readable inherited prefix.

Unavailable attachments have a separate `attachments_not_kept` occurrence count
on every list item. They do not lower conversation completeness. Structured
references stream with their record ordinals into the existing projection;
deduplicated target lists still bound capture work. Cumulative count breakpoints
avoid scanning projected events for each list item or inherited prefix. Counter
generations publish with source metadata and recover staged work on reopen.
Capture outcomes are canonical evidence so offline reparse reports the original
reason without probing live files. Diagnostic detail contains only
`outside_roots`, `missing`, `unsafe_path`, `authority_unavailable` or `remote`;
target paths and signed URLs never become diagnostic detail.

The maintainer reviewed the native/physical boundary separation, read-time chain
coverage and attachment accounting before implementation. Follow-up fixes keep
parser gaps and attachment occurrences bounded by the inherited prefix, preserve
failed raw-transcript/root coverage, and deduplicate actual target capture while
retaining each reference occurrence. The three additional regressions and the
five existing-test adaptations are listed in tests/README.md. Supplied contract
assertions and synthetic fixture bytes are unchanged.

Current validation is 288 passing tests on this macOS clone with the specified
debug/TZ settings, strict Clippy, rustfmt and the 100-character src check. No new
real-data run, platform run, large-corpus benchmark or exhaustive counter-publication
crash matrix is claimed. The earlier coverage limits still apply. This round
changes no dependencies, Cargo files or files outside the crate, and remains
uncommitted.
