# M14-B Fuzzy Candidate Benchmark

> Evidence note for `WORK-M14-B`. This is a bounded-candidate proof, not a
> search-quality benchmark.

## Purpose

M14-B may only run fuzzy scoring after SQLite FTS/trigram has reduced the search
space. The benchmark verifies the candidate window is capped before Rust-side
edit-distance scoring runs.

## Probe

Command:

```bash
cargo test --manifest-path src-tauri/Cargo.toml -p vault-core search_projection::tests::fuzzy_trigram_candidate_probe_is_limited_before_rust_rerank -- --test-threads=1
```

The test uses the same workspace `rusqlite` / bundled SQLCipher feature set as
PathKeep product code. It creates an in-memory FTS5 trigram table with 500 rows
matching the fuzzy trigram OR query for `github`, then runs the production-shape
bounded subquery:

```sql
SELECT rowid
FROM fuzzy_probe
WHERE fuzzy_probe MATCH ?1
ORDER BY bm25(fuzzy_probe, 1.0) ASC
LIMIT 200
```

Observed on 2026-05-03:

| Metric                       | Value |
| ---------------------------- | ----- |
| Unbounded matching documents | 500   |
| Bounded candidate documents  | 200   |
| Test runtime                 | 0.03s |

## Shipping Guard

The product fallback uses two caps:

- `FUZZY_CANDIDATE_URL_LIMIT = 200`
- `FUZZY_CANDIDATE_VISIT_LIMIT = 400`

Edit distance never runs inside SQL and never runs against the canonical visits
table directly. Regex remains unaffected and keeps its explicit manual
post-filter boundary.
