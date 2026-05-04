# OpenCC Script Folding

> Implementation record for M14 Traditional/Simplified Chinese recall folding.

## Decision

M14-D restores Traditional/Simplified Chinese keyword recall through official
OpenCC dictionary assets and a repo-owned Rust converter.

The product does **not** link OpenCC's C++ library in this slice. The local
probe found the C++ route is still toolchain-fragile on this machine because
`cmake` and `pkg-config` are not available on `PATH`. Shipping a build script
that silently relies on Homebrew or host-installed dynamic libraries would make
`bun run check` and release packaging less reproducible.

The shipped route is:

1. Vendor the minimal official OpenCC 1.3.0 Apache-2.0 dictionary subset needed
   for `t2s` and `tw2sp` search normalization.
2. Parse those dictionaries once per process with `LazyLock`.
3. Index and query both `t2s` and `tw2sp` Simplified variants when they differ,
   so examples such as `設定` / `设定` still match even when OpenCC's Taiwan
   idiom path also yields `设置`.

This is a search-normalization contract, not a general-purpose conversion UI.

## Provenance

Official source:

- Repository: `https://github.com/BYVoid/OpenCC`
- Tag: `ver.1.3.0`
- Tag object: `eb33bcbcc2b4bc693aa8c5403ee4cb0d97c5d681`
- Peeled commit: `4d23eff614fcb9c2fe4460d9e610f93efb35ff11`
- License: Apache-2.0

Vendored files:

| File                                                | SHA-256                                                            |
| --------------------------------------------------- | ------------------------------------------------------------------ |
| `vendor/opencc/LICENSE`                             | `b534e465949558eec2597b04f5092b5e161236a68dfbfd04d547592ac3964308` |
| `vendor/opencc/dictionary/TSCharacters.txt`         | `ad870b4feeb494cfa7b3b05242bd79af574b22f6b2bdeb89a1633e4b50ed0a3c` |
| `vendor/opencc/dictionary/TSPhrases.txt`            | `84dc4ad5005a739c16a70b1fa01018209afebfd803c90cbd2e0acffc252973d5` |
| `vendor/opencc/dictionary/TWPhrasesRev.txt`         | `478db978f1f24adf08e5e06ca0500df43ac53b75234d81a531991bab269cf3ed` |
| `vendor/opencc/dictionary/TWVariants.txt`           | `89473e96e3f61e9bd3f2e303b9d88ac9caa61effb1faadcef94ff5e65b8ed54b` |
| `vendor/opencc/dictionary/TWVariantsRevPhrases.txt` | `6b58c0687af26b13cde81c1442dd6f570cc93f398c7ff361782244b47941ff43` |

These files are enough for the search analyzer's bounded `t2s` and `tw2sp`
folding path:

- `t2s`: `TSPhrases`, then `TSCharacters`
- `tw2sp`: `TWPhrasesRev`, `TWVariantsRevPhrases`, reversed `TWVariants`,
  then `TSPhrases`, then `TSCharacters`

The repo does not vendor `.ocd2` binaries because OpenCC generates those through
`opencc_dict` during a CMake build. Keeping the text dictionaries makes the data
diff auditable and avoids native build-time code execution.

## Local Toolchain Probe

Probe date: 2026-05-03.

Observed on the current development host:

- `cmake`: not found on `PATH`
- `pkg-config`: not found on `PATH`
- `python3`: `/opt/homebrew/bin/python3`, version `3.14.4`
- `clang++`: `/usr/bin/clang++`, Apple clang `21.0.0`
- `brew`: `/opt/homebrew/bin/brew`, Homebrew `5.1.8`

OpenCC's official CMake build requires:

- CMake (`cmake_minimum_required(VERSION 3.5)`)
- C++17 for current builds
- Python for dictionary generation
- bundled `marisa` by default unless `USE_SYSTEM_MARISA=ON`
- `opencc_dict` before dictionary `.ocd2` assets can be generated

That route remains allowed, but product code must not depend on it until both
local and CI prerequisites are explicit.

## CI And Packaging Contract

Current M14-D product code does not need extra CI packages because it links no
OpenCC native library and loads no runtime assets from the host.

If a future slice switches from repo-owned Rust conversion to OpenCC C++ linking,
the slice must first go through the project-scoped native dependency manager
defined in [native-dependency-management.md](native-dependency-management.md) and
do all of the following:

1. Declare the native library path in `vcpkg.json` and pin the registry through
   `vcpkg-configuration.json`; do not rely on Homebrew, apt, winget, or a global
   `pkg-config`.
2. Prove `scripts/native-deps.mjs install --feature=opencc` on Linux, Windows,
   and macOS release targets, including Apple Silicon or an audited overlay port
   if the stock vcpkg port still rejects arm targets.
3. Choose static linking by default; if dynamic linking is chosen, document
   `.app`, `.deb` / AppImage, and Windows DLL packaging paths before code lands.
4. Prove release builds do not depend on Homebrew, system OpenCC, or user
   machine library paths.
5. Add rollback steps that remove the native build script and restore the
   current official-assets converter.

## Runtime Contract

The Rust converter is intentionally narrow:

- It chooses the first OpenCC dictionary value for ambiguous entries.
- It applies longest-match priority and preserves dictionary source order for
  ties.
- It emits at most two variants per input in the current analyzer path: direct
  `t2s`, plus `tw2sp` when different.
- It never reads user-writable dictionary files at runtime.
- It never loads SQLite extensions, shared libraries, Jieba, or plugin ABI.

This keeps M14 inside the non-embedding lexical recall boundary while fixing the
expected `設定` / `设定` class of recall failures.

## Rollback

To roll back this slice:

1. Remove `archive::search_opencc` and its call from `search_lexical`.
2. Delete `src-tauri/crates/vault-core/vendor/opencc`.
3. Rebuild the `history-search.sqlite` derived projection; it is rebuildable
   state and still attached as plaintext `KEY ''` in M14.
4. Revert docs that claim Traditional/Simplified folding is shipped.
