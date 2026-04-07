# Program — Repo Baseline

> 這份文檔記錄的是「今天的 repo 實際長什麼樣」，不是我們想像中的目標狀態。  
> 它的作用很簡單：避免後面的計劃建立在錯誤認知上。

---

## 已驗證的基線信號

- [x] `PG-BL-001` 讀完整份 `docs/` 體系，確認新的 vision / features / architecture / design 文檔都已完成重寫。
- [x] `PG-BL-002` 盤點 `src/`、`src-tauri/`、`tests/`、`scripts/`、`.github/workflows/` 的主要入口和模組邊界。
- [x] `PG-BL-003` 執行 `bun run typecheck`，確認目前 TS 型別基線可通過。
- [x] `PG-BL-004` 執行 `bun run test:unit`，確認 Vitest 目前可通過，8 個 test files / 142 tests。
- [x] `PG-BL-005` 執行 `cargo test --manifest-path src-tauri/Cargo.toml --workspace --all-targets --quiet`，確認 Rust 測試基線可通過。
- [x] `PG-BL-006` 執行 `bun run test:e2e`，確認 Playwright 目前失敗在舊 shell 斷言，不是環境起不來。

---

## 前端基線

### 觀察

- [`src/main.tsx`](../../../src/main.tsx) 仍然直接載入 [`src/AppNew.tsx`](../../../src/AppNew.tsx)。
- [`src/AppNew.tsx`](../../../src/AppNew.tsx) 仍把 onboarding、dashboard、explorer、insights、activity、import、settings 綁在同一個 context-driven shell 裡，資訊架構和新 prototype 不一致。
- [`src/App.css`](../../../src/App.css) 1880 行、[`src/AppNew.test.tsx`](../../../src/AppNew.test.tsx) 3081 行、[`src/lib/i18n.ts`](../../../src/lib/i18n.ts) 1547 行，都已經不是舒服的小步演進狀態。
- [`src/lib/backend.ts`](../../../src/lib/backend.ts) 同時承擔 Tauri IPC、browser preview fixture、舊產品名稱、舊 app path、假資料模型和部分 UI 語意。
- [`src/lib/app-context.tsx`](../../../src/lib/app-context.tsx) 把全域狀態、導航、設定草稿、session key、provider secrets、業務動作都收在一個 context 裡，這對新 shell 不是理想起點。
- 目前頁面仍是 [`src/pages/dashboard.tsx`](../../../src/pages/dashboard.tsx)、[`src/pages/explorer.tsx`](../../../src/pages/explorer.tsx)、[`src/pages/insights.tsx`](../../../src/pages/insights.tsx)、[`src/pages/activity-log.tsx`](../../../src/pages/activity-log.tsx)、[`src/pages/import.tsx`](../../../src/pages/import.tsx)、[`src/pages/onboarding.tsx`](../../../src/pages/onboarding.tsx) 和舊 settings 子頁。
- prototype 目前是單一 HTML/CSS/JS 原型，確實定義了 9 個主要畫面，但還沒有 production 級的 onboarding 細節、empty states、error states、locale variants 規格。

### 判斷

- 舊前端不適合繼續逐頁修。需要先在 M0 建立新 shell、route tree、layout、token 和 page contract，再開始做具體頁面。
- 舊 [`src/AppNew.test.tsx`](../../../src/AppNew.test.tsx) 的價值主要是當舊 UI 行為樣本，不應繼續被當成新產品的主要 regression harness。
- 新 UI 開始前，必須先明確區分「可復用的底層工具」和「應該直接淘汰的舊頁面與舊 shell」。

### 待辦

- [ ] `PG-BL-FE-001` 盤點所有前端檔案，標記為 `delete` / `rewrite` / `reference only` / `keep with refactor`。
- [ ] `PG-BL-FE-002` 產出「prototype 已覆蓋畫面」和「仍需補設計稿畫面」清單。
- [ ] `PG-BL-FE-003` 把 browser-preview mock data 從正式 IPC contract 中拆開，避免新前端繼續依賴假資料模型。
- [ ] `PG-BL-FE-004` 定義新的 route tree、sidebar IA、page title / breadcrumb 規範、global search entry 規範。
- [ ] `PG-BL-FE-005` 為新的 page / component / test 結構建立 naming convention，避免 `AppNew` 這種 legacy placeholder 命名長期留在主幹。

---

## 後端與資料平面基線

### 觀察

- [`src-tauri/crates/vault-core/src/archive/mod.rs`](../../../src-tauri/crates/vault-core/src/archive/mod.rs) 目前仍承擔 backup orchestration、history query、export、rekey、doctor、Firefox ingest、Safari ingest、favicon ingest 等多項責任，但 schema bootstrapping 已先抽到 [`src-tauri/crates/vault-core/src/archive/schema.rs`](../../../src-tauri/crates/vault-core/src/archive/schema.rs)。
- [`src-tauri/crates/vault-core/src/chrome.rs`](../../../src-tauri/crates/vault-core/src/chrome.rs) 1229 行，實際上不只 Chrome discovery，還包含 Firefox / Safari discovery、staging copy、path heuristics。
- [`src-tauri/crates/vault-core/src/ai.rs`](../../../src-tauri/crates/vault-core/src/ai.rs) 1916 行、[`src-tauri/crates/vault-core/src/insights.rs`](../../../src-tauri/crates/vault-core/src/insights.rs) 2481 行，很多 intelligence 相關邏輯已經提前塞進 canonical SQLite 旁邊。
- Rust workspace 現在已有 `browser-history-parser` crate，但 `vault-core` 尚未全面切換成它的消費者；目前仍處於 M0 foundation 階段。
- canonical schema v1 已有 [`migrations/001_initial.sql`](../../../src-tauri/crates/vault-core/src/migrations/001_initial.sql) 和 migration executor；舊 [`archive-schema.sql`](../../../src-tauri/crates/vault-core/src/archive-schema.sql) 只剩 legacy runtime bridge 角色。
- [`src-tauri/src/lib.rs`](../../../src-tauri/src/lib.rs) 暴露了很多 Tauri commands，但命令集合和命名仍然緊貼舊 UI 與舊產品假設。
- [`src-tauri/crates/vault-worker/src/lib.rs`](../../../src-tauri/crates/vault-worker/src/lib.rs) 1577 行，兼任了 desktop orchestration、CLI worker、MCP server、keyring / schedule bridge 等多個角色。

### 判斷

- 問題不只是「功能還不完整」，而是**很多功能已經先長錯地方**。如果 M0 不先重切 module boundary，之後每個 feature 都還會繼續把複雜度堆在巨檔裡。
- `browser-history-parser` 已經作為正式 workstream 開始，但還需要在 M1 把更多 Firefox / Safari / Takeout parsing 和 `vault-core` 消費鏈接上。
- migration system 已在 M0 foundation 落地；接下來的風險轉成「如何讓 M1 archive engine 正式切換到 canonical runtime」，而不是「還沒有 migration ledger」。

### 待辦

- [ ] `PG-BL-BE-001` 建立 `vault-core` 模組責任地圖，標記哪些函數會搬到 parser crate、哪些留在 archive plane、哪些移到 worker / platform。
- [ ] `PG-BL-BE-002` 建立現有 Tauri commands 和未來 UI use cases 的對照表。
- [ ] `PG-BL-BE-003` 盤點目前已存在的 AI / insight / enrichment 表和衍生狀態，標記哪些應留在 canonical SQLite、哪些應改為 sidecar / derived-state。
- [ ] `PG-BL-BE-004` 盤點現有 archive schema 和 [data-model.md](../../architecture/data-model.md)、[archive.md](../../features/archive.md) 之間的差距，形成正式 gap table。
- [ ] `PG-BL-BE-005` 盤點目前的 takeout / rollback / doctor / schedule / remote backup 行為是否和新的 PME / Trust 原則一致。

---

## 品質、測試與發版基線

### 觀察

- `typecheck`、Vitest 和 Rust 測試都通過，說明 repo 目前有穩定的可執行基線。
- [`tests/e2e/shell.spec.ts`](../../../tests/e2e/shell.spec.ts) 仍然驗證舊 setup shell 的 heading 和文案，因此 Playwright 失敗代表「驗收目標過期」，不是環境本身壞掉。
- README 和 workflow 仍大量描述「Browser History Backup」而不是 PathKeep，也還在宣稱一些舊產品敘事和已完成度。
- [standards.md](../../standards.md) 的最終標準是 100% coverage + mutation，但重寫期需要把「repo-wide gate」和「新碼 slice 標準」分開寫清楚，否則只會讓舊架構繼續佔據主線。
- `.github/workflows/ci.yml`、`release.yml`、`mutation.yml` 已存在，但 release 文案和 asset naming 都還停留在舊名稱。

### 判斷

- 現有 tests 的價值是保留現有行為樣本，但不能直接當作新產品的 acceptance criteria。
- M0 必須先重建 test taxonomy：shell smoke、workflow integration、parser fixture tests、archive engine tests、rollback / doctor tests、AI optional tests。
- release / README / badge / bundle 文案都需要和產品 pivot 同步，這不能留到最後才碰。

### 待辦

- [ ] `PG-BL-QA-001` 定義新的 test pyramid 和 ownership，明確哪些測試屬於 parser、archive、worker、frontend、e2e。
- [ ] `PG-BL-QA-002` 重寫 Playwright smoke 目標，從舊 setup shell 改成新 app shell / onboarding / dashboard smoke。
- [ ] `PG-BL-QA-003` 定義重寫期 quality policy：repo-wide coverage / mutation 暫時不擋主線，但新碼與整段重寫模組仍必須達到 100% coverage + mutation verification。
- [ ] `PG-BL-QA-004` 盤點 mutation test 現況和成本，先支持 targeted verification，再決定何時恢復整倉 sweep。
- [ ] `PG-BL-QA-005` 重寫 README / release workflow 文案，使其描述和新產品定位一致。

---

## 命名遷移基線

### 觀察

- `package.json` 的 package name 仍是 `browser-history-backup`。
- [`src-tauri/tauri.conf.json`](../../../src-tauri/tauri.conf.json) 的 `productName`、window title、identifier 仍是舊名字。
- README 標題已經叫 PathKeep，但正文仍寫 `Browser History Backup is a local-first desktop app...`，品牌敘事是混雜的。
- 前端 mock、i18n、backend fixtures、Tauri product string、keyring / schedule 文案、export 標題、AI / MCP skill 文案等仍保留大量舊名字串。
- `Chrome History Backup` / `Chrome History Vault` 仍作為 legacy app name 殘留在 config / platform guidance 中。

### 判斷

- 這不是單純的 rename task，而是產品定位切換的一部分；因為沒有正式用戶，M0 應直接做乾淨切換，不需要設計長期兼容窗口。

### 待辦

- [ ] `PG-BL-NM-001` 列出所有對外可見名稱、內部 service name、filesystem path name、bundle id、schedule label、MCP skill 名稱。
- [ ] `PG-BL-NM-002` 標記哪些舊名字串應直接清除，哪些確實與資料恢復相關、需要被保留到對應重寫完成。
- [ ] `PG-BL-NM-003` 建立一次性 rename cleanup checklist，確保命名清理不遺漏 build metadata、資料目錄與 automation artifact。
