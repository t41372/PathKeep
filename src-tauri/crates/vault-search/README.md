# vault-search

PathKeep's rebuildable page-search engine: Tantivy 0.26.2 through vault-crypto,
without a dependency on vault-core. It owns indexing and search. The caller owns
canonical data, outbox SQL, normalization, worker scheduling and presentation.
These synchronous APIs belong on blocking workers, not the UI thread.

`SearchIndex::create(path, Option<&DataKey>, config)` explicitly creates an empty
index. `open` never creates or repairs implicitly. `Some(key)` passes the data key
to `EncryptedDirectory::open`, which derives and owns the search sub-key. `None`
uses `MmapDirectory` for a user-selected plaintext archive. This crate implements
no cryptography.

## Schema and analyzers

One live document per `page_id`. An upsert deletes its old ID before adding a full
replacement. Rebuild input is strictly ordered by page ID, enforcing uniqueness
with one previous ID rather than an archive-sized set.

| Field                     | Type                                             | Weight |
| ------------------------- | ------------------------------------------------ | -----: |
| `url`                     | positioned text                                  |      6 |
| `title`                   | positioned text                                  |     12 |
| `search_terms`            | positioned text                                  |      4 |
| `normalized_url`          | positioned text                                  |      5 |
| `normalized_title`        | positioned text                                  |     10 |
| `normalized_search_terms` | positioned text                                  |      4 |
| `cjk_grams`               | positioned text                                  |      2 |
| `enrichment_text`         | positioned text                                  |      7 |
| `notes_text`              | positioned text                                  |      9 |
| `tags_text`               | positioned text                                  |      6 |
| `trigrams`                | Unicode 3-grams from `compact_text`, frequencies |      1 |
| `page_id`                 | indexed u64, fast                                |      — |
| `newest_visit_ms`         | i64, fast                                        |      — |
| `domain`                  | exact indexed string, fast                       |      — |
| `profile_ids`             | indexed u64, multi-valued fast                   |      — |

Text contains **already normalized, ASCII-whitespace-delimited tokens**. The
crate performs no case folding, Unicode normalization, stemming, language
segmentation or URL parsing. An empty profile list means no membership. The caller
must delete invisible pages or exclude them in eligibility; a zero timestamp is
not a visibility marker.

Schema and analyzer versions are both 1, stored with the outbox sequence in every
commit payload. Opening also compares the actual schema. A mismatch returns
`Error::RebuildNeeded` instead of searching incompatible data.

### Complete prefixes with BM25

Each normalized token produces an exact term (`=token`) and its Unicode prefixes
(`^`, `^t`, `^to`, …, `^token`) in the same weighted field, all at its original
position. Phrase positions select exact or prefix terms. Every input token gets the namespace, including
literal strings starting with `=` or `^`, so literals cannot collide with aliases.

Tantivy's `PhrasePrefixQuery` caps expansions at 50 by default, including a lone
prefix; its uncapped automaton query uses constant scores. Here `42*` becomes one
BM25 term query for `^42`, with its own document and term frequencies, however many
different words begin with `42`. There is no query-time vocabulary expansion,
matching-document bitset or candidate limit. The cost is extra indexed terms and
postings, measured in [the benchmark report](../../../docs/architecture/research/2026-10-vault-search-benchmark.md). Tantivy field norms count the
aliases too. These are versioned Tantivy BM25 scores, not numerical FTS5 parity.

Tantivy silently drops terms beyond `MAX_TOKEN_LEN`. Text tokens whose namespace
would exceed that engine limit are rejected with `Error::Input`; the entire write
rolls back without advancing the outbox watermark. The caller must correct the
projection rather than acknowledge a partly searchable document.

## Typed AST

| Expression                         | Meaning                                                                                                                |
| ---------------------------------- | ---------------------------------------------------------------------------------------------------------------------- |
| `Term { token, field }`            | Exact token; `None` searches all ten weighted fields.                                                                  |
| `Prefix { token, field }`          | Literal prefix; empty matches any token in the field.                                                                  |
| `Phrase { tokens, field }`         | Adjacent, ordered `PhraseTerm::Exact` or `Prefix` tokens within one field. One token is a term; empty matches nothing. |
| `Not(child)`                       | All live pages except the child; contributes zero score.                                                               |
| `And(children)`                    | Intersection; empty matches all live pages.                                                                            |
| `Or(children)`                     | Union; empty matches nothing.                                                                                          |
| `Trigrams { text, minimum_match }` | At least this many distinct Unicode 3-grams match. Threshold must be positive.                                         |

No query string parser is used. Punctuation is literal. A trigram query means gram
similarity, not an edit-distance guarantee. A threshold greater than the number of
generated grams matches nothing. The caller chooses thresholds and how to combine
exact and gram expressions. Unsegmented Chinese of three or more characters works
directly; shorter tokens use the supplied `cjk_grams` projection. There is no fuzzy
candidate cap.

## Ranking, filters and pagination

The custom bounded heap ranks all eligible matches by BM25 descending, then
`newest_visit_ms` descending, then `page_id` descending. Equal scores are never
pruned. All matches are considered; only the requested page is retained. Memory
is O(page size), including deep pages. There is no growing offset heap, window,
oversampling or alternative ranking fallback. Empty AND has Tantivy's constant
all-document score.

Domain/profile posting frequency chooses the plan: at most half of the snapshot's
pages pushes the filter into a zero-score query intersection; broader filters use
fast fields in the collector. Estimates affect speed only. Both plans check exact
membership, including every value of a multi-valued profile field.
`session.plan()` exposes the choice for profiling.

`Filters::eligibility` is the caller's **immutable** page-ID predicate. vault-core
computes canonical visibility and correlated date/profile/same-visit eligibility.
The predicate should use a cheap membership lookup, not a database request per page.
A latest-visit field cannot prove an arbitrary historical interval. Ranking uses
the supplied page's `newest_visit_ms`, not date-dependent recency recomputed here.
Hydration and final canonical validation remain with the caller.

`index.search(ast, filters)` pins a reader snapshot, query and eligibility in a
`SearchSession`. Its final page carries a cursor containing the last complete
ranking key. The next page accepts keys strictly below it. Cursors preserve f32
scores exactly and are bound to that session; another session rejects them. A
nonempty final page always has a cursor; an empty next page ends iteration. Every
match remains reachable once during concurrent writes, merges and rebuilds.

Changing filters or canonical visibility starts a new session. In particular,
discard sessions after a revoke when previously visible pages must immediately
stop appearing. Cursors are in-process handles, not serialized tokens. Drop all
sessions, cursors and progress handles when finished and on vault lock: they keep
readers and the encrypted directory alive. Retaining arbitrarily many snapshots
necessarily retains their old segments and reader memory.

## Progressive results and counts

```rust,ignore
let session = index.search(ast, filters)?;
let mut page = session.start_page(100, None)?;
loop {
    let step = page.step(std::time::Duration::from_millis(8))?;
    // Publish step.hits and step.progress. Provisional hits may change order.
    if !step.provisional {
        // Only the final step has a stable continuation cursor.
        break;
    }
}
let mut count = session.start_count()?;
// Advance count.step(budget) independently; its total is exact when not provisional.
```

Creating a handle does no statistics I/O. Ranking first gathers global BM25
statistics one field/segment at a time inside `step`, exposing exact
`statistics_fields_processed` / `statistics_fields_total` counters. It then retains
its scorer and position within a segment. Weights use global snapshot statistics,
not segment-local statistics. Counts can advance independently while ranking loads
statistics. Progress reports candidate
documents processed and completed/total segments. Rejected candidates and
tombstones count as work; pushed filters reduce this work. Counts are monotone
lower bounds until final and use the same filters as ranking.

The time budget is cooperative, checked after each statistics field, every 256
candidates and at segment boundaries. Scorer/column initialization, I/O and the caller predicate cannot be
preempted. Zero budget still advances a chunk or boundary. `Duration::MAX` runs to
completion; one-shot `page` and `count` use that path. Counting does not calculate
BM25 or allocate a ranking heap. Repeating a completed step returns its final result.

## Writes, replay and rebuild

`Batch` is a complete prefix of canonical outbox work: full upserts, deletes and
highest `last_seq`. Deletes precede upserts; the last upsert for an ID wins. Already
committed sequences are ignored for idempotent replay. The caller must not advance
the sequence past unapplied rows.

Each commit payload is JSON:

```json
{ "schema_version": 1, "analyzer_version": 1, "last_seq": 123 }
```

Tantivy writes segments before atomically replacing `meta.json` through the
directory. `last_applied_seq()` reads this durable payload, including after a crash
between publication and the caller receiving success. Replay outbox entries after
it; prune acknowledged entries only after the index commit.

`rebuild(stream, last_seq, progress, cancel)` takes fallible documents in ascending
page-ID order from a consistent canonical snapshot at `last_seq`. Later canonical
changes remain in the outbox. Writes are serialized while rebuild runs; searches
keep serving the old commit. Documents stream into fresh segments with no
intermediate commit. `delete_all_documents` removes old segments from pending
writer metadata, not published reader snapshots. One final metadata commit
atomically publishes the new generation.

Progress fires at the start, every 1,024 documents and after publication.
Cancellation is checked for every document, before flushing, and after preparing
the commit. Cancellation, invalid order or source errors roll back; none publishes
a partial rebuild. An older source sequence is rejected. The committing flush
itself is synchronous. A crash leaves the old or new complete commit. Existing
sessions keep their original snapshot.

`open` reports missing files, incompatible versions and observed corruption as
`RebuildNeeded`. Authentication is lazy: unread damaged blocks fail on first
access, not in a whole-index scan at open. Both successful reader initialization
and Tantivy errors check `integrity_error()`, since Tantivy can swallow a damaged
positions-file error. `Error::RebuildNeeded::integrity` preserves that cause.
Operational I/O errors remain errors. The owner explicitly replaces an unusable
sidecar, creates an index and streams canonical data; reads never trigger repair.

### Local crash workaround

Killing a delete commit before the metadata rename can leave an uncommitted
`<segment>.<opstamp>.del`. Replay reuses that opstamp and otherwise fails with
`FileAlreadyExists`. A fresh writer runs Tantivy garbage collection **before**
accepting work; rollback does the same before reuse. Last-committed metadata and
live reader references decide what is garbage. No dependency is patched.

Reproduction/regression: `killed_commits_and_rebuilds_reopen_at_a_whole_sequence_and_replay`
in `tests/crash.rs`: commit IDs 1/2, kill their replacement by IDs 3/4 at
`atomic-replace/temp-synced@meta.json:1`, reopen, replay sequence 10.

## Memory and verification

Defaults and measured justification: [the benchmark report](../../../docs/architecture/research/2026-10-vault-search-benchmark.md).
`writer_memory_bytes` is total indexing arena memory divided among `writer_threads`,
not an RSS limit. Tantivy also owns ingestion queues, merge and serialization
buffers. `cache_bytes` bounds retained decrypted blocks/ranges; active readers can
still reference buffers evicted from the cache.

`AfterCommit` reloads synchronously after publication. `Manual` requires `reload()`
before new sessions see a commit. Both use Tantivy's manual reader internally,
without a delayed watcher thread. Existing sessions are never reloaded. Read-only
opening does not allocate a writer.

There is no archive-sized result set, prefix expansion list, eligibility copy or
ID set. A ranking handle keeps O(k) hits and O(query terms) statistics; counting keeps an integer. Each retains
its query weight and one active segment scorer/filter/column set, plus Tantivy's
snapshot readers. Prefix generation uses O(longest token) scratch. Norms, fast
fields, term dictionaries, postings and segment metadata remain Tantivy-owned and
scale with the index. The caller owns eligibility storage and session lifetimes.

Every module begins with its responsibilities, its failure modes and the tests that cover them.
