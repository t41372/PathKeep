# Module Boundary Map

> M0 凍結的 crate / module 邊界。這份文檔描述「誰可以知道什麼」，避免 M1 之後又把功能堆回巨型檔案。

---

## Crate 責任

| Crate / Layer            | 核心責任                                                                                            | 明確不負責                                                                                 |
| ------------------------ | --------------------------------------------------------------------------------------------------- | ------------------------------------------------------------------------------------------ |
| `browser-history-parser` | 針對「已提供的檔案路徑」做 SQLite schema inspection、row parsing、warning surface                   | Installed browser discovery、staging copy、archive 寫入、Tauri command、keyring、scheduler |
| `vault-core`             | canonical archive schema、migration ledger、backup/import/rollback/query/export/doctor domain logic | Tauri macros、桌面 IPC facade、scheduler artifact 安裝                                     |
| `vault-platform`         | OS path heuristics、browser discovery、scheduler artifact、keyring / platform capability adapter    | canonical schema 設計、Tauri command facade                                                |
| `vault-worker`           | orchestration、background job entrypoint、desktop CLI / MCP bridge                                  | Tauri command 宏與直接 UI 命名                                                             |
| `src-tauri/src/`         | Tauri command facade 與 session bridge                                                              | 業務邏輯、資料模型決策、parser implementation                                              |

## Current Hotspots And Intended Homes

> 這張表不是要否認現在可用的 code path，而是把「目前暫住在哪裡」和「長期應該屬於哪裡」講清楚，避免 closeout 之後又往錯的地方加碼。

| Current symbol / file                                                | 現在實際做什麼                                                   | 長期應屬於哪層           | 2026-04-09 簽收立場                                                                    |
| -------------------------------------------------------------------- | ---------------------------------------------------------------- | ------------------------ | -------------------------------------------------------------------------------------- |
| `vault_core::chrome::discover_profiles`                              | 掃描 Chromium / Firefox / Safari profiles                        | `vault-platform`         | accepted legacy hotspot；後續不要再把新 discovery heuristics 塞進 `vault-core`。       |
| `vault_core::chrome::stage_profile_snapshot`                         | 將 live DB 複製到 staging、攜帶 sidecar DB                       | `vault-platform`         | accepted legacy hotspot；parser 仍只吃 provided path，不碰 staging / locking。         |
| `vault_core::archive::parse_profile_snapshot`                        | 將 parser output 轉進 canonical ingest                           | `vault-core`             | signed-off canonical ingest boundary。                                                 |
| `vault_core::archive::{doctor,repair_health_issues}`                 | canonical integrity / recoverability checks                      | `vault-core`             | signed-off core responsibility；finding taxonomy 仍可演進，但不應搬回 worker / UI。    |
| `vault_worker::app_snapshot` / `schedule_status` / `security_status` | 組裝 desktop-facing read model、平台能力與 session-aware warning | `vault-worker`           | signed-off worker orchestration boundary。                                             |
| `src_tauri::worker_bridge::*`                                        | 將 Tauri command 轉成 worker 調用與 string-based error envelope  | `src-tauri/src`          | signed-off façade boundary；這層不應知道 archive schema 細節。                         |
| `browser_history_parser::*`                                          | SQLite inspection、row parsing、warning surface                  | `browser-history-parser` | signed-off parser boundary；此 crate 不應新增 archive / Tauri / platform side effect。 |

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

## Parser API And Versioning

`browser-history-parser` 的 day-one public API 先凍結在這個等級：

- Input:
  - `HistoryDatabaseSet` — 呼叫端已提供的 `History` / `Favicons` 路徑
  - provider-specific incremental cursor，例如 `ChromiumReadCursor`
- Output:
  - `DatabaseInspection`
  - provider-specific parsed rows，例如 `ParsedUrl`、`ParsedVisit`、`ParsedDownload`、`ParsedSearchTerm`、`ParsedFavicon`
  - `ParserWarning` / `ParseError`
- Guarantee:
  - parser crate 不知道 archive schema、不知道 `run_id`、不知道 Tauri
  - provider modules 可以新增欄位或 warning code，但不應破壞既有欄位語義
  - Firefox / Safari provider 已落地 history baseline；後續 richer metadata、downloads / search / favicon coverage 應以 additive 欄位或 warning code 演進，不要打破 module path 與 type boundary

Versioning policy:

- `0.x` 期間可以調整 API，但所有 breaking changes 都必須同步更新這份文檔與 fixture/tests。
- 一旦 `vault-core` 在 M1 開始正式消費 parser crate，row type 欄位語義就視為 stability contract，之後若有 breaking changes 必須經過新的 ADR 或 migration note。

---

## Derived State Boundary

- Canonical source of truth:
  - `runs`
  - `source_profiles`
  - `urls`
  - `visits`
  - `downloads`
  - `search_terms`
  - `raw_row_versions`
  - `manifests`
  - `snapshots`
  - `settings`
- Rebuildable derived state:
  - FTS projection tables
  - aggregation / heatmap / timeline bucket tables
  - AI / insight / semantic index sidecars
- Consequence:
  - `ai.rs` / `insights.rs` 在 M0 先視為 derived-state domain，不能再偷偷反向定義 canonical schema

---

## Tauri Command Surface Draft

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
