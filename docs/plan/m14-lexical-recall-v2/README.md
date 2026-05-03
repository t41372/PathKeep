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

- Add a shared dependency-free lexical analyzer used by both search projection writes and
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

- `vault-core` owns a shared dependency-free lexical analyzer for index and
  query paths.
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
for this milestone because its native OpenCC / marisa build depends on CMake and
bindgen being available in the checker environment. The interim `ferrous-opencc`
dependency was then removed because it did not meet the repo dependency trust
gate. M14-A no longer ships OpenCC or NFKC folding; traditional/simplified
conversion and full-width/half-width folding require a future approved
dependency or in-repo implementation.

Validation:

- `cargo test --manifest-path src-tauri/Cargo.toml -p vault-core search_ -- --test-threads=1`
- `cargo test --manifest-path src-tauri/Cargo.toml -p vault-core lexical_recall -- --test-threads=1`
- `cargo test --manifest-path src-tauri/Cargo.toml -p vault-core archive:: -- --test-threads=1`
- `bun run fmt:rust && bun run lint:rust`
- targeted Explorer / preview Vitest slices
- `bun run coverage:js`
- `bun run check`

## WORK-M14-B — Bounded Fuzzy Recall And Query Expansion

- Status: blocked follow-up
- Blocker: run a dedicated candidate-volume benchmark and fuzzy-recall design
  window.

This follow-up may add Rust-side typo tolerance and a small alias dictionary
only after FTS/trigram has produced a bounded candidate set. SQL full-scan edit
distance and SQLite extension loading remain forbidden.
