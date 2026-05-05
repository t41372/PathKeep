# Program — Quality Matrix

> 2026-04-07（`WORK-QC-A`）起，這份文檔是 PathKeep 在進入 M4 前的 quality gate source of truth。  
> 原則很簡單：**文檔怎麼寫，repo 就怎麼擋**。凡是會被宣稱成 blocking、release 或 deep check 的驗收，都必須能在 scripts 與 workflow 裡兌現。

---

## Mainline Blocking Path

| Gate           | Command / Workflow                       | 保護範圍                                                                                                                                                                                                              | 備註                                                                                               |
| -------------- | ---------------------------------------- | --------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | -------------------------------------------------------------------------------------------------- |
| Strict checker | `bun run check` / GitHub `CI` workflow   | base checks、100% JS/Rust coverage、browser build、browser-preview e2e、desktop-bridge truth gate、desktop-contract JS mutation、Codecov upload                                                                       | 這是 signed-off per-commit checker；`main` push / PR / manual CI 與本地使用同一條 effective gate。 |
| Base triage    | `bun run check:base`                     | Prettier、ESLint、i18n parity/raw-English guard、TypeScript、Vitest、desktop contract slice、Rust fmt / clippy / workspace tests、supply-chain audit、host-matched platform-native checks、release-config drift guard | 只作 fast triage helper；不能替代 `bun run check`。                                                |
| JS coverage    | `bun run coverage:js`                    | active `src/**/*.{ts,tsx}` runtime source                                                                                                                                                                             | 100% statement / branch / function / line coverage。                                               |
| Rust coverage  | `bun run coverage:rust`                  | full `src-tauri/**/src/*.rs` workspace source surface                                                                                                                                                                 | 100% line + function coverage；舊 quality slice 只保留作 triage helper。                           |
| Browser build  | `bun run build`                          | TypeScript compile + Vite bundle                                                                                                                                                                                      | 已由 `bun run check` 觸發；可單獨跑作 build triage。                                               |
| Browser smoke  | `bun run test:e2e`                       | browser preview 的 shell / onboarding / dashboard / trust / intelligence smoke                                                                                                                                        | 已由 `bun run check` 觸發；這是 preview surface smoke，不等於完整 desktop / Tauri signoff。        |
| Desktop bridge | `bun run test:e2e:desktop-bridge:truth`  | Chrome + Playwright 透過 feature-gated bridge 驗證真實 Rust desktop command façade                                                                                                                                    | 已由 `bun run check` 觸發；仍不是完整 Tauri WebView/plugin signoff。                               |
| JS mutation    | `bun run mutation:js` / `check:mutation` | `src/main.tsx`、`src/lib/ipc/bridge.ts` desktop-contract slice                                                                                                                                                        | Stryker high / low / break thresholds 都是 100；這是 per-commit mutation gate。                    |

### `bun run check` 內含的 targeted sub-gate

`bun run check:base` 目前固定包含 `bun run check:desktop-contract` 與 `bun run check:platform`，保護：

- `src/main.tsx`
- `src/lib/ipc/bridge.ts`
- host-matched platform-native keyring / scheduler / launcher / discovery / biometric smoke
- desktop updater / launcher command surface 與 debug desktop build smoke
- release workflow / Tauri bundle / support-link drift，包含 unsigned Windows installer、WebView2 offline installer、PathKeep updater URL 與不得重新加入 Windows signing gate

這些 sub-gate 的責任是保護 desktop entry、typed IPC contract 與 platform-specific host truth；它們不是替 shell / route / sidebar / trust-critical flows 做全站背書。GitHub `CI` workflow 現在會在 `main` push、PR 與 manual dispatch 時安裝 Linux desktop/native dependencies 後直接跑 `bun run check`，再把 `coverage/js/lcov.info` 與用同一 Rust verifier 口徑產出的 `coverage/rust-codecov.lcov.info` 上傳到 Codecov；所以 hosted runner 也要承擔同一條 per-commit checker；manual `Platform Native` workflow 只保留作 host-sensitive parity / triage。

---

## Current Quality Surfaces

> 2026-04-27 gate-cost note：`bun run check` 本身就是 per-commit gate。`check:full` 只是 `check` alias；`verify` 在 strict checker 之後額外跑 debug desktop build。全量 JS/Rust mutation 因實測成本與 current cargo-mutants sandbox fragility，不再是 per-commit hard gate，改由 `check:deep` / scheduled `Mutation` workflow 承接。

### JS coverage / mutation quality surface

`bun run coverage:js` 目前對齊 active frontend runtime surface：

- include：`src/**/*.{ts,tsx}`
- allowed excludes：tests、fixtures、assets、generated declarations、type-only contract files、以及已證明不是 runtime surface 的 reference-only files。
- required thresholds：coverage lines / functions / branches / statements = 100。

這代表前端 shell / route / sidebar / primitives / page-scoped providers 都回到 checker 裡；不能再用 desktop-contract slice 或舊 living M0-M3 helper list 代替全站 runtime coverage surface。

`bun run mutation:js` 是 per-commit desktop-contract mutation gate，範圍固定為 `src/main.tsx` 與 `src/lib/ipc/bridge.ts`。`bun run mutation:js:full` 仍保留 active frontend runtime surface 的 full Stryker sweep，供 `check:deep`、scheduled workflow 或高風險 release 候選使用；surviving mutant 仍必須用補測、修產品碼、或 narrow equivalent/inapplicable annotation 處理。

### Rust coverage quality surface

`bun run coverage:rust` 現在以 `full` scope 驗證 `src-tauri/**/src/*.rs` 的 100% line + function coverage。舊的 desktop command / bridge contract slice 保留為 `bun run coverage:rust:quality`，只用於縮小 regression 追查範圍，不再是 `coverage:rust` 的預設語義：

- `src-tauri/src/file_manager.rs`
- `src-tauri/src/lib.rs`
- `src-tauri/src/main.rs`
- `src-tauri/src/session.rs`
- `src-tauri/src/worker_bridge/`

如果 full coverage gate 失敗，不能用 quality slice 代替 release signoff；必須把 uncovered path 補測、降出正式 surface，或在 source docs 中明確記錄不能達標的原因與後續 work block。

### Rust mutation quality surface

`bun run mutation:rust` 現在指向 `bun run mutation:rust:full`，也就是 whole-workspace cargo-mutants sweep，但它是 manual / deep gate，不再是 per-commit `bun run check` 的一部分。舊的 focused parser + AI helper contract 保留為 `bun run mutation:rust:quality`，只用於縮小失敗 triage 範圍：

- `browser-history-parser` crate
- `src-tauri/crates/vault-core/src/ai.rs` 的 status/helper slice：
  - `ai_index_status`
  - `ai_queue_status`
  - `reconcile_ai_queue_controls`
  - `provider_capabilities`
  - `provider_connection_failure_report`
  - `test_provider_connection`

whole-workspace mutation 是 deep/release investigation gate；若成本或 surviving mutants 無法在當前 closeout 修乾淨，必須把 surviving mutant 清單與原因寫成明確缺陷，而不是把 focused contract 說成全後端驗收。2026-04-27 實測顯示 full Rust mutation 有 5869 個 candidate mutants，且 current copy-sandbox baseline 會因 repo-root `reference/.../safari.sqlite` fixture path 缺失而失敗；在修復 fixture/copy contract 前，Rust mutation 不可作 per-commit hard gate。

---

## Focused / Release Helpers

| Gate                                   | Command / Workflow                                                                          | 用途                                                                            | 備註                                                                                                                                                                   |
| -------------------------------------- | ------------------------------------------------------------------------------------------- | ------------------------------------------------------------------------------- | ---------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Platform Rust native sweep             | `bun run test:platform:rust` / GitHub `Platform Native` workflow                            | 直接驗證 host keyring / scheduler / launcher / discovery / biometric capability | hosted runner 成本高且 host-sensitive，所以移到 manual workflow；Linux job 仍會在隔離 `dbus-run-session` 內啟動 `gnome-keyring-daemon`。                               |
| Platform desktop slice                 | `bun run test:platform:desktop` / GitHub `Platform Native` workflow                         | debug desktop build + updater / launcher desktop command slice                  | 這是 desktop command truth，不等於 browser preview e2e。                                                                                                               |
| Release config guard                   | `bun run release:check`                                                                     | release workflow / updater URL / Windows bundle config drift                    | 已納入 `check:base`；保護 unsigned Windows release path、WebView2 offline installer、PathKeep support/updater URLs，且不得把 Windows signing gate 加回來。             |
| Project-scoped native dependency proof | `bun run native-deps:doctor` + OpenCC vcpkg install / GitHub `Native Dependencies` workflow | 驗證 repo-local vcpkg native dependency contract                                | 只在 manual dispatch、PR、或 `main` push 觸及 native-deps/vcpkg contract path 時跑；macOS proof 用 `macos-15-intel` + `x64-osx`，不屬於每次 commit 的 strict checker。 |
| Chrome desktop bridge smoke            | `bun run test:e2e:desktop-bridge:truth`                                                     | 啟動 feature-gated desktop bridge，讓 Chrome / Playwright 驗證真實 Rust command | 已納入 `bun run check`；單獨跑只作 bridge triage。                                                                                                                     |
| Desktop-contract JS mutation           | `bun run mutation:js` / `bun run check:mutation`                                            | 對 desktop entry + typed IPC contract 做 lightweight mutation gate              | 已納入 `bun run check`；2026-04-27 current-host wall time 約 50 秒，break threshold 是 100。                                                                           |
| Full JS mutation sweep                 | `bun run mutation:js:full` / GitHub `Mutation` workflow `javascript-mutation`               | 對 active frontend runtime surface 做 repo-level mutation investigation         | Manual / scheduled deep gate；2026-04-27 dry-run 約 2m20s，full sweep 21769 mutants，按 44m/32% 實測估算約 2-3 小時。                                                  |
| Rust mutation sweep                    | `bun run mutation:rust:full` / GitHub `Mutation` workflow `rust-mutation`                   | whole-workspace cargo-mutants sweep                                             | Manual / scheduled deep gate；focused `mutation:rust:quality` 只作 triage helper。                                                                                     |
| Full local sweep                       | `bun run check:full`                                                                        | `bun run check` alias                                                           | 保留給舊 muscle memory；不再是比 `check` 更嚴的 gate。                                                                                                                 |
| Release-style local verification       | `bun run verify`                                                                            | `check` + `desktop:build:debug`                                                 | 作為 release / milestone closeout 的本地預演；會先透過 `check` 自動觸發 coverage、e2e 與 mutation。                                                                    |
| Deep local verification                | `bun run check:deep`                                                                        | `check` + full JS/Rust mutation sweep                                           | 只用於 release candidate / long-running manual pass；不作每次 commit 要求。                                                                                            |

> 2026-04-27 gate-cost decision：舊的 2026-04-08 signed-off parser / AI helper mutation contract 仍是 focused triage helper；不能被誤報成 Rust mutation gate。`WORK-QA-GATE-A` 的當前 truth 以 `STATUS.md` 為準，100% JS/Rust coverage 仍是 stop-ship，full JS/Rust mutation 改為 deep/manual evidence 而不是 per-commit blocker。

---

## Honest Boundaries

- `bun run test:e2e` 是 browser preview smoke，不是 Tauri desktop、worker process、scheduler artifact、keyring 或 filesystem side effect 的最終驗收。
- `bun run test:e2e:desktop-bridge` 證明 Chrome / Playwright 能透過 dev-only localhost bridge 打到真實 desktop command façade，現在也能覆蓋 updater install / relaunch 的 mirrored command transport；但它仍不是完整的 Tauri WebView / plugin guest API signoff，progress events 等 event-driven plugin surface 仍需 Tauri 實機驗證。
- `bun run check:platform` 才是目前對 macOS / Linux host-native scheduler、keyring、launcher 與 updater desktop slice 的 blocking signoff；preview e2e 不能拿來替這些能力背書。
- schedule / security / import / intelligence 這些高風險 surface 的 desktop truth，仍要靠 Rust tests、worker bridge tests、Tauri command tests 與對應的 PME / product docs 對齊。
- `coverage:js` 現在覆蓋 active frontend runtime source；若某個 runtime owner 尚未被測試保護，這是 checker failure，不是文檔例外。
- `coverage:rust` 已恢復 full `src-tauri/**/src/*.rs` 100% gate；如果實際命令失敗，失敗本身就是 release blocker，不能再降回 quality slice 後宣稱全後端達標。
