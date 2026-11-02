# Semantic index control

`WORK-AI-INDEX-CONTROL` backend contract. Intelligence schema v2 adds lineage-scoped embedding provenance; archive schema is unchanged.

The chosen index is the configured embedding provider ID and its default model.
`semantic_index` reads readiness from the ledger primary key, completion rows through
provider-ID prefixes, and active jobs through the existing state index. It never
counts archive visits or embedding rows during status polling.

The serving index is chosen when ready, otherwise the latest ready pair whose
provider remains configured, otherwise chosen as partial or empty. Worker context
resolves that pair with the provider's current address and secret and the pair's
model. History and MCP use that runtime; Ask gives it to both search tools and
run_code for the turn. Ranked search adds `previousModel` only after reading the semantic plane of a pair
that differs from chosen; blank queries, date sorting, empty indexes and drift fallbacks do not. The in-app sentinel uses the same static engine for query and
build, with the current vault's model paths. One process-wide strong `Arc` per canonical
model directory retains the engine between requests. Directory, verified-marker and model-file
size/mtime changes invalidate it; cache misses reverify hashes before loading. Query embedding
and drift validation share the selected engine. This retains model memory until replacement or
removal instead of rereading and converting the weights for every search.

Queue payload JSON gains an optional model snapshot. Existing columns keep their
meaning: stop 1 cancels, stop 2 pauses, and error_code distinguishes user, service
and queue pauses. Queue resume only touches NULL pause reasons. A stop wins over
a concurrent success or failure in the same Intelligence transaction. Readiness
checks the persisted stop flag in its final transaction too.

Legacy payloads without a recorded model cannot run, including queued cursor-zero jobs.
Startup cancels every active such job and keeps the index files. A new explicit incremental
request can fill the missing visits without binding old work to today's model.

User enqueue and automatic enqueue make their decision inside the owner write
transaction. User requests reuse active builds for the same pair and cancel other
writers, including clear jobs; automatic work requires a ready chosen ledger and no active same-pair job.
Explicit reset abandons old jobs and queues a clean rebuild. Running jobs keep
their lease until they acknowledge cancellation. Claims allow one index writer;
the worker owns a waiting admission token from claim through physical cleanup, so a new
claim never waits behind cleanup with an aging lease. No mutex remains locked during the work. Configuration is reloaded
inside that guard. Explicit run-now uses a nonblocking lock attempt and returns status
when another build owns it. Poisoning does not wedge this data-free admission gate.
Build status excludes clear-only jobs and prefers a running build over other matching work.

Only new durable cursor progress renews retry attempts. Typed embedding-service
errors survive anyhow contexts; exhausted retries pause with the complete error
chain. A drain remains alive while a queued retry waits for its available_at,
without depending on UI polling. It sleeps until the earliest due time, capped at 30 seconds,
and exits when paused or no queued index job remains. Local database, configuration and consistency
errors remain failures.

Job start retains chosen and serving. Once the job's success is settled, a complete
full-archive pass retires every other pair, including ledger rows and all vector
files. An incremental first fill is such a pass; the fullRebuild flag decides that every visit
is read again, not whether a finished replacement may retire its predecessor. A pair without a
complete index writes `building` (`clearing` for clear-only); a full rebuild of a complete one
keeps it `ready`, because it is built beside it (below).
An update that does not wipe leaves an already `ready` ledger and its full-pass coverage
watermark and timestamps intact; interruption or working-set completion cannot demote that index. Only `building` or
`clearing` can become `incomplete`. Working-set updates also preserve the full-pass watermark and timestamps of a ready pair,
so unscanned visits or enrichment remain stale. A successful full-archive incremental pass
advances that coverage. Working-set fills of an unfinished index remain incomplete.
Cleanup failure is logged and appended to the job summary, preserving the chosen index and
build outcome. Start-of-job cleanup failure also allows the build to proceed; later jobs retry
remaining deletions. A changed selection during a build retains the current chosen and serving
pairs instead of deleting the fallback.

## Every pass publishes one immutable generation

The ledger's `sidecar_table` is the relative serving location
`<table>/<lineage>/g<run-id>` under `derived/vectors/`. An empty pointer has no serving index.
`<table>` remains `ai_sidecar::provider_table_name(service, model)`.
A new queue build uses lineage `j<job-id>`; a build outside the queue uses `r<run-id>`.
Full rebuilds start a new lineage. Incremental and working-set passes reuse the serving lineage.
A resumed full rebuild reuses only its own lineage with the same service/model identity and
available sources; otherwise it starts that lineage again from visit zero with no old provenance.

A lineage holds append-only page sources (`.pkvec`, `.pkmap`) and the distinct keyword-search
store. Each pass writes a separate `g<run-id>/` with the binary/int8 planes, reverse and forward
visit maps, visit positions, keyword-search projections, and `serving.json`. The JSON captures
source headers, identities, text-format stamp and vector count for that complete projection.
Readers never inspect mutable source headers. Published files are immutable; the writer never
renames over a serving file. Projection remains a streaming pass over the lineage source.

After appending a chunk, the writer fsyncs its source files and new directory entries before
committing provenance or advancing the durable cursor. Before publication it fsyncs every source
and generation file, then the generation, lineage and directory ancestors, using `durable_io`.
The existing helper uses the full filesystem barrier on macOS; its directory barrier is a no-op
on non-Unix, so Windows directory-entry persistence is not claimed by this implementation.

`record_index_ledger_success` changes the serving pointer, marks the queue job succeeded, and
stores its final summary in one Intelligence transaction. The summary subtracts updated rows from
the indexed total to report new rows honestly, and includes durable progress from earlier attempts. It first checks that the job is running and has no stop request.
This is the only publication point for complete coverage and queue success. With a complete
serving index, a stop before it keeps the old pointer; a stop after it cannot change committed
success. Initial fills without a complete serving index also have the incomplete-publication path
below; a stopped job remains stopped even when its partial index is made searchable. Start and failure bookkeeping preserve the pointer. Clear-only publishes
an empty pointer and no generation, then ordinary garbage collection reclaims the former lineage.
Archive run bookkeeping and search-group warming happen after publication; failures there are
logged and cannot undo index success. A crash before the success transaction never declares a complete index. A durable initial-fill
intent may later authorize local recovery to publish only incomplete coverage.
The transaction and version marker use the owner's existing `synchronous=FULL` WAL connection.

`generations::resolve` reads the pointer first, briefly locks registry to pin an unreserved name,
then unlocks and rechecks the pointer. Metadata, plane length/fingerprint and visit projection
validation happen under the pin, outside registry. A missing or unreadable generation is no index.
Build admission follows the same read/pin/recheck rule; database callbacks receive plain strings,
counts and file signatures, never leases. GC reserves candidate names under registry, re-reads
authoritative ownership after unlocking, then locks only for renames to trash. Existing writer
pins prevent reservation; publication before reservation is caught by the second database read.
GC never reserves the generation a pointer names, nor the whole lineage holding it: a reader or
build cannot tell a reservation from a missing index, and a build that saw none would start a
new lineage and embed every visit again. It reads the pointer once before reserving. Builds and
partial recovery publish only while a writer pins the new generation, and the legacy move runs at
open before any collector exists; a writer can still drop its pin between GC's look and its
reservation, so GC may briefly reserve a generation that has just become serving. The ownership
read after reserving keeps it, and readers and builds wait for the reservation to be released and
read the pointer again instead of reporting no index; failing would end a queued job as needing
review. A build reuses moved v0.4.0 (`legacy`) sources only while the ledger carries a recovery
reason; after a Clear they are leftovers for GC, and reusing them would bring the cleared vectors
back. A query keeps the lease through embedding-service waits, scoring and map reads.
Related pages, period topics, similar searches, grouping, snapshot warming/signatures and status
use the same boundary. Nested grouping helpers inherit a lease-backed vault view, so a warm cannot
combine signatures and payloads from separate generations. Snapshot publication still revalidates
its pointer and archive/Insights inputs on the database writer. Missing `.pkpos` falls back only
to the serving `.pkfwd`; queries never rebuild mappings from a lineage's mutable `.pkmap`.

Intelligence v2 adds `lineage TEXT NOT NULL` to the uniqueness/primary keys of
`ai_embeddings` and `ai_embedding_totals`. Released v1 stays unchanged. Its setup transaction
renames the old tables to `_v1`, creates the new empty tables and their permanent
`idx_ai_embeddings_lineage_id` / `idx_ai_embeddings_lineage_history` indexes, and records version 2.
The history-prefix index is deliberately retained: on Turso 0.8.1, direct history reads use the
UNIQUE autoindex, but a `requested` batch join without the explicit history-index hint chooses
`idx_ai_embeddings_lineage_id` using only provider/model/lineage. That loses the per-history seek.
The regression compares both plans; depending on an engine-generated autoindex name as a query
hint was rejected. Fresh archives run the same v1 → v2 path. A real migration/upsert/reopen test
confirms explicit ids advance `sqlite_sequence`; no redundant sequence repair is performed.
A setup failure rolls back to v1. The old index names remain on the old tables until the move empties them;
creating an index over a populated new table would defeat the memory bound.

Before any reader pool, background work or ready signal starts, the owner's migration runner
finishes v2's resumable backfill, including when version 2 was committed by a previous launch.
It moves at most 50,000 old rows per transaction, preserving every id, field and total and adding
`legacy`: each batch copies an id range and deletes the same range from the old table, and the
phase, last moved id and processed count commit with them in `ai_embedding_upgrade`. Moving
instead of copying first keeps the file from holding both copies at once (several GB at 14.4M
visits, on disks that are often nearly full); the old table's freed pages take the next batch.
Each batch finds its range from the old table's first 50,000 ids, never from the growing new
table, so the move stays linear. It then drops the empty old tables and leaves the permanent
`done` marker. A crash resumes after the last committed batch; no row can move twice or be lost. Each version's
pending backfill finishes before the next schema version can start. Older binaries refuse version 2.

The startup writer checkpoints/truncates WAL between batches, before reporting committed progress.
This bounds both dirty-page commit buffers and retained WAL state. The existing
`ArchiveUpgradeProgress` contract carries `SchemaMigration`, `Intelligence`, table and processed/total
ticks. Native initialize commands can forward them through their existing callback, and automatic
opens emit the same desktop event. No frontend fields changed. The current frontend routes only
SQLCipher-conversion assessment to `UpgradeScreen`; automatic native opens still use its boot/unlock
surface. Showing these native schema ticks there requires frontend routing work, which this backend
change deliberately leaves to the frontend owner.

Separate seed/upgrade processes measured the complete registered upgrade, including every batch
commit and checkpoint, in the local macOS debug test build (2026-10-30, the batch move; other builds
were compiling on the same machine, so the times are upper bounds):

| Old rows  | Upgrade elapsed | Process peak RSS | Database folder before → after |
| --------- | --------------- | ---------------- | ------------------------------ |
| 1,000,000 | 16.416 s        | 145.3 MiB        | 368.6 MiB → 416.6 MiB          |
| 4,000,000 | 66.918 s        | 127.1 MiB        | 1,456.6 MiB → 1,568.6 MiB      |

The previous copy-then-delete version measured 26.545 s / 129.210 s and 83.9 / 86.2 MiB. Its time
grew faster than the row count because every batch found its cursor with `COALESCE(MAX(id),0)`
over the growing new table: Turso 0.8.1 takes its MIN/MAX shortcut only when the result column
is the bare aggregate, so the wrapper made each batch scan every row moved so far. The move reads
its range from the old table's first 50,000 ids instead, and 4M now takes 4.08 times as long as
1M. Peak RSS is higher because each commit now also carries the batch's deletes; it is still set
by one batch, not by the table. The folder after the move is 8–13% larger than before (the new
`lineage` column and indexes); the old version held both full copies before cleanup.

The fixture has one service/model, fixed URL/title/profile strings and a 64-byte hash. Peak RSS
comes from `/usr/bin/time -l` for the upgrade process alone; the separately seeded databases are
removed after each run. Reproduce with the ignored `measure_intelligence_v2_migration` Rust test,
`PATHKEEP_MIGRATION_ROWS`, `PATHKEEP_MIGRATION_ROOT`, and `PATHKEEP_MIGRATION_PHASE=seed` then `upgrade`.
At 14.4M rows the move takes 288 batches at this cap; memory remains governed
by one batch, row widths and the fixed page caches, rather than multiplying the measured peak by
14.4. The 1M/4M results support that bound for this fixture; 14.4M and other row widths were not measured.
The superseded one-transaction copy measured 1.10 / 4.20 GiB RSS and was rejected. No Turso fork,
patch, version change or new dependency is involved.

Candidate hashes and every write use the build lineage; status provenance counts use the pointer lineage identity (even when its files are unavailable),
never another build lineage. Storage counts all retained lineages. A build also checks source mapping/key presence
before trusting a hash: files left by this branch's older abandoned rebuilds can have legacy rows
for visits never written to the live source. Doctor, SQLCipher conversion and total reconstruction
carry the same lineage; converted rows are `legacy`. Cancelling a rebuild cannot delete live update
rows, and publishing it cannot make later updates trust provenance from the replaced lineage.

GC runs at archive open (directory/marker work only), around every queue attempt, after direct
builds, and in a coalesced reaper when a retired generation's lease drops. It keeps the serving
lineage, active queue lineages, all reader leases and writer pins. Serving and vanished names leave
`retired`. The collector's admission token owns no mutex during database/file work. Only decision
bookkeeping and same-volume trash renames run under registry; fsyncs, recursive deletes, source
file deletion and row cleanup run after unlocking. Trash is swept at open and later collections.
A rename or deletion failure leaves retryable state; one broken lineage does not hide other items.

Row cleanup checks archive closure between batches of at most 512 rows. Explicit close shuts
collector admission before waiting for the current pass, so it waits for at most the current
batch/file operation, rather than a complete 14.4M-row cleanup. Identity remains until all rows and
totals are gone. The next owner/pass resumes deletion through the lineage/id index. Windows
rename failures retain the original directory for another pass; no Windows persistence claim is
added beyond the existing `durable_io` behavior.

GC compares and clears a definitively dangling/invalid serving pointer and marks its ledger
incomplete with a reserved recovery reason. Definitive means the files are gone, torn or not
the ones the metadata describes; a read that failed for a reason that may pass next time (a
sharing violation while another program scans a new file, too many open files, a disk error)
leaves the pointer alone and is logged, because a cleared pointer makes the next build start a
new lineage and embed every visit again. A pointer that is not a generation path is a v0.4.0
flat index whose startup move has not finished; repair leaves it to the move. The count a
generation's metadata records is the planes' count (one record per content key), and a pass
checks its prepared generation the way readers will before it publishes, so a lineage source
holding a key twice after a torn resume cannot produce a generation readers reject. That reason conservatively retains same-pair lineages
across failed build/clear attempts until successful publication or clear. Legacy migration records
its recovery reason before the first file mutation, including totals-only pairs. These exact reasons
are centralized; actual job errors remain in job/run diagnostics. Recovery ownership also preserves
the original coverage watermark and completion/indexing timestamps through build/clear start and
failure. A later legacy retry must not restore ready with a failed attempt’s newer coverage. An ordinary empty, never-served
ledger does not grant retention, so a stopped replacement beside a complete fallback loses its
unowned lineage/rows. Pending initial-fill intents independently protect useful partial work. A pointer naming nothing cannot authorize deleting a valid replacement.
Readers never guess that an unreferenced prepared generation was committed. History uses words
with the existing `EmptySemanticIndex` build-needed note. Status remains readable, `ready=false`,
with the existing localized vectors-missing/build-needed warning. No automatic embedding or
network work starts to repair it. The user runs Build/Update or Reset through the normal queue:
a retained full rebuild can resume its own valid sources; otherwise the normal build creates a
fresh lineage and scans from the beginning. Migrated legacy sources can be reused by an
incremental pass even when their serving projections are absent.

Startup first finishes old `job-N.publish` copies according to the previous binary's manifest
contract: `.publish` plus a readable, validated manifest is its on-disk commit point, independent
of whether the worker already marked the queue succeeded. The old format has no run identity,
so exact ledger-transaction correlation cannot be reconstructed. Remaining files replace flat
predecessors by rename (int8 last), and same-pair files absent from the committed manifest are
removed. Directory barriers make interrupted installation retryable. Uncommitted old copies lose
their files but retain their manifest until a normal job removes their legacy provenance by the
old timestamp boundary in bounded batches; archive open performs no such row scan.

Flat migration moves files, without corpus copies, to `<table>/legacy/` sources and `legacy/g0/`
projections. Before the first source mutation, generation, lineage, table and root directories are synced
bottom-up so a durable flat unlink cannot outlive destination reachability. Each individual
destination wins on retry, including when a stale flat duplicate is
corrupt. It trims torn page/search/map tails at the authoritative source and fsyncs before any
source reader. Every rename syncs both directories; a retry repeats destination barriers even
when all flat names already moved. Missing projections stay missing: open never constructs maps
or vector planes. Such a ledger remains incomplete with no pointer, retaining its sources for the
next normal pass. Only a complete durable destination gets a serving manifest and ledger pointer.
Migration failures are logged and archive open succeeds; destructive startup GC is skipped on
that failed open. The next open retries, while search keeps using words for the affected pair.
A pair is moved only when flat files exist or its ledger carries the legacy recovery reason
written before the first move. A `legacy` folder alone is not evidence of a move in progress: it
can be a cleared index whose row cleanup was cut short at quit, or one GC recreated from leftover
rows, and publishing it would bring back an index with no vectors. v0.4.0 projected planes only at
the end of a pass, so a pass it stopped left more source vectors than projected ones; the move
serves the plane's own count, which is what v0.4.0 served, and publishes only what passes the same
validation readers use. Anything else stays unpublished with its note for the next pass;
meanwhile status counts its `legacy` rows and reports their vectors missing, so it asks for a
pass instead of showing an index that was never built.

Cursor JSON records its lineage before scanning. An absent or changed lineage resets position,
new/updated counts and progress denominators together. Changed-row provenance and the durable
cursor/counters commit together after source fsync, so a crash before the worker callback cannot
lose the job's counts. Unchanged chunks still advance through the normal ledger callback. A full
rebuild retains its captured archive boundary; an incremental resume against another lineage
starts at the beginning of that lineage's needs, including changed low-id visits.

The lease/closure audit covers status, query/staleness reads, related pages, topics, similar
searches, grouping `Prepared._generation`, grouping event/opened-page reads, snapshot signatures
and snapshot publication. `Prepared` stays on its caller, including the equal-digest warm branch;
only its plain input lists enter database callbacks. File signatures and status storage traversal
are captured before opening a database session. Snapshot warming uses a waiting admission token,
so publication cannot drop a required follow-up warm. Related-page cutoff sampling also runs
outside its cache mutex; harmless duplicate samples are preferable to blocking other generations
on mmap page faults.

A full rebuild still requires space for both lineages until readers release the old one. Ordinary
passes also retain the previous serving projection while preparing a new one, but reuse the much
larger f32 sources. Storage accounting already walks nested directories and includes both. Export
and move intentionally omit derived indexes; wipe removes the entire derived tree, so both keep
their existing behavior with this layout.

When the background drain finishes handling an index job (succeeded, failed, cancelled, paused
by the service, or waiting for a retry), the desktop shell emits `pathkeep://ai-index-job-ended`
with the job's id and state; the dev bridge forwards it. The app reads its snapshot again on it,
whichever screen is open, so History's semantic search and Settings learn about builds and
after-backup updates that end while they are not watching the queue.

Feature acceptance belongs to reviewer E2E. Rust regressions cover consent,
stop/failure races, legacy recovery, cursor durability, serving-model isolation and the rebuild
generation publication, pause, cancel, crash, migration and lease/GC cases (`tests_native/indexing/rebuild.rs`).

## 2026-10 review: verified premises and failure cases

The reviewed HEAD copies flat files and constructs missing projections during open; propagates
migration errors from `Vault::open`; drops committed old `.publish` folders; resolves pointers
without inspecting their targets; takes registry before database reads in resolve, build and GC;
moves status leases into a reader closure; deletes files while holding registry; never checks
close between lineage-delete batches; retains serving paths in `retired`; commits queue success
without its summary; and resumes incremental cursors without a lineage identity. The history
index duplicates the UNIQUE index's leading columns, but its removal requires a Turso plan test.
Explicit-id migration sequence behavior likewise requires a real Turso insert/upsert test.

Failure cases specified before the regression tests and implementation:

- Legacy recovery: an interruption after any rename, duplicate source/destination names, torn
  page/search/map tails, absent projections, ENOSPC/permission failures, and an interrupted
  committed `.publish` install must neither copy the corpus nor block archive open. A failed
  pair must remain unserved and retryable; no projection may be generated during open.
- Pointer recovery: missing directories, unreadable/invalid metadata or missing planes must
  return no serving index. Neither repeated GC nor an intervening failed build may reclaim
  another same-pair lineage on that evidence. A successful normal queue build or explicit clear
  ends this conservative retention.
- Synchronization: saturated reader pools must not block lease release behind resolve/build/GC;
  no database closure may own a lease. Publication between a GC snapshot and reservation must
  not allow deletion of the newly serving generation. Reader and writer pins must win against
  a later reservation; reservations must prevent new pins until validation/rename completes.
- Reclamation: large deletes and fsyncs must run outside registry; failed trash deletion and
  interrupted row cleanup must remain discoverable. Close must stop between batches, preserve
  identity until all rows are gone, close collector admission before waiting, and terminate a
  dirty reaper. Serving and nonexistent paths must leave the retired set.
- Queue atomicity: success without the final summary, a stop racing success, a paused L cursor
  resumed against M, legacy cursors without lineage, and reset progress preserving old targets
  must not lose counts or skip changed low-id visits.
- Schema: removing the redundant index must retain prefix seeks for candidate and upsert reads;
  explicit migration ids followed by an upsert must allocate above the migrated maximum and
  preserve every migrated row.

### Review choices and rejected alternatives

- Same-volume rename preserves inode identity and bounded disk use. Durable corpus copies and
  projection construction during open were rejected because optional derived work must not gate
  archive availability. A partial move is deliberately unserved until retry or normal indexing.
- Missing pointers return no semantic index; guessing another prepared generation could publish
  cancelled work. Conservative retention can keep disk bytes until the next successful normal
  job or explicit clear; immediate GC on an empty pointer would destroy recoverable lineages.
- Pin/reservation plus database revalidation replaces lock-spanning reads. A reservation never
  covers what a pointer names, so readers do not fall back to words because of GC, and a build
  waits for a reservation on its lineage instead of failing. Holding registry for DB reads,
  metadata, directory barriers or deletion can deadlock lease drops and delay quit.
- Trash renames leave retryable disk state after a crash or failed unlink. Deleting under registry
  was rejected; removing identity before bounded row cleanup finishes would lose retryability.
- Waiting work admission preserves queued writers and snapshot warms. A try-only background gate
  was rejected because it busy-polls queued work or drops the newest generation's warm request;
  foreground run-now retains its nonblocking admission behavior.
- Final summary, serving pointer and queue success share one commit; a later worker update cannot
  supply a summary once the job is already succeeded. New/updated counters also survive pause and
  retry, and advance atomically with provenance instead of depending on a later callback.
- The history-prefix index remains for the measured Turso batch plan. An unhinted autoindex
  replacement loses history seeks; hard-coding an engine-generated autoindex name couples reads
  to incidental schema naming. `sqlite_sequence` already advances in Turso, so manually rewriting
  it adds no correctness benefit.
- Old `.publish` manifests retain their historical commitment semantics. Requiring queue success
  would discard completed copies from the publication/worker-success crash window; reconstructing
  exact ledger linkage is impossible from that old format. Old timestamp cleanup is restricted to
  `legacy`, deferred from open and made resumable; new generations never use timestamps as owners.

### Initial-fill interruption: premises and failure cases (before tests/code)

The E2E finding matches the reviewed implementation: an error drops BuildGeneration before any
projection/pointer publication; once the job is cancelled, GC has no active owner for its lineage.
With no initial serving generation this loses both partial recall and resumable provenance.

- Status must release file leases before entering a reader callback, including errors; concurrent
  resolve, last-lease drop and GC must finish with both Intelligence readers occupied.
- Pause, cancel, service/local failure or process death after a durable initial-fill chunk must
  preserve its source/provenance, publish only an incomplete immutable generation, and keep that
  lineage searchable and available to the next incremental job without re-embedding known pages.
- A usable complete serving index (including another configured model) forbids partial replacement;
  an interrupted rebuild keeps the old complete pointer and discards its own unowned lineage.
- A rebuild or reset over a partial index that stops after a chunk does not take the place of a
  partial that covers more: that one keeps serving, and the rebuild's lineage waits for its job.
- A job that ended failed keeps its lineage, like a paused one: Replay continues from its cursor
  instead of embedding everything again. Nothing changes a failed job afterwards, so the lineage
  goes once the index it was building is replaced — a later job's lineage serves that provider
  and model, or that provider and model were cleared after it failed (the ledger's
  `last_cleared_at`; a reset of another model does not count, a reset replayed from an older job
  does). A later update of the lineage that was serving does not count.
- A build whose lineage another writer holds (partial recovery projecting it after open) waits for
  that writer, as it waits for a GC reservation, instead of ending the job.
- Intent must precede appends. GC must retain pending partial recovery even when a reset's source
  lineage is not yet serving and its job is cancelled. A crash before/after projection or pointer
  commit must be idempotent, trim torn tails, and never demote ready or revive an explicit clear.
- Crash recovery may project locally after open returns; archive open itself must not project,
  embed, require a provider/network, or fail on recovery errors. Until projection completes search
  uses words. Source projection once per stopped pass avoids a quadratic full scan per chunk.
- Partial finalization does not mark a stopped job successful or advance full-pass coverage. A
  newer build/clear or complete publication wins against stale recovery intent. Failed preparation
  leaves retryable intent and no false serving pointer. Source writers and recovery never overlap.

### Serving a stopped initial fill

When no usable complete index exists among configured services (including a fallback model),
BuildGeneration durably writes `partial.json` before returning a source writer. It records the
build run, intended generation and prior pointer. GC retains this lineage even if its queue job
is cancelled. The build owner holds its writer pin through handled pause/cancel/failure, trims
source tails, prepares the existing local bytes once, and conditionally publishes the generation
with ledger state `incomplete`. Job state, queue success and complete-coverage fields do not
advance. Status reports usable semantic vectors and their lineage visit count, while
`chosenComplete=false` keeps the existing “not finished: N visits so far” display. Search and
related semantic reads use the immutable partial generation. The next normal incremental build
reuses this lineage and only embeds missing/changed visits; it does not restart a cancelled fill.
An explicitly requested reset still starts a new lineage.

A usable complete index forbids this partial replacement, even when it belongs to another model.
The complete-serving rebuild retains its original isolation: its stopped, unowned lineage and
rows are discarded. Incomplete publication compares `last_run_id`, ledger state and prior/current
pointer inside its transaction. Explicit clear, a newer run or complete publication wins; a stale
intent cannot resurrect or demote an index. An empty pointer produced by dangling-pointer repair
is allowed only under the same run authorization and non-cleared/non-clearing/non-ready state.
Pointer repair itself requires definitive validation under a successful pin: registry reservations
and pointer movement are not evidence of missing files.

After process death, archive open reads/retains intent and schedules local recovery after open;
it does not project or embed. Until recovery finishes, reads use words. Recovery is independent
of the embedding queue's paused state and makes no provider/network call. It runs under collector
admission with a source-writer pin, so it cannot overlap appends. Close closes admission, cancels
projection and waits for the active collector; a queued recovery cannot write after owner release.
Both vector-source passes, map reads, sidecar/position writes and merge passes check shutdown at
bounded intervals. Sorts operate on blocks of 32,768 pairs before checked merge passes. Compared
with the previous in-place sort, the merge buffer adds 16 bytes per visit (230.4 MB at 14.4M
visits); this explicit memory cost buys bounded shutdown during sorting. It has not been profiled
at 14.4M for this change.

Recovery reuses a valid prepared projection but always repeats source/output/ancestor fsyncs
before the pointer transaction. A manifest alone is never a durability proof: a crash or ENOSPC
may have interrupted barriers after manifest creation. Failed preparation/barriers keep the intent
for another local recovery pass/open. Successful publication or an obsolete authorization removes
the marker durably. Crash tests cover before projection, prepared files, committed partial pointer,
clear/new complete winning, lost prior partial, paused-queue reopen, barrier failures and close.

Publishing once when a pass stops avoids projecting the whole accumulated source after every
chunk, which would make initial fill quadratic. The brief lexical fallback during crash recovery
is deliberate: synchronous projection during open would violate archive availability. Keeping the
partial lineage after cancellation is required for searchable progress and no duplicate provider
requests; this exception never publishes a stopped rebuild over a complete index.

### Fresh settings and damaged-pointer retention: failure cases before refinement

A worker can outlive provider edits. Complete-fallback eligibility and GC must use the same fresh
configuration snapshot as provider selection. A real local HTTP fixture completes A, writes one B
chunk, then fails B; B must have no partial pointer or retained abandoned rows while A stays ready.
An ordinary empty, never-served ledger is not proof of a damaged generation. Retention reasons
must distinguish dangling-pointer/migration recovery from an abandoned B. Such recovery reasons
must survive failed build/clear attempts and must be written before the first legacy file mutation,
including totals-only pairs. Successful complete/partial publication or clear removes them.

The worker wraps its execution vault in the freshly loaded configuration before collection,
provider work, cursor persistence and partial-publication decisions. Reusing the archive-open
configuration here was rejected: services added while unlocked could otherwise hide a complete
fallback from partial eligibility. The HTTP worker regression completes model A, fails B after one
durable chunk, and verifies both A's pointer/ready state and reclamation of B's abandoned rows.

Legacy ancestor-barrier fault injection fails each of the four pre-move directory barriers. The
flat source must remain named after every failure, and a successful retry retains the same inode.
This closes the crash window where syncing only the immediate destination directory could leave
its new ancestor entry unpersisted after the source-directory unlink was already durable.
