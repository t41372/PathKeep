# Agent history core

Status: implementation contract, 2026-11-11; identity, what a run observes,
retention, user state and optional encryption were added 2026-11-13, for the
reasons in [agent-history-product-design.md](agent-history-product-design.md).
This document records the independently usable library requested by the user;
product UI, onboarding, scheduling and IPC integration remain outside this work.

## Scope and evidence

Required adapters: Claude Code (including locally retained desktop/Cowork records),
Codex CLI/desktop, pi, and `deepseek-ai/deepseek-harness`. Additional formats require
source evidence. Cloud-only history is not silently fetched. Resume and source
migration are research only. Source files are never modified.

Research lives in [research/agent-history](research/agent-history/). Each report
records upstream revisions, exact storage behavior, differences between raw backup
and searchable projections, and limitations. Local private examples stay outside
Git; checked-in fixtures contain synthetic content in verified native formats.

## Boundaries

The `agent-history` Rust crate owns discovery, capture, retained revisions,
normalization, queries, exports and capability reports. Its standalone
`pathkeep-agent-history` CLI is the real E2E entry point. It has no Tauri, UI,
provider credentials, cloud service or model dependency. It must not confuse
imported agent history with PathKeep's own assistant `derived/agent.db`.

Adapters describe source layouts and interpret native evidence. They do not own
archive writes, retention, project merging or UI labels. The capture engine
handles streaming I/O, source stability, cancellation and raw byte preservation.
The archive handles bounded transactions and publication. Queries use stable
identifiers and bounded pages. Source-specific data remains available alongside
the common model instead of being forced through a lossy message-only schema.

A **source** is one file. A **conversation** is what a person would call one: it
has one or more sources, an id derived from native evidence (never from a path, a
counter or capture order), links to a parent or to the conversation it was forked
from that are resolved when read, and user state (star, put away, trash, title)
that is stored with the evidence and survives reparsing, rebuilding the index, a
moved file and a second root. Each adapter states its own identity rule beside
the reason for it.

## Preservation contract

- Preserve every captured byte, including unknown event types, malformed records,
  incomplete tails, older source generations and compressed streams. A parser
  limitation must not turn a successful raw capture into discarded evidence.
- Do not identify a file solely by a session ID. Codex lineage can reference a
  physical rollout ID; DeepSeek generations belong to one logical session. A
  Codex rollout that continues another names it with two numbers that count
  differently: the byte offset is a position in the named rollout's own file,
  the ordinal counts along that rollout's whole history (what it inherits plus
  its own records before the offset). The line of a conversation is read through
  the whole chain. If a continuation repeats a physical source, traversal stops
  at its first repetition. Each affected current line remains readable and
  incomplete, with a path-scoped `lineage_unresolved` diagnostic whose detail is
  `cycle`. An undecodable native prefix also leaves the continuation unresolved.
- Preserve native event identity, parent relationships, source locator, record
  ordinal, timestamps, origin hints and native payload. Unknown origin stays
  unknown. Folder slugs are not reliable project paths.
- Report discovery failures, missing lineage/attachments, unstable sources,
  unknown versions and normalization limits explicitly. Raw coverage and query
  coverage are separate facts. A partial run cannot report complete coverage.
- Retain previous successful versions when a source is rewritten, truncated,
  disappears, becomes inaccessible, or parsing fails. Never mirror deletions. A
  revision that is a byte prefix of its successor keeps its record and shares
  the successor's bytes; a snapshot of a tool's state database keeps only its
  latest verified copy. A source's retention never moves to a kind that keeps
  less once it has revisions.
- Every run records which known sources it saw and which it did not, for the
  roots it could read. A root that cannot be read says nothing about its files.
- Attachments are captured only through known adapter locations or explicit
  references within approved roots. Conversation text cannot authorize arbitrary
  filesystem reads. No network fetching of referenced URLs. An attachment that
  was not kept is counted on its conversation and reported with its class, never
  its target; it does not make the conversation incomplete. Offline reparse
  reports `missing` when a binding identifies a revision whose bytes expired.
- One bounded read beyond the tools' folders: the git facts (top level, common
  directory, branch, remote URLs without credentials) of a folder that a session
  record names in its structured working-directory field. Only `.git`, `HEAD`,
  `commondir`, `gitdir` and `config` are read, each capped, no link below `.git`
  is followed, `git` is never run, and `backup --no-git` reads nothing.
- A capture is a validated file snapshot, not a claim that all harness processes
  stopped at the same instant. SQLite needs an actual consistent read/snapshot;
  copying its database/WAL files independently is not a consistency proof.

## Storage decisions

Follow ADR-012: new relational storage uses Turso, full-text search uses Tantivy.
Archive, projection and search files follow the archive's encryption choice,
which may be none (`init --no-encryption`); temporary spill files are encrypted
either way. Reuse `vault-store` and `vault-crypto`, including the password
envelope; do not introduce another crypto format or plaintext staging directory. The standalone archive owns its root, envelope and format marker.

In the desktop app, blocking callers obtain an `Arc<agent_history::Archive>` from
`Vault::agent_history()`. All clones of one open vault share the first successful
attachment. The library checks both conversation roles and their encryption mode,
then runs the same recovery as standalone open. The vault supplies the dedicated
search directory and its derived subkey in encrypted mode. Opening or attaching
does not discover sources. A failed attachment can be retried.

Capture, reparse, search and exports run on their caller's blocking thread. A
maintenance lease stops admission and drains active file users before freezing
the store. Close permanently revokes them, releases the search key and closes
the store. A retained raw reader checks admission even for a buffered slice.
Callbacks must return for draining to complete. Use a cancellation token per
capture or reparse. The owner can signal it during maintenance or shutdown.

Raw bytes are split into 4 MiB slices, compressed with zstd and addressed by the
SHA-256 of their uncompressed bytes, inside the canonical database. Files the
tool already compressed are stored as they are. This gives atomic deduplication and avoids a second durability
protocol for object files. A revision publishes only after its complete chunk
manifest and source validation are committed. Incomplete attempts cannot replace
the current revision. Old raw revisions remain available for byte-exact export.

Retention queues unreferenced slices in the publication transaction and attempts
collection after commit. Backup checks the queue again after publishing the
search index. If collection still fails, the backup returns a partial run with
`slice_reclamation_deferred`. The next backup or archive open retries the queue.

A mark returns the state committed to the evidence database. If its catalog
mirror fails, the existing pending edit survives. Session reads, listing and
search retry the mirror before exposing user state and propagate a continuing
mirror failure. Opening the archive also replays pending edits.

Normalized projections are rebuildable. Publication keeps the last complete
generation visible until a replacement is ready. Bounded transactions and
streaming record parsing avoid whole-corpus materialization. Any resource limit
has an explicit diagnostic; there is no silent truncation for indexing.

Keyword search uses an agent-specific encrypted Tantivy schema, not browser URL
fields. Canonical publication and index progress are tracked separately so a
crash cannot silently return incomplete search results. Search synchronization
is an explicit write operation; ordinary reads never trigger hidden rebuilding.

## Validation order

The user requires tests before implementation and test execution at the end.
First write failure scenarios and native-format fixtures, then CLI E2E and
targeted integrity tests, then production code. Run no test/build loops during
development. At the final verification phase, execute the core E2E, real local
read-only capture into an isolated archive, and `bun run check`. Fix observed
failures and rerun affected checks. Record actual platforms and missing evidence.

Required failure scenarios: live append and incomplete last record; same-size
rewrite; truncation/rotation; unknown events; corrupt compressed frame; absent
and escaping attachments; missing lineage; duplicate native IDs across roots;
multi-generation sessions; parser failures after successful raw capture; wrong
password; tampered encrypted state; interrupted writes and restart; index lag;
source permissions/disappearance; stable pagination; exact raw export; and
bounded processing of a large conversation. Every E2E leaves a repeatable report.

## Implementation and evidence

The [implementation notes](research/agent-history/implementation-notes.md) and
[capability matrix](../features/agent-history.md) record the shipped API,
supported formats, limits and remaining gaps. The [verification record](research/agent-history/validation/README.md)
contains macOS/Linux results and the 14.4M synthetic run under enforced 4 CPU /
8 GiB limits. Native replay, UI integration and Windows runtime validation remain
outside those results; an adapter name alone is not a completeness claim.

### Physical reads beside the native projection

A retained revision can be scanned into physical locators and per-record interpreter contexts.
JSONL spans include BOM and line endings; native arrays use the element span. A zstd locator
also identifies the independently decodable frame containing the record's first byte.
Range reads verify touched archive slices and bind the revision's source and digest. A full
scan through EOF also verifies the revision digest. A short or damaged native frame has its
own error code. A malformed JSON row retains its exact bytes and reports framing coverage.

The `agent_history::read_model` API streams at most 200 selected records per window.
Callers keep the locator and context together. Each row carries a bounded exact-byte page;
a complete object also supplies native fields and typed text, reasoning, call-input and
call-result parts. An oversized preview supplies a continuation offset within its own record.
The scan and window callbacks run on the caller's blocking worker outside store transactions.
Maintenance signals their cancellation token before draining active reads. A callback waiting
on downstream capacity must observe that token. Source membership and publication selection are described below.

The [removal matrix](research/agent-history/read-model-removal.md) tracks the existing SQL,
CLI and fault-injection contracts through cutover. The [measurement commands and oracles](research/agent-history/read-model-measurements.md)
cover both archive modes, native framing, source-order windows and outline rank seeks.

### Versioned conversation reads

A source publication is staged from its retained revision. The source projection commits a dirty ticket for every affected conversation. Private metadata batches resolve copy election, native base boundaries, alternative rollouts, recorded Pi leaves, child links, coverage and prefix-scoped attachment counts. The final ticket comparison and head switch run in one read-model transaction. A superseded reader gets a stale cursor. Interrupted work leaves a recoverable preceding head.

Use `reader_header` to pin a publication. `reader_sources` pages its member descriptors. `reader_outline` and `reader_window` address physical records on either the selected line or an earlier member branch. `reader_visible_window` uses ranked display bitmaps and the latest recorded Pi continuation. `reader_leaves` and `reader_leaf_window` expose alternatives. `reader_lookup` returns exact native-ID occurrences, `reader_children` pages resolved children, and `reader_record_bytes` expands exact evidence within the authorized member or inherited prefix.

A source receives rendered pages if any independently decoded native frame exceeds 32 MiB. Transport suffix and total source length alone do not establish that condition. Rendered reads bind the source digest and renderer version. Fallback reinterprets the evidence, using the original parser budgets if the outline itself needs recovery. Large display rows carry a truncation flag and their physical address. Native exports and raw download continue to use evidence.

`rebuild_reader_catalog` regenerates the reader tables from retained bytes and existing native session membership with live roots absent. It preserves the legacy event projection, runs, user marks and search index. Complete regeneration of those other subsystems from a blank pair of derived stores is the later full-rebuild step. The [stored values and lifecycle table](archive-compatibility.md#agent-conversation-reader-publications-revision-1) identifies the owner of every new table.
