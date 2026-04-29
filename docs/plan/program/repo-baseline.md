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
- prototype 已成功轉成 production shell 方向；route-level deep-link、prototype gap list 與 non-prototype state coverage 現在都已有 source docs，剩餘的是全站 accessibility / release polish。

### 判斷

- M0 前端骨架重置已完成，現在的 repo 足以承接 M1 archive UX，而不用再繞回舊 setup-first shell。
- 新前端的主要剩餘風險不再是「入口還在舊結構」或 prototype gap 未定義，而是 route state contract、preview data 替換成真實 IPC，以及 release-level desktop truth / accessibility signoff。
- `backend.ts` / `app-context.tsx` 已從主 shell critical path 移開，但仍是後續要清掉的 legacy debt，不應重新成為新頁面依賴中心。

### 待辦

- [x] `PG-BL-FE-001` 盤點所有前端檔案，標記為 `delete` / `rewrite` / `reference only` / `keep with refactor`。
- [x] `PG-BL-FE-002` 產出「prototype 已覆蓋畫面」和「仍需補設計稿畫面」清單。見 [screens-and-nav.md](../../design/screens-and-nav.md) 的 `Prototype Coverage Snapshot`。（2026-04-07，`WORK-QC-B`）
- [x] `PG-BL-FE-003` 把 browser-preview mock data 從正式 IPC contract 中拆開，避免新前端繼續依賴假資料模型。
- [x] `PG-BL-FE-004` 定義新的 route tree、sidebar IA、page title / breadcrumb 規範、global search entry 規範。
- [x] `PG-BL-FE-005` 為新的 page / component / test 結構建立 naming convention，避免 `AppNew` 這種 legacy placeholder 命名長期留在主幹。

---

## 後端與資料平面基線

### 觀察

- [`src-tauri/crates/vault-core/src/archive/mod.rs`](../../../src-tauri/crates/vault-core/src/archive/mod.rs) 目前仍承擔 backup orchestration、history query、export、rekey、doctor 等多項責任，但 M1-A 已把主 runtime 切到 canonical `runs` / `source_profiles` / `urls` / `visits` / `downloads` / `search_terms` / `favicons` 寫入面。
- [`src-tauri/crates/vault-core/src/intelligence/mod.rs`](../../../src-tauri/crates/vault-core/src/intelligence/mod.rs) 9152 行、[`src-tauri/crates/vault-core/src/intelligence_runtime.rs`](../../../src-tauri/crates/vault-core/src/intelligence_runtime.rs) 2172 行、[`src-tauri/crates/vault-core/src/ai.rs`](../../../src-tauri/crates/vault-core/src/ai.rs) 2116 行，很多 intelligence 相關邏輯仍集中在少數主檔。
- Rust workspace 現在已有 `browser-history-parser` crate，且 `vault-core` 已用它接通 Chromium、Firefox 與 Safari baseline backup ingest；後續重點已從「多瀏覽器是否存在」轉成 parser 深度、capability caveat 與 fixture 擴充。
- canonical schema v1 + v2 runtime foundation 已落在 [`migrations/001_initial.sql`](../../../src-tauri/crates/vault-core/src/migrations/001_initial.sql) 與 [`migrations/002_archive_runtime_foundation.sql`](../../../src-tauri/crates/vault-core/src/migrations/002_archive_runtime_foundation.sql)；archive init 已統一走 migration executor。
- [`src-tauri/crates/vault-core/src/archive/schema.rs`](../../../src-tauri/crates/vault-core/src/archive/schema.rs) 現在已退回 canonical archive bootstrap；search / intelligence plane 不再靠 archive-side compatibility views 或 runtime backfill 存活。
- [`src-tauri/src/lib.rs`](../../../src-tauri/src/lib.rs) 暴露了很多 Tauri commands，但命令集合和命名仍然緊貼舊 UI 與舊產品假設。
- [`src-tauri/crates/vault-worker/src/lib.rs`](../../../src-tauri/crates/vault-worker/src/lib.rs) 與 [`src-tauri/crates/vault-core/src/chrome.rs`](../../../src-tauri/crates/vault-core/src/chrome.rs) 已在後續 work blocks 裡大幅瘦身；它們仍是重要邊界，但不再是 repo 目前最大的 mega-file hotspot。
- [`src-tauri/crates/vault-platform/src/lib.rs`](../../../src-tauri/crates/vault-platform/src/lib.rs) 已具備 macOS preview / manual / apply 與 Windows / Linux preview / manual schedule surface，Linux timer contract 也已明確切到 `OnCalendar=` + `Persistent=true`，並透過較小 calendar 步長 + worker `--due-only` 支援分鐘級自訂備份間隔。
- Dashboard / Audit / Explorer 的第一批 read models 已存在於 [`src-tauri/crates/vault-core/src/models.rs`](../../../src-tauri/crates/vault-core/src/models.rs) 與 [`src-tauri/crates/vault-core/src/archive/mod.rs`](../../../src-tauri/crates/vault-core/src/archive/mod.rs)，包含 `DashboardSnapshot`、`AuditRunDetail`、`StorageSummary` 與擴充後的 `HistoryQuery`。

### 判斷

- M1-A 已把「如何讓 archive engine 正式切到 canonical runtime」這個主要風險拿掉；M2-A 又把多瀏覽器 ingest、Takeout rollback / restore、doctor repair 基線接通。現在最大的後端風險變成巨型模組拆分、Safari capability caveat 管理，以及 UI 端如何接好完整 PME contract。
- `browser-history-parser` 已不只是 foundation，而是被 `vault-core` 真正消費的 runtime 依賴；下一步要擴的是 richer Firefox / Safari metadata、Takeout / browser fixture 深度與 parser coverage，而不是再回頭討論 parser 是否存在。
- legacy surface 並沒有完全消失，但它已退到 compatibility bridge。接下來應避免再把新功能寫回 `visit_events` / `profiles` 這類舊名稱。

### Current Boundary Gap Table（2026-04-09 / `WORK-QC-C`）

| Hotspot / current file                                                | 現在實際承擔什麼                                                                        | 目標邊界                                            | 目前 truth stance                                                                                                                        |
| --------------------------------------------------------------------- | --------------------------------------------------------------------------------------- | --------------------------------------------------- | ---------------------------------------------------------------------------------------------------------------------------------------- |
| `vault-core/src/chrome.rs`                                            | browser discovery、path heuristics、staging copy、provider glue                         | discovery / staging 最終應收斂到 `vault-platform`   | accepted legacy hotspot；M1 / M2 已可用，但這個檔案不應再接新責任。                                                                      |
| `vault-core/src/archive/{mod,schema}.rs`                              | canonical schema、backup ingest、query、doctor、rekey、storage summary                  | 維持在 `vault-core`                                 | shipping canonical domain；`archive/mod.rs` 仍是 mega-file，但其責任本身是正確的。                                                       |
| `vault-worker/src/lib.rs`                                             | orchestration、desktop read models、CLI、MCP、AI queue / intelligence bridge            | 保持 orchestration / bridge                         | accepted worker hotspot；不要把新的 Tauri naming 或 UI 文案直接塞回 worker。                                                             |
| `src-tauri/src/{lib,worker_bridge,session}.rs`                        | Tauri command façade、session bridge、desktop refusal path                              | 保持 desktop-only façade                            | signed-off desktop surface；這層受 `coverage:rust` 直接保護。                                                                            |
| `browser-history-parser/src/*`                                        | provided-path inspection、row parsing、warning surface                                  | 保持 parser-only crate                              | signed-off parser surface；這層受 parser tests + Rust mutation contract 保護。                                                           |
| `vault-core/src/{ai,intelligence/mod,intelligence_runtime,remote}.rs` | derived-state intelligence、queue/runtime、remote bundle / verify、advanced read models | canonical metadata + derived sidecar / bundle logic | Core Intelligence living hotspot；`WORK-CI-C` 已刪除 legacy `insights.rs`，接下來不應再把 snapshot-era contract 或 module alias 塞回來。 |

### Current Archive Schema Gap Table（2026-04-09 / `WORK-QC-C`）

| Surface                              | 現況                                                                                                                                                                   | Truthful boundary                                                                                    |
| ------------------------------------ | ---------------------------------------------------------------------------------------------------------------------------------------------------------------------- | ---------------------------------------------------------------------------------------------------- |
| Canonical core tables                | `001_initial.sql` 已落地 `runs` / `source_profiles` / `urls` / `visits` / `downloads` / `search_terms` / `favicons` / `manifests` / `snapshots` / `settings`           | 這是目前正式 source of truth。                                                                       |
| Runtime helper / compatibility layer | `002_archive_runtime_foundation.sql` 增加 `profile_watermarks`、`import_batches`、compat views / triggers                                                              | 這些是 accepted adjunct / transition surface，不是新的 canonical 命名方向。                          |
| Search projection                    | `003_history_search_fts.sql` 導入 FTS5 `history_search` projection                                                                                                     | 這是 rebuildable derived state，不是 canonical source table。                                        |
| Legacy DB 升級 harness               | fresh init、migration replay、checksum drift 已有；legacy-to-new one-shot conversion 仍無正式 harness                                                                  | deferred with rationale：目前沒有正式 legacy user base，shipping path 以 fresh canonical init 為主。 |
| Snapshot restore preview / execute   | Audit 現在可對 saved raw-source checkpoint 做 preview / replay，並留下 `snapshot_restore` run / manifest / artifact；rekey 的 archive safety snapshot 也有 review path | shipping 的是 checkpoint replay restore；若 archive-file snapshot 需要舊 key，仍維持 manual-first。  |
| Approval / manual-step 寫入 `runs`   | `runs` 已記錄 trigger / timezone / warnings / stats / artifacts                                                                                                        | approval reason / manual intervention 仍主要停在 preview artifact 和 UI copy，尚未成為 schema 欄位。 |

### Current Intelligence / Derived-State Boundary Table（2026-04-09 / `WORK-QC-D`）

| Surface                               | 目前位置 / 形式                                   | Truthful boundary                                                                   |
| ------------------------------------- | ------------------------------------------------- | ----------------------------------------------------------------------------------- |
| `ai_jobs`, `ai_index_ledger`          | canonical archive SQLite                          | durable operational metadata；不是 canonical history facts                          |
| `ai_embeddings`                       | `derived/history-intelligence.sqlite`             | compact semantic metadata / rebuild accounting；不是 vector payload source of truth |
| LanceDB semantic tables               | app data directory sidecar                        | primary ANN / vector store；整個 sidecar 可刪除後重建                               |
| enrichment / topics / threads / cards | canonical archive SQLite 的 derived tables        | rebuildable derived state；clear / rebuild 不可觸動 canonical visits                |
| MCP / skill integration preview files | preview payload only（command + JSON / markdown） | manual-copy artifact；PathKeep 不會在 preview 時偷偷安裝外部工具設定                |
| remote backup bundles                 | zip artifact + manifest / checksum                | review-first portability artifact；不是 live source of truth                        |
| `history_search` FTS projection       | SQLite FTS5 rebuildable projection                | fast-path search index；不是 canonical archive table                                |

### 待辦

- [x] `PG-BL-BE-001` 建立 `vault-core` 模組責任地圖，標記哪些函數會搬到 parser crate、哪些留在 archive plane、哪些移到 worker / platform。（2026-04-09，`WORK-QC-C`：見上方 `Current Boundary Gap Table` 與 [../../architecture/module-boundary-map.md](../../architecture/module-boundary-map.md)）
- [x] `PG-BL-BE-002` 建立現有 Tauri commands 和未來 UI use cases 的對照表。（2026-04-09，`WORK-QC-C`：見 [../../architecture/desktop-command-surface.md](../../architecture/desktop-command-surface.md) 的 `Implemented Command Map`）
- [x] `PG-BL-BE-003` 盤點目前已存在的 AI / insight / enrichment 表和衍生狀態，標記哪些應留在 canonical SQLite、哪些應改為 sidecar / derived-state。（2026-04-09，`WORK-QC-D`：見上方 `Current Intelligence / Derived-State Boundary Table`）
- [x] `PG-BL-BE-004` 盤點現有 archive schema 和 [data-model.md](../../architecture/data-model.md)、[archive.md](../../features/archive.md) 之間的差距，形成正式 gap table。（2026-04-09，`WORK-QC-C`：見上方 `Current Archive Schema Gap Table`）
- [x] `PG-BL-BE-005` 盤點目前的 takeout / rollback / doctor / schedule / remote backup 行為是否和新的 PME / Trust 原則一致。（2026-04-09，`WORK-QC-C`：M1-DB / M1-OPS acceptance matrices 已補齊；remote backup truth matrix 保留到後續 `WORK-QC-D`）

---

## 品質、測試與發版基線

### 觀察

- `typecheck`、Vitest 和 Rust 測試都通過，說明 repo 目前有穩定的可執行基線。
- [`tests/e2e/shell.spec.ts`](../../../tests/e2e/shell.spec.ts) 已改成驗證新 shell、Review onboarding 入口與 dashboard preview，Playwright smoke 可通過。
- `vitest.desktop-contract.config.ts` 與 `stryker.desktop-contract.config.json` 已建立 desktop contract verification，讓 `src/main.tsx` / `src/lib/ipc/bridge.ts` 這條非前端 contract slice 可獨立做到 100% coverage + mutation。
- `vitest.quality.config.ts` 已把 `coverage:js` 收斂到 living M0-M3 JS quality surface；`coverage:rust` 也已有明確的 Tauri desktop command / bridge quality scope，而不是再用失真的 repo-wide 敘事。
- `mutation:js` 已恢復為 living M0-M3 JS surface 的 repo-level mutation sweep，`Mutation` workflow 則把 JS / Rust mutation 都收回到 scheduled / manual deep check。
- README、release workflow、Tauri metadata、app-facing strings 已切到 PathKeep。
- M4-L 又把 updater `latest.json` / signatures、`bun run release:bump -- <semver>`、`bun run release:size-audit` 與 `com.yi-ting.pathkeep` clean-break namespace 收進正式 release contract。
- [quality-matrix.md](quality-matrix.md)、[standards.md](../../standards.md)、[AGENTS.md](../../../AGENTS.md)、CI workflows 現在已對齊同一套 blocking / release gate 說法。

### 判斷

- 現在的 gate 不再只剩 desktop contract slice 與 browser smoke；living M0-M3 quality surface 的 coverage 與 deep-check 分層已恢復到可兌現狀態。
- repo 目前至少已回到「文檔怎麼寫，scripts / workflows 就怎麼擋」的程度，不再需要靠口頭補充來解釋哪些 gate 其實沒開。
- QC-B 已把 product / design / doc parity 與 preview-vs-desktop 邊界收回 source docs；`WORK-QC-C` 又補齊 test taxonomy / ownership。下一步真正還缺的是 release-style desktop signoff，以及 M4 的 enrichment / remote / intelligence closeout。

### Test Pyramid And Ownership（2026-04-09 / `WORK-QC-C`）

| Layer / owner                                 | 主要驗收入口                                                               | 備註                                                                                      |
| --------------------------------------------- | -------------------------------------------------------------------------- | ----------------------------------------------------------------------------------------- |
| Parser / fixture owner                        | `cargo test -p browser-history-parser`                                     | 專注 provided-path parsing；由 Rust mutation contract 補強。                              |
| Archive / import / doctor owner               | `cargo test -p vault-core`                                                 | 驗證 canonical ingest、takeout rollback / restore、doctor / repair、remote bundle logic。 |
| Worker / platform / schedule / security owner | `cargo test -p vault-worker --lib`、`cargo test -p pathkeep-desktop --lib` | 驗證 orchestration、desktop bridge、schedule / security / app lock refusal path。         |
| Desktop command façade owner                  | `bun run coverage:rust`                                                    | 只保護 `src-tauri/src/*` surface，不假裝整個 Rust workspace 都納入 100% coverage。        |
| Frontend contract / component owner           | `bun run test:unit`                                                        | `src/app`、`src/lib`、`src/pages`、shared primitives 的單元與 integration tests。         |
| Trust / IA / product-flow owner               | `bun run test:unit:product-flows`、`bun run test:e2e`                      | 聚焦 onboarding、schedule、security、import、dashboard / intelligence 等跨頁 UX。         |
| Release / deep-check owner                    | `bun run verify`、GitHub `Mutation` workflow                               | 屬於 milestone closeout / release 預演，不是每次 PR 的 blocking path。                    |

### 待辦

- [x] `PG-BL-QA-001` 定義新的 test pyramid 和 ownership，明確哪些測試屬於 parser、archive、worker、frontend、e2e。（2026-04-09，`WORK-QC-C`：見上方 `Test Pyramid And Ownership`）
- [x] `PG-BL-QA-002` 重寫 Playwright smoke 目標，從舊 setup shell 改成新 app shell / onboarding / dashboard smoke。
- [x] `PG-BL-QA-003` 定義重寫期 quality policy：repo-wide coverage / mutation 暫時不擋主線，但新碼與整段重寫模組仍必須達到 100% coverage + mutation verification。
- [x] `PG-BL-QA-004` 盤點 mutation test 現況和成本，先支持 targeted verification，再決定何時恢復整倉 sweep。
- [x] `PG-BL-QA-005` 重寫 README / release workflow 文案，使其描述和新產品定位一致。
- [x] `PG-BL-QA-006` 回收 pre-M4 quality matrix：恢復 `coverage:js`、`coverage:rust`、`mutation:js` 的 honest scope，並把 blocking path / deep checks 對齊 docs、scripts 與 CI。（2026-04-07，`WORK-QC-A`）

---

## 命名遷移基線

### 觀察

- `package.json` 已切到 `pathkeep`，Tauri `productName` / window title / identifier 也已改成 PathKeep。
- bundle id、keyring service、scheduler labels 與 app data root naming 已統一切到 `com.yi-ting.pathkeep`。
- README、CONTRIBUTING、release workflow、frontend public strings、Tauri product strings 與大部分 worker / platform 文案都已切到 PathKeep。
- repo 內仍有少量 `Chrome History Backup` / `Chrome History Vault` 殘留，但它們現在只出現在 explicit legacy alias / migration comment，用於升級與資料恢復敘事，而不是現行品牌。

### 判斷

- M0 的 rename sweep 對 public surface 與 build metadata 已足夠完成；剩餘舊名字串是受控的 legacy reference，不代表命名遷移未完成。
- M4-L 之後，desktop namespace rename 已不再保留 `dev.codex.pathkeep` 自動遷移；這是 user-approved clean break，而不是遺漏。
- 因為沒有正式用戶，repo 不需要為舊品牌維持長期兼容窗口；接下來只要避免在新功能把舊名字重新帶回來即可。

### 待辦

- [x] `PG-BL-NM-001` 列出所有對外可見名稱、內部 service name、filesystem path name、bundle id、schedule label、MCP skill 名稱。
- [x] `PG-BL-NM-002` 標記哪些舊名字串應直接清除，哪些確實與資料恢復相關、需要被保留到對應重寫完成。
- [x] `PG-BL-NM-003` 建立一次性 rename cleanup checklist，確保命名清理不遺漏 build metadata、資料目錄與 automation artifact。
