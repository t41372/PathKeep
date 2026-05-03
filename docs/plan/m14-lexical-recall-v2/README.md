# M14 — Lexical Recall V2

> Source of truth for the non-embedding recall upgrade. This milestone keeps
> PathKeep's day-one Explorer recall deterministic, local-first, and backed by
> rebuildable SQLite projections.

## Goal

Move keyword recall beyond unicode61 prefix matching without introducing
embedding, semantic search, network providers, or SQLite loadable extensions.

The primary path is:

- shared query/index normalization
- FTS5 unicode61 terms with prefix indexes
- SQLCipher-backed FTS5 trigram recall for compact substring matching
- CJK gram recall for unspaced Chinese substring queries
- explicit relevance ranking when a keyword query is active

## WORK-M14-A — Lexical Recall V2 Primary Path

- Status: completed 2026-05-03
- Entry point: `docs/plan/STATUS.md`
- Design doc: `docs/architecture/lexical-recall-v2.md`

### Deliverables

- Add a shared lexical analyzer used by both search projection writes and
  keyword query parsing.
- Rebuild the derived `history-search.sqlite` schema with normalized fields,
  compact text, CJK grams, `history_search_terms`, and `history_search_trigram`.
- Query keyword recall through a bounded FTS candidate union and rank by BM25
  relevance unless the user explicitly selects newest or oldest.
- Add Explorer `Relevance` sort control with `en` / `zh-CN` / `zh-TW` parity.
- Cover SQLCipher FTS capabilities, analyzer behavior, search integration,
  import-batch refresh, and preview URL-state behavior.

### Non-goals

- Do not enable embedding, semantic, or hybrid search.
- Do not add `spellfix1`, SQLite loadable extensions, Jieba, or fuzzy rerank.
- Do not encrypt the derived search DB in this block; it remains rebuildable
  plaintext derived state attached with `KEY ''`.
- Do not rebuild the entire search projection during import finalization except
  when the projection schema itself is reset and must be seeded.

### Closeout

M14-A shipped the primary deterministic recall path:

- `vault-core` owns a shared lexical analyzer for index and query paths.
- The search projection is schema-versioned and now writes raw fields,
  normalized fields, compact text, CJK grams, `history_search_terms`, and
  `history_search_trigram`.
- Keyword query mode defaults to relevance through an FTS candidate union and
  BM25 ranking; explicit newest / oldest still override relevance.
- Relevance pagination uses opaque `r|score|visitTime|id` cursors while legacy
  chronological cursors remain accepted.
- Explorer exposes `Relevance` sort with `en` / `zh-CN` / `zh-TW` parity and
  browser-preview fixture behavior aligned to the backend contract.

Supply-chain remediation note: the original `opencc-rs` candidate was rejected
for the first M14 implementation because its native OpenCC / marisa build
depends on CMake and bindgen being available in the checker environment. The
interim `ferrous-opencc` dependency was then removed because it did not meet the
repo dependency trust gate. The approved follow-up restored NFKC and
full-width/half-width folding through ICU4X `icu_normalizer`, which is
maintained by the Unicode Consortium and was already present in the dependency
graph. M14-D then restored Traditional/Simplified folding with official OpenCC
1.3.0 dictionary assets plus repo-owned Rust conversion code. Low-trust Rust
bindings remain out of scope; native OpenCC C++ linking remains a future path
only after the CMake/C++/CI/package contract is proven.

Validation:

- `cargo test --manifest-path src-tauri/Cargo.toml -p vault-core search_ -- --test-threads=1`
- `cargo test --manifest-path src-tauri/Cargo.toml -p vault-core lexical_recall -- --test-threads=1`
- `cargo test --manifest-path src-tauri/Cargo.toml -p vault-core archive:: -- --test-threads=1`
- `bun run fmt:rust && bun run lint:rust`
- targeted Explorer / preview Vitest slices
- `bun run coverage:js`
- `bun run check`

## WORK-M14-B — Bounded Fuzzy Recall And Query Expansion

- Status: completed 2026-05-03
- Entry condition: run a dedicated candidate-volume benchmark before changing
  ranking behavior.

This follow-up shipped Rust-side typo tolerance and a small alias dictionary
without adding a new dependency:

- `gh`, `yt`, and `pr` expand to `github`, `youtube`, and `pull request` before
  FTS query construction.
- Latin typo tolerance runs only when the normal FTS/trigram query returns zero
  results.
- Fuzzy candidates come from an FTS5 trigram OR query capped at 200 URL
  documents and 400 visible visits before repo-owned bounded edit-distance
  scoring runs in Rust.
- Explicit newest / oldest sort still overrides relevance. Regex mode remains
  unchanged.

`strsim` remains approved for a future bounded rerank path, but M14-B did not
need it. SQL full-scan edit distance, SQLite extension loading, `spellfix1`,
Jieba, embedding, semantic search, and vector sidecars remain forbidden.

Validation:

- `cargo test --manifest-path src-tauri/Cargo.toml -p vault-core search_projection::tests::fuzzy_trigram_candidate_probe_is_limited_before_rust_rerank -- --test-threads=1`
- `cargo test --manifest-path src-tauri/Cargo.toml -p vault-core search_lexical -- --test-threads=1`
- `cargo test --manifest-path src-tauri/Cargo.toml -p vault-core lexical_recall -- --test-threads=1`

## WORK-M14-D — Official OpenCC Toolchain And Script Folding

- Status: completed 2026-05-03
- Decision record: `docs/architecture/opencc-script-folding.md`

This follow-up shipped the official-asset path, not the C++ library path:

- Vendored the minimal Apache-2.0 OpenCC `ver.1.3.0` dictionary subset needed
  for `t2s` and `tw2sp` recall variants.
- Added a repo-owned Rust converter that parses those assets once per process,
  applies longest-match dictionary order, and emits both variants when OpenCC's
  direct Traditional/Simplified and Taiwan idiom configs differ.
- Documented that the native C++ path is still blocked from product code on this
  host because `cmake` and `pkg-config` are not on `PATH`; future C++ linking
  must first prove CI packages, static/dynamic link strategy, release packaging,
  and rollback.
