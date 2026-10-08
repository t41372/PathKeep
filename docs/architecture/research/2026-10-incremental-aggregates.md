# Insights aggregates after an append: rebuild or update

Status: proposal, waiting for the user's decision (2026-10-08). It would change a recorded
decision: `store-schema.md` and `data-model.md` say an append build extends the
fact/daily/navigation groups and _replaces_ the aggregates.

## The problem

Measured on the 14.4M-visit benchmark (`ipc-performance.md` §11, Modal VM with 4 vCPU and
8 GiB), appending 10,000 visits:

| stage                                                      | time   |
| ---------------------------------------------------------- | ------ |
| visit-derived facts                                        | 6 s    |
| daily rollups                                              | 3.5 s  |
| sessions and search trails                                 | 2.6 s  |
| aggregates: refind pages, then habits, paths, compare sets | 43 min |
| all-time snapshot warmed again                             | 14 min |
| **total**, peak resident memory 3.1 GB                     | 57 min |

Every backup pays this in the background. At 387k visits it is about two minutes, which is
why nobody noticed. The target is 14.4M visits on a 4-core, 8 GB machine.

The aggregate stage walks every visit twice per append: once by page for refind pages, once
by time for habits, common paths and compare sets. It then reads every keyword search for
query families. It writes a new aggregate generation, and readers switch to it when it is
published. Memory grows with history: every site's set of visit days for habits (the
all-time habits read alone took 1.85 GB) and every path seen.

The snapshot warm is a separate cost. It is addressed by the shared site-day read (step ③),
which reads a range's site-day rows once instead of once per card. Its effect on the warm will
be measured before this decision.

## Options

**A. Keep rebuilding, but walk less.** Merge the two walks into one, and skip compare sets for
trails no new visit touched. Still a walk over every visit per append. At 14.4M that is at
least the cost of one walk (about 20 minutes on the benchmark), and memory still grows with
history.

**B. Update only what the new visits touch.** An append knows its new visits. Each aggregate
is keyed, and each key can be recomputed on its own:

- **Refind pages**, by page: recompute the pages the new visits opened, from that page's visits
  through the canonical index the walk already uses.
- **Habits**, by site: recompute the sites the new visits went to, from the site's daily rollups
  through `idx_domain_daily_site`.
- **Common paths**, by path: the session path segments this build saved (`session_paths`) hold
  every new path. Add the paths that end in new sites. For the segment the last build saved
  open, remove what it counted before and add it again.
- **Compare sets**, by search trail: recompute the trails the new visits joined.
- **Query families**: they are built online, each search in visit order joining the first
  family it matches. Continuing from the stored families with only the new searches gives the
  same families a full replay gives. Reopened investigations follow from the changed families.

The rows change in place, in the current aggregate generation, one batch of keys per write.
That is what an append already does to facts, daily rollups and sessions: a reader during the
build sees some keys updated and others not yet. A full rebuild (late visits, a changed rule,
the Settings button) still builds a new generation from scratch, exactly as now.

Cost grows with the visits appended and the history of the pages, sites and trails they touch,
not with the archive. Memory holds the touched keys only.

**C. No all-time tables: compute at read time.** Read all-time aggregates from per-day and
per-segment rows when asked. That moves the cost to every all-time read and every snapshot warm,
which is worse for the user than a background build.

## Recommendation

B. It is the only option whose cost does not grow with the archive, and its risk is checkable:
for any archive, an append build must give the same aggregate rows as a full rebuild of the same
visits.

How it would be verified:

- An equivalence test for every aggregate: build a fixture in batches with appends (sessions,
  trails and paths cut by batch boundaries, a page and a site visited before and after the
  append, a query joining an existing family), then compare every aggregate table with a full
  rebuild of the same archive.
- The 14.4M benchmark's append of 10,000 visits, time and peak memory, before and after.

What it costs in code: each aggregate gets a "recompute these keys" path next to its full
build, and both share the per-key logic, so the two cannot drift apart.
