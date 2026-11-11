# Agent history core

Status: implementation contract, 2026-11-11. This document records the independently
usable library requested by the user; product UI, onboarding, scheduling and IPC
integration remain outside this work.

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

## Preservation contract

- Preserve every captured byte, including unknown event types, malformed records,
  incomplete tails, older source generations and compressed streams. A parser
  limitation must not turn a successful raw capture into discarded evidence.
- Do not identify a file solely by a session ID. Codex lineage can reference a
  physical rollout ID; DeepSeek generations belong to one logical session.
- Preserve native event identity, parent relationships, source locator, record
  ordinal, timestamps, origin hints and native payload. Unknown origin stays
  unknown. Folder slugs are not reliable project paths.
- Report discovery failures, missing lineage/attachments, unstable sources,
  unknown versions and normalization limits explicitly. Raw coverage and query
  coverage are separate facts. A partial run cannot report complete coverage.
- Retain previous successful versions when a source is rewritten, truncated,
  disappears, becomes inaccessible, or parsing fails. Never mirror deletions.
- Attachments are captured only through known adapter locations or explicit
  references within approved roots. Conversation text cannot authorize arbitrary
  filesystem reads. No network fetching of referenced URLs.
- A capture is a validated file snapshot, not a claim that all harness processes
  stopped at the same instant. SQLite needs an actual consistent read/snapshot;
  copying its database/WAL files independently is not a consistency proof.

## Storage decisions

Follow ADR-012: new relational storage uses Turso, full-text search uses Tantivy,
all archive/projection/search/spill files stay encrypted. Reuse `vault-store` and
`vault-crypto`, including the password envelope; do not introduce another crypto
format or plaintext staging directory. The standalone archive has its own root
and data key. Future product integration must explicitly own its lifecycle.

Raw bytes are split into bounded content-addressed chunks inside the encrypted
canonical database. This gives atomic deduplication and avoids a second durability
protocol for object files. A revision publishes only after its complete chunk
manifest and source validation are committed. Incomplete attempts cannot replace
the current revision. Old raw revisions remain available for byte-exact export.

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
