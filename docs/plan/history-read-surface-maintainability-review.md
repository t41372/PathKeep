# Archive History Read Surface Maintainability Review

> Review artifact for `WORK-HISTORY-MAINT-A`. This document records the current
> `vault-core::archive::history` ownership map before any behavior-preserving
> split work.

## Scope

`src-tauri/crates/vault-core/src/archive/history.rs` reached 1229 lines after
M14 lexical recall and fuzzy fallback work. That crosses the repo's 1200-line
maintainability review threshold.

This review covers only the Explorer-facing archive history read surface:

- visible canonical history query
- keyword / lexical / fuzzy recall dispatch
- regex post-filter recall
- cursor and pagination envelope shaping
- lazy favicon hydration
- export artifact rendering

This review does not change Rust product code. It does not change ranking,
pagination, regex behavior, favicon fallback, export formats, SQLCipher attach
policy, or the lexical analyzer.

## Current Architecture Map

`history.rs` currently acts as one file with seven owners:

| Owner               | Current symbols                                                                                                                    | Responsibility                                                                | Risk if changed casually                                              |
| ------------------- | ---------------------------------------------------------------------------------------------------------------------------------- | ----------------------------------------------------------------------------- | --------------------------------------------------------------------- |
| Public facade       | `list_history`, `export_history`, `load_history_favicons`                                                                          | Opens archive connections and exposes the archive history command surface     | Breaking worker/Tauri public archive API                              |
| Baseline SQL recall | `list_history_with_sql`, `LIST_HISTORY_SQL`, `COUNT_HISTORY_SQL`                                                                   | Non-regex, non-lexical visible visit filtering and chronological cursor pages | Page drift, hidden/reverted row leakage                               |
| Lexical recall      | `LIST_HISTORY_LEXICAL_SQL`, `COUNT_HISTORY_LEXICAL_SQL`, `list_history_with_lexical_search`                                        | SQLCipher FTS candidate union, BM25 ranking, relevance cursor pages           | Relevance instability, bad cursor compatibility                       |
| Fuzzy recall        | `LIST_HISTORY_FUZZY_CANDIDATES_SQL`, `list_history_with_fuzzy_fallback`, `sort_fuzzy_items`, `fuzzy_start_index`, `FuzzyCandidate` | Bounded trigram candidate window plus Rust-side edit-distance scoring         | Unbounded scans, inconsistent newest/oldest overrides                 |
| Regex recall        | `list_history_with_regex`                                                                                                          | Manual regex post-filter over visible canonical SQL rows                      | Accidental FTS normalization, invalid-regex error drift               |
| Pagination envelope | `HistoryCursor`, cursor parse/encode helpers, `page_count`, response builders                                                      | Shared cursor compatibility and response metadata                             | Cursor breakage, wrong `has_next` / `has_previous`                    |
| Favicon hydration   | `LOAD_FAVICON_*`, `load_history_favicons`, `load_entry_favicon`, `query_favicon_statement`                                         | Lazy post-render icon lookup with exact/page/host/registrable fallback        | Large-table scans, future-icon leakage, duplicate hydration           |
| Export rendering    | `collect_history_for_export`, `render_export_content`, format renderers                                                            | Cursor-walk full visible result set and write local artifact                  | Exporting only the current UI page, memory growth beyond page windows |
| Row shaping         | `history_entry_from_row`, `history_entry_with_score_from_row`                                                                      | Converts SQL rows into `HistoryEntry`                                         | Column-order regressions across SQL owners                            |

The file is not a classic unreadable blob yet: each block is coherent and the
test suite covers the key behavior. The problem is that M14 added enough recall
surface area that future edits now require understanding unrelated owners before
touching a local change.

## Existing Test Protection

Current useful coverage lives mostly in
`src-tauri/crates/vault-core/src/archive/tests.rs`:

- `lexical_recall_matches_cjk_script_folding_and_compact_substrings`
- `lexical_recall_expands_aliases_and_uses_bounded_fuzzy_fallback`
- `lexical_recall_defaults_to_relevance_and_accepts_time_sort_override`
- backup/import history assertions that cover baseline visible history queries
- favicon lookup assertions for duplicate suppression, future-icon refusal,
  same-profile/cross-profile page fallback, host fallback, registrable-domain
  fallback, and query-plan index usage
- export assertions for paged-query export, HTML/Markdown/Text/JSONL formats,
  and multi-page cursor walking

The current guard set is strong enough for behavior-preserving extraction if
the split keeps signatures and SQL text stable. It is not strong enough to
justify rewriting the query planner or replacing the cursor contract in the same
window.

## Finding

Splitting is warranted, but it should be staged. Keeping everything in one file
is now worse than a behavior-preserving module split because:

- Favicon hydration and export rendering do not need to see lexical SQL,
  relevance cursors, or fuzzy scoring internals.
- Lexical/fuzzy recall can evolve independently from baseline SQL and regex
  mode, but their current helpers share a large local namespace.
- Cursor envelope helpers are a cross-mode contract and should be isolated from
  SQL details before more recall modes are added.
- The 1229-line file is already over the review threshold, and the next small
  recall or favicon fix would push it further without adding product value.

Do not do a one-shot rewrite. The high-risk boundary is lexical/fuzzy SQL plus
cursor pagination. It should move only after lower-risk owners have already
been extracted and targeted tests prove no behavior drift.

## Recommended Target Shape

Use `history.rs` as the public facade for the first implementation slice. It can
own dispatch and submodule wiring while submodules live under
`src-tauri/crates/vault-core/src/archive/history/`.

Recommended owners:

| Target module                   | Owns                                                                         | Notes                                                                                                                                                     |
| ------------------------------- | ---------------------------------------------------------------------------- | --------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `archive/history.rs`            | public facade, mode dispatch, shared row mapper until lexical split          | Keep exported names stable: `list_history`, `export_history`, `load_history_favicons`                                                                     |
| `archive/history/pagination.rs` | `HistoryCursor`, sort normalization, cursor encode/decode, response builders | Shared by SQL, lexical, fuzzy, regex, and export cursor walking                                                                                           |
| `archive/history/favicons.rs`   | favicon SQL constants and lazy hydration lookup                              | Lowest-risk extraction because it has a separate public function and targeted query-plan tests                                                            |
| `archive/history/export.rs`     | export cursor walk and format renderers                                      | Low-risk extraction if it calls `super::list_history` and keeps page-window behavior                                                                      |
| `archive/history/lexical.rs`    | lexical/fuzzy SQL, fuzzy candidate shape, relevance query path               | Move after pagination helpers are isolated                                                                                                                |
| `archive/history/sql.rs`        | baseline SQL recall and regex post-filter                                    | Move last, because `LIST_HISTORY_SQL` / `COUNT_HISTORY_SQL` currently live in `archive/mod.rs` and should not be churned until the facade split is stable |

## Implementation Plan

### WORK-HISTORY-MAINT-B - Owner Extraction

First code slice:

1. Add `archive/history/pagination.rs`, `archive/history/favicons.rs`, and
   `archive/history/export.rs`.
2. Keep `archive/history.rs` as the public facade instead of converting it to a
   directory module in the same change.
3. Move only behavior-preserving code:
   - cursor parse/encode and response builders
   - favicon hydration SQL/helpers
   - export collection/rendering
4. Re-export only the functions needed by `archive/mod.rs`.
5. Run targeted archive history tests, then `bun run check`.

Expected result: `history.rs` drops below the 1200-line threshold without
touching lexical SQL or baseline SQL behavior.

### Later Slice - Lexical SQL Extraction

Move lexical/fuzzy SQL into `archive/history/lexical.rs` only after the first
slice lands. This slice should introduce a narrow query-context struct to reduce
the current `too_many_arguments` surface, but it must not change ranking,
cursor, candidate limits, or fuzzy fallback activation.

### Deferred Slice - Baseline SQL Placement

Consider moving `LIST_HISTORY_SQL` and `COUNT_HISTORY_SQL` from `archive/mod.rs`
to `archive/history/sql.rs` only if a later query-surface cleanup needs it.
Those constants are stable and not the current maintenance bottleneck.

## Explicit Non-Goals

- No query behavior changes.
- No SQL planner changes.
- No new dependency.
- No semantic/vector/embedding recall.
- No SQLite loadable extension.
- No full-scan edit distance.
- No merge of regex mode into lexical mode.
- No export streaming rewrite in the extraction slice.

## Acceptance Gates

For the first extraction slice:

```bash
cargo test --manifest-path src-tauri/Cargo.toml -p vault-core lexical_recall -- --test-threads=1
cargo test --manifest-path src-tauri/Cargo.toml -p vault-core archive::tests -- --test-threads=1
bun run check
```

The full archive test filter is intentionally included because favicon and
export tests are currently embedded in larger archive integration tests rather
than isolated unit modules.
