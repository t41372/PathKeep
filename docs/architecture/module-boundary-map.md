# Module Boundary Map

> M0 凍結的 crate / module 邊界。這份文檔描述「誰可以知道什麼」，避免 M1 之後又把功能堆回巨型檔案。

---

## Crate 責任

| Crate / Layer            | 核心責任                                                                                                                                                 | 明確不負責                                                                                 |
| ------------------------ | -------------------------------------------------------------------------------------------------------------------------------------------------------- | ------------------------------------------------------------------------------------------ |
| `browser-history-parser` | 針對「已提供的檔案路徑」做 schema observation、capability snapshot、canonical fact extraction、typed/native evidence extraction                          | Installed browser discovery、staging copy、archive 寫入、Tauri command、keyring、scheduler |
| `vault-core`             | hot canonical archive、cold source-evidence archive、backup/import/rollback/query/export/doctor domain logic、field promotion / re-extract orchestration | Tauri macros、桌面 IPC facade、scheduler artifact 安裝                                     |
| `vault-platform`         | OS path heuristics、browser discovery、scheduler artifact、keyring / platform capability adapter                                                         | canonical schema 設計、Tauri command facade                                                |
| `vault-worker`           | orchestration、background job entrypoint、desktop CLI / MCP bridge                                                                                       | Tauri command 宏與直接 UI 命名                                                             |
| `src-tauri/src/`         | Tauri command facade 與 session bridge                                                                                                                   | 業務邏輯、資料模型決策、parser implementation                                              |

## 前端（2026-10 redesign 之後）

前端只透過 typed client 跟 Rust 說話，沒有 browser-preview 假後端：不在 Tauri 裡、也沒有 dev IPC bridge 時，`src/lib/backend-client/shared.ts` 的 `call()` 直接丟錯。瀏覽器裡要看真資料用 `bun run dev:demo`（真後端 + 合成瀏覽器資料）。

| 目錄 / 檔案                                    | 負責                                                                                                                                                                                                                                                                         | 不負責                                                     |
| ---------------------------------------------- | ---------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | ---------------------------------------------------------- |
| `src/main.tsx`                                 | 首屏前套用主題與 `html[lang]`、掛載 App、安裝 runtime diagnostics。受保護入口                                                                                                                                                                                                | 任何產品邏輯                                               |
| `src/app/index.tsx`、`router.tsx`              | Provider 堆疊（Query、Theme、I18n、Tooltip、Session、BackupRunner、Toaster）；依 session 狀態選全螢幕畫面或路由；hash router 與 lazy 畫面                                                                                                                                    | 畫面內容                                                   |
| `src/app/session.tsx`                          | 開機流程與 session 狀態機（booting / error / app-locked / archive-locked / recovery / upgrade / onboarding / ready）；解鎖、鎖定、閒置自動鎖定、`restart()`；把 snapshot 放進查詢快取                                                                                        | 渲染這些狀態                                               |
| `src/app/backup-runner.tsx`                    | 唯一的手動備份執行與進度；選單列發起的備份也在這裡顯示；`refreshAfterArchiveChange()` 讓所有 archive 相關查詢失效                                                                                                                                                            | 排程                                                       |
| `src/app/shell/`                               | 視窗外框、導航列、⌘K、鎖定畫面、開機 / 錯誤 / 升級 / 還原畫面、路由錯誤畫面                                                                                                                                                                                                  | 各畫面資料                                                 |
| `src/features/<screen>/`                       | 一個畫面一個資料夾（home、history、insights、ask、backup、settings、onboarding，加上 settings 與 onboarding 共用的 `ai-setup`）。各自持有自己的查詢（`queries.ts`）、狀態與元件                                                                                              | 直接呼叫 `invoke` 或 `ipc/bridge.ts`                       |
| `src/components/ui/`                           | shadcn/ui 元件，照 registry 原樣保留（lint 放寬），用 class 或 props 調整，不改檔                                                                                                                                                                                            | 業務邏輯                                                   |
| `src/components/evilcharts/`                   | EvilCharts（Recharts）圖表，照 registry 原樣保留                                                                                                                                                                                                                             | 業務邏輯、資料聚合                                         |
| `src/components/app/`                          | 跨畫面共用的 app 元件：卡片與頁首（`section-card`）、設定列、heatmap、favicon、品牌標誌、備份按鈕與狀態膠囊                                                                                                                                                                  | 自己去拉資料（備份按鈕例外，讀 backup runner 與 snapshot） |
| `src/lib/backend-client/`                      | 每個領域一個 typed client（`appClient`、`archiveClient`、`explorerClient`、`insightsClient`、`intelligenceClient`、`scheduleClient`、`securityClient`、`starsClient`、`annotationsClient`、`migrationClient`、`dataWipeClient`、`desktopClient`…），把命令名與參數形狀藏起來 | 使用者看得到的文字、快取                                   |
| `src/lib/query.ts`、`src/lib/queries/`         | 唯一的 TanStack Query client（staleTime 5 分鐘、不在 focus 時重拉、不重試）；共用 query key，`archive` 開頭的 key 在備份 / 匯入後一起失效；跨畫面共用的查詢（snapshot、dashboard、schedule、語言）                                                                           | 單一畫面的查詢（放在 feature 裡）                          |
| `src/lib/ipc/`                                 | `bridge.ts`（Tauri `invoke` 或 dev bridge HTTP，受保護入口）、命令錯誤型別與判斷、各種事件訂閱（備份進度、匯入進度、升級進度、模型下載、AI 串流、`pathkeep://open-command-palette`、`pathkeep://backup-finished`）                                                           | 業務邏輯                                                   |
| `src/lib/i18n/`                                | 翻譯 runtime、`defineMessages` 型別、格式化（數字、日期、位元組、相對時間）；`messages/<feature>.ts` 每個 feature 一份三語文案                                                                                                                                               | —                                                          |
| `src/lib/types/`、`src/lib/core-intelligence/` | 後端回傳形狀的 TypeScript 型別                                                                                                                                                                                                                                               | 執行期邏輯                                                 |
| `src/lib/` 其他                                | `theme.tsx`、`runtime.ts`（判斷 Tauri / dev bridge）、`update.ts`、`platform-guidance.ts`、`errors.ts`（`describeError`）等                                                                                                                                                  | —                                                          |

依賴方向：

```text
src/app/router ──lazy──▶ src/features/*
src/features/* ──▶ src/app/{session,backup-runner}、src/components/*、src/lib/{queries,backend-client,i18n,ipc 事件與錯誤判斷}
src/components/app ──▶ src/components/ui、src/lib（backup-button 另讀 src/app/backup-runner）
src/lib/backend-client ──▶ src/lib/ipc/bridge.ts ──▶ Tauri invoke | dev IPC bridge
```

- feature 之間可以共用（例如 Backup 用 `features/home/queries` 的 source stats、onboarding 用 `features/backup/schedule-actions` 與 `features/ai-setup`），但不要反過來讓 `src/lib` 依賴 feature。
- 桌面事件與命令契約見 [desktop-command-surface.md](desktop-command-surface.md)（含 2026-10 的開機啟動、選單列圖示與兩個 `pathkeep://` 事件）。

## Current Hotspots And Intended Homes

> 這張表不是要否認現在可用的 code path，而是把「目前暫住在哪裡」和「長期應該屬於哪裡」講清楚，避免 closeout 之後又往錯的地方加碼。

| Current symbol / file                                                    | 現在實際做什麼                                                                                                                                  | 長期應屬於哪層           | 2026-04-09 簽收立場                                                                                                                                                                                                                   |
| ------------------------------------------------------------------------ | ----------------------------------------------------------------------------------------------------------------------------------------------- | ------------------------ | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `vault_core::chrome::discover_profiles`                                  | 掃描 Chromium / Firefox / Safari profiles                                                                                                       | `vault-platform`         | accepted legacy hotspot；後續不要再把新 discovery heuristics 塞進 `vault-core`。                                                                                                                                                      |
| `vault_core::chrome::stage_profile_snapshot`                             | 將 live DB 複製到 staging、攜帶 sidecar DB                                                                                                      | `vault-platform`         | accepted legacy hotspot；parser 仍只吃 provided path，不碰 staging / locking。                                                                                                                                                        |
| `vault_platform::{keyring,scheduler,launcher,host_capability,discovery}` | native keyring truth、scheduler artifact、launcher invocation、host capability、browser discovery wrapper                                       | `vault-platform`         | 2026-04-10 起已拆成明確子模組，native integration tests 也移到這層，不再把 host smoke 藏在 worker / bridge mega-tests 裡。                                                                                                            |
| `vault_core::archive::parse_profile_snapshot`                            | 將 parser output 轉進 canonical ingest                                                                                                          | `vault-core`             | signed-off canonical ingest boundary。                                                                                                                                                                                                |
| `vault_core::takeout::browser_history`                                   | Browser Direct local DB inspect/import、staging snapshot、import batch / source-evidence / search refresh                                       | `vault-core`             | 2026-04-24 起作為 Browser Direct backend boundary（2026-10 起前端沒有入口，命令仍在）；不得再把 Safari / Chrome local DB path 送進 Takeout parser。                                                                                   |
| `vault_core::archive::{doctor,repair_health_issues}`                     | canonical integrity / recoverability checks                                                                                                     | `vault-core`             | signed-off core responsibility；finding taxonomy 仍可演進，但不應搬回 worker / UI。                                                                                                                                                   |
| `vault_core::archive::history`                                           | Explorer-visible history recall、lexical/fuzzy/regex dispatch、pagination envelope、favicon hydration、export rendering                         | `vault-core`             | 2026-05-03 maintainability split landed first slice: `history.rs` keeps the public facade while `archive/history/{pagination,favicons,export}.rs` own extracted helpers; lexical/baseline SQL extraction remains deferred.            |
| `vault_core::visit_taxonomy::*`                                          | evidence-first URL normalization、search parser、taxonomy v2 基礎                                                                               | `vault-core`             | 2026-04-10 起作為 `WORK-M5-A` foundation home；不要再把新的 visit taxonomy helper 塞回 `insights.rs`。                                                                                                                                |
| `vault_core::intelligence::*`                                            | deterministic grouping、topic aggregation、user-facing summary builders、derived-state persistence / load                                       | `vault-core`             | 2026-04-15 storage-plane reset 後由 `intelligence/` 子模組（`intelligence_core_persist`、`day_insights`、`host_artifacts` 等）取代早期 `insights::{grouping,topics,surfaces,storage}`（該模組已不存在）；新增邏輯按責任進對應子模組。 |
| `vault_core::intelligence_runtime::*`                                    | first-party enrichment registry、queue review、retry / cancel 邊界                                                                              | `vault-core`             | 2026-04-10 起作為 `WORK-M5-A` runtime home；只承諾內建 plugin，不承諾 third-party execution。                                                                                                                                         |
| `vault_worker::app_snapshot` / `schedule_status` / `security_status`     | 組裝 desktop-facing read model、平台能力與 session-aware warning                                                                                | `vault-worker`           | signed-off worker orchestration boundary。                                                                                                                                                                                            |
| `src_tauri::worker_bridge::*`                                            | 將 Tauri command 轉成 worker 調用與 structured `CommandError` envelope（`command_error.rs`：message chain + `code`/`action_hint`/`retry_hint`） | `src-tauri/src`          | signed-off façade boundary；這層不應知道 archive schema 細節。2026-07-26 起 error envelope 已按本文件的 error model 條款結構化。                                                                                                      |
| `src_tauri::updater::*`                                                  | updater check / download / install / relaunch 與 progress event                                                                                 | `src-tauri/src`          | 2026-04-10 起作為 desktop-only IPC boundary；前端不再直接調 plugin updater / process guest API。                                                                                                                                      |
| `browser_history_parser::*`                                              | schema observation、capability snapshot、canonical facts、typed evidence、native entities                                                       | `browser-history-parser` | signed-off extractor boundary；此 crate 不應新增 archive / Tauri / platform side effect。                                                                                                                                             |

2026-04-14 backend rustdoc follow-up:

- `vault-worker/src/lib.rs`、`vault-core/src/{chrome,ai,archive/mod}.rs` 的大型 regression suites 已移到 sibling `tests.rs` 模組（早期的 `insights.rs` 已在 2026-04-15 reset 中移除）。
- 這些 runtime 主檔現在保留 orchestration / public boundary / helper contract，test-only fixtures 不再和 shipping code 混在同一個 mega-file。
- 後續若再拆 hotspot，優先拆 runtime 責任本身，不要把 regression suite 又塞回主檔充當“順手一起放”的雜物間。

---

## 依賴方向

```text
browser-history-parser
  ├── allowed: chrono, rusqlite, serde, thiserror
  └── forbidden: tauri, vault-core, vault-platform, vault-worker, keyring, directories

vault-core
  ├── allowed: browser-history-parser, rusqlite, serde, anyhow, ...
  └── forbidden: tauri command macros, vault-worker

vault-platform
  ├── allowed: vault-core, browser-history-parser, OS/platform crates
  └── forbidden: vault-worker, tauri command macros

vault-worker
  ├── allowed: vault-core, vault-platform, browser-history-parser
  └── forbidden: tauri command macros, desktop-only product naming

pathkeep-desktop (src-tauri/src)
  └── allowed: all workspace crates, but only as IPC facade
```

---

## Extractor API And Versioning

`browser-history-parser` 的 public API 改凍結在這個等級：

- Input:
  - `HistoryDatabaseSet` — 呼叫端已提供的 `History` / `Favicons` 路徑
  - provider-specific incremental cursor，例如 `ChromiumReadCursor`
- Output:
  - `SchemaObservation`
  - `CapabilitySnapshot`
  - `CanonicalFactsBatch`
  - `TypedEvidenceBatch`
  - `NativeEntityBatch`
  - `ParserWarning` / `ParseError`
- Guarantee:
  - extractor crate 不知道 archive schema、不知道 `run_id`、不知道 Tauri
  - extractor 一律 `introspection first, version second`
  - provider modules 可以 additive 地新增 evidence / capability / warning code，但不應破壞既有欄位語義

Versioning policy:

- `0.x` 期間可以調整 API，但所有 breaking changes 都必須同步更新這份文檔與 fixture/tests。
- 一旦 `vault-core` 正式消費 extractor contract，capability tag、typed evidence family、native entity semantics 就視為 stability contract；breaking changes 必須同步更新 ADR / fixture / dev guides。

---

## Derived State Boundary

- Canonical source of truth:
  - `runs`
  - `source_profiles`
  - `urls`
  - `visits`
  - `downloads`
  - `search_terms`
  - checkpoint / snapshot / manifest trace
  - `manifests`
  - `snapshots`
  - `settings`
- Archived source-native evidence:
  - `source_batches`
  - schema / capability observation
  - typed evidence tables
  - `native_entities`
- Rebuildable derived state:
  - FTS projection tables
  - aggregation / heatmap / timeline bucket tables
  - AI / insight / semantic index sidecars
- Consequence:
  - `ai.rs` / `intelligence/*` 在 M0 先視為 derived-state domain，不能再偷偷反向定義 canonical schema

---

## Tauri Command Surface Draft

> M0 的草案，留作命名原則的紀錄。實際的命令清單與 2026-10 的增刪見 [desktop-command-surface.md](desktop-command-surface.md)。

Tauri facade 暫時仍集中在 `src-tauri/src/lib.rs`，但命名與回傳 envelope 應收斂到四種接口類型：

### 1. Read Models

- `app.snapshot`
- `app.lock_status`
- `onboarding.discover_profiles`
- `dashboard.get_summary`
- `explorer.query_history`
- `audit.list_runs`
- `audit.get_run_detail`
- `settings.get_config`
- `schedule.get_status`
- `security.get_status`

### 2. Preview / Manual / Execute Commands

- `archive.backup.preview`
- `archive.backup.execute`
- `app.lock.configure`
- `app.lock.lock`
- `app.lock.unlock`
- `archive.import.preview`
- `archive.import.execute`
- `archive.rollback.preview`
- `archive.rollback.execute`
- `schedule.install.preview`
- `schedule.install.execute`
- `schedule.install.repair`
- `security.rekey.preview`
- `security.rekey.execute`

### 3. Long-Running Job Triggers

- `jobs.start_backup`
- `jobs.start_import`
- `jobs.start_doctor`
- `jobs.observe_run`

### 4. Artifact Fetch

- `artifacts.list_for_run`
- `artifacts.get_manifest`
- `artifacts.get_snapshot_metadata`
- `artifacts.get_preview_bundle`

Command surface rules:

- facade 保持在 desktop layer，但 request / response shape 以 domain 命名，不以頁面組件或舊 UI 名稱命名
- mutating command 一律能對應到 `run_id` 或 preview artifact
- user-facing error model 要帶上 `error_code`、`action_hint`、`retry_hint`，而不只是裸字串
- App Lock 屬於 session guard，不是 archive encryption 的別名；locked state 下除了 `app.lock_status` / `app.lock.unlock` / recovery helper 之外，其餘 data read surface 必須走一致的 refusal path
