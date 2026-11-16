# A data folder made by PathKeep v0.5.0

`upgrade-v05.spec.ts` opens this folder the way an installed v0.5.0 would be updated, restores the safety copy v0.5.0 made, and imports v0.5.0's move file. Everything in it was written by v0.5.0's own app, not by code in this tree.

| File                     | What it is                                                                                                                                                                                                                                                       |
| ------------------------ | ---------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `data.tar.gz`            | The data folder after v0.5.0 exited: the encrypted archive and source evidence, Insights, Ask and search data, the browsers' history files a backup copied, the safety copy from the switch to encryption, audit files, saved page text, config, App Lock files. |
| `move-file.pathkeep`     | v0.5.0's "move to another computer" file (format 2) of the same data, with its own password, made by its `export_app_data`. It carries the safety copy too, as every file v0.5.0 wrote did.                                                                      |
| `expected.json`          | What v0.5.0 reported about it: visit totals, per-page visit counts, stars, note, tags, the saved page text records, both imports, the chat, the safety copy. The upgrade test compares against these.                                                            |
| `make-archive.config.ts` | Playwright config that seeds the browser data and the Google Takeout folder, and starts v0.5.0's app with its dev bridge.                                                                                                                                        |
| `make-archive.spec.ts`   | The steps, all through v0.5.0's dev bridge commands.                                                                                                                                                                                                             |

## What v0.5.0 did

1. Created a plaintext archive, turned page fetching on and backed up two Chrome profiles, Personal (1,407 visits over a year) and Work (401). They include two spellings of one address (`%7Equokka?utm_source=old` and `~quokka`), Google searches, and a page with a Chinese path and title.
2. Imported a third Chrome `History` file by hand ("Old laptop", 120 visits) and a Google Takeout folder (24 visits to six `almanac.example.net` pages).
3. Switched the archive to encrypted (password in `expected.json`) and remembered the password in its (test) keychain. The switch left v0.5.0's safety copy of the plaintext archive in `raw-snapshots/maintenance/`.
4. Starred a page and a domain, added a note and two tags.
5. Fetched the text of `https://example.com/` and `https://www.iana.org/help/example-domains` over the internet.
6. Saved a chat, built Insights, exported the move file, and set an App Lock passcode.
7. Was given a move file this tree's build made (format 3): it refused it at the check, `unsupported bundle format 3`, before asking for a password, and its archive's visit total stayed the same (`replies/preview_app_data_import-later.json`, `replies/query_history-total-after-later.json`).

Debug build with the `devtools-bridge` feature (the dev bridge exists only there). v0.5.0's code that runs only in debug builds is for tests, benchmarks and the platform test sandbox, none of which this run uses, so it writes the same files as the release.

## Making it again

The generator runs only in a v0.5.0 checkout, set up as CI sets one up (`bun install --frozen-lockfile`, Playwright's Chromium), on Linux with no desktop: `dbus-run-session` for the keychain and `xvfb-run` for the window.

```sh
git worktree add --detach ../pathkeep-v050 v0.5.0
cd ../pathkeep-v050
cp ../<this repo>/tests/fixtures/v0.5-archive/make-archive.config.ts playwright.v05-archive.config.ts
cp ../<this repo>/tests/fixtures/v0.5-archive/make-archive.spec.ts tests/e2e/v05-archive.spec.ts
# Optional: V05_LATER_MOVE_FILE=<a move file a later build exported> for step 7.
dbus-run-session -- xvfb-run -a node scripts/run-playwright.mjs --config playwright.v05-archive.config.ts
cp -a "$(node -p "require('./artifacts/e2e/v05-archive/made.json').projectRoot")" artifacts/e2e/v05-archive/data
```

The config starts the app through the supervised launcher (`PATHKEEP_E2E_RESTART_FILE`) and waits for the bridge's `/health`: a plain `tauri dev` gives up after 180 seconds of waiting for Vite while the worker sidecar is still compiling. The data folder is copied after the app has exited, so no database is mid-write. `artifacts/e2e/v05-archive/` then holds `data/`, `v05-move-file.pathkeep`, `made.json` and every bridge reply under `replies/`; `expected.json` is built from those replies. `data.tar.gz` is `data/`'s contents (`COPYFILE_DISABLE=1 tar --no-mac-metadata --no-xattrs -czf data.tar.gz -C data .` on macOS). Visit times are relative to the day it runs, so a new folder has new dates; the counts and texts stay the same. Remove the worktree afterwards.

This one was made on 2026-11-16 on a Linux machine (Ubuntu 24.04, 4 vCPU) with the packages CI installs, Node 22.22.2, Bun 1.4.4 and Rust 1.96.0.
