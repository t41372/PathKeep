# Build identity: a development build never touches the installed app

Every PathKeep binary is compiled as one of two classes. A **distribution** build is what users install. Everything else is a **development** build: `tauri dev`, the test binaries, `bun run desktop:build`, a local `cargo build --release`, the `verify` rehearsal, the unsigned Windows test binary.

The class decides which data folder, keychain entries, schedule and installer identity the binary uses by default. A development build has its own of each.

## Why

Before this rule every build defaulted to the installed app's data folder. A development build started without `CHB_PROJECT_ROOT` opened the developer's real archive, could run recovery, a wipe or a migration on it, and shared the installed app's schedule, login item and installer identity. A build that writes a newer archive format would leave the installed, older app unable to open the archive.

## The rule

`src-tauri/crates/vault-build` holds the classification and both sets of names. It depends only on `std`.

- A build is a distribution build only when `PATHKEEP_DISTRIBUTION=1` was set **at compile time** and debug assertions are off. Setting the variable when starting an already-built binary changes nothing.
- An explicit, non-empty `CHB_PROJECT_ROOT` always wins and is used as given. An empty value is an error, never a fallback to a default.
- Without the override, a distribution build uses the released data folder and a development build uses the development one. Nothing is copied or migrated between them.

| What                                   | Distribution (must never change)                                       | Development                                                    |
| -------------------------------------- | ---------------------------------------------------------------------- | -------------------------------------------------------------- |
| Data folder, macOS                     | `~/Library/Application Support/com.yi-ting.pathkeep`                   | `~/Library/Application Support/com.yi-ting.pathkeep-dev`       |
| Data folder, Windows                   | `%LOCALAPPDATA%\yi-ting\pathkeep\data`                                 | `%LOCALAPPDATA%\yi-ting\pathkeep-dev\data`                     |
| Data folder, Linux                     | `$XDG_DATA_HOME/pathkeep` or `~/.local/share/pathkeep`                 | `$XDG_DATA_HOME/pathkeep-dev` or `~/.local/share/pathkeep-dev` |
| Bundle identifier and keychain service | `com.yi-ting.pathkeep`                                                 | `com.yi-ting.pathkeep.dev`                                     |
| Backup schedule label / task / unit    | `com.yi-ting.pathkeep.backup`                                          | `com.yi-ting.pathkeep.dev.backup`                              |
| Older schedule labels it cleans up     | `dev.codex.pathkeep.backup`, `dev.codex.browser-history-backup.backup` | none                                                           |
| macOS login item                       | `com.yi-ting.pathkeep.login`                                           | `com.yi-ting.pathkeep.dev.login`                               |
| Package name, Windows/Linux autostart  | `PathKeep`                                                             | `PathKeep Dev`                                                 |
| Windows installer upgrade code         | `818daeb2-ee49-5696-a1db-bee51050939c`                                 | `37e94b55-0506-58ec-9137-e48b4c732ae8`                         |

The distribution column is a contract with every installed copy. Tests pin each value as a literal (`vault-build/tests/classification.rs`, `vault-core/tests/build_root.rs`).

The name shown inside the app is `PathKeep` in both classes.

`src-tauri/tauri.conf.json` carries the development identity, so a plain `tauri build` is safe by default. `src-tauri/tauri.distribution.conf.json` is the overlay that restores the released product name, bundle identifier and upgrade code.

## Every resource on the machine that is keyed on the app's identity

| Resource                                                          | What separates a development build from the installed app                                                                                                                                                                                                                             |
| ----------------------------------------------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Archive, config, logs, locks                                      | The data folder rule above.                                                                                                                                                                                                                                                           |
| Remembered archive key, AI provider keys                          | The keychain service. A development build also refuses the installed app's service name when an environment override asks for it: reading that entry would hand over the real archive key.                                                                                            |
| Backup schedule                                                   | The label. A development build does not look for or repair the older labels, and refuses a saved plan whose label, file name or plist `Label` is not its own.                                                                                                                         |
| Open at login                                                     | The login item label on macOS, the autostart name on Windows and Linux.                                                                                                                                                                                                               |
| Single instance, owner socket or pipe, owner lock                 | Already derived from the data folder path. Different folders, different owners.                                                                                                                                                                                                       |
| Updater                                                           | A development build does not load the updater. It reports updates as unsupported and refuses to install: the update feed serves the released installer, which would replace the installed app.                                                                                        |
| Installer and bundle identity (Launch Services, TCC, MSI upgrade) | Bundle identifier, product name and upgrade code.                                                                                                                                                                                                                                     |
| WebView storage                                                   | No code needed. On macOS a bundled development app gets WebKit's default store for its own bundle identifier and a bare executable gets one named after the executable. On Windows and Linux the store sits under the runtime identifier, which is derived from the data folder path. |
| Linux package files under `/usr/bin`                              | No code needed. The executables keep their names. A development package has a different package name, so `dpkg` and `rpm` refuse to install it over files the released package owns. Installing both side by side is not supported.                                                   |
| OS facts (locale, OS version, biometrics, user id, tray support)  | Shared on purpose. They are facts about the machine, not state of the installed app.                                                                                                                                                                                                  |
| Browser profiles, files the user picks, external provider config  | Shared on purpose. They are inputs the user chooses.                                                                                                                                                                                                                                  |

The test seams keep working in every development build, optimized or not: `PATHKEEP_PLATFORM_TEST_SANDBOX_DIR` (schedule and login item as files), `PATHKEEP_PLATFORM_TEST_KEYRING_DIR` (keychain as files) and the label and service overrides. A distribution build ignores all of them. A development build whose data folder is overridden but has no sandbox refuses to reach the OS scheduler or login items.

## A release that is not a distribution build must not ship

If packaging ever built without the designation, every user would open an empty development folder after updating and believe their archive was gone. Three things stand in the way:

1. Both executables answer `--build-identity` with their compiled class and names as JSON. They answer before resolving a data folder, logging, opening a window or connecting to anything.
2. `bun run release:build` is the only way a release is packaged. It sets the designation, applies the overlay, builds, and then opens every package it produced (and every `.app`), runs the packaged desktop and the packaged worker with `--build-identity`, and fails unless both report the distribution class, every released name, no debug assertions, and agree with each other. A binary that does not contain the probe is refused without being run. It runs before the release workflow uploads anything.
3. In the Release workflow, preparing the worker sidecar fails when the designation is missing, and `release:check:config` fails if the workflow stops calling `release:build`.

`bun run release:check --package <file>` runs the same package check by hand. `bun run release:check:config` is the source-only check in the per-commit gate; it says nothing about a built package.

Not yet proven: the package check has run against synthetic packages only. Opening real MSI, NSIS, DEB, RPM, AppImage and DMG output, and running an Intel macOS binary on an Apple Silicon runner, first happens in the release workflow. It fails closed, so the cost of a mistake there is a blocked release, and it has to be rehearsed with an unsigned preview before the first release that uses it (`WORK-RELEASE-REHEARSAL-V06`).

## Adding a resource

Anything new that lives outside the data folder and is named after the app (a second keychain service, a helper's launchd label, a URL scheme, a shared cache folder) gets a field in `vault-build`'s `Identity` with a value for each class, and a test in which a development build leaves a stand-in for the installed app's copy untouched.
