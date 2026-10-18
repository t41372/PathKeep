# Searches grouped by meaning (2026-10-17)

Can a period's searches, grouped by meaning, name what the user was looking into, where page topics could not ([2026-10-topics-of-a-period.md](2026-10-topics-of-a-period.md))? Measured on the user's Takeout, six months, on this machine; the user allowed reading the results. Only aggregates and descriptions are written here.

## Data

- Searches are recognised by the built-in engine rules (`site_dictionary/search_rules.rs`). Searches that look like an address are left out, as the keyword cards do.
- Trails are built the way the structural build does (`intelligence_structural_stream.rs`): a new search within 15 minutes that shares words with the last one rewords the trail; a page joins when it is on a site the trail already reached or its title shares a word with the trail's searches; three unrelated visits in a row or 15 minutes end it.
- Takeout records no `from_visit`, so a result opened straight from the results page joins only through a shared word or site. The pages counted here are a lower bound; a Chrome import links them.
- 4,107 search visits, 2,519 distinct searches. Per month: 290–560 distinct keyword searches in 340–690 trails. Searches and the pages their trails reached are 10–17 % of the month's visits.

## Methods

Per month, Qwen3-Embedding-0.6B (through LM Studio) and the in-app static model:

1. Leader clustering on the search text alone, each search visit counted. The largest groups were short one-word searches (names, abbreviations) that sit close together, and a results page reloaded many times counted as many searches.
2. Each search counted once per trail; average linkage, threshold the month's 98th percentile of random-pair similarity.
3. As 2, with three vectors for a search: its text alone; its text embedded together with the titles of the three pages its trails opened most; its vector plus the mean vector of those pages.

The existing query families merge little: with Jaccard ≥ 0.5 or containment, ignoring their 24-hour and same-engine limits, 291–561 searches become 236–414 families a month.

## Findings

- Qwen3 with the opened pages (3, either form): 147–290 groups a month. In 30–69 of them, two or more lexical families join: typos, other wordings, other languages, sibling course numbers, competing products.
- Of each month's ten largest groups that reached pages, about seven are one clear thing looked into over several days: a purchase compared across days, models of one piece of hobby equipment, a set of university courses, a tool being set up, a phone plan being switched. The name is the user's own most frequent wording.
- Failures:
  - Each month's largest group is often a broad area (dozens of AI tools searched once each). Naming it by its most frequent search misleads.
  - Searches with no opened page, and very short ones (one to three characters, a lone name, a typo), gather into groups of unrelated things.
  - Searching the same text alone (1) merges by name more than by subject.
- The summed vector does as well as embedding the text with titles. It needs only the search embedded once with the serving model, because page vectors are already in the semantic index, and it follows new trails without embedding anything again.
- The static model groups by shared words, close to what the lexical families do.

## What it would take (not decided)

- A vector per distinct keyword search, embedded with the serving model during index passes, and the vectors of its trails' pages from the page index.
- A group is shown only when it reached pages or recurs on several days; very short searches stay out; a broad group shows its top wordings and size rather than one wording.
- Candidate uses: an Ask report of what a period's searches were about; "similar searches" on the search drill-in; an Insights card of things looked into (overlaps today's search cards, so it needs the user's decision).
- Check again on a Chrome import with `from_visit` before fixing thresholds.

## In the app

Ask exposes `intelligence_report` with `report="searches"`, local dates covering at most 31 days, and `group="s3"` for every search and opened page in a group. The search drill-in lists up to eight similar searches across all time. Neither read calls a model; both explain the static model's reliance on shared words.

Each index pass fills missing distinct keyword searches after pages, sending exactly `normalized_query`. Separate files reuse the vector store and int8 formats, with the serving model's fingerprint and no visit maps or schema change. Current-generation events also join visible archive visits: an undone import must disappear before Insights finishes rebuilding. Settings explains that saved searches are sent; result pages already reached the service as page documents.

Grouping uses average linkage, the fixed 20,000-pair 98th percentile and a 4,096-search cap (64 MiB matrix). Short, unindexed and excess searches are accounted for. The drill-in uses search vectors alone and its own 99th-percentile cut. At millions of distinct searches its memory limit is the same as page search: BACKLOG `WORK-VECTOR-MEMORY-AT-SCALE`.

Release measurements on this M5 Max, 2026-10-17, warm OS cache, three runs, in milliseconds:

| Read                                      | 256 dimensions | 1,024 dimensions |
| ----------------------------------------- | -------------: | ---------------: |
| 4,096-search average linkage              |            224 |              683 |
| Similar searches, 1M searches, first read |            895 |            1,020 |
| Similar searches, 1M searches, later      |             50 |               75 |

Linkage is synthetic normalised directions on one worker, without reading trails or combining page vectors. The drill-in read uses real Archive and Insights databases and mapped int8 files, without inference. Its first read goes through every current search event once to learn which searches the Insights generation still serves and how often each was searched; that list, the eligible plane positions and the cut-off are kept in memory until the Insights generation, the archive's last backup, import, undo or restore run, or the search plane changes. Before that cache every read took 1.8 s. Turso did not plan a correlated subquery over a JSON list as index seeks (0.9 s for the eight shown spellings), so the drill-in looks up each shown wording with its own seek, and the Ask report reads trail pages with one primary-key seek per trail and a group's searches with one index range per wording. These are not target-machine measurements.

```sh
CARGO_TARGET_DIR=src-tauri/target PATHKEEP_SEARCH_GROUPS_BENCH=1 \
  PATHKEEP_SEARCH_GROUPS_OUT=/tmp/search-groups-timings.jsonl \
  cargo test --manifest-path src-tauri/Cargo.toml -p vault-core --release --lib \
  ai::search_groups_bench::search_groups_measurement -- --ignored --nocapture
```

Rust tests cover numeric edges, linkage against a direct implementation, the cap and shared trails. The new E2E scenario requires opened-page context to reunite unrelated wordings, checks report accounting and the drill-in, and checks undo with Insights paused. It writes reports, rows and a screenshot when run.
