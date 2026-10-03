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

### 測試

- E2E 是證明功能的主要手段（甚至是唯一手段），跑在真實 app 上：Rust 後端 + 前端 + Playwright。不跑 mutation test，也不強制覆蓋率門檻。原則見 [AGENTS.md](../AGENTS.md)。
- 每個 E2E run 都留下可驗證、可重複的 artifact（HTML report、`results.json`、trace / screenshot），位置 `artifacts/e2e/<config>/`。
- Rust 側保留 `cargo test`，用於 E2E 碰不到的失敗模式（例如 crash window）。
- 不在寫完代碼後補 unit test；真的需要隔離測試時，先列出所有可能的失敗方式，再寫代碼。

### 目前的 blocking / release gate

- 現行 gate 以 [docs/plan/program/quality-matrix.md](plan/program/quality-matrix.md) 為準。
- mainline blocking path 是 `bun run check`：format、lint、i18n、typecheck、build、Rust fmt / clippy / test、release-config 檢查、desktop-bridge E2E。
- `bun run check:slow`（supply-chain audit、host-matched platform 測試）與 `bun run verify`（`check` + debug desktop build）用於 release 前。
- release / platform / support 變更除了跑命令，還必須同步維護 `README.md`、`RELEASE.md`、`TESTING.md`、`TROUBLESHOOTING.md`、`SUPPORT.md` 與對應的 `docs/` source docs，不能把 operator contract 留在聊天記錄裡。
- desktop-bridge E2E 不等於完整 Tauri WebView / keyring / scheduler 驗收；那些由 `check:platform` 與真實主機驗收承接。
- route / feature 大改時，要補至少一輪 keyboard / locale smoke review，尤其是設定、排程、安全、導入等高風險頁面。
- 驗證舊產品假設的測試應直接刪除或重寫，不保留作長期 legacy harness。
- 不接受把「目前 typecheck 會紅」「先跳過 E2E 再說」寫成完成狀態。

### 代碼品質

- Rust：clippy + cargo fmt + cargo deny（supply chain audit）。
- JS：ESLint + Prettier + TypeScript strict mode。
- Pre-commit hooks 執行所有 linter 和 formatter。

### CI/CD

- GitHub Actions：
  - `CI` workflow（PR + manual）直接執行 `bun run check`，安裝 Playwright browser 與 Linux desktop/native dependencies，另跑 `check:supply-chain`，並上傳 `artifacts/e2e/` 為 `e2e-artifacts`。
  - `Platform Native` workflow 可保留作 host-sensitive triage / parity，但不能替代 `CI` 的 strict checker。
- Release pipeline：多平台構建 + 自動產出安裝檔。
- Release pipeline 要做 version-sync preflight，並產出 checksum 與 release manifest，避免 tag / artifact / repo version 漂移。
- README badges 顯示 CI 狀態。

## 支援與診斷

- 使用者回報 bug 時，至少要能從 UI 取得 app version、git commit short SHA、資料目錄、archive DB path、audit repo path、scheduler state、keyring backend。
- troubleshooting / support 文檔必須和真實 UI 一起演進；不接受文檔要求使用者提供畫面上根本沒有的資訊。
- 預設支援診斷是 metadata-first：優先收集 run id、audit path、checksum / verify 結果、sanitized screenshot，而不是直接要求 archive DB、raw history export 或秘密值。
- 不可要求使用者分享 master password、API key、S3 secret、完整 archive DB；除非是最小化 repro fixture，而且使用者明確同意。
- support bundle strategy 在當前版本仍是 manual-first，不做自動上傳，也不做使用行為資料收集。

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
- 不做使用行為資料收集或自動診斷上傳；support / bug report 一律由使用者手動整理與分享。
