# Program — Quality Matrix

> 2026-04-07（`WORK-QC-A`）起，這份文檔是 PathKeep 在進入 M4 前的 quality gate source of truth。  
> 原則很簡單：**文檔怎麼寫，repo 就怎麼擋**。凡是會被宣稱成 blocking、release 或 deep check 的驗收，都必須能在 scripts 與 workflow 裡兌現。

---

## Mainline Blocking Path

| Gate            | Command / Workflow       | 保護範圍                                                                                                                                                   | 備註                                                                                           |
| --------------- | ------------------------ | ---------------------------------------------------------------------------------------------------------------------------------------------------------- | ---------------------------------------------------------------------------------------------- |
| Repo checks     | `bun run check`          | Prettier、ESLint、TypeScript、Vitest、desktop contract slice、Rust fmt / clippy / workspace tests、supply-chain audit、host-matched platform-native checks | `check` 本身不含 repo-wide coverage、browser preview e2e 或 full mutation。                    |
| Platform native | `bun run check:platform` | host-matched platform-native Rust integration、desktop updater / launcher slice、debug desktop build smoke                                                 | macOS / Linux 為 mainline required；Windows job 與命令已落地，runner 可用時轉為同等 blocking。 |
| JS coverage     | `bun run coverage:js`    | living M0-M3 JS quality surface                                                                                                                            | 100% statement / branch / function / line coverage。由 CI `frontend` job 直接執行。            |
| Rust coverage   | `bun run coverage:rust`  | Tauri desktop command / bridge quality surface                                                                                                             | 100% line + function coverage。由 CI `rust` job 直接執行。                                     |
| Browser build   | `bun run build`          | TypeScript compile + Vite bundle                                                                                                                           | 保證 browser shell 可建置。                                                                    |
| Browser smoke   | `bun run test:e2e`       | browser preview 的 shell / onboarding / dashboard / trust / intelligence smoke                                                                             | 這是 preview surface smoke，不等於完整 desktop / Tauri signoff。                               |

### `bun run check` 內含的 targeted sub-gate

`bun run check` 目前固定包含 `bun run check:desktop-contract` 與 `bun run check:platform`，保護：

- `src/main.tsx`
- `src/lib/ipc/bridge.ts`
- host-matched platform-native keyring / scheduler / launcher / discovery / biometric smoke
- desktop updater / launcher command surface 與 debug desktop build smoke

這些 sub-gate 的責任是保護 desktop entry、typed IPC contract 與 platform-specific host truth；它們不是替 shell / route / sidebar / trust-critical flows 做全站背書。

---

## Current Quality Surfaces

### JS coverage / mutation quality surface

`bun run coverage:js` 與 `bun run mutation:js` 目前對齊以下 living M0-M3 JS surface：

- `src/main.tsx`
- `src/app/shell-data.tsx`
- `src/lib/backend.ts`
- `src/lib/format.ts`
- `src/lib/intelligence.ts`
- `src/lib/ipc/bridge.ts`
- `src/lib/platform-guidance.ts`
- `src/lib/stronghold.ts`
- `src/lib/trust-review.ts`
- `src/lib/i18n/context.ts`
- `src/lib/i18n/hooks.ts`
- `src/lib/i18n/provider.tsx`

這代表我們不再只 mutate 3 個 helper，也不把已退場的舊 shell 當成 coverage 主體；但它仍然不是「整個前端所有 route / component 已被 100% gate 保護」的意思。`WORK-QC-B` 已把 prototype / doc parity、product-flow signoff 與 trust-critical UI debt 收回 source docs；剩餘的全站 accessibility / release polish 留在 M4。

### Rust coverage quality surface

`bun run coverage:rust` 的 `quality` scope 目前只保護最直接承接 desktop command / bridge contract 的 Tauri surface：

- `src-tauri/src/file_manager.rs`
- `src-tauri/src/lib.rs`
- `src-tauri/src/main.rs`
- `src-tauri/src/session.rs`
- `src-tauri/src/worker_bridge.rs`

這條 gate 的目的是誠實保護桌面命令邊界，而不是假裝 `vault-core` / `vault-worker` / parser 巨型模組都已達到 repo-wide 100% coverage。那些 deeper crates 目前由 workspace Rust tests、targeted Rust acceptance tests，以及下方的 deep checks 補強。

### Rust mutation quality surface

`bun run mutation:rust` 與 GitHub `Mutation` workflow 的 `rust-mutation` job 目前對齊以下 Rust mutation contract：

- `browser-history-parser` crate
- `src-tauri/crates/vault-core/src/ai.rs` 的 status/helper slice：
  - `ai_index_status`
  - `ai_queue_status`
  - `reconcile_ai_queue_controls`
  - `provider_capabilities`
  - `provider_connection_failure_report`
  - `test_provider_connection`

這條 contract 的目標是先把 parser 與 AI control-plane/status surface 收回可驗證狀態。`bun run mutation:rust:full` 保留作 exploratory whole-workspace sweep，用來挖出後續 backlog 或 deferred rationale；它不是目前的 signed-off mutation gate。

---

## Scheduled / Release Gates

| Gate                             | Command / Workflow                                                       | 用途                                                                            | 備註                                                                                                      |
| -------------------------------- | ------------------------------------------------------------------------ | ------------------------------------------------------------------------------- | --------------------------------------------------------------------------------------------------------- |
| Platform Rust native sweep       | `bun run test:platform:rust`                                             | 直接驗證 host keyring / scheduler / launcher / discovery / biometric capability | Linux job 會在隔離 `dbus-run-session` 內啟動 `gnome-keyring-daemon`；Windows 測試已準備好供 runner 接手。 |
| Platform desktop slice           | `bun run test:platform:desktop`                                          | debug desktop build + updater / launcher desktop command slice                  | 這是 desktop command truth，不等於 browser preview e2e。                                                  |
| Chrome desktop bridge smoke      | `bun run test:e2e:desktop-bridge`                                        | 啟動 feature-gated desktop bridge，讓 Chrome / Playwright 驗證真實 Rust command | local / manual / pre-closeout gate；不是 mainline blocking path。                                         |
| JS mutation sweep                | `bun run mutation:js` / GitHub `Mutation` workflow `javascript-mutation` | 對 living M0-M3 JS quality surface 做 repo-level mutation gate                  | 目前 break threshold 是 80。屬於 scheduled / manual deep check；進 M4 與 release closeout 前必須能跑通。  |
| Rust mutation contract           | `bun run mutation:rust` / GitHub `Mutation` workflow `rust-mutation`     | `browser-history-parser` + `vault-core` AI status/helper slice                  | 不在每次 PR blocking path；屬於 scheduled / manual / pre-release gate。                                   |
| Exploratory Rust mutation sweep  | `bun run mutation:rust:full`                                             | whole-workspace cargo-mutants discovery run                                     | 用於 backlog / deferred rationale，不是目前的 signed-off gate。                                           |
| Full local sweep                 | `bun run check:full`                                                     | 本地一次跑 `check` + coverage + mutation + e2e                                  | 適合大 closeout 或 merge 前自我驗收。                                                                     |
| Release-style local verification | `bun run verify`                                                         | `check:full` + `build` + `desktop:build:debug`                                  | 作為 release / milestone closeout 的本地預演。                                                            |

> 2026-04-08 closeout 註記：`bun run check`、`bun run build`、`bun run coverage:js`、`bun run coverage:rust`、`bun run mutation:js`、`bun run test:e2e` 與 `bun run desktop:build:debug` 均已通過；隨後 `WORK-M4-D` 把 Rust mutation baseline 收斂成誠實的 signed-off contract：parser crate + AI status/helper slice。更廣的 `bun run mutation:rust:full` 仍保留給 exploratory whole-workspace triage，不再被誤報成已簽收 gate。

---

## Honest Boundaries

- `bun run test:e2e` 是 browser preview smoke，不是 Tauri desktop、worker process、scheduler artifact、keyring 或 filesystem side effect 的最終驗收。
- `bun run test:e2e:desktop-bridge` 證明 Chrome / Playwright 能透過 dev-only localhost bridge 打到真實 desktop command façade，但它仍不是完整的 Tauri WebView / plugin guest API signoff。
- `bun run check:platform` 才是目前對 macOS / Linux host-native scheduler、keyring、launcher 與 updater desktop slice 的 blocking signoff；preview e2e 不能拿來替這些能力背書。
- schedule / security / import / intelligence 這些高風險 surface 的 desktop truth，仍要靠 Rust tests、worker bridge tests、Tauri command tests 與對應的 PME / product docs 對齊。
- `coverage:js` / `mutation:js` 的 quality surface 已恢復到 living M0-M3 modules，但還不是整個前端 UI 的 100% signoff。`WORK-QC-B` 已把剩餘的 prototype gap、doc parity 與 trust-critical flow debt 收斂回 docs / tests；更廣的 release-level AX 與 desktop validation 留在 M4。
- `coverage:rust` 故意只保護 Tauri desktop command / bridge surface；如果未來要把 `vault-core` / parser / worker 納回同一條 100% coverage gate，必須先有明確的 surface 定義與成本說明，而不是重新回到失真的「全都算已驗」敘事。
