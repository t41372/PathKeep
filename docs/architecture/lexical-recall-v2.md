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
it does not meet the repo's dependency trust gate. It was removed before any
follow-up normalization shipped.

Approved boundary after the M14 remediation:

| Area                  | Decision                                                                                                                            |
| --------------------- | ----------------------------------------------------------------------------------------------------------------------------------- |
| NFKC / full-width     | Shipped through ICU4X `icu_normalizer`, maintained by the Unicode Consortium and already present in the dependency graph            |
| OpenCC script folding | Shipped through official OpenCC 1.3.0 dictionary assets plus repo-owned Rust conversion code; low-trust Rust bindings remain banned |
| OpenCC C++ tooling    | Still allowed only through project-scoped vcpkg manifest proof; no Homebrew / apt / global `pkg-config` product dependency          |
| Fuzzy typo tolerance  | Shipped with repo-owned bounded edit distance over FTS/trigram top-N candidates; `strsim` remains approved but is not needed yet    |

The detailed provenance, local toolchain probe, and C++ rollback contract live
in [opencc-script-folding.md](opencc-script-folding.md). The general C / C++
dependency manager contract lives in
[native-dependency-management.md](native-dependency-management.md).

## Analyzer Contract

The same analyzer is used for both projection writes and keyword query parsing.

Pipeline:

1. Unicode NFKC compatibility folding through ICU4X.
2. OpenCC-derived `t2s` and `tw2sp` script variants from official dictionary
   assets.
3. Unicode lowercase folding with the Rust standard library.
4. Term tokenization for unicode61 FTS prefix recall.
5. Compact text generation by removing punctuation and whitespace, so
   `git hub`, `git-hub`, and `github` can share a substring path.
6. CJK 2-gram and 3-gram generation from compact CJK runs.

The analyzer is deterministic. It does not infer semantics, classify intent, or
call an external tokenizer. When OpenCC configs differ, indexed documents keep
both variants and queries OR their variants. This is why `設定`, `设定`, and
`设置` can meet without adding embedding or fuzzy search.

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

M14-B adds two deterministic query-expansion layers:

- Small short-alias expansion before FTS: `gh -> github`, `yt -> youtube`,
  and `pr -> pull request`.
- Latin typo fallback only when the normal FTS/trigram candidate count is zero.
  The fallback asks the trigram table for an OR-of-trigrams candidate window,
  caps that window at 200 URL documents and 400 visible visits, and then applies
  repo-owned bounded edit distance in Rust. It does not run edit distance in SQL
  and it does not scan canonical rows outside the FTS candidate window.

Explorer keyword input also accepts a local subset of Google-like advanced
operators before lexical analysis:

- `site:` maps to archived URL/site/domain filtering and can be combined with
  the explicit Domain field.
- Leading `-` excludes words or quoted phrases from URL/title/search-term
  evidence, enabling queries such as `site:github.com -pathkeep` without regex
  look-around.
- Uppercase or lowercase `OR` splits rankable keyword segments into an FTS OR
  query, matching Google's "any of these words" behavior for archived evidence.
- Quoted phrases become exact URL/title/search-term constraints while still
  using lexical FTS to produce candidates.
- `intitle:`, `inurl:`, `filetype:`/`ext:`, `after:`, and `before:` become
  SQL-side local-history constraints.

These operators do not change the local-first boundary: they only evaluate
archived URL/title/search-term/visit-time facts and the rebuildable search
projection. Google web-index features that need page language, region, license,
related pages, image metadata, or remote index knowledge remain out of scope.

Regex mode remains a manual post-filter over visible canonical rows and does not
use the analyzer.

Sort behavior:

- No keyword query: default remains newest first.
- Keyword query with no explicit sort: default is relevance.
- Explicit newest or oldest overrides relevance.

Relevance uses FTS5 BM25 with title weighted above URL, and URL/search terms
above compact/CJK support fields. Lower BM25 scores rank first; ties fall back
to newest visit time and then visit id.

Fuzzy fallback relevance uses Rust-side edit-distance scores with the same field
intent: title beats URL, URL beats search terms, and compact substring support is
lowest. Because the fallback is deliberately bounded, its `total` reflects the
accepted candidate window, not a global approximate count.

Cursor behavior:

- New relevance cursors are opaque `r|score|visitTime|id` strings.
- Legacy `visitTime|id` cursors remain accepted for newest/oldest pagination.

## Follow-up Boundary

Pinyin and match explanations remain out of M14. Any future expansion must keep
the same rule: FTS/trigram first, bounded Rust-side work second, and no SQLite
loadable extension or SQL full-scan edit distance.
