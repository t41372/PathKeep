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

- `artifacts/release/<timestamp>-size-audit/size-attribution.json`
- `artifacts/release/<timestamp>-size-audit/summary.md`

The audit script now walks the Vite manifest graph from each entrypoint, follows `imports` / `dynamicImports` / `css` / `assets`, and dedupes shared files before attributing totals. Same-day runs use full timestamped directories so evidence never overwrites an earlier local snapshot from the same date.

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

## 2026-04-10 Backend Binary Audit Follow-Up

- pre-fix unsigned release executable: `190M`
- post-fix `cargo build --release --bin pathkeep-desktop` executable: `104M`
- reduction: `86M` smaller, about `45%`
- the pre-fix binary was not bloated by frontend payload; `dist/` stayed under `1M`, while the Mach-O executable itself carried the weight
- the biggest avoidable regression was macOS building through the umbrella `keyring` crate, which transitively pulled `db-keystore` and `turso*` artifacts even though PathKeep only needed the native keychain backend on that platform
- the other large avoidable factor was missing release-size optimizations; enabling stripped symbols plus thin LTO removed a large `__LINKEDIT` / dead-code tail from the final executable
- the dominant remaining cost is still the optional intelligence stack that is linked into the default desktop binary: `lancedb` / `lance` / `datafusion` / `rig-core`, plus `bundled-sqlcipher-vendored-openssl`
- current truth: a ~100 MB Rust desktop executable for this feature set is still heavy, but materially more honest than the previous ~190 MB binary; any further major drop now requires a deeper runtime-boundary decision, not just link-time cleanup

## 2026-04-10 Packaging Sign-Off / 2026-04-11 UTC Snapshot

- 2026-04-10：使用者明確 sign off，PathKeep 預設桌面版**維持**把 optional AI / MCP / semantic runtime 與 archive / shell-critical runtime 一起 shipping；不拆 helper，不改成 feature-SKU build。
- refreshed artifact bundle: `artifacts/release/2026-04-11-size-audit/`（generated at `2026-04-11T05:09:56.896Z`）
- web total: `903585` bytes
- base shell entry (`index.html` manifest entry, JS + CSS): `387414` bytes
- heaviest route chunk: `src/pages/settings/index.tsx` at `63696` bytes
- local installer bundle found: `dmg/PathKeep_0.1.0_aarch64.dmg` at `43117049` bytes
- local updater bundle found: `macos/PathKeep.app.tar.gz` at `44796557` bytes
- unsigned app binary inside the local bundle: `macos/PathKeep.app/Contents/MacOS/pathkeep-desktop` at `109198880` bytes（約 `104 MiB`）
- current truth: web payload 仍低於 `1 MB`；主要重量依然在 desktop executable，而這部分現在屬於接受的產品取捨，不再是假裝還沒拍板的 blocker
- 後續若還要再做 size work，重點應放在一般 dependency hygiene、supply-chain review 與 evidence refresh；若要改變 default shipping surface，則必須重新開決策文檔，而不是在 cleanup 裡偷偷拆掉 optional intelligence
