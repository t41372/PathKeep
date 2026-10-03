# E2E run artifacts

Every run of `bun run test:e2e` writes its output here. Nothing in this
folder is committed.

| Path            | What it is                                                                                         |
| --------------- | -------------------------------------------------------------------------------------------------- |
| `fixture.json`  | The synthetic browser data the run used: seed, `now`, and every visit per profile.                 |
| `report/`       | HTML report. Each test attaches the numbers it checked, as `{ expected, actual }`.                 |
| `results.json`  | Machine-readable results for every test.                                                           |
| `test-results/` | Traces, screenshots and (on failure) videos for each test.                                         |

Open the report and its traces with:

```bash
bunx playwright show-report artifacts/e2e/report
bunx playwright show-trace artifacts/e2e/test-results/<test>/trace.zip
```

To repeat a run with the same data, take `now` from its `fixture.json`:

```bash
PATHKEEP_E2E_NOW=<now> bun run test:e2e
```

Playwright clears `test-results/` and `report/` at the start of the next run,
so copy a run out of here if you need to keep it. CI uploads this folder for
every run, pass or fail.
