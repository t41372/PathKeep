# E2E run artifacts

Every Playwright run writes its output to `artifacts/e2e/<config>/`, where
`<config>` is `desktop-bridge` (the gating suite) or `preview` (browser-only
preview). Nothing in these folders is committed.

| Path           | What it is                                          |
| -------------- | --------------------------------------------------- |
| `report/`      | HTML report. Open `report/index.html`.              |
| `results.json` | Machine-readable results for every test.            |
| `test-results/`| Traces, screenshots, and videos for each test.      |

Open the report and its traces with:

```bash
bunx playwright show-report artifacts/e2e/desktop-bridge/report
bunx playwright show-trace artifacts/e2e/desktop-bridge/test-results/<test>/trace.zip
```

Playwright wipes `test-results/` and `report/` at the start of the next run
of the same config, so copy a run out of here if you need to keep it. CI
uploads this folder for every run, pass or fail.
