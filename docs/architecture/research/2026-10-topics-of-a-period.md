# Topics of a period (2026-10-17)

Input for `WORK-EMBED-USES` item 1: Ask's "總結一下這週" can count a week (`intelligence_report`) and find pages by words (`search_history`), but it cannot say what the week was about. This measures how to group a period's pages by meaning with the vectors the semantic index already has.

## Data and measure

The sample Takeout: 131,322 distinct pages (address and title), 254,988 visits, embedded as PathKeep sends them with the in-app static model (potion-multilingual-128M, 256 dims) and with Qwen3-Embedding-0.6B (1,024 dims, through sentence-transformers on this Mac; 373 s). Nothing was printed but counts and rates; the vectors were deleted afterwards.

There is no ground truth for "topics" in real history, so the measure uses weak labels:

- **Together**: pairs of pages the user opened within three minutes after the same Google search, before the next one (4,346 searches with two or more such pages). They are mostly about one thing; the share of these pairs a method puts in one topic should be high. It cannot reach 1: one search's results are not always one subject.
- **Apart**: random pairs of the window's pages. Most are unrelated; the share a method puts together should be low.
- **Usefulness for a summary**: how many topics hold half the window's visits, and how many pages are in topics of three or more.

Windows (results are means across them): the 8 weeks and 4 months (30 days) with the most labelled pairs, and the busiest year (the whole sample).

## Methods

Week windows (about 3,500 pages), static model:

| Method                              | Together | Apart (random) | Topics for half the visits |   Time |
| ----------------------------------- | -------: | -------------: | -------------------------: | -----: |
| Average-link clustering, cosine 0.6 |    0.397 |          0.042 |                         12 |  0.9 s |
| Leader clustering, 0.5              |    0.427 |          0.048 |                         10 | 0.15 s |
| Leader clustering, 0.6              |    0.356 |          0.029 |                         16 | 0.17 s |
| HDBSCAN, min. size 3                |    0.251 |          0.004 |                         86 |  1.3 s |

- **Leader clustering**: pages in order of visits; each joins the most similar topic centre if it is at least the threshold, otherwise starts a topic; then three passes move every page to its nearest centre (or alone, below the threshold) and recompute the centres, weighted by visits.
- Leader and average-link lie on the same trade-off curve between together and apart: the limit is the vectors, not the method. Average-link needs all pairwise distances (out of reach above about 20,000 pages); leader is linear in pages times topics and simple to write in Rust.
- HDBSCAN leaves most pages in tiny groups: precise, useless for a summary.

## The threshold

Similarity means different things per model. Random pairs of a week's pages:

|        | median | 90th | 95th | 99th percentile | Labelled pairs, median |
| ------ | -----: | ---: | ---: | --------------: | ---------------------: |
| Static |   0.19 | 0.47 | 0.60 |            0.78 |                   0.52 |
| Qwen3  |   0.37 | 0.55 | 0.61 |            0.75 |                   0.61 |

The static model's random pairs spread wide because the embedded text includes the site and address words: pages of one site are alike whatever they are about. A threshold taken from the window itself, the 95th percentile of similarity between random pairs of its pages (from a separate sample of 20,000 pairs), gives 0.58–0.61 for both models here and matches the best fixed value for each:

| Leader at the window's 95th percentile | Together | Apart | Topics for half the visits | Pages in topics of 3+ |
| -------------------------------------- | -------: | ----: | -------------------------: | --------------------: |
| Static, week                           |    0.363 | 0.024 |                         17 |                  0.90 |
| Qwen3, week                            |    0.389 | 0.014 |                         18 |                  0.91 |
| Static, month (about 12,000 pages)     |    0.340 | 0.017 |                         32 |                  0.93 |
| Qwen3, month                           |    0.366 | 0.005 |                         41 |                  0.93 |

Qwen3 keeps unrelated pages apart two to three times better at the same granularity. With the static model a site that covers many subjects (a video site, a forum) tends to become one topic.

## Long windows

One leader pass over the whole year (131,322 pages) took 69 s (static) and 121 s (Qwen3) in numpy, with 94 and 143 topics for half the visits. Clustering each 30 days and merging their topic centres was faster but merged unrelated pages: monthly topics of one broad site join across months into one topic holding about a fifth of the year (random pairs together 3.3–4.4 % for the static model whatever merge threshold, from τ to τ + 0.2). So a topic list covers at most 31 days per call; a year is asked month by month.

## Decision

- Ask gets `intelligence_report` `report="topics"` for a window of at most 31 days, computed when asked from the serving semantic index.
- Leader clustering, three refinement passes, threshold the window's 95th percentile of random-pair similarity, deterministic order and seed.
- Every page visited in the window is in exactly one topic. Topics are listed by visits; each has its most central page's title as the label, its visits, pages, first and last visit, sites and its largest pages. A page without a vector yet is counted, not hidden.

## Not covered

- Quality beyond the weak labels. The fixture's six labelled topics check the grouping in E2E; the user judges real topics in Ask.
- The 4-core target and the 14.4M worst case. The Rust measurement below covers a synthetic 60,000-page month on this Mac, not the target machine.

## In the app (2026-10-17)

`ai/topics.rs` does the above when Ask calls `intelligence_report` with `report="topics"`:

- One read of the window's visible visits. A page's vector is the int8 record of its newest indexed visit in the serving index; a page without one, or with a zero vector, is counted in `notIndexed`.
- The threshold comes from 20,000 sampled pairs with a fixed seed, and ties are broken by visits and page id, so the same window gives the same topics.
- Titles are looked up only for labels and example pages; `topic="t3"` lists every page of one topic.

Clustering 60,000 synthetic pages in a release build on this Mac takes 0.39 s on one worker and 0.19 s on four at 256 dimensions, 1.54 s and 0.73 s at 1,024 (`ai::topics::tests::month_clustering_measurement`, `PATHKEEP_TOPICS_BENCH=1`). Ask uses one worker. Synthetic vectors say nothing about quality.
