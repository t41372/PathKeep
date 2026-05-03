# Lexical Recall V2

> Accepted implementation record for PathKeep's non-embedding keyword recall
> upgrade.

## Context

PathKeep already stores lexical recall in the rebuildable
`derived/history-search.sqlite` plane. Before M14, Explorer keyword mode used
one FTS5 `unicode61 remove_diacritics 2` table over URL, title, and search
terms. That was adequate for Latin prefix recall, but Chinese text behaved close
to exact matching because unicode61 does not segment unspaced Chinese phrases.

M14 keeps the local-first search stack deterministic:

- no embedding provider
- no vector sidecar
- no SQLite loadable extension
- no network-backed language service

## SQLCipher Capability Baseline

Do not use the system `sqlite3` binary as the source of truth. PathKeep links
SQLite through `rusqlite` with `bundled-sqlcipher-vendored-openssl`.

The M14 preflight probe against that actual Rust/SQLCipher path returned:

- SQLite `3.50.4`
- SQLCipher `4.10.0 community`
- `ENABLE_FTS5`
- `tokenize='unicode61 remove_diacritics 2'` works
- `tokenize='trigram'` works
- `bm25(...)` works

The derived search DB remains plaintext rebuildable derived state in this
milestone. Archive connections still attach it with `ATTACH ... AS search KEY
''`. M14 records that boundary but does not change the key policy.

## OpenCC Supply Chain Boundary

M14 initially evaluated `opencc-rs 0.5.1` backed by
`opencc-sys 0.4.1+1.3.0`. That binding ships embedded OpenCC `.ocd2` assets,
but it builds native OpenCC and marisa through CMake/bindgen. The local M14
compile probe failed because `cmake` was not on the per-commit gate `PATH`.
Relying on a host-specific `CMAKE=/opt/homebrew/...` override would make
`bun run check` fragile.

An interim pure-Rust OpenCC crate was rejected after supply-chain review because
it does not meet the repo's dependency trust gate. The shipped M14 code now has
no OpenCC dependency and no new Unicode normalization dependency. Traditional /
simplified Chinese conversion and full-width / half-width folding are therefore
not claimed by `WORK-M14-A`; they require a future design window that either
uses an approved official OpenCC path or receives explicit user approval after a
package audit.

Current M14-A lexical normalization is deliberately narrower:

| Item             | Decision                                                                  |
| ---------------- | ------------------------------------------------------------------------- |
| New dependencies | None beyond the already committed core stack after remediation            |
| Native build     | No CMake, C++, libclang, bindgen, or loadable extension path              |
| OpenCC behavior  | Not shipped until an approved official/supply-chain-audited path exists   |
| NFKC behavior    | Not shipped until an approved dependency or in-repo implementation exists |

## Analyzer Contract

The same analyzer is used for both projection writes and keyword query parsing.

Pipeline:

1. Unicode lowercase folding with the Rust standard library.
2. Term tokenization for unicode61 FTS prefix recall.
3. Compact text generation by removing punctuation and whitespace, so
   `git hub`, `git-hub`, and `github` can share a substring path.
4. CJK 2-gram and 3-gram generation from compact CJK runs.

The analyzer is deterministic. It does not infer semantics, classify intent, or
call an external tokenizer.

## Projection Schema

The derived search projection stores raw fields and derived fields:

- raw URL, title, and normalized search terms
- normalized URL, title, and search terms
- compact recall text
- CJK gram text

FTS tables:

- `history_search_terms`: unicode61 FTS5 table with prefix indexes for Latin
  terms, normalized fields, and CJK grams.
- `history_search_trigram`: FTS5 trigram table over compact text.

The schema is versioned inside the derived search DB. When the version changes,
the projection may be reset because it is rebuildable state. Import finalization
still refreshes only the touched import batch after the schema has been seeded.

## Query And Ranking Contract

Keyword mode builds a candidate union from:

- term/prefix matches
- CJK gram matches
- compact trigram matches when the compact query has at least three codepoints

Regex mode remains a manual post-filter over visible canonical rows and does not
use the analyzer.

Sort behavior:

- No keyword query: default remains newest first.
- Keyword query with no explicit sort: default is relevance.
- Explicit newest or oldest overrides relevance.

Relevance uses FTS5 BM25 with title weighted above URL, and URL/search terms
above compact/CJK support fields. Lower BM25 scores rank first; ties fall back
to newest visit time and then visit id.

Cursor behavior:

- New relevance cursors are opaque `r|score|visitTime|id` strings.
- Legacy `visitTime|id` cursors remain accepted for newest/oldest pagination.

## Follow-up Boundary

Levenshtein, Jaro-Winkler, alias expansion, pinyin, and match explanations are
explicitly out of `WORK-M14-A`. `WORK-M14-B` may add Rust-side fuzzy reranking
only after FTS/trigram has produced a bounded candidate set.
