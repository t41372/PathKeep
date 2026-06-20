# Native Dependency Management

> Architecture contract for C / C++ dependencies such as OpenCC, marisa, and
> other native libraries that may enter PathKeep later.

## Decision

PathKeep's long-term native dependency manager is **vcpkg manifest mode** with a
pinned registry baseline and repo-local install root.

The project now owns:

- `vcpkg.json` — the native dependency manifest.
- `vcpkg-configuration.json` — the pinned Microsoft vcpkg registry baseline.
- `scripts/native-deps.mjs` — a project-local bootstrap / install helper.
- `.github/workflows/native-deps.yml` — an explicit native dependency proof
  workflow, separate from the per-commit product checker.

Native C / C++ libraries must not be discovered from Homebrew, apt, winget, or a
developer's global `/usr/local` / `/opt/homebrew` install. Allowed paths for
native dependencies, in priority order:

1. **Rust crate vendored/bundled compilation** (preferred) — crates like
   `rusqlite` with `bundled-sqlcipher`, `ring`, `brotli`, etc. compile C/C++
   source via `cc`/`cmake` during `cargo build`. This is standard Rust practice
   and the project already uses it. Choosing a Rust crate with a C/C++ backend
   for performance reasons requires no extra approval beyond the normal
   supply-chain trust gate in `AGENTS.md`.
2. **vcpkg manifest mode** — for standalone native libraries that have no quality
   Rust crate wrapper. Declare in `vcpkg.json`, install under
   `var/native-deps/vcpkg_installed`, and prove in CI before the product link
   lands.
3. **Tauri / OS SDK platform frameworks** — explicit host prerequisites.

## Why vcpkg

vcpkg is maintained by Microsoft, supports manifest mode, lockable registry
baselines, features, CMake integration, binary caching, and cross-platform
native dependency builds. It fits PathKeep's supply-chain rule better than
low-trust language bindings or ad hoc CMake fetches.

The remaining host prerequisite is the platform compiler / SDK. That is a real
boundary: macOS still needs Xcode Command Line Tools, Windows still needs MSVC,
and Linux still needs a compiler toolchain. PathKeep's rule is that native
**libraries and build helper packages** are project-scoped; OS SDKs and Tauri's
platform framework packages remain explicit platform prerequisites until the
desktop build itself is moved into a full hermetic environment.

## OpenCC Status

The shipped product still uses official OpenCC dictionary assets plus
repo-owned Rust conversion code. It does **not** link OpenCC's C++ library.

The vcpkg path is available as a proof lane, not a product dependency:

- Current pinned vcpkg registry commit:
  `522253caf47268c1724f486a035e927a42a90092`
- Current vcpkg OpenCC port: `opencc 1.1.9#1`
- OpenCC port provenance: downloads `BYVoid/OpenCC` tag `ver.${VERSION}` with a
  SHA-512 source archive check.
- Current port support expression: `!(arm | uwp)`
- Windows proof triplet: `x64-windows-static`, because OpenCC's `marisa-trie`
  dependency rejects the default dynamic `x64-windows` triplet and only supports
  Windows through static or MinGW triplets.

That support expression means the stock vcpkg port is not yet acceptable for a
PathKeep Apple Silicon release. A future native OpenCC slice must either:

1. upstream or overlay an audited OpenCC port that supports `arm64-osx`, or
2. keep the existing official-assets Rust converter.

## Commands

Run a non-network local configuration check:

```bash
bun run native-deps:doctor
```

Bootstrap pinned vcpkg under `var/native-deps/vcpkg`:

```bash
bun run native-deps:bootstrap
```

Install the OpenCC proof feature into the repo-local install root:

```bash
bun run native-deps:install:opencc
```

Print environment values for future CMake / Rust build script integration:

```bash
bun run native-deps:env
```

The generated tree lives under ignored `var/native-deps/` and must not be
committed.

## CI Contract

`.github/workflows/native-deps.yml` is a native dependency proof workflow. It
runs on:

- `ubuntu-latest` with `x64-linux`
- `macos-15-intel` with `x64-osx`
- `windows-latest` with `x64-windows-static`

It intentionally does not run on Apple Silicon yet because the current vcpkg
OpenCC port rejects arm targets. The macOS lane still needs an Intel runner for
the `x64-osx` triplet, but it must use GitHub's current `macos-15-intel` label
rather than the retired `macos-13` image. This is not a product release proof
for native OpenCC; it is a regression guard for the project-scoped dependency
manager.

`bun run check` does not run vcpkg because the default product build currently
does not link native OpenCC. If a future product slice starts linking native
libraries, that slice must add a fast deterministic probe to `bun run check` and
keep slow full native rebuilds in the native dependency workflow or release
workflow.

## Banned Patterns

- No `build.rs` that shells out to `brew`, `apt`, `winget`, or global
  `pkg-config` to find product native dependencies.
- No ad-hoc `build.rs` that downloads, `git clone`s, or compiles C/C++ source
  outside of a published crate's vendored build — use a Rust crate or vcpkg.
- No `FetchContent` / CPM path for product native libraries unless a separate
  ADR proves why vcpkg cannot work.
- No low-trust language binding as a shortcut around native dependency
  management.
- No dynamic library path that only works on the developer's machine.

## Future Nix Boundary

Nix / devbox can still be useful for a fully pinned developer shell, but it is a
separate environment-management decision. This repository does not add a Nix
flake yet because the current host does not have Nix installed and there is no
local proof for a locked flake. If PathKeep later adopts Nix, it must be a
checked-in `flake.nix` + `flake.lock` that wraps Rust, Bun, Playwright, vcpkg,
and platform prerequisites without replacing the vcpkg manifest as the native
library source of truth.
