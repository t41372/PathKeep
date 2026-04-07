# Program — Repo Baseline

> 這份文檔記錄的是「今天的 repo 實際長什麼樣」，不是我們想像中的目標狀態。  
> 它的作用很簡單：避免後面的計劃建立在錯誤認知上。

---

## 已驗證的基線信號

- [x] `PG-BL-001` 讀完整份 `docs/` 體系，確認新的 vision / features / architecture / design 文檔都已完成重寫。
- [x] `PG-BL-002` 盤點 `src/`、`src-tauri/`、`tests/`、`scripts/`、`.github/workflows/` 的主要入口和模組邊界。
- [x] `PG-BL-003` 執行 `bun run typecheck`，確認目前 TS 型別基線可通過。
- [x] `PG-BL-004` 執行 `bun run test:unit`，確認 Vitest 目前可通過，且新 shell 已有 route-scoped tests 與 focused verification config。
- [x] `PG-BL-005` 執行 `cargo test --manifest-path src-tauri/Cargo.toml --workspace --all-targets --quiet`，確認 Rust 測試基線可通過。
- [x] `PG-BL-006` 執行 `bun run test:e2e`，確認 Playwright smoke 已切到新 shell / onboarding / dashboard，並可通過。（2026-04-06）

---

## 前端基線

### 觀察

- [`src/main.tsx`](../../../src/main.tsx) 已切到 [`src/app/index.tsx`](../../../src/app/index.tsx)，舊 `AppNew` shell 已不在主流程。
- [`src/app/router.tsx`](../../../src/app/router.tsx) 已凍結 10 個 top-level screens、sidebar section、page title / subtitle metadata，並把 onboarding 拆成獨立 shell。
- [`src/styles/tokens.css`](../../../src/styles/tokens.css)、[`src/lib/tokens.ts`](../../../src/lib/tokens.ts) 與 [design-tokens.md](../../design/design-tokens.md) 已形成正式 token layer，取代舊 `App.css`。
- [`src/components/sidebar/`](../../../src/components/sidebar/)、[`src/components/topbar/`](../../../src/components/topbar/)、[`src/components/primitives/`](../../../src/components/primitives/) 已建立新 shell primitives，涵蓋 empty / loading / error / permission gate 等共用狀態。
- route-scoped skeleton 頁面已搬到 [`src/pages/dashboard/index.tsx`](../../../src/pages/dashboard/index.tsx) 等新結構；舊 flat page files 與舊 `AppNew.test.tsx` 已刪除。
- [`src/lib/ipc/bridge.ts`](../../../src/lib/ipc/bridge.ts) 已把 typed IPC wrapper 從 UI preview data 中拆出；[`src/lib/backend.ts`](../../../src/lib/backend.ts) 仍是 legacy / compatibility surface，後續還要繼續瘦身。
- prototype 已成功轉成 production shell 方向，但 route-level deep-link、visual gap list、a11y baseline 仍未全部定稿。

### 判斷

- M0 前端骨架重置已完成，現在的 repo 足以承接 M1 archive UX，而不用再繞回舊 setup-first shell。
- 新前端的主要剩餘風險不再是「入口還在舊結構」，而是 route state contract、preview data 替換成真實 IPC、以及 prototype 缺口補決策。
- `backend.ts` / `app-context.tsx` 已從主 shell critical path 移開，但仍是後續要清掉的 legacy debt，不應重新成為新頁面依賴中心。

### 待辦

- [x] `PG-BL-FE-001` 盤點所有前端檔案，標記為 `delete` / `rewrite` / `reference only` / `keep with refactor`。
- [ ] `PG-BL-FE-002` 產出「prototype 已覆蓋畫面」和「仍需補設計稿畫面」清單。
- [x] `PG-BL-FE-003` 把 browser-preview mock data 從正式 IPC contract 中拆開，避免新前端繼續依賴假資料模型。
- [x] `PG-BL-FE-004` 定義新的 route tree、sidebar IA、page title / breadcrumb 規範、global search entry 規範。
- [x] `PG-BL-FE-005` 為新的 page / component / test 結構建立 naming convention，避免 `AppNew` 這種 legacy placeholder 命名長期留在主幹。

---

## 後端與資料平面基線

### 觀察

- [`src-tauri/crates/vault-core/src/archive/mod.rs`](../../../src-tauri/crates/vault-core/src/archive/mod.rs) 目前仍承擔 backup orchestration、history query、export、rekey、doctor 等多項責任，但 M1-A 已把主 runtime 切到 canonical `runs` / `source_profiles` / `urls` / `visits` / `downloads` / `search_terms` / `favicons` 寫入面。
- [`src-tauri/crates/vault-core/src/chrome.rs`](../../../src-tauri/crates/vault-core/src/chrome.rs) 1229 行，實際上不只 Chrome discovery，還包含 Firefox / Safari discovery、staging copy、path heuristics。
- [`src-tauri/crates/vault-core/src/ai.rs`](../../../src-tauri/crates/vault-core/src/ai.rs) 1916 行、[`src-tauri/crates/vault-core/src/insights.rs`](../../../src-tauri/crates/vault-core/src/insights.rs) 2481 行，很多 intelligence 相關邏輯已經提前塞進 canonical SQLite 旁邊。
- Rust workspace 現在已有 `browser-history-parser` crate，且 `vault-core` 已用它接通 Chromium、Firefox 與 Safari baseline backup ingest；後續重點已從「多瀏覽器是否存在」轉成 parser 深度、capability caveat 與 fixture 擴充。
- canonical schema v1 + v2 runtime foundation 已落在 [`migrations/001_initial.sql`](../../../src-tauri/crates/vault-core/src/migrations/001_initial.sql) 與 [`migrations/002_archive_runtime_foundation.sql`](../../../src-tauri/crates/vault-core/src/migrations/002_archive_runtime_foundation.sql)；archive init 已統一走 migration executor。
- [`src-tauri/crates/vault-core/src/archive/schema.rs`](../../../src-tauri/crates/vault-core/src/archive/schema.rs) 現在同時承擔 migration runtime、runtime backfill，以及 `profiles` / `visit_events` compatibility views，供 AI / insights / takeout 過渡期繼續工作。
- [`src-tauri/src/lib.rs`](../../../src-tauri/src/lib.rs) 暴露了很多 Tauri commands，但命令集合和命名仍然緊貼舊 UI 與舊產品假設。
- [`src-tauri/crates/vault-worker/src/lib.rs`](../../../src-tauri/crates/vault-worker/src/lib.rs) 1577 行，兼任了 desktop orchestration、CLI worker、MCP server、keyring / schedule bridge 等多個角色。
- [`src-tauri/crates/vault-platform/src/lib.rs`](../../../src-tauri/crates/vault-platform/src/lib.rs) 已具備 macOS preview / manual / apply 與 Windows / Linux preview / manual schedule surface，Linux timer contract 也已明確切到 `OnCalendar=` + `Persistent=true`。
- Dashboard / Audit / Explorer 的第一批 read models 已存在於 [`src-tauri/crates/vault-core/src/models.rs`](../../../src-tauri/crates/vault-core/src/models.rs) 與 [`src-tauri/crates/vault-core/src/archive/mod.rs`](../../../src-tauri/crates/vault-core/src/archive/mod.rs)，包含 `DashboardSnapshot`、`AuditRunDetail`、`StorageSummary` 與擴充後的 `HistoryQuery`。

### 判斷

- M1-A 已把「如何讓 archive engine 正式切到 canonical runtime」這個主要風險拿掉；M2-A 又把多瀏覽器 ingest、Takeout rollback / restore、doctor repair 基線接通。現在最大的後端風險變成巨型模組拆分、Safari capability caveat 管理，以及 UI 端如何接好完整 PME contract。
- `browser-history-parser` 已不只是 foundation，而是被 `vault-core` 真正消費的 runtime 依賴；下一步要擴的是 richer Firefox / Safari metadata、Takeout / browser fixture 深度與 parser coverage，而不是再回頭討論 parser 是否存在。
- legacy surface 並沒有完全消失，但它已退到 compatibility bridge。接下來應避免再把新功能寫回 `visit_events` / `profiles` 這類舊名稱。

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
- [`tests/e2e/shell.spec.ts`](../../../tests/e2e/shell.spec.ts) 已改成驗證新 shell、Review onboarding 入口與 dashboard preview，Playwright smoke 可通過。
- `vitest.desktop-contract.config.ts` 與 `stryker.desktop-contract.config.json` 已建立 desktop contract verification，讓 `src/main.tsx` / `src/lib/ipc/bridge.ts` 這條非前端 contract slice 可獨立做到 100% coverage + mutation。
- README、release workflow、Tauri metadata、app-facing strings 已切到 PathKeep。
- [standards.md](../../standards.md)、[AGENTS.md](../../../AGENTS.md)、M0 planning docs 現在都明確區分：repo-wide deep checks 暫不擋主線，但新碼 / 重寫 slice 仍要求 100% coverage + mutation verification。

### 判斷

- 現在的 smoke 與 targeted verification 已開始保護新產品骨架，而不是再被舊 setup shell 反向綁住。
- M0 的品質規則已從「一刀切 repo-wide coverage gate」調整成「新碼 slice 必須完整驗證」；這比較符合重寫期現實。
- 下一步真正還缺的是更完整的 test taxonomy / ownership，而不是繼續修正舊 smoke 目標。

### 待辦

- [ ] `PG-BL-QA-001` 定義新的 test pyramid 和 ownership，明確哪些測試屬於 parser、archive、worker、frontend、e2e。
- [x] `PG-BL-QA-002` 重寫 Playwright smoke 目標，從舊 setup shell 改成新 app shell / onboarding / dashboard smoke。
- [x] `PG-BL-QA-003` 定義重寫期 quality policy：repo-wide coverage / mutation 暫時不擋主線，但新碼與整段重寫模組仍必須達到 100% coverage + mutation verification。
- [x] `PG-BL-QA-004` 盤點 mutation test 現況和成本，先支持 targeted verification，再決定何時恢復整倉 sweep。
- [x] `PG-BL-QA-005` 重寫 README / release workflow 文案，使其描述和新產品定位一致。

---

## 命名遷移基線

### 觀察

- `package.json` 已切到 `pathkeep`，Tauri `productName` / window title / identifier 也已改成 PathKeep。
- README、CONTRIBUTING、release workflow、frontend public strings、Tauri product strings 與大部分 worker / platform 文案都已切到 PathKeep。
- repo 內仍有少量 `Chrome History Backup` / `Chrome History Vault` 殘留，但它們現在只出現在 explicit legacy alias / migration comment，用於升級與資料恢復敘事，而不是現行品牌。

### 判斷

- M0 的 rename sweep 對 public surface 與 build metadata 已足夠完成；剩餘舊名字串是受控的 legacy reference，不代表命名遷移未完成。
- 因為沒有正式用戶，repo 不需要為舊品牌維持長期兼容窗口；接下來只要避免在新功能把舊名字重新帶回來即可。

### 待辦

- [x] `PG-BL-NM-001` 列出所有對外可見名稱、內部 service name、filesystem path name、bundle id、schedule label、MCP skill 名稱。
- [x] `PG-BL-NM-002` 標記哪些舊名字串應直接清除，哪些確實與資料恢復相關、需要被保留到對應重寫完成。
- [x] `PG-BL-NM-003` 建立一次性 rename cleanup checklist，確保命名清理不遺漏 build metadata、資料目錄與 automation artifact。
