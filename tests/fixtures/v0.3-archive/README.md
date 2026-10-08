# A data folder made by PathKeep v0.3.0

`upgrade.spec.ts` updates this folder the way an installed v0.3.0 would be updated. Everything in it was written by v0.3.0's own app, not by code in this tree.

| File                     | What it is                                                                                                                                                                                    |
| ------------------------ | --------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `data.tar.gz`            | The data folder after v0.3.0 exited: encrypted SQLCipher archive, source evidence, Insights, agent and search databases, raw snapshots, audit files, saved page text, config, App Lock files. |
| `bundle.zip`             | v0.3.0's "move to another computer" file (format 1) of the same data, made by its `export_app_data`.                                                                                          |
| `expected.json`          | What v0.3.0 reported about it: visit totals, per-page visit counts, stars, note, tags, the saved page text records, the import batch, the chat. The upgrade test compares against these.      |
| `make-archive.config.ts` | Playwright config that seeds the browser data and starts v0.3.0's app with its dev bridge.                                                                                                    |
| `make-archive.spec.ts`   | The steps, all through v0.3.0's dev bridge commands.                                                                                                                                          |

## What v0.3.0 did

1. Created an encrypted archive (password in `expected.json`), remembered the password in its (test) keychain and turned page fetching on.
2. Backed up two Chrome profiles, Personal (1,407 visits over a year) and Work (401). They include two spellings of one address (`%7Equokka?utm_source=old` and `~quokka`), Google searches, and a page with a Chinese path and title.
3. Imported a third Chrome `History` file by hand ("Old laptop", 120 visits).
4. Starred a page and a domain, added a note and two tags.
5. Fetched the text of `https://example.com/` and `https://www.iana.org/help/example-domains` over the internet; v0.3.0 only fetches public https pages.
6. Saved a chat, built Insights, exported the bundle, and set an App Lock passcode.

Debug build with the `devtools-bridge` feature (the dev bridge exists only there). v0.3.0's core library has no debug-only code paths, so it writes the same files as the release.

## Making it again

The generator runs only in a v0.3.0 checkout. It builds and runs v0.3.0 on Modal:

```sh
git worktree add --detach ../pathkeep-v030 v0.3.0
cd ../pathkeep-v030
cp ../<this repo>/tests/fixtures/v0.3-archive/make-archive.config.ts playwright.v03-archive.config.ts
cp ../<this repo>/tests/fixtures/v0.3-archive/make-archive.spec.ts tests/e2e/v03-archive.spec.ts
# The current .modal-offload.toml, minus the worker-sidecar step v0.3.0 doesn't have:
sed 's/ && bun run prepare:worker-sidecar --profile debug//' ../<this repo>/.modal-offload.toml > .modal-offload.toml
~/.agents/skills/modal-offload/offload run --key v030 --timeout-min 90 -- '
  node scripts/run-playwright.mjs --config playwright.v03-archive.config.ts &&
  cp -a "$(node -p "require(\"./artifacts/e2e/v03-archive/made.json\").projectRoot")" artifacts/e2e/v03-archive/data'
```

The data folder is copied after the app has exited, so no database is mid-write. `artifacts/e2e/v03-archive/` then holds `data/`, `v03-bundle.zip`, `made.json` and every bridge reply under `replies/`; `expected.json` is built from those replies. Visit times are relative to the day it runs, so a new folder has new dates; the counts and texts stay the same. Remove the worktree afterwards.
