# Program — Quality Matrix

> 2026-04-07（`WORK-QC-A`）起，這份文檔是 PathKeep 在進入 M4 前的 quality gate source of truth。  
> 原則很簡單：**文檔怎麼寫，repo 就怎麼擋**。凡是會被宣稱成 blocking、release 或 deep check 的驗收，都必須能在 scripts 與 workflow 裡兌現。

---

## Mainline Blocking Path

| Gate          | Command / Workflow      | 保護範圍                                                                                                                                                                                  | 備註                                                                                                                        |
| ------------- | ----------------------- | ----------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | --------------------------------------------------------------------------------------------------------------------------- |
| Repo checks   | `bun run check`         | Prettier、ESLint、i18n parity/raw-English guard、TypeScript、Vitest、desktop contract slice、Rust fmt / clippy / workspace tests、supply-chain audit、host-matched platform-native checks | `check` 本身不含 repo-wide coverage、browser preview e2e 或 full mutation；本地仍會在 matching host 上跑 `check:platform`。 |
| JS coverage   | `bun run coverage:js`   | living M0-M3 JS quality surface                                                                                                                                                           | 100% statement / branch / function / line coverage。                                                                        |
| Rust coverage | `bun run coverage:rust` | full `src-tauri/**/src/*.rs` workspace source surface                                                                                                                                     | 100% line + function coverage；舊 desktop command / bridge slice 保留為 `coverage:rust:quality`。                           |
| Browser build | `bun run build`         | TypeScript compile + Vite bundle                                                                                                                                                          | 保證 browser shell 可建置。                                                                                                 |
| Browser smoke | `bun run test:e2e`      | browser preview 的 shell / onboarding / dashboard / trust / intelligence smoke                                                                                                            | 這是 preview surface smoke，不等於完整 desktop / Tauri signoff。                                                            |

### `bun run check` 內含的 targeted sub-gate

`bun run check` 目前固定包含 `bun run check:desktop-contract` 與 `bun run check:platform`，保護：

- `src/main.tsx`
- `src/lib/ipc/bridge.ts`
- host-matched platform-native keyring / scheduler / launcher / discovery / biometric smoke
- desktop updater / launcher command surface 與 debug desktop build smoke

這些 sub-gate 的責任是保護 desktop entry、typed IPC contract 與 platform-specific host truth；它們不是替 shell / route / sidebar / trust-critical flows 做全站背書。GitHub `CI` workflow 不再把 `check:platform` 掛在每個 PR / branch push 上，因為 hosted runner 的 launchctl / keyring / desktop build 條件與分鐘成本都不穩定；那條真相現在改由 manual `Platform Native` workflow 與本地 matching-host 驗收承接。

---

## Current Quality Surfaces

> 2026-04-24 recovery note：mutation scripts 已重新掛回 release-style gate。`check:full` / `verify` 會跑 coverage、browser-preview e2e 與 JS + Rust mutation；所有 mutation threshold 重新拉到 100。`check:i18n` 會在日常 `check:js` 中量化 catalog key parity 並擋住中文 UI catalog 的 raw backend/debug 英文。

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

`bun run coverage:rust` 現在以 `full` scope 驗證 `src-tauri/**/src/*.rs` 的 100% line + function coverage。舊的 desktop command / bridge contract slice 保留為 `bun run coverage:rust:quality`，只用於縮小 regression 追查範圍，不再是 `coverage:rust` 的預設語義：

- `src-tauri/src/file_manager.rs`
- `src-tauri/src/lib.rs`
- `src-tauri/src/main.rs`
- `src-tauri/src/session.rs`
- `src-tauri/src/worker_bridge/`

如果 full coverage gate 失敗，不能用 quality slice 代替 release signoff；必須把 uncovered path 補測、降出正式 surface，或在 source docs 中明確記錄不能達標的原因與後續 work block。

### Rust mutation quality surface

`bun run mutation:rust` 現在指向 `bun run mutation:rust:full`，也就是 whole-workspace cargo-mutants sweep。舊的 focused parser + AI helper contract 保留為 `bun run mutation:rust:quality`，只用於縮小失敗 triage 範圍：

- `browser-history-parser` crate
- `src-tauri/crates/vault-core/src/ai.rs` 的 status/helper slice：
  - `ai_index_status`
  - `ai_queue_status`
  - `reconcile_ai_queue_controls`
  - `provider_capabilities`
  - `provider_connection_failure_report`
  - `test_provider_connection`

whole-workspace mutation 是 release-style gate；若成本或 surviving mutants 無法在當前 closeout 修乾淨，必須把 surviving mutant 清單與原因寫成明確缺陷，而不是把 focused contract 說成全後端驗收。

---

## Scheduled / Release Gates

| Gate                             | Command / Workflow                                                          | 用途                                                                            | 備註                                                                                                                                                      |
| -------------------------------- | --------------------------------------------------------------------------- | ------------------------------------------------------------------------------- | --------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Platform Rust native sweep       | `bun run test:platform:rust` / GitHub `Platform Native` workflow            | 直接驗證 host keyring / scheduler / launcher / discovery / biometric capability | hosted runner 成本高且 host-sensitive，所以移到 manual workflow；Linux job 仍會在隔離 `dbus-run-session` 內啟動 `gnome-keyring-daemon`。                  |
| Platform desktop slice           | `bun run test:platform:desktop` / GitHub `Platform Native` workflow         | debug desktop build + updater / launcher desktop command slice                  | 這是 desktop command truth，不等於 browser preview e2e。                                                                                                  |
| Chrome desktop bridge smoke      | `bun run test:e2e:desktop-bridge:truth` / GitHub `Platform Native` workflow | 啟動 feature-gated desktop bridge，讓 Chrome / Playwright 驗證真實 Rust command | local / manual / pre-closeout gate；可覆蓋 updater install / relaunch command transport 與 bridge 健康 / disconnect 退化，但不是 mainline blocking path。 |
| JS mutation sweep                | `bun run mutation:js` / GitHub `Mutation` workflow `javascript-mutation`    | 對 living M0-M3 JS quality surface 做 repo-level mutation gate                  | break threshold 是 100；由 `bun run check:mutation` 與 `bun run check:full` 觸發。                                                                        |
| Rust mutation sweep              | `bun run mutation:rust` / GitHub `Mutation` workflow `rust-mutation`        | whole-workspace cargo-mutants sweep                                             | break threshold 以 cargo-mutants surviving mutant = 0 為準；focused `mutation:rust:quality` 只作 triage helper。                                          |
| Full local sweep                 | `bun run check:full`                                                        | 本地一次跑 `check` + coverage + e2e + mutation                                  | 適合大 closeout 或 merge 前自我驗收；不能用只跑 `check` 代替 release-style gate。                                                                         |
| Release-style local verification | `bun run verify`                                                            | `check:full` + `build` + `desktop:build:debug`                                  | 作為 release / milestone closeout 的本地預演；會透過 `check:full` 自動觸發 coverage、e2e 與 mutation。                                                    |

> 2026-04-08 closeout 註記：`bun run check`、`bun run build`、`bun run coverage:js`、`bun run coverage:rust`、`bun run mutation:js`、`bun run test:e2e` 與 `bun run desktop:build:debug` 均已通過；隨後 `WORK-M4-D` 把 Rust mutation baseline 收斂成誠實的 signed-off contract：parser crate + AI status/helper slice。更廣的 `bun run mutation:rust:full` 仍保留給 exploratory whole-workspace triage，不再被誤報成已簽收 gate。

---

## Honest Boundaries

- `bun run test:e2e` 是 browser preview smoke，不是 Tauri desktop、worker process、scheduler artifact、keyring 或 filesystem side effect 的最終驗收。
- `bun run test:e2e:desktop-bridge` 證明 Chrome / Playwright 能透過 dev-only localhost bridge 打到真實 desktop command façade，現在也能覆蓋 updater install / relaunch 的 mirrored command transport；但它仍不是完整的 Tauri WebView / plugin guest API signoff，progress events 等 event-driven plugin surface 仍需 Tauri 實機驗證。
- `bun run check:platform` 才是目前對 macOS / Linux host-native scheduler、keyring、launcher 與 updater desktop slice 的 blocking signoff；preview e2e 不能拿來替這些能力背書。
- schedule / security / import / intelligence 這些高風險 surface 的 desktop truth，仍要靠 Rust tests、worker bridge tests、Tauri command tests 與對應的 PME / product docs 對齊。
- `coverage:js` / `mutation:js` 的 quality surface 仍是 living M0-M3 module set，不等於每個 route/component 都已被 mutation 覆蓋；route-level regressions 需要 product-flow unit tests、browser-preview e2e 與 native desktop truth pass 補上。
- `coverage:rust` 已恢復 full `src-tauri/**/src/*.rs` 100% gate；如果實際命令失敗，失敗本身就是 release blocker，不能再降回 quality slice 後宣稱全後端達標。
