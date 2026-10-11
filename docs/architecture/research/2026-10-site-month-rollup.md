# Custom Insights ranges on a dense archive: add a site-month rollup or not

Status: for the user to decide. This is step ⑤ (d) of `WORK-INSIGHTS-RESTORE-2026-10`. Nothing
here is built.

## Where it stands

The target the user set on 2026-10-08: at 14.4M visits, the first batch of a one-year range is no
slower than the 30-day batch (3.3 s), on both benchmark shapes.

Preset ranges meet it. All time, 1 year, 90, 30 and 7 days ending today, over every profile,
are snapshots. They are built after each Insights build, and warmed again when the app opens
on a new day (⑤ (c)). On the 14.4M benchmark, a preset year's first batch takes 5.7 ms.

What is left is a range read live: custom start and end dates, or any range filtered to one
profile. Measured on 14.4M visits (Modal, 4 vCPU, 8 GiB):

| archive                                            | site-day rows a day | custom year, first batch | read again |
| -------------------------------------------------- | ------------------- | ------------------------ | ---------- |
| `cycle` benchmark: every visit a different site    | 8,640               | 39.9 s                   | 1.6 s      |
| `habits` benchmark: 50 daily sites, 400 that churn | about 450           | 1.74 s                   | 0.24 s     |
| copy of real data (374,889 visits, 527 days)       | 39 (p99 99)         | 67 ms                    |            |

The 39.9 s is two reads of about 20 s each. The KPI row counts the range's sites and the sites
of the year before it, so it can compare them. Each count walks the shared site-day rows for one
year: 3.15M rows in the `cycle` shape. Grouping by site in SQL instead of in Rust (⑤ (b)) took
18.7 s against 19.9 s. The time goes into walking the rows, not into carrying them to Rust.

How likely the `cycle` shape is: it has about 8,600 different sites every day, out of 20,000 in
all, each one visited every two or three days. The real copy averages 39 sites a day and
reaches 119 at most. That is one site-day row per 18 visits; in the `cycle` shape it is one per
visit. At the real density, 14.4M visits would make about 790,000 site-day rows over the whole
history, a quarter of what the `cycle` shape holds for a single year.

## Options

**A. Keep the site-day read as it is.** How long a live range takes grows with the distinct
(site, day) pairs in it. Real browsing and the `habits` shape are well inside the target. Only
an archive that sees thousands of different sites every day is slow, and only the first time:
the read is kept until the Insights data changes (1.6 s to read it again). Meanwhile the read runs
off the UI thread and the cards show their skeletons, so the window stays responsive. Nothing
changes in the data model.

**B. Add a site-month rollup.** A table with one row per site, profile and month. It holds
visits, days visited, first and last day, and the sum of squared gaps between visit days. It is
written alongside the daily site rollup.

- **How a range reads it.** Whole months come from the monthly rows. The part-months at the two
  ends come from the daily rows, as now.
- **Habits stay exact.** A site's days merge month by month: counts add, first and last carry
  over, and the gap across a month boundary is added where two months join. That is the same
  running total `site_days::VisitDays` keeps now.
- **What it saves.** In the `cycle` shape a year is 12 × 20,000 = 240,000 monthly rows instead of
  3.15M daily ones, so the two reads should take about 3 s. That is a row-count estimate; it
  would be measured. In the `habits` shape the churning sites rarely come back within a month,
  so a month has about 12,000 rows against 13,500 site-days: almost no gain. On real data there
  is nothing to gain.
- **What it costs.**
  - Every build that writes the daily site rollup has to write the matching months in the same
    transaction: a full build, an append (`build_append.rs`), and a reclassify
    (`build_reclassify.rs`), which redoes only the days whose categories changed.
  - An equivalence test has to hold for each of those paths: any range read through months gives
    the same cards as the same range read through days.
  - `data-model.md` and `store-schema.md` (accepted) gain a rollup table.
  - The Turso store format is not released yet, so no migration is needed.

**Not enough on its own: compare to the previous period later.** Moving the previous-period site
count to the second batch halves the first batch in the `cycle` shape, to about 20 s. That is
still far over the target, and the second batch becomes 20 s instead.

## Recommendation

A. The one archive that misses the target has thousands of different sites a day, and no real
browsing in the measurements comes close. B would add a third copy of every site's numbers. Every
write path would have to keep it exact, and a copy that drifts shows wrong numbers, which is
worse than a slow first read. If a real archive ever shows a custom year over 3.3 s, B is the
fix, and `PATHKEEP_BENCH_SHAPE=cycle` measures the gain.

The target was set for both shapes, though, and only B meets it for `cycle`. If the target
stands as set, the choice is B.
