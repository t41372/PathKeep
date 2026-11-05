# Typo fallback benchmark — 2026-11-05

A search with no exact match retries with typos allowed. PR #16 changed how that retry reads a
multi-word query, and most queries now walk every page, so this compares it with `main` at full
size: 14,400,000 visits and 2,900,000 distinct pages, encrypted, through the whole owner path
(`search::benchmark::encrypted_archive_search`).

## How it was measured

One Modal microVM, 4 vCPU and 8 GB, release build, Linux. The base (`dc42cd7a`, plus the benchmark
cases) and the head of the PR each seeded their own archive and ran every case, alternating:
head, base, head, base. Times below are the median of the warm runs, averaged over both rounds.

Keep both sides on one machine. Two separate sandboxes of the same size differed by about 30% on
work neither side changed (seeding, rebuild, backup), which looked like a 30% regression on exact
searches until the runs were repeated here.

## Results

| Search                                    | Base (s) | Head (s) | Head / base | Base finds | Head finds |
| ----------------------------------------- | -------: | -------: | ----------: | ---------: | ---------: |
| `archive`, exact                          |     16.7 |     16.9 |        1.02 |  2,900,000 |  2,900,000 |
| `webassembly`, exact                      |     0.30 |     0.30 |        1.01 |      9,148 |      9,148 |
| `archive`, 2% date range                  |     0.52 |     0.54 |        1.04 |     57,600 |     57,600 |
| `archive`, one profile                    |      8.2 |      8.2 |        1.00 |    966,666 |    966,666 |
| `raresentinl`, one word misspelled        |    143.3 |     92.4 |        0.65 |          1 |          1 |
| `archive raresentinl`, two words          |    148.6 |    139.3 |        0.94 |      **0** |          1 |
| `quartz lantern`, spelled right, no match |    140.3 |    116.1 |        0.83 |          0 |          0 |

Both sides read all 2.9M documents for the last three. For these searches the time is the whole
walk; CPU time equals it, so a walk keeps one core busy from start to finish.

How soon the first hit of `raresentinl` appears depends on where its page sits in the index
segments, which differs between seeds (19 to 44 s across all four runs), so it says nothing about
either side.

What a walk does to everything else, head then base:

| While a walk runs                                  |         Head |         Base |
| -------------------------------------------------- | -----------: | -----------: |
| Cancel, next keystroke (12 trials each, all walks) | 0.4 to 26 ms | 1.0 to 35 ms |
| Small write, median                                |      0.58 ms | 0.52–0.67 ms |
| Small write, p95                                   | 0.73–0.76 ms | 0.82–1.43 ms |
| Small write, slowest of 100                        |    1.4–77 ms |     7–107 ms |
| Longest step of a search                           |       101 ms |       121 ms |
| Resident memory during the walk                    |    ≤ 165 MiB |    ≤ 161 MiB |

With nothing running, the same write takes about 0.6 ms at the median and about 1 ms at worst.

## What this does not show

- The pages are synthetic, and each holds `articles` and `archive`, so the old whole-run trigram
  filter matched every one. On real pages it could skip some; how many is not measured, so base
  may be faster there than it is here.
- A real page has more text than these, so a walk over a real archive of this size may take longer.
- The 5.8 GB archive fits in this VM's page cache, so the walks read almost nothing from storage.
  A cold archive adds that I/O, and one larger than memory adds it on every walk.
- This VM is sized like the target machine; it is not one.

The absolute cost stands either way: on an archive this size, a search with no exact match takes
one and a half to two and a half minutes to finish. `WORK-SEARCH-TYPO-WALK` tracks that.

[Raw results](2026-11-typo-walk-benchmark.jsonl), one line per run, with the side and round added.
