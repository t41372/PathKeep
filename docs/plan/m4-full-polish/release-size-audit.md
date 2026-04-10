# M4 — Release Size Audit

> `WORK-M4-L` closeout artifact. This file defines how PathKeep records bundle-size evidence instead of relying on one-off local observations.

## Command

Run:

```bash
bun run release:size-audit
```

The script reads:

- `dist/.vite/manifest.json` for web chunk attribution
- `dist/release/` for downloaded release assets such as `latest.json`, checksums, manifests, and signatures when present
- `src-tauri/target/release/bundle/` for locally built installer / updater bundle artifacts

## Output

- `artifacts/release/<date>-size-audit/size-attribution.json`
- `artifacts/release/<date>-size-audit/summary.md`

The generated bundle is expected to answer four questions:

1. Which web entry chunks dominate the browser bundle?
2. Which installer / package artifacts dominate local bundle size?
3. Are updater metadata files such as `latest.json`, signatures, checksums, and `RELEASE-MANIFEST.json` present when release assets are downloaded locally?
4. Did the audit run against only local builds, or also against downloaded GitHub release assets?

## Current M4 Closeout Stance

- local size attribution is a required release-closeout artifact
- downloaded release assets are optional in local development, but the workflow must still produce `latest.json`, signatures, checksums, and `RELEASE-MANIFEST.json`
- size evidence belongs in checked-in artifacts or reproducible script output, not in release notes alone

## 2026-04-10 Local Snapshot

- artifact bundle: `artifacts/release/2026-04-10-size-audit/`
- web total: `852181` bytes
- base shell entry (`index.html` manifest entry, JS + CSS): `383527` bytes
- heaviest route chunk: `src/pages/settings/index.tsx` at `57026` bytes
- local installer bundle found: `dmg/PathKeep_0.1.0_aarch64.dmg` at `70884740` bytes
- local support-file payload is still dominated by the unsigned app binary inside `macos/PathKeep.app/Contents/MacOS/pathkeep-desktop`
- downloaded `dist/release/` assets were not present in this local run, so `latest.json` / checksums / manifest remain workflow-verified rather than locally downloaded
