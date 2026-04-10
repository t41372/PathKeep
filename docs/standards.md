# 品質標準、國際化與社區

> 從 [vision-and-requirements.md](../vision-and-requirements.md) 抽出。

---

## 國際化（i18n）

- 支援語言：英文、簡體中文、繁體中文。
- 自動檢測用戶設備語言，預設選擇匹配的語言。
- 可在設定中手動切換。
- 所有用戶可見的文字都走 i18n，包括錯誤信息和通知。
- 翻譯目錄採 namespace-based 結構（至少區分 `common`、`shell`、`navigation` 與 route / feature namespaces），避免單一巨檔持續膨脹。
- trust-critical flow 必須同步交付 `en` / `zh-CN` / `zh-TW`；不接受靠英語 fallback 混過核心操作、warning、empty state 或修復 CTA。
- 新功能的 shipping contract 也包含 placeholder、aria-label、loading / skeleton label、busy overlay detail、route metadata、browser preview honesty copy；這些都不算「之後再補的 polish」。
- 日期時間、相對時間、檔案大小等格式化要走共用 helper，並跟語系解析規則一起測試。
- route / page 級的 copy 變更至少要補一輪缺 key coverage 檢查，並保留 pseudo-locale 或等價 smoke 來提早發現 layout 溢出。
- 活躍 TSX surface（`src/app/`、`src/components/`、`src/pages/`）由 `src/lib/i18n-literal-guard.test.ts` 擋 raw user-visible literal；新增 UI 文案若需要例外，必須先說明為什麼它不是翻譯內容。
- browser preview fixture 不是 i18n 豁免區；新增或改動 `src/lib/backend.ts` 的 preview / honesty 文案時，也要同步考慮 locale 行為與長字串布局。

---

## 系統信息

- 前端顯示當前版本號和 git commit short SHA。
- 設定頁面中顯示數據存儲目錄，並提供「在文件管理器中打開」的按鈕。

---

## 品質標準

### 測試覆蓋

- 最終標準：
  - Rust 側：100% test coverage + mutation test + integration test。
  - JS/TS 側：100% statement/branch/function/line coverage + mutation test。
- E2E：Playwright spec 覆蓋關鍵用戶流程。

### 目前的 blocking / release gate（2026-04-08，M4 closeout baseline）

- 現行 gate 以 [docs/plan/program/quality-matrix.md](plan/program/quality-matrix.md) 為準。
- mainline blocking path 是：
  - `bun run check`
  - `bun run coverage:js`
  - `bun run coverage:rust`
  - `bun run build`
  - `bun run test:e2e`
- `bun run mutation:js` 已恢復成 living M0-M3 JS surface 的 repo-level mutation gate，但目前放在 scheduled / manual deep check，而不是每次 PR 都同步阻擋。
- `bun run mutation:rust`、`bun run check:full`、`bun run verify` 屬於高成本 deep checks / release checks；它們必須可執行、可追蹤，但不強迫每次小變更都付同樣成本。
- `bun run mutation:rust` 目前代表誠實的 Rust mutation contract：`browser-history-parser` crate，加上 `vault-core/src/ai.rs` 的 AI status/helper slice（`ai_index_status`、`ai_queue_status`、`reconcile_ai_queue_controls`、`provider_capabilities`、`provider_connection_failure_report`、`test_provider_connection`）。
- `bun run mutation:rust:full` 保留作 exploratory whole-workspace cargo-mutants sweep，用來發現新的 hardening backlog 或產出 deferred rationale，而不是假裝整個 Rust workspace 已經被同一條 gate 簽收。
- 2026-04-08 的 release closeout 已確認 `bun run check`、`bun run build`、`bun run coverage:js`、`bun run coverage:rust`、`bun run mutation:js`、`bun run test:e2e` 與 `bun run desktop:build:debug` 通過；`WORK-M4-D` 隨後把 parser / AI misses 收斂回上述 scoped Rust mutation contract，並把 parser `open_readonly` 的 `|`/`^` 等價 mutant 明確記錄在 cargo-mutants config，而不是繼續把它們誤當成真缺測。
- release / platform / support 變更除了跑命令，還必須同步維護 `README.md`、`RELEASE.md`、`TESTING.md`、`TROUBLESHOOTING.md`、`SUPPORT.md` 與對應的 `docs/` source docs，不能把 operator contract 留在聊天記錄裡。
- 所有**新建**或**整段重寫**的模組，仍必須有 colocated tests，並讓該 slice 達到 100% coverage + mutation verification。
- `bun run check` 內含 `bun run check:desktop-contract`；這條 targeted JS sub-gate 只保護 `src/main.tsx` 與 `src/lib/ipc/bridge.ts`。
- 前端 shell / route / sidebar / primitives 與 page-scoped data provider 仍不能被誤報成「因為 desktop contract gate 通過，所以整個 UI 已簽收」。
- browser preview e2e 只代表 preview shell smoke；不等於完整 desktop / worker / filesystem / keyring acceptance。
- route / feature 大改時，除了 unit / contract test，也要補至少一輪 keyboard / locale smoke review，尤其是設定、排程、安全、導入等高風險頁面。
- 驗證舊產品假設的測試應直接刪除或重寫，不保留作長期 legacy harness。
- 不接受把「目前 typecheck 會紅」「先關掉 coverage 再說」寫成完成狀態；即使 deep checks 分層執行，也不代表可以接受失真的驗收敘事。

### 代碼品質

- Rust：clippy + cargo fmt + cargo deny（supply chain audit）。
- JS：ESLint + Prettier + TypeScript strict mode。
- Pre-commit hooks 執行所有 linter 和 formatter。

### CI/CD

- GitHub Actions：
  - mainline CI 直接執行 `check:js`、`check:desktop-contract`、`coverage:js`、`check:rust`、`coverage:rust`、`check:supply-chain`、`build`、`test:e2e`。
  - `mutation:js` 與 `mutation:rust` 由 scheduled / manual `Mutation` workflow 執行，作為 deep check 與 pre-release gate。
  - `check:full`、`verify` 是本地 closeout / release rehearsal 指令，不要求每個 PR 都全跑。
- Release pipeline：多平台構建 + 自動產出安裝檔。
- Release pipeline 要做 version-sync preflight，並產出 checksum 與 release manifest，避免 tag / artifact / repo version 漂移。
- README badges 顯示 CI 狀態、coverage。

## 支援與診斷

- 使用者回報 bug 時，至少要能從 UI 取得 app version、git commit short SHA、資料目錄、archive DB path、audit repo path、scheduler state、keyring backend。
- troubleshooting / support 文檔必須和真實 UI 一起演進；不接受文檔要求使用者提供畫面上根本沒有的資訊。
- 預設支援診斷是 metadata-first：優先收集 run id、audit path、checksum / verify 結果、sanitized screenshot，而不是直接要求 archive DB、raw history export 或秘密值。
- 不可要求使用者分享 master password、API key、S3 secret、完整 archive DB；除非是最小化 repro fixture，而且使用者明確同意。
- support bundle strategy 在當前版本仍是 manual-first，不做自動上傳、不做隱形 telemetry。

---

## 開源與社區

- 協議：GPL v3。
- README：完整的功能介紹、構建指南、從源碼運行指南。
- CONTRIBUTING.md：開發環境設定、測試方式、commit 規範、PR 流程。
- Conventional Commits 規範。

---

## 不做的事情（Explicit Non-Goals）

- 不做雲端同步或雲端存儲（除非用戶主動配置 S3）。
- 不做人格心理分析或敏感維度推斷。
- 不寫回瀏覽器的 live 數據庫。
- 不做背景常駐的 autonomous agent。
- 不做 SaaS 或 subscription model。
- 不做 hidden telemetry。
- 若明確 shipping frontend analytics，必須同時滿足：使用者 opt-in、first-party JSON endpoint、coarse event family、browser preview / dev / test 不送、且 payload 不得包含 archive content、URL、query、profile id、run id、prompt 或 filesystem path。
