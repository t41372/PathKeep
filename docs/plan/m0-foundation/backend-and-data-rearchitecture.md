# M0-BE — Backend And Data Rearchitecture

> 讀這份文檔的時機：當你要把現有巨型 Rust 模組拆回健康邊界、建立新 canonical data plane、為 M1 的 archive engine 打地基時。  
> 這份文檔不追求把所有 Archive 功能做完；它追求的是把功能放回正確的位置。

---

## Quick-Start Implementation Guide

以下是本工作包的建議執行順序。每個步驟都標註了要讀的文檔、要改的文件、和驗收方式。

> **前提**：`PG-RD-ARCH-001` 到 `PG-RD-ARCH-006` 是 **schema / migration / rollback** 相關步驟（Step 2 之後）的前提。  
> `browser-history-parser` crate skeleton 與 module boundary 文檔可以先做，但不能偷偷把未決的 schema 決策硬編進 API。

### Step 1: Create browser-history-parser crate

**要讀的文檔**

- `docs/architecture/tech-stack.md` — 確認 crate 邊界原則
- `docs/plan/program/repo-baseline.md` 的「後端基線」段落 — 理解現有 chrome.rs 職責邊界

**要建立的文件**

```
src-tauri/crates/browser-history-parser/Cargo.toml
src-tauri/crates/browser-history-parser/src/lib.rs
src-tauri/crates/browser-history-parser/src/types.rs        # ParsedUrl, ParsedVisit, ParsedDownload, ParsedSearchTerm, ParserWarning
src-tauri/crates/browser-history-parser/src/error.rs        # ParseError
src-tauri/crates/browser-history-parser/src/chromium/mod.rs # Chromium parsing + provided-path inspection helpers
src-tauri/crates/browser-history-parser/src/firefox/mod.rs  # Firefox stub
src-tauri/crates/browser-history-parser/src/safari/mod.rs   # Safari stub
src-tauri/crates/browser-history-parser/src/takeout/mod.rs  # Google Takeout stub
```

**Cargo.toml 起手式**

```toml
[package]
name = "browser-history-parser"
version = "0.1.0"
edition = "2024"
license.workspace = true

[dependencies]
chrono.workspace = true
rusqlite.workspace = true
serde.workspace = true
thiserror.workspace = true
# skeleton 階段不允許依賴：tauri、vault-core、vault-platform、vault-worker、keyring、directories、anyhow、serde_json
```

**將什麼從現有代碼搬過來**

- 從 `src-tauri/crates/vault-core/src/chrome.rs` 搬入的邏輯：
  - `INGEST_URLS_SQL`、`INGEST_VISITS_SQL`、`DOWNLOADS_SQL`、`SEARCH_TERMS_SQL`、`FAVICONS_SQL` 常數（僅 SQL 字串，不含執行邏輯）
  - `FIREFOX_HISTORY_SQL`、`SAFARI_HISTORY_SQL` 常數
  - Row 解析函式（`row -> ParsedUrl`、`row -> ParsedVisit` 等）
  - Chromium 時間轉換工具（`chrome_time_to_ms` 等）
- **留在** `vault-core` 或 `vault-platform` 的邏輯：
  - Installed profile discovery（目錄掃描、OS 路徑 heuristics）
  - Staging copy（把 SQLite 複製到暫存位置，避免鎖衝突）
  - Archive DB 的讀寫操作
  - 權限檢查（例如 Full Disk Access / keyring 可用性）

> parser crate 可以提供「對某個已給定 profile 路徑做 metadata inspection」的 helper，但**不負責**全局掃描本機安裝的瀏覽器 profiles。

**要更新的 workspace 清單**

```toml
# src-tauri/Cargo.toml
[workspace]
members = ["crates/vault-core", "crates/vault-platform", "crates/vault-worker", "crates/browser-history-parser"]
```

**驗收**

```bash
cargo test -p browser-history-parser   # 所有 unit tests pass
cargo clippy -p browser-history-parser -- -D warnings
# 確認 browser-history-parser 的 Cargo.toml 裡沒有 tauri、vault-* 依賴
```

**Commit**: `feat(parser): create browser-history-parser crate skeleton`

---

### Step 2: Write canonical schema v1

**要讀的文檔**

- `docs/architecture/data-model.md` — 這是 schema 的 source of truth，逐表對照
- `src-tauri/crates/vault-core/src/archive-schema.sql` — 現有 schema，理解 gap

**要建立的文件**

```
src-tauri/crates/vault-core/src/migrations/001_initial.sql
```

**Tables required**（欄位命名規則：`_ms` suffix = `INTEGER NOT NULL` Unix epoch 毫秒，`_iso` suffix = `TEXT` UTC ISO 8601 輔助欄位）

```sql
-- Migration tracking
CREATE TABLE schema_migrations (
  version     INTEGER PRIMARY KEY,
  applied_at  TEXT    NOT NULL,  -- ISO 8601
  checksum    TEXT    NOT NULL   -- SHA-256 of migration file content
);

-- Source browser profiles
CREATE TABLE source_profiles (
  id              INTEGER PRIMARY KEY AUTOINCREMENT,
  browser_kind    TEXT    NOT NULL,  -- 'chromium' | 'firefox' | 'safari'
  browser_version TEXT,
  profile_name    TEXT    NOT NULL,
  profile_path    TEXT    NOT NULL,
  discovered_at   TEXT    NOT NULL,
  enabled         INTEGER NOT NULL DEFAULT 1
);

-- Canonical URL records
CREATE TABLE urls (
  id                  INTEGER PRIMARY KEY AUTOINCREMENT,
  url                 TEXT    NOT NULL,
  title               TEXT,
  visit_count         INTEGER NOT NULL DEFAULT 0,
  typed_count         INTEGER NOT NULL DEFAULT 0,
  first_visit_ms      INTEGER NOT NULL,
  last_visit_ms       INTEGER NOT NULL,
  first_visit_iso     TEXT,
  last_visit_iso      TEXT,
  source_profile_id   INTEGER NOT NULL REFERENCES source_profiles(id),
  created_by_run_id   INTEGER NOT NULL REFERENCES runs(id)
);

-- Canonical visit events
CREATE TABLE visits (
  id                  INTEGER PRIMARY KEY AUTOINCREMENT,
  url_id              INTEGER NOT NULL REFERENCES urls(id),
  visit_time_ms       INTEGER NOT NULL,
  visit_time_iso      TEXT,
  transition_type     INTEGER,
  visit_duration_ms   INTEGER,
  source_profile_id   INTEGER NOT NULL REFERENCES source_profiles(id),
  created_by_run_id   INTEGER NOT NULL REFERENCES runs(id),
  reverted_at         TEXT,
  reverted_by_run_id  INTEGER REFERENCES runs(id)
);

-- Downloads（同 visits pattern）
-- search_terms（同 visits pattern）

-- 原始行快照（用於 audit 和 rollback）
CREATE TABLE raw_row_versions (
  id                  INTEGER PRIMARY KEY AUTOINCREMENT,
  table_name          TEXT    NOT NULL,
  source_pk           TEXT    NOT NULL,
  schema_fingerprint  TEXT    NOT NULL,
  browser_version     TEXT,
  payload_json        TEXT    NOT NULL,
  run_id              INTEGER NOT NULL REFERENCES runs(id)
);

-- Run ledger（backup / import / revert / doctor / snapshot_restore 共用）
CREATE TABLE runs (
  id                  INTEGER PRIMARY KEY AUTOINCREMENT,
  run_type            TEXT    NOT NULL,  -- 'backup' | 'import' | 'revert' | 'doctor' | 'snapshot_restore'
  trigger             TEXT    NOT NULL,  -- 'manual' | 'schedule' | 'cli'
  started_at          TEXT    NOT NULL,
  finished_at         TEXT,
  timezone            TEXT,
  status              TEXT    NOT NULL,  -- 'running' | 'succeeded' | 'failed' | 'cancelled'
  profile_scope_json  TEXT,
  stats_json          TEXT,
  warnings_json       TEXT,
  error_message       TEXT
);

-- Manifest chain
CREATE TABLE manifests (
  id                  INTEGER PRIMARY KEY AUTOINCREMENT,
  run_id              INTEGER NOT NULL REFERENCES runs(id),
  parent_manifest_id  INTEGER REFERENCES manifests(id),
  content_hash        TEXT    NOT NULL,
  row_counts_json     TEXT,
  created_at          TEXT    NOT NULL
);

-- Safety net snapshots
CREATE TABLE snapshots (
  id          INTEGER PRIMARY KEY AUTOINCREMENT,
  run_id      INTEGER NOT NULL REFERENCES runs(id),
  file_path   TEXT    NOT NULL,
  file_size   INTEGER,
  checksum    TEXT,
  reason      TEXT,
  created_at  TEXT    NOT NULL
);

-- App settings KV store
CREATE TABLE settings (
  key        TEXT PRIMARY KEY,
  value      TEXT,
  updated_at TEXT NOT NULL
);
```

**Gap table**（現有 → 新 schema 對照，在 migration 文件頭部以註解形式記錄）

- `profiles` → `source_profiles`（新增 `browser_kind`、`browser_version`、`enabled`；移除 `user_name`、`chrome_version`）
- `backup_runs` → `runs`（新增 `run_type`、`trigger`、`timezone`；統一所有操作類型到同一表）
- `url_versions` → `urls`（新增 `first_visit_ms`、`first_visit_iso`；移除 `_ms` 以外的純 timestamp integer）
- `visit_events` → `visits`（新增 `reverted_at`、`reverted_by_run_id`）
- `source_schemas` → 保留為 `raw_row_versions.schema_fingerprint`
- `profile_watermarks` → 暫時保留，M1 決定是否搬到 `runs` stats

**驗收**

```bash
# 在 temp 目錄測試 migration SQL 能跑過
sqlite3 /tmp/test.db < src-tauri/crates/vault-core/src/migrations/001_initial.sql
sqlite3 /tmp/test.db ".tables"  # 應列出所有新表
```

**Commit**: `feat(schema): canonical archive schema v1 migration`

---

### Step 3: Implement migration system

**要讀的文檔**

- `docs/architecture/data-model.md` 的 migration 段落
- Step 2 產出的 `001_initial.sql`

**要建立的文件**

```
src-tauri/crates/vault-core/src/migrations/mod.rs   # 或 migration.rs
```

**核心 API**（在 `vault-core` 內部）

```rust
/// 按 version 順序執行所有尚未 apply 的 migration。
/// 每次執行完一個 migration，寫入 schema_migrations 表並記錄 checksum。
/// 如果 checksum 不符（已 apply 的 migration 被修改），回傳 Err。
pub fn run_migrations(conn: &Connection) -> Result<()>;

/// 回傳目前 DB 的 schema version（schema_migrations 裡最大的 version）。
pub fn current_version(conn: &Connection) -> Result<i64>;
```

**Migration 執行流程**

1. `PRAGMA journal_mode = WAL;`
2. 讀取 `schema_migrations` 中已 apply 的版本清單（如果表不存在，代表是全新 DB）
3. 對每個 `.sql` 檔案（按 version 排序）：
   - 計算 SHA-256 checksum
   - 如果 version 已在表中但 checksum 不符 → `Err`（migration 被篡改）
   - 如果 version 已在表中且 checksum 相符 → skip
   - 如果 version 不在表中 → 在 transaction 內執行 SQL，成功後 INSERT 進 schema_migrations
4. 所有 migration 執行完畢 → `Ok(())`

**取代現有 `create_schema()`**

- 現有 `archive.rs` 的 `create_schema()` 函式呼叫 `include_str!("archive-schema.sql")` 並直接執行
- 替換為：`run_migrations(&conn)?`
- 舊 `archive-schema.sql` 保留作 reference，但 M0 結束後可移除

**要建立的測試**

```rust
#[test] fn migration_from_scratch_succeeds()
#[test] fn migration_is_idempotent()
#[test] fn migration_checksum_mismatch_returns_err()
#[test] fn migration_version_reported_correctly()
```

**驗收**

```bash
cargo test -p vault-core migration   # 上面 4 個 tests pass
```

**Commit**: `feat(vault-core): implement SQL migration system`

---

### Step 4: Split archive.rs

**要讀的文檔**

- 現有 `src-tauri/crates/vault-core/src/archive.rs`（2078 行）

**責任拆分策略**

| 目標模組      | 內容                                                                                                                        |
| ------------- | --------------------------------------------------------------------------------------------------------------------------- |
| `schema.rs`   | `run_migrations`（已在 Step 3 建立）、`open_archive_connection`、`create_schema`（legacy，可刪）、`ARCHIVE_SCHEMA_SQL` 常數 |
| `backup.rs`   | `run_backup`、`ingest_chromium_profile`、`ingest_firefox_profile`、`ingest_safari_profile`、watermark 讀寫、manifest 寫入   |
| `query.rs`    | `query_history`、`LIST_HISTORY_SQL`、`INGEST_*_SQL` 常數（最終搬到 parser crate）、`archive_status`                         |
| `export.rs`   | `export_history`、`ExportFormat` 相關邏輯                                                                                   |
| `doctor.rs`   | `run_doctor`、`health_check`、`HealthReport` 邏輯                                                                           |
| `rollback.rs` | `rollback_run`、`list_rollback_candidates`、`reverted_at` 更新邏輯                                                          |
| `security.rs` | `rekey_archive`、`open_archive_connection`（加密模式）、key rotation 邏輯                                                   |
| `mod.rs`      | Re-export 所有上述模組的 public API，保持對外界面不變                                                                       |

**拆分順序**（避免 mega PR）

1. PR 1：建立 `mod.rs` 並把 `schema.rs` 拆出（最小改動，確保 tests pass）
2. PR 2：把 `backup.rs` 拆出（核心備份邏輯）
3. PR 3：把 `query.rs`、`export.rs` 拆出
4. PR 4：把 `doctor.rs`、`rollback.rs`、`security.rs` 拆出
5. 每個 PR 都必須讓 `cargo test -p vault-core` 全過

**要更新的文件**

- `src-tauri/crates/vault-core/src/lib.rs`：把 `mod archive;` 換成 `mod archive { mod schema; mod backup; ... }`
- 或改用目錄結構：`src-tauri/crates/vault-core/src/archive/mod.rs`

**驗收**

```bash
cargo test -p vault-core           # 所有既有 tests 仍然 pass
cargo clippy -p vault-core -- -D warnings
# archive.rs 已不再存在，改為 archive/ 目錄
ls src-tauri/crates/vault-core/src/archive/
# 應看到：mod.rs schema.rs backup.rs query.rs export.rs doctor.rs rollback.rs security.rs
```

**Commit**: `refactor(vault-core): split archive.rs into focused modules`

---

### Step 5: Define module boundary and dependency rules

**要讀的文檔**

- `docs/architecture/tech-stack.md` — crate 責任說明
- Step 1 到 4 的成果

**要建立的文件**

```
docs/architecture/module-boundary-map.md   # 新文檔，記錄每個 crate 的職責和允許依賴方向
```

**依賴方向規則**（寫進 `module-boundary-map.md` 並在 PR review 中執行）

```
browser-history-parser
  ├── 可依賴：anyhow, chrono, rusqlite, serde, thiserror
  └── 禁止依賴：tauri, vault-core, vault-platform, vault-worker, keyring, directories

vault-core
  ├── 可依賴：browser-history-parser, anyhow, rusqlite, serde, ...
  └── 禁止依賴：tauri (不含 tauri 本身), vault-worker

vault-platform
  ├── 可依賴：vault-core, browser-history-parser, ...
  └── 禁止依賴：vault-worker, tauri commands

vault-worker
  ├── 可依賴：vault-core, vault-platform, browser-history-parser
  └── 禁止依賴：tauri command macros（worker 層不應直接暴露 Tauri commands）

browser-history-backup-desktop (src-tauri/src/)
  ├── 可依賴：所有上述 crate
  └── 責任：Tauri command facade，只做 IPC bridge，不含業務邏輯
```

**驗收**

```bash
cargo tree -p browser-history-parser   # 確認無 tauri / vault-* 依賴
cargo test --workspace --all-targets   # 整個 workspace tests pass
```

**Commit**: `docs(arch): add module boundary map`

---

### Step 6: Establish fixture structure for parser tests

**要建立的文件**

```
src-tauri/crates/browser-history-parser/tests/fixtures/chromium/
  normal_history.db         # 正常 Chromium History SQLite（最小樣本）
  locked_history.db         # 模擬鎖住的檔案（測試 staging copy 錯誤處理）
  missing_columns.db        # 缺少某些欄位（歷史 schema 兼容性測試）
  empty_history.db          # 空資料庫
  malformed_timestamps.db   # 異常時間戳（Unix time 0、負數、2100 年後）
src-tauri/crates/browser-history-parser/tests/fixtures/firefox/
  normal_places.sqlite
src-tauri/crates/browser-history-parser/tests/fixtures/takeout/
  BrowserHistory.json       # 最小 Takeout JSON 樣本
```

**要建立的測試**

```rust
// tests/chromium_parser_test.rs
#[test] fn parse_normal_chromium_history_returns_expected_rows()
#[test] fn parse_empty_db_returns_empty_vecs()
#[test] fn parse_missing_columns_returns_warnings_not_panic()
#[test] fn parse_malformed_timestamps_clamps_or_warns()
```

**驗收**

```bash
cargo test -p browser-history-parser
# 至少 4 個 Chromium fixture tests pass
```

**Commit**: `test(parser): add fixture-based parser tests`

---

## Source Inputs

- [../../vision-and-requirements.md](../../vision-and-requirements.md)
- [../../architecture/data-model.md](../../architecture/data-model.md)
- [../../architecture/tech-stack.md](../../architecture/tech-stack.md)
- [../../database-selection-decision-2026-04-05.md](../../database-selection-decision-2026-04-05.md)
- [../../features/archive.md](../../features/archive.md)
- [../program/research-and-decisions.md](../program/research-and-decisions.md)
- [../program/repo-baseline.md](../program/repo-baseline.md)

---

## 本工作包要交付什麼

- `browser-history-parser` 的 crate 邊界、API 和 fixture strategy
- `vault-core` / `vault-platform` / `vault-worker` / Tauri command 的責任重切
- canonical archive schema、migration ledger、run model、timestamp contract 的正式設計
- 新 command surface 和 job orchestration foundation
- 可支撐 M1 的測試基線和 fixture 結構

---

## Open Blockers

- [!] `M0-BE-BLK-001` 先完成 [../program/research-and-decisions.md](../program/research-and-decisions.md) 中 `PG-RD-ARCH-002` 到 `PG-RD-ARCH-006` 的核心決策，否則 schema 重構會一直返工。
- [x] `M0-BE-BLK-002` Archive reset strategy 已由 [ADR-001](../../architecture/decisions/001-archive-reset-strategy.md) 凍結：採 fresh schema，legacy DB 走一次性 upgrade path。

---

## WBS

### Module Boundary Reset

- [ ] `M0-BE-MD-001` 盤點 [`src-tauri/crates/vault-core/src/archive.rs`](../../../src-tauri/crates/vault-core/src/archive.rs) 內所有責任，拆成 schema、backup pipeline、query、export、doctor、rollback、security、browser ingestion 類別。
- [ ] `M0-BE-MD-002` 盤點 [`src-tauri/crates/vault-core/src/chrome.rs`](../../../src-tauri/crates/vault-core/src/chrome.rs) 內的 browser discovery、path heuristics、staging copy、profile metadata，分出 parser-layer 和 platform-layer 邊界。
- [ ] `M0-BE-MD-003` 盤點 [`src-tauri/crates/vault-core/src/ai.rs`](../../../src-tauri/crates/vault-core/src/ai.rs) 和 [`src-tauri/crates/vault-core/src/insights.rs`](../../../src-tauri/crates/vault-core/src/insights.rs) 中哪些屬於 M3 derived state，哪些暫時保留，哪些應搬走。
- [ ] `M0-BE-MD-004` 建立新的 module map，明確定義 parser、archive core、platform adapters、worker orchestration、desktop command facade 的責任。
- [ ] `M0-BE-MD-005` 決定 Tauri commands 是否繼續全部留在 [`src-tauri/src/lib.rs`](../../../src-tauri/src/lib.rs) facade 下，或拆出 command modules；要求命名和 use case 對齊而不是對齊舊實作。
- [ ] `M0-BE-MD-006` 為每個 crate 定義可接受依賴方向，禁止 parser 依賴 Tauri、禁止 archive core 依賴 UI 命名與桌面框架。
- [ ] `M0-BE-MD-007` 為 worker 和 core 的共用型別建立最小共享層，避免跨 crate copy-paste 或反向依賴。

### Browser History Parser Extraction

- [ ] `M0-BE-PR-001` 建立 `browser-history-parser` crate 草案結構，至少分出 Chromium、Firefox、Safari、Takeout 的 provider module。
- [ ] `M0-BE-PR-002` 定義 parser crate 的輸入輸出：原始檔案路徑 / 目錄、staging source、parsed rows、metadata、warning surface。
- [ ] `M0-BE-PR-003` 凍結 parser crate 不碰 canonical schema、不碰 Tauri command、不碰 keyring 和 scheduler。
- [ ] `M0-BE-PR-004` 抽出 Chromium visit / url / download / search-term 解析邏輯到 parser crate，建立最小 compile/test pass。
- [ ] `M0-BE-PR-005` 抽出 Firefox 解析邏輯到 parser crate，保留 profile discovery 和 staging 決策在 platform / core。
- [ ] `M0-BE-PR-006` 抽出 Safari 解析和 macOS 特有 path knowledge 的邊界，避免平台檢測和資料解析混寫。
- [ ] `M0-BE-PR-007` 抽出 Google Takeout parsing 和 validation 基礎，至少形成 fixture 可跑的 importer parser。
- [ ] `M0-BE-PR-008` 為 parser crate 建立 edge-case fixture：鎖檔、缺欄位、歷史 schema、空資料、異常時間戳、損毀列。
- [ ] `M0-BE-PR-009` 決定 parser crate 的版本管理、公開 API 和 internal helper 的穩定性等級。

### Canonical Schema And Migration Foundation

- [ ] `M0-BE-SC-001` 依照 [../../architecture/data-model.md](../../architecture/data-model.md) 產出新 canonical schema v1 草案，不直接沿用現有 `archive-schema.sql`。
- [ ] `M0-BE-SC-002` 凍結 timestamp contract：毫秒整數欄位、ISO 顯示輔助欄位、timezone metadata、unknown timezone fallback 規則。
- [ ] `M0-BE-SC-003` 凍結 run ledger 模型，決定 `backup`、`import`、`revert`、`doctor`、`snapshot restore` 是否共用同一張 run table。
- [ ] `M0-BE-SC-004` 凍結 rollback visibility model，區分 immutable raw facts、logical visibility、derived state rebuild policy。
- [ ] `M0-BE-SC-005` 設計 `schema_migrations` 和 migration execution flow，取代 ad-hoc 升級方式。
- [ ] `M0-BE-SC-006` 決定 migration 檔案格式、命名、checksum、idempotency 和測試策略。
- [ ] `M0-BE-SC-007` 設計 manifest、snapshot、watermark、source profile、run artifact 的表關係和外鍵策略。
- [ ] `M0-BE-SC-008` 明確標記哪些 intelligence 表是 canonical、哪些是 derived、哪些應搬到 sidecar。
- [ ] `M0-BE-SC-009` 產出現有 schema 到新 schema 的 gap table，標明 `drop`、`rename`、`migrate`、`defer to M3`。

### Core Service And Command Surface

- [ ] `M0-BE-CM-001` 盤點現有 Tauri command 清單，對應到新畫面 use case，刪掉只有舊 UI 會呼叫的接口。
- [ ] `M0-BE-CM-002` 為 onboarding、dashboard、explorer、audit、schedule、security、settings 定義新的 command / read-model 分層。
- [ ] `M0-BE-CM-003` 區分 query API、mutating command、long-running job trigger、artifact fetch 四種接口類型。
- [ ] `M0-BE-CM-004` 為每種 mutating command 定義 preview / manual / execute 所需的 request / response envelope。
- [ ] `M0-BE-CM-005` 為 run detail / audit artifact 建立一致的 serialization contract，避免每個命令各自拼字串。
- [ ] `M0-BE-CM-006` 定義錯誤模型：user-facing error code、action hint、retry hint、support / debug payload 分層。

### Worker And Orchestration Foundation

- [ ] `M0-BE-WK-001` 盤點 [`src-tauri/crates/vault-worker/src/lib.rs`](../../../src-tauri/crates/vault-worker/src/lib.rs) 的角色，拆出 desktop run orchestration、future queue worker、MCP bridge 的邊界。
- [ ] `M0-BE-WK-002` 定義 long-running job 最小生命週期：queued、previewed、running、succeeded、failed、rolled_back、cancelled。
- [ ] `M0-BE-WK-003` 決定桌面端前景執行和背景 worker 的責任分配，避免 schedule 和 manual run 走兩套不同邏輯。
- [ ] `M0-BE-WK-004` 定義 artifact 落盤策略：preview artifact、run log、manifest、snapshot metadata、doctor report 放在哪裡。
- [ ] `M0-BE-WK-005` 決定 worker 如何回報進度、warning、partial result、final artifact index 給前端。
- [ ] `M0-BE-WK-006` 為未來 M3 job queue 預留最小抽象，但不提前把 AI 複雜度混入 M0 archive orchestration。

### Test Fixtures And Verification

- [ ] `M0-BE-QA-001` 建立 parser fixtures 目錄結構，區分 Chromium、Firefox、Safari、Takeout、damaged samples。
- [ ] `M0-BE-QA-002` 為 migration system 建立 from-scratch、upgrade-from-legacy、upgrade-twice、checksum mismatch 測試。
- [ ] `M0-BE-QA-003` 為 run ledger / timestamp contract 建立 deterministic tests，避免 timezone 和 clock 引入 flake。
- [ ] `M0-BE-QA-004` 為 Tauri command facade 建立 contract tests，確保前端型別和後端回傳一致。
- [ ] `M0-BE-QA-005` 在 M0 結束前把核心巨檔拆分到足以讓 coverage、mutation 和 review 具可讀性。
- [ ] `M0-BE-QA-006` 產出正式的重構分支順序和合併策略，避免 parser / core / schema / commands 同時改造成難以 review 的 mega PR。

---

## Exit Artifacts

- crate / module boundary map
- parser crate skeleton 和 fixture strategy
- canonical schema v1 草案與 migration ledger 設計
- 新 command surface 草案
- worker / orchestration 基礎設計
