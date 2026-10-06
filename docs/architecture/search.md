# Search ownership and wire contract

Search uses `vault-search` and canonical Turso reads. `Vault` owns one indexer and a session registry. App Lock retains the Vault; closing it cancels jobs, drops every reader and joins the indexer before closing the store. The index lives under `derived/search-index/`, with the archive's encryption mode and a derived search key.

The indexer reads ordered outbox prefixes, loads current page documents and profile memberships, commits the highest sequence with `SearchIndex::apply`, then removes acknowledged rows with a Batch write. Reopen resumes from the durable index sequence. Reapplying a page replaces it. Missing, incompatible and corrupt generations rebuild from canonical page IDs. Healthy rebuilds publish a new generation only after its source scan and outbox tail are complete. Existing sessions retain their old reader until released. Initial builds publish increasing prefixes; every response identifies incomplete coverage.

Normalization remains in vault-core: ICU NFKC, accent folding, OpenCC, aliases, compact text and CJK segmentation. Visible titles from different profiles retain phrase boundaries. Date/profile/browser predicates apply to the same canonical visit. Selective dates use the visible time indexes; other searches resolve posting batches through page→URL→visible-visit indexes. Hydration verifies current visibility. Canonical changes restart membership and exact counting on the pinned reader; a write during hydration suppresses that mixed step.

A session pins the complete committed visit-ID prefix as well as its index reader. New backup visits cannot move an old page across a cursor; current revocations still exclude data. A compact delivered-page bitset prevents repeats when revocation changes a shared page’s representative visit. One lookahead hit determines continuation independently of the count. This is snapshot isolation, not a fixed-size retrieval window.

Ranking covers every eligible posting and retains only the requested top-k: score descending, newest eligible visit descending, page ID descending. Explicit newest/oldest orders remain available. Counting has an independent posting walk. Short fuzzy targets whose allowed edits can destroy every trigram also walk the complement of the trigram query. The existing full-text edit predicate validates membership, and exact/CJK branches remain eligible. Longer targets use trigrams only when the edit bound guarantees a surviving gram. Regex scans every eligible canonical visit in bounded chunks; a mandatory literal can prefilter through exact positional trigrams only while index coverage is current. AI fusion ranks complete lexical and vector streams before using the same engine collector and cursors. Model message size, SQL batch size and step budgets never truncate the matching universe.

Keyword `OR` joins bare-word groups. Field, phrase, exclusion and metadata
constraints apply to every group, as in the existing parser. Title/URL constraints
retain canonical substring semantics; index prefixes/compact fragments are only
complete candidate prefilters for them. Phrases use actual text fields, not the
arbitrary order of a gram set or concatenated normalization variants. Quoting a
short alias keeps it literal. Regex syntax is enabled only by `regex: true`; its
slashes and operator-looking text remain literal parts of the Rust regex pattern.

## Desktop, worker and dev IPC

Tauri commands:

```ts
start_search({ request: SearchRequest, onEvent: Channel<SearchEvent> }): Promise<SearchStarted>
next_search_page({ sessionId: string, cursor: string }): Promise<void>
cancel_search({ sessionId: string }): Promise<void>
search_indexer_state(): Promise<IndexerState>
```

The worker exports these same operations with a Rust event callback. The dev bridge uses `POST /commands/{command}` with the same argument object (omit `onEvent`), and `GET /searches/{sessionId}/events` supplies SSE `data: <SearchEvent JSON>`. Dropping the SSE connection cancels its session. Slow SSE readers receive the latest provisional step; terminal events clear retained result data. A final page does not close a session while continuation is possible. Call cancel when leaving a search. Vault close invalidates all session IDs and cursors.

```ts
type SearchRequest = {
  query?: string // default ""
  pageSize?: number // default 100, positive
  sort?: 'relevance' | 'newest' | 'oldest' | null
  profileId?: string | null
  browserKind?: string | null
  domain?: string | null
  startTimeMs?: number | null
  endTimeMs?: number | null
  regex?: boolean // default false
  fuzzy?: boolean // default false
  starred?: boolean // default false
}
type IndexerState = {
  phase:
    | 'building'
    | 'updating'
    | 'ready'
    | 'rebuilding'
    | 'cancelled'
    | 'failed'
    | 'closed'
  documentsProcessed: number // rebuild documents; then cumulative applied replacements
  documentsTotal: number // build target; update progress uses appliedSeq/targetSeq
  appliedSeq: number
  targetSeq: number
  error: string | null
}
type SearchStarted = { sessionId: string; indexer: IndexerState }
type SearchCount = { value: number; exact: boolean }
type SearchProgress = {
  documentsProcessed: number
  segmentsCompleted: number
  segmentsTotal: number
  statisticsFieldsProcessed: number
  statisticsFieldsTotal: number
}
type SearchCoverage = {
  indexer: IndexerState
  snapshotSeq: number
  coverageComplete: boolean
}
type AiSearchNote =
  | {
      code:
        | 'lexicalFallbackNoProvider'
        | 'emptySemanticIndex'
        | 'semanticMatchesFilteredOut'
        | 'configDriftDimension'
        | 'configDriftFingerprint'
    }
  | { code: 'stale'; reason: 'watermark' | 'enrichment' }
  | { code: 'providerResolutionFailed'; reason: string }
type SearchStep = SearchCoverage & {
  page: number // starts at 1; unchanged by count-only progress
  items: Array<{
    pageId: number
    score: number
    visit: HistoryEntry
    reason?: string
  }>
  provisional: boolean
  progress: SearchProgress
  count: SearchCount // matching pages
  visits: SearchCount // matching visible visits
  countProgress: SearchProgress
  cursor: string | null
  searchNotes?: AiSearchNote[]
}
type SearchEvent =
  | { type: 'indexer'; data: IndexerState }
  | { type: 'step'; data: SearchStep }
  | { type: 'error'; data: string }
  | { type: 'cancelled' }
type HistoryEntry = {
  id: number
  profileId: string
  url: string
  title: string | null
  domain: string
  favicon: null
  visitedAt: string
  visitTime: number
  durationMs: number | null
  transition: number | null
  sourceVisitId: number
  appId: string | null
  enrichmentExcerpt: string | null
  visitCount?: number
}
```

`provisional` concerns ranking; `count.exact` concerns the independent count. Both must finish before a page is entirely settled and a continuation cursor is published. Replace the provisional contents of the identified page on each step; do not append every step as a new page. `exact` means exact within the pinned reader's current canonical eligibility; `coverageComplete` separately states whether that reader includes the current archive prefix. An empty step during an initial build is never an assertion that the archive contains no matches. Start a new session to include subsequently indexed data. Cursors are opaque, session-bound and valid for one next-page request; changing query, filters or page size requires a new search.

Existing history/AI responses include `searchState: SearchCoverage | null`. Their synchronous adapters finish the same engine page. Their serialized cursors include the session ID; no numeric offset or archive window is accepted. MCP `search-history` accepts `{query, profileId?, domain?, limit?, cursor?}` and returns `{items, total, nextCursor, searchState, providerId, model, notes}`. Search exports wait for indexing through the canonical sequence captured at export start, then walk one reader to exhaustion with cancellation. Timeline exports retain their separate timeline reader.

## Dependency behavior reproduced locally

Tantivy 0.26's stock n-gram tokenizer assigns overlapping grams position zero. Treating those grams as phrase positions therefore cannot prove a literal substring. Minimal input: `abcdef` must contain the adjacent grams `abc`, `bcd`, `cde`, `def`; their positions must be 0, 1, 2, 3. The local positional tokenizer and substring contract tests cover this without a dependency patch.

Tantivy also discards terms exceeding its token byte limit. Rejecting a whole page would make it unreachable. The local analyzer encodes long exact/prefix terms with a separate length-and-SHA256 namespace; short terms use a raw namespace. Rolling hashes keep long-prefix generation linear in input bytes. The oversize contract test indexes and searches tokens beyond the engine limit, including their prefixes. Schema, analyzer and caller projection revisions all require a rebuild when incompatible. No changes to vault-crypto are needed.

Code-mode `query_history` returns `{rows, notes, total, hasMore, nextCursor,
searchState}`. `total` is the exact page count for that snapshot; a script must
check `searchState.coverageComplete` before asserting archive-wide absence or a
complete aggregate. Model-facing search summaries include `snapshot_total` and
serialized `search_state`, including for empty partial builds. Sandbox execution
limits remain explicit. Programs that stop before exhausting a search must return
their continuation and mark their aggregate incomplete.
