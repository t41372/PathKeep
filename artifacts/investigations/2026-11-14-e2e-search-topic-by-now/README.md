# `insights.spec.ts` fails at some fixture times

`a search made in several browsers is one row among the searches you came back to most` failed in the
gate on 2026-11-14 at 18:48 MST (2026-11-15 01:48 UTC), at line 716: the 台北天氣 family was not in
the Work profile's five most searched. The spec failed before it opened the app.

The E2E fixture is generated from `Date.now()` under `TZ=UTC` (`playwright.config.ts`,
`scripts/run-playwright.mjs`). Which visits a profile gets and on which day moves with `now`, and
so do the five families the spec reads.

`write-fixtures.mjs` writes the real fixture (`writeSyntheticBrowsers`, seed 7) every two hours
across two UTC days and keeps each one's visits; `taipei-check.mjs` runs the spec's check on each:

    TZ=UTC node write-fixtures.mjs <repo> <out>
    TZ=UTC bun taipei-check.mjs <repo> <out>

Output on 2026-11-14 (`now` in UTC; the family with its searches; the fifth row):

    Sat Nov 14 2026 00:30–10:30  pass  台北天氣:46        5th: tanstack virtual dynamic height:36
    Sat Nov 14 2026 12:30–18:30  pass  台北天氣:59        5th: half marathon training plan:38
    Sat Nov 14 2026 20:30        pass  台北天氣預報:47    5th: jlpt n3 grammar:35
    Sat Nov 14 2026 22:30        pass  台北天氣預報:39    5th: 台北天氣預報:39
    Sun Nov 15 2026 00:30–10:30  FAIL  台北天氣:38        5th: n3 listening practice:39
    Sun Nov 15 2026 01:48:43     FAIL  台北天氣:38        (the failed run's own `now`)
    Sun Nov 15 2026 12:30–18:30  pass  台北天氣預報:42    5th: jlpt n3 grammar:37
    Sun Nov 15 2026 20:30        pass  台北天氣:47        5th: n3 listening practice:37
    Sun Nov 15 2026 22:30        pass  台北天氣:44        5th: shadcn sidebar:33

The failed run's `fixture.json` gives the same 台北天氣:38, seventh. No code is involved: any
branch, main included, fails this spec with a fixture made in that window. Under a local time zone
instead of UTC the windows move, which is why a first attempt without `TZ=UTC` disagreed with the
run. BACKLOG `WORK-E2E-SEARCH-TOPIC-BY-NOW`.

## 2026-11-15: finer samples

A clean E2E run started at 10:35 UTC on 2026-11-15 failed the same way (台北天氣:38, not in the five).
The same scripts with `now` every ten minutes from 10:30 to 12:30 UTC that day, then every minute
from 11:25 to 12:00:

    10:30–11:00  FAIL  台北天氣:38        5th: n3 listening practice:39
    11:10        pass  台北天氣:42        5th: shadcn sidebar:35
    11:20        FAIL  台北天氣:38        5th: sqlite fts5 chinese tokenizer:38
    11:25–12:00  pass  (every minute)
    12:10–12:30  pass  台北天氣預報:42    5th: jlpt n3 grammar:37

So the times that fail are not one stretch: the result can turn within ten minutes. A run cannot
be timed around them reliably; the spec has to stop depending on `now`.
