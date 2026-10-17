# Vector search: Turbovec, LEANN and our flat index (2026-10-16)

The user asked to evaluate and measure Turbovec and LEANN for semantic search. This is also the precondition of `WORK-EMBED-USES`: every new use of embeddings is one more pass over all vectors, so we first need to know what a pass costs at 14.4M.

Short version:

- A semantic search spends about 7 % of its time on the vector scan. Hydration and ranking take the rest: 5.75 s per search at 1M visits, about 80 s or more at 14.4M. No vector engine changes that.
- The shipped scan is about 90× slower than a plain scan of the same int8 codes, because it reads every record from disk with its own seek, read and three allocations.
- Turbovec is the fastest scan measured (12.7 ms at 14.4M × 256 on one thread) and needs a third (4-bit) to two thirds (2-bit) less memory than int8. Alone it loses recall: recall@10 0.948 with the static model, 0.76 when a site filter leaves 1 % of the pages. As a first stage that picks 2,000 candidates for an int8 rescore it lost nothing on our 131,322 pages, filters included; whether 2,000 is still enough at millions of pages is not measured. Its API returns the top k only, which conflicts with complete ranking (below).
- LEANN was not measured. It recomputes embeddings at query time, which with an embeddings service means hundreds of requests per search, and we bundle no engine.

## How we got here

| Date       | What happened                                                                                                                                                                                                                                                                                                                                                                                                     |
| ---------- | ----------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| 2026-06-20 | D5 in [02-architecture-decisions.md](../../plan/program/ai-redesign-2026/02-architecture-decisions.md) picks Turbovec behind the `VectorIndex` trait. The user vouched for the maintainer, which met the supply-chain rule.                                                                                                                                                                                       |
| 2026-06-21 | `WORK-AI-5` ships a hand-written index instead: sign bits in RAM pick 2,000 candidates, int8 codes on disk rescore them. The changelog gives the reason as Turbovec failing the star rule, which contradicts D5's record; there is no trade-off document. Measured then: p50 105 ms at 14.4M × 256 for the index alone.                                                                                           |
| 2026-10-07 | The move to Turso and Tantivy removes the 2,000-candidate pool as a coverage violation ([search-integration-validation](2026-10-search-integration-validation.md): "Every stored vector is scored; canonical facets cannot hide a lower-ranked eligible page"). `visit_scores` now reads every int8 record off disk one at a time. The `05` revision and the `vector_planes.rs` header still describe two stages. |
| 2026-10-16 | Turbovec has 17.4k stars (MIT, pure Rust), so it passes the supply-chain rule on its own. LEANN has 13.0k (MIT).                                                                                                                                                                                                                                                                                                  |

## Where a semantic search spends its time

`semantic_stage_bench.rs` (`PATHKEEP_SEMANTIC_STAGE_BENCH=1`) builds a real plaintext Vault with two visits per page and a 256-dim index, then times each step of `add_semantic` and the ranking after it. Embedding is excluded: queries are precomputed vectors. The sum of the stages matches the public entry point (`semantic_search_history` with the test embedding stub) within 3 ms at the median, and the first page is the same. 20 queries per size, warm OS cache, median milliseconds:

| Stage                                                  | 264,388 visits (132,194 pages) | 1,000,000 visits (500,000 pages) |
| ------------------------------------------------------ | -----------------------------: | -------------------------------: |
| Keyword ranking and checks (incl. the staleness check) |                             31 |                              115 |
| Index open (reads the whole sign-bit plane)            |                              4 |                               14 |
| Vector scan into a score map                           |                             98 |                              369 |
| Hydration (every visit, SQL in batches of 512)         |                            454 |                            1,654 |
| Ranking and first page (every page)                    |                            731 |                            3,588 |
| **Total**                                              |                      **1,318** |                        **5,755** |

- Time grows a little faster than the data (4.4× for 3.8× the visits). At 14.4M visits a search would take at least 80 s.
- Hydration looks up each visit's score and asks the archive for its page and time under the search's filters: 28,000 queries at 14.4M visits. Ranking then reads every page's newest visit, sorts all pages and stores a score for each in the session before the first page is cut.
- Fixed costs paid on every search: `FlatVectorIndex::open` reads the whole sign-bit plane (27 ms at 1M vectors, 106 ms at 4M, about 380 ms at 14.4M) although the scan only needs its length, and `search_store::staleness` runs `COUNT(*)` and `MAX(id)` over `visits`.
- The peak memory the bench reports (1.8 GB at 1M visits) includes seeding the archive, so it is not the search's own.

## Scans compared

`tests/vector_scan_bench/` (`PATHKEEP_SCAN_BENCH=1`) runs each scan over the same vectors and checks every result for count, finite scores and filters.

| Name           | What it is                                                                                                                           |
| -------------- | ------------------------------------------------------------------------------------------------------------------------------------ |
| shipped        | `FlatVectorIndex` as it is today: open, then `visit_scores` over the int8 plane on disk                                              |
| int8           | the same int8 codes held contiguously in RAM, one pass, top k                                                                        |
| binary + int8  | sign bits pick `max(2000, 20k)` candidates, int8 rescores them (the June design; a plain popcount loop, so its speed is not a bound) |
| Turbovec 4 / 2 | `turbovec` 1.1.2 `IdMapIndex`, 4-bit or 2-bit, calibrated on a random sample of 2,048 vectors                                        |
| exact          | f32 dot products over a mapped file                                                                                                  |

Data:

- **Real**: the 131,322 pages of the sample Takeout, embedded as PathKeep sends them, with the static model (potion-multilingual-128M, 256 dims) and with Qwen3-Embedding-0.6B (1,024 dims). The queries are the user's own 4,430 Google searches from the same Takeout; the pages opened right after each search are the relevance labels. Filters keep whole sites until they hold 1 % or 10 % of the pages. The vectors stayed on this machine and were deleted afterwards; only aggregates were printed.
- **Synthetic**: random unit vectors, generated in batches, at 1M, 4M and 14.4M × 256 and 1M and 4M × 1,024. The shipped index is skipped above 4M because it would have to write its f32 source first. Random vectors say nothing about recall.

Machine: Apple M5 Max (18 cores, 64 GB). Its memory bandwidth is several times that of the 4-core, 8 GB target, and every scan here is bound by memory bandwidth, so the target will be slower by more than the core count suggests. Turbovec's staged search ran everywhere here; on x86 it needs AVX-512 VBMI and VNNI and otherwise scans the whole index.

### Speed

Search only, p50 milliseconds, one thread / four threads. The shipped column is single-threaded and leaves out its open time.

| Vectors      | Shipped | int8        | binary + int8 | Turbovec 4  | Turbovec 2  | exact       |
| ------------ | ------: | ----------- | ------------- | ----------- | ----------- | ----------- |
| 131k × 256   |      99 | 1.10 / 0.40 | 3.3 / 1.2     | 0.19 / 0.11 | 0.21 / 0.08 | 12.5 / 4.0  |
| 1M × 256     |     742 | 5.7 / 1.8   | 21.8 / 6.0    | 0.89 / 0.31 | 0.91 / 0.32 | 73 / 19     |
| 4M × 256     |   2,916 | 24.1 / 7.1  | 85 / 23       | 3.4 / 1.1   | 3.4 / 1.1   | 291 / 74    |
| 14.4M × 256  |       — | 82 / 26     | 306 / 84      | 12.7 / 3.9  | 12.0 / 3.9  | 1,048 / 267 |
| 131k × 1,024 |     221 | 2.3 / 0.87  | 3.0 / 1.1     | 0.47 / 0.17 | 0.51 / 0.19 | 58 / 15     |
| 1M × 1,024   |   1,174 | 16.4 / 6.9  | 24.0 / 6.9    | 3.3 / 1.0   | 3.3 / 1.0   | 435 / 110   |
| 4M × 1,024   |   4,733 | 89 / 25     | 93 / 25       | 12.1 / 4.3  | 12.2 / 3.7  | 1,708 / 440 |

The in-RAM scans check a filter per vector before scoring (int8 at 256 dims: 1.1 ms unfiltered, 1.5 ms at 1 %, 2.6 ms at 10 %). The shipped scan scores every vector and filters afterwards, so a filter saves it nothing.

### Memory and disk

Measured resident bytes per vector, and what that comes to at three corpus sizes. 14.4M is the worst case of one page per visit; 7.2M is two visits per page, as in the stage bench; 3M is the upper end of the "1–3M after dedup" estimate in `05`. The target has 8 GB in total.

| Code                              | Bytes per vector (256 / 1,024 dims) | 3M × 256 | 7.2M × 256 | 14.4M × 256 | 3M × 1,024 | 7.2M × 1,024 | 14.4M × 1,024 |
| --------------------------------- | ----------------------------------: | -------: | ---------: | ----------: | ---------: | -----------: | ------------: |
| int8                              |                         260 / 1,028 |  0.78 GB |    1.87 GB |     3.74 GB |    3.08 GB |      7.40 GB |      14.80 GB |
| Turbovec 4-bit (incl. its id map) |                           176 / 600 |     0.53 |       1.27 |        2.54 |       1.80 |         4.32 |          8.64 |
| Turbovec 2-bit (incl. its id map) |                           107 / 328 |     0.32 |       0.77 |        1.54 |       0.98 |         2.36 |          4.72 |
| sign bits, packed                 |                            32 / 128 |     0.10 |       0.23 |        0.46 |       0.38 |         0.92 |          1.84 |

- The shipped index keeps the sign bits as `Vec<(u64, Vec<u8>)>`, about 64 bytes per vector at 256 dims (0.92 GB at 14.4M), and reads it again on every search.
- On disk the shipped index takes 1,340 bytes per vector at 256 dims and 5,276 at 1,024, four fifths of it the f32 source `.pkvec`, which search never reads. At 7.2M × 1,024 that is 38 GB.
- Turbovec loads its whole file into RAM; there is no mapped mode.

### Recall

Python over all 4,430 queries (`recall.py`, `shortlist.py`, `fullscan.py`). A result counts as found when its exact score reaches the exact k-th score: identical pages tie, and counting by id punished a scan for picking the other copy. The Rust bench now counts ties the same way. On an even 500 of the queries it gives, unfiltered at 256 dims, int8 0.991, sign bits + int8 0.959, Turbovec 4-bit 0.950 and 2-bit 0.827. Its site filters pick sites in id order rather than by size, so its filtered figures are not comparable with the table. hit@10 is the share of searches where a page the user opened is in the top 10.

| Scan                        | Static 256: recall@10 | recall@100 | hit@10 | 1 % sites | 10 % sites | Qwen3 1,024: recall@10 | recall@100 | hit@10 | 1 % sites | 10 % sites |
| --------------------------- | --------------------: | ---------: | -----: | --------: | ---------: | ---------------------: | ---------: | -----: | --------: | ---------: |
| exact f32                   |                     1 |          1 | 0.5878 |         1 |          1 |                      1 |          1 | 0.7284 |         1 |          1 |
| int8, every vector          |                0.9936 |     0.9939 | 0.5878 |    0.9688 |     0.9869 |                 0.9961 |     0.9958 | 0.7287 |    0.9809 |     0.9937 |
| sign bits 2,000 + int8      |                0.9511 |     0.8128 | 0.5867 |    0.9688 |     0.8701 |                 0.9957 |     0.9833 | 0.7280 |    0.9809 |     0.9933 |
| sign bits 10,000 + int8     |                0.9820 |     0.9343 | 0.5871 |    0.9688 |     0.9833 |                 0.9961 |     0.9955 | 0.7284 |    0.9809 |     0.9937 |
| Turbovec 4-bit alone        |                0.9477 |     0.9406 | 0.5892 |    0.7589 |     0.8962 |                 0.9714 |     0.9687 | 0.7316 |    0.9193 |     0.9565 |
| Turbovec 2-bit alone        |                0.8239 |     0.8136 | 0.5817 |    0.4756 |     0.6619 |                 0.8860 |     0.8733 | 0.7325 |    0.5646 |     0.8260 |
| Turbovec 4-bit 2,000 + int8 |                0.9936 |     0.9939 | 0.5865 |    0.9688 |     0.9869 |                 0.9961 |     0.9958 | 0.7280 |    0.9809 |     0.9937 |
| Turbovec 2-bit 2,000 + int8 |                0.9936 |     0.9939 | 0.5874 |    0.9688 |     0.9869 |                 0.9961 |     0.9958 | 0.7280 |    0.9809 |     0.9937 |

- The filtered columns are recall@10 against the exact top 10 among the allowed pages. A Turbovec first stage of 10,000 gives the same figures as 2,000 in every column. With the 1 % filter only 1,508 pages are allowed, so any first stage of 2,000 keeps them all and the row equals int8.
- With the staged search turned off (`TURBOVEC_4BIT_PLANES=0`, `TURBOVEC_2BIT_PLANES=0`), Turbovec's recall is the same to the third decimal: its loss is quantization, not the shortlist. That is also what an x86 CPU without AVX-512 gets.
- Pages of one site embed close together, so a site filter leaves candidates whose scores differ by less than the 4-bit or 2-bit rounding. That is why Turbovec alone falls apart under filters, and why an int8 rescore of its candidates repairs it completely.
- Sign bits are a poor first stage at 256 dims (recall@100 0.81) and nearly lossless at 1,024 (0.98). Turbovec's codes are lossless as a first stage at both.
- All of this is on 131,322 pages, where 2,000 candidates are 1.5 % of the corpus. At 7.2M pages they are 0.03 %, and more pages crowd the scores near the top, so the shortlist probably has to grow with the corpus. Option B or C below must not be chosen on these numbers alone: measure the shortlist size it needs on a corpus of millions of real pages first.
- hit@10 moves by about 0.006 at most across every scan, Turbovec 2-bit alone included: the pages users opened are found either way. The recall losses show up lower in the list and under filters, which is exactly where the coverage rule applies.

## Turbovec beyond the numbers

- **Top k only.** `search(queries, k, allowlist)` returns the best k. Complete ranking would need k equal to the whole corpus, or a growing k per page of results. Its kernels are crate-internal, so we cannot ask it for every score.
- **Filters inside the kernel.** An allowlist is applied before the heap, so a filter never leaves fewer than k results. It does not help the rounding problem above.
- **x86.** The staged search needs AVX-512 VBMI and VNNI: AMD Zen 4 and later, Intel server chips, a few 10th–11th generation laptops. Intel consumer chips since Alder Lake have no AVX-512 and get the whole scan; its speed there is unmeasured, its recall is the same.
- **Format churn.** The project is seven months old and its file format is at v8; v1–v4 files cannot be decoded and must be rebuilt from source vectors. A rebuild is cheap only while we keep the f32 source; without it, a format break means embedding everything again.
- **No checksums on codes.** By its own docs, a flipped code bit loads cleanly and changes results.
- **Supply chain.** MIT, pure Rust, 17.4k stars, already in `Cargo.lock` at 1.1.2 as a dev-dependency for the bench. It meets the rule.

## LEANN

Not measured. LEANN keeps a pruned proximity graph, stores no vectors, and recomputes embeddings for the nodes it visits during each search. Its backends are Python with C++ (boost, protobuf, ZeroMQ) and there is no Rust path. Since 2026-10-16 PathKeep bundles no inference engine, so recomputing means sending the visited pages to the user's embeddings service: hundreds of requests per search, seconds at best, and nothing when the service is down. It also targets recall around 0.9 and has no filter that would satisfy the coverage rule. Its disk saving targets the f32 vectors, which we can drop without it.

## What this means

Two changes keep every current decision and should go first (both done 2026-10-17, see the next section):

1. **Rank without per-visit SQL.** Keep a position → page mapping next to the planes, apply the search's filters as an eligibility mask before selecting, keep scores in a dense array indexed by position, and hydrate only the rows the current page shows. Every vector is still scored and nothing is hidden; only when the work happens changes.
2. **Scan the int8 plane as one mapped, contiguous block** with a SIMD dot product, and stop reading the sign-bit plane and counting `visits` on every search. That is the 90× above.

Then one question is the user's, because it touches the coverage rule. At large sizes complete int8 ranking does not stay in 8 GB next to the app and the browser: 3.74 GB at 14.4M × 256, 7.40 GB at 7.2M × 1,024. Mapped from disk, the OS evicts it and every search reads the file again: over a second at NVMe speeds. The options:

| Option                                                                                                 | Every page ranked?                                                  | Memory at 7.2M (256 / 1,024)                         | Needs                                                                         |
| ------------------------------------------------------------------------------------------------------ | ------------------------------------------------------------------- | ---------------------------------------------------- | ----------------------------------------------------------------------------- |
| A. Complete int8 pass, mapped                                                                          | yes, near-exact order                                               | 1.87 / 7.40 GB, evictable                            | nothing new; slow once it no longer stays in memory                           |
| B. A compact code scores every vector; int8 rescores the head, and further chunks as the user pages on | yes; beyond the rescored head the order comes from the compact code | 0.77 / 0.92 GB resident (Turbovec 2-bit / sign bits) | user agreement that order beyond the head may be approximate; a trade-off doc |
| C. Shortlist of k, as in June                                                                          | no                                                                  | as B                                                 | reverses the 2026-10-07 decision; trade-off doc and user agreement            |

For B, the measurements at 131k pages point to Turbovec 2-bit codes at low dimensions (sign bits lose too much there) and sign bits at 1,024 and above (as good, and 2.6× smaller); how large the rescored head must be at millions of pages is the open measurement. Turbovec's top-k API makes B awkward: each chunk is one more search with a larger k, which is cheap (12 ms at 14.4M here) but is not a dense score array. Our own sign bits give one directly.

The f32 source on disk (four fifths of the index) is a separate question: dropping it saves 29 GB at 7.2M × 1,024, but every format change or code change then means embedding again through the user's service.

## After the pipeline change (2026-10-17)

`WORK-SEMANTIC-PIPELINE` did both changes above, a little differently in the details:

- The int8 plane is mapped and every record is scored in one pass over up to four threads, with the query kept in f32. Search no longer reads the sign-bit plane.
- A new sidecar `.pkpos` holds each visit's position in the int8 plane. It is written at the end of an index pass, after the reverse sidecars, and carries the projection ids it was built from; when it is missing or from another projection, search builds the same array in memory.
- One statement streams every visit in the search's scope. It fills the session's page membership (newest and first visit, visit counts) and gives each page the best score of its in-scope visits. It replaces both the per-visit hydration and the per-page newest-time lookups. When the scope tests nothing but the visit, it joins only `urls`.
- Stars come from one query, fused scores are arrays indexed by page id, and the staleness check reads the archive totals instead of counting `visits`.
- During an index pass, search keeps the previous complete projection; the new one takes over when the pass projects it at the end.

Same benchmark, same machine, medians in milliseconds:

| Stage                             |  264,388 visits | 1,000,000 visits |
| --------------------------------- | --------------: | ---------------: |
| Keyword ranks and session         |              22 |               82 |
| Vector scores and visit positions |               2 |                6 |
| Scope pass                        |              99 |              385 |
| Rank, fuse, stars                 |               4 |               15 |
| Install and first page            |               3 |               10 |
| **Total** (before)                | **130** (1,145) |  **500** (5,200) |

Every probe's first page and total match the public entry point. The scope pass is now nearly the whole cost, and it grows with the visits: about 5.5 s at 14.4M on this Mac, more on the target. Getting below that means not reading the visits at all for the common scopes; that is `WORK-SEMANTIC-VISIT-SIDECAR` in the backlog.

## Reproduce

```sh
# Stages (writes a Vault into a temp dir; ~5 min at 1M visits)
PATHKEEP_SEMANTIC_STAGE_BENCH=1 PATHKEEP_SEMANTIC_VISITS=1000000 PATHKEEP_SEMANTIC_OUT=stage.jsonl \
  cargo test --manifest-path src-tauri/Cargo.toml -p vault-core --lib --release semantic_stage_benchmark -- --ignored --nocapture

# Scans on synthetic vectors (temp files under PATHKEEP_SCAN_TMP; keeps 10 GB free)
PATHKEEP_SCAN_BENCH=1 PATHKEEP_SCAN_SCALES=1000000,4000000 PATHKEEP_SCAN_DIMS=256 PATHKEEP_SCAN_THREADS=1,4 \
  PATHKEEP_SCAN_QUERIES=100 PATHKEEP_SCAN_OUT=scan.jsonl \
  cargo test --manifest-path src-tauri/Cargo.toml -p vault-core --test vector_scan_bench --release -- --nocapture

# Scans on real vectors: a folder with meta.json, vectors.f32, queries.f32, groups.u32
PATHKEEP_SCAN_BENCH=1 PATHKEEP_SCAN_REAL=<folder> PATHKEEP_SCAN_QUERIES=500 PATHKEEP_SCAN_OUT=scan.jsonl ...
```

The Python scripts behind the two-stage rows and the whole-scan Turbovec check read real browsing data and were not kept.

## Not covered

- The 4-core x86 target. Every number here is from the M5 Max; a GitHub Actions x86 runner could repeat the synthetic scans, with the user's agreement since it spends their minutes.
- Search-time memory of the full pipeline at 4M visits and above.
- Recall at 14.4M: there is no real corpus that large, and random vectors cannot stand in.
