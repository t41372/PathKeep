# Related pages in History (2026-10-17)

What the "Related pages" section in History's detail panel shows, and why. Measured first on the user's real history (six months, about 60,000 pages, Qwen3-Embedding-0.6B and the in-app static model), on this machine; only aggregates are written here.

## On real history

- Without folding, the nearest pages of a page are mostly the same page again: about 30 % have the same title at another address (forum page numbers and anchors, a board name appended to the title, unread counts such as "Inbox (28)").
- Similarity cannot tell duplicates apart: with the static model, different videos of one series score 0.94–0.95. So duplicates are folded by title.
- After folding, 77 % (Qwen3) and 87 % (static) of the six nearest are on the page's own site. The pages on other sites are often the useful ones (the repository behind a forum thread, the competing apps), so the list keeps room for them.
- The 98th percentile of random-pair similarity was 0.63 (Qwen3) and 0.71 (static). Three in four pages have at least three neighbours above it; below it, neighbours share a broad area rather than a subject.

## Rules

- Fold key: registrable domain plus a headline. The headline is the lower-cased title cut at the first separator (a hyphen, bar, en or em dash with a space on each side, or an underscore) when at least six characters come before it, then without bracketed counts and whitespace. Known cost: `download_manga.py` becomes `download`.
- The page itself (its URL and its fold key) is left out; each fold key appears once, with its best score and its newest visible visit.
- Score at least the 98th percentile of 20,000 random pairs of the plane (fixed seed), sampled once per published projection.
- Six by score, at most three from the page's own site while other sites have candidates above the cut-off; ties go to the newer page.

## Read path

`ai/related_pages.rs` uses the serving index under the same checks as Ask's topics. The source vector comes from one `u32` read in `.pkpos` after its header is checked against the current planes and forward sidecar; a missing or mismatched file answers "updating" instead of rebuilding the array on a click. A selected visit without a vector falls back to the newest indexed visit of the same URL. The whole plane is scored; candidates are read in three batches at most (256, 512, 1,024 positions), because most pages' neighbours are all on their own site and reading further would not change that. Each candidate content key needs one visit from the reverse sidecar to find its page, and the page is shown by its newest visible visit, one indexed seek per URL. No embedding request, no search scope stream.

## Timings

`ai::related_pages_bench::related_pages_measurement` (opt-in, release; `PATHKEEP_RELATED_BENCH=1`, `PATHKEEP_RELATED_DIM`, `PATHKEEP_RELATED_OUT`): 1,000,000 indexed visits, 500,000 vectors, warm cache, this Mac, five pages. Milliseconds; the first read of a projection also samples the cut-off.

| Stage                 | 256 dims | 1,024 dims |
| --------------------- | -------: | ---------: |
| Cut-off (first read)  |       13 |         32 |
| Scan                  |      5–7 |      16–22 |
| Reverse lookups       |        3 |          3 |
| Newest visible visits |      3–4 |        3–4 |
| Total, later reads    |    13–14 |      28–30 |
| Total, first read     |       30 |         61 |

The scan grows with the index; the rest follows the candidates. A 14.4M × 1,024 plane does not fit the 8 GB target, the same limit as semantic search (`WORK-VECTOR-MEMORY-AT-SCALE` in BACKLOG).

## Tests

Two Rust tests next to the module cover folding, selection and ties, the cut-off on one and on identical vectors, and point lookups against missing, torn and replaced projections. `semantic-index.spec.ts` drives the panel: two subjects on three sites, a duplicate title, an undone import still in the index, a page imported after the pass, moving between related pages without changing the list, and removing the embedding service.
