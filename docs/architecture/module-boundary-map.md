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
  - stub providers（Firefox / Safari / Takeout）可先回傳 `UnsupportedProvider`，但 module path 與 type boundary 現在就固定下來

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
