# Program — Quality Matrix

> 2026-04-07（`WORK-QC-A`）起，這份文檔是 PathKeep 在進入 M4 前的 quality gate source of truth。  
> 原則很簡單：**文檔怎麼寫，repo 就怎麼擋**。凡是會被宣稱成 blocking、release 或 deep check 的驗收，都必須能在 scripts 與 workflow 裡兌現。

---

## Mainline Blocking Path

| Gate          | Command / Workflow      | 保護範圍                                                                                                              | 備註                                                                                |
| ------------- | ----------------------- | --------------------------------------------------------------------------------------------------------------------- | ----------------------------------------------------------------------------------- |
| Repo checks   | `bun run check`         | Prettier、ESLint、TypeScript、Vitest、desktop contract slice、Rust fmt / clippy / workspace tests、supply-chain audit | `check` 本身不含 repo-wide coverage、browser preview e2e 或 full mutation。         |
| JS coverage   | `bun run coverage:js`   | living M0-M3 JS quality surface                                                                                       | 100% statement / branch / function / line coverage。由 CI `frontend` job 直接執行。 |
| Rust coverage | `bun run coverage:rust` | Tauri desktop command / bridge quality surface                                                                        | 100% line + function coverage。由 CI `rust` job 直接執行。                          |
| Browser build | `bun run build`         | TypeScript compile + Vite bundle                                                                                      | 保證 browser shell 可建置。                                                         |
| Browser smoke | `bun run test:e2e`      | browser preview 的 shell / onboarding / dashboard / trust / intelligence smoke                                        | 這是 preview surface smoke，不等於完整 desktop / Tauri signoff。                    |

### `bun run check` 內含的 targeted sub-gate

`bun run check` 目前固定包含 `bun run check:desktop-contract`，保護：

- `src/main.tsx`
- `src/lib/ipc/bridge.ts`

這條 sub-gate 的責任是保護 desktop entry 與 typed IPC contract，不是替 shell / route / sidebar / trust-critical flows 背書。

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

---

## Scheduled / Release Gates

| Gate                             | Command / Workflow                                                       | 用途                                                           | 備註                                                                                                     |
| -------------------------------- | ------------------------------------------------------------------------ | -------------------------------------------------------------- | -------------------------------------------------------------------------------------------------------- |
| JS mutation sweep                | `bun run mutation:js` / GitHub `Mutation` workflow `javascript-mutation` | 對 living M0-M3 JS quality surface 做 repo-level mutation gate | 目前 break threshold 是 80。屬於 scheduled / manual deep check；進 M4 與 release closeout 前必須能跑通。 |
| Rust mutation sweep              | `bun run mutation:rust` / GitHub `Mutation` workflow `rust-mutation`     | 對 Rust workspace 做高成本 mutation deep check                 | 不在每次 PR blocking path；屬於 scheduled / manual / pre-release gate。                                  |
| Full local sweep                 | `bun run check:full`                                                     | 本地一次跑 `check` + coverage + mutation + e2e                 | 適合大 closeout 或 merge 前自我驗收。                                                                    |
| Release-style local verification | `bun run verify`                                                         | `check:full` + `build` + `desktop:build:debug`                 | 作為 release / milestone closeout 的本地預演。                                                           |

---

## Honest Boundaries

- `bun run test:e2e` 是 browser preview smoke，不是 Tauri desktop、worker process、scheduler artifact、keyring 或 filesystem side effect 的最終驗收。
- schedule / security / import / intelligence 這些高風險 surface 的 desktop truth，仍要靠 Rust tests、worker bridge tests、Tauri command tests 與對應的 PME / product docs 對齊。
- `coverage:js` / `mutation:js` 的 quality surface 已恢復到 living M0-M3 modules，但還不是整個前端 UI 的 100% signoff。`WORK-QC-B` 已把剩餘的 prototype gap、doc parity 與 trust-critical flow debt 收斂回 docs / tests；更廣的 release-level AX 與 desktop validation 留在 M4。
- `coverage:rust` 故意只保護 Tauri desktop command / bridge surface；如果未來要把 `vault-core` / parser / worker 納回同一條 100% coverage gate，必須先有明確的 surface 定義與成本說明，而不是重新回到失真的「全都算已驗」敘事。
