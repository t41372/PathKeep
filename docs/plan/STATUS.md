# STATUS.md — 當前工作

> Agent 每次開工讀這個檔案。完成任務後按 AGENTS.md 的收工流程更新。

**當前 Milestone：PG → M0 過渡期**（PG 決策收尾 + M0 前置任務）

---

## CURRENT FOCUS

> 按優先順序排。永遠挑第一個 `[ ]`（未完成）的任務。
> 完成任何 TASK 後，除了更新 `STATUS.md` / `CHANGELOG.md`，也要同步更新該任務對應的 `PG-RD-*`、milestone checklist、以及 `BACKLOG.md` 裡被解鎖的 inline `[!blocked: ...]` 標記。
> **已知流程 blocker**：`bun run check` 目前會因 repo-wide Markdown / Prettier debt 失敗。開始任何需要 commit 的任務前，先把 `BACKLOG.md` 裡的 `TASK-019` 提升進 CURRENT FOCUS 並完成，否則 agent 會被硬 gate 卡住。

- [ ] **TASK-001** — 寫 ADR: Archive Reset Strategy
- [ ] **TASK-002** — 寫 ADR: Run Model (unified run ledger)
- [ ] **TASK-003** — 寫 ADR: Rollback Visibility Model
- [ ] **TASK-004** — 從 prototype CSS 抽取 design tokens 表
- [ ] **TASK-005** — 建立 `browser-history-parser` crate skeleton

---

### TASK-001 — 寫 ADR: Archive Reset Strategy

**解鎖關係**：這個 ADR 決定了 M0 的 schema 重構策略。沒有它，後面所有 schema 相關的工作都不知道要對齊什麼。

**決策已知答案**（agent 直接採用，不需要再研究）：
- **決策**：Fresh schema。沒有真實用戶，沒有向下相容需求。新 canonical schema v1 獨立建立，現有 DB 提供一次性升級路徑。
- **理由**：`browser-history-backup` 時代的 schema 是 ad-hoc 疊加的，累積了太多 `ensure_column()` 補丁。新產品新起點比帶著舊包袱演進乾淨得多。

**讀先**：
- `docs/architecture/data-model.md`
- `docs/plan/program/research-and-decisions.md` (PG-RD-ARCH-001)
- `src-tauri/crates/vault-core/src/archive-schema.sql` — 了解現有 schema 長什麼樣

**要建立的檔案**：
- `docs/architecture/decisions/001-archive-reset-strategy.md`

**ADR 格式**：
```markdown
# ADR-001: Archive Reset Strategy

## Status
Accepted

## Context
[描述目前 ad-hoc schema 的問題]

## Decision
Fresh schema (no backward compat). 提供一次性升級轉換工具。

## Consequences
[列出 migration 工具需要做什麼]
```

**驗收**：檔案存在且包含 Status / Context / Decision / Consequences 四節。

**Commit**：`docs(adr): add ADR-001 archive reset strategy`

---

### TASK-002 — 寫 ADR: Run Model

**解鎖關係**：決定 `runs` 表是否統一設計。M0-BE-SC-003 依賴這個。

**決策已知答案**：
- **決策**：所有 run 類型（`backup`、`import`、`revert`、`doctor`、`snapshot_restore`）共用同一 `runs` ledger table，以 `run_type` enum 欄位區分。
- **理由**：統一 ledger 意味著 Audit UI 只需要一個 query surface，run ID 可以跨類型引用，manifest chain 不需要拆 table。

**讀先**：
- `docs/architecture/data-model.md` (Section 2: Schema 演化)
- `docs/plan/program/research-and-decisions.md` (PG-RD-ARCH-003)
- `docs/plan/m0-foundation/backend-and-data-rearchitecture.md` (M0-BE-SC-003)

**要建立的檔案**：
- `docs/architecture/decisions/002-run-model.md`

**決策需覆蓋**：
1. `runs` 表的最小欄位集：id, run_type, trigger_source, started_at_ms, finished_at_ms, timezone, result, artifact_index
2. `run_type` 的合法值：`backup | import | revert | doctor | snapshot_restore`
3. `run_artifacts` 如何關聯 run

**驗收**：檔案存在，包含 runs 表欄位草案。

**Commit**：`docs(adr): add ADR-002 unified run model`

---

### TASK-003 — 寫 ADR: Rollback Visibility Model

**解鎖關係**：M0-BE-SC-004 和 M2-IR-RB-001 都依賴這個。決定了哪些表需要 `reverted_at` 欄位。

**決策已知答案**：
- **決策**：Soft-delete via `reverted_at` + `reverted_by_run_id` columns on rows that were inserted by a run (i.e. `visits`, `urls`, `downloads`, `search_terms`).
- **Immutable raw facts**：`runs`, `manifests`, `raw_row_versions` — 這些永遠不標 reverted，只記歷史。
- **Derived state**：`fts_index`, embedding sidecar — 回滾時直接刪除並標記 stale，讓重建任務去處理。
- **可見性查詢規則**：所有 query 預設加上 `WHERE reverted_at IS NULL`，除非明確要看「包含回滾資料的完整視圖」。

**讀先**：
- `docs/plan/program/research-and-decisions.md` (PG-RD-ARCH-004)
- `docs/plan/m0-foundation/backend-and-data-rearchitecture.md` (M0-BE-SC-004)
- `docs/features/archive.md` (Section 6: 審計與可信性)

**要建立的檔案**：
- `docs/architecture/decisions/003-rollback-visibility-model.md`

**ADR 必須回答**：
1. 哪些表有 `reverted_at` / `reverted_by_run_id`
2. 哪些表是 immutable raw facts（不可 revert）
3. 前端 query 的預設可見性規則
4. un-revert 的語義（清空 `reverted_at`）

**驗收**：檔案存在，明確列出哪些表 mutable / immutable。

**Commit**：`docs(adr): add ADR-003 rollback visibility model`

---

### TASK-004 — 從 prototype CSS 抽取 design tokens 表

**解鎖關係**：M0-FE-DS-001 和 M0-FE-DS-002 的先決條件。前端重寫需要 token 表才能建立 design system。

**決策**：這不是 opinion task，是 extraction task。從 prototype 的 `style.css` 精確抽出現有的 CSS 變數和設計決策，不需要新建。

**讀先**：
- `reference/PathKeep — Desktop UI Design/style.css` (如果存在，否則找 `reference/` 目錄下的 HTML/CSS prototype 檔案)
- `docs/design/ux-principles.md` — 確認 dark-first、data density 等原則
- `docs/plan/program/research-and-decisions.md` (PG-RD-UX-003)

**要建立的檔案**：
- `docs/design/design-tokens.md`

**文檔格式要求**（每一類 token 都要有）：
```markdown
## Colors
| Token name | Dark value | Light value | Semantic meaning |
|...

## Typography
| Token | Value | Usage |
|...

## Spacing
| Token | Value | Usage |
|...

## Radius, Shadow, Motion
...
```

**如果找不到 prototype 檔案**：在文檔裡明確寫 `STATUS: prototype CSS not found, tokens derived from screenshots and ux-principles.md`，然後用 `ux-principles.md` 裡的原則建立最小 token 集（dark-first、適中 density、tech aesthetic）。

**驗收**：`docs/design/design-tokens.md` 存在，至少包含 colors / typography / spacing 三類 token。

**Commit**：`docs(design): extract design tokens from prototype`

---

### TASK-005 — 建立 `browser-history-parser` crate skeleton

**解鎖關係**：M0-BE-PR-001。這個 crate skeleton 是 M0 後端重構的起點，也是之後所有 parser 工作的落地點。

**決策已知答案**：
- `browser-history-parser` 是獨立 crate，不依賴 Tauri、不依賴 canonical schema、不依賴 keyring。
- Installed profile discovery、權限檢查、staging copy 留在 `vault-platform` / `vault-core`；parser crate 只做 parsing 和對已提供路徑的 inspection helper。
- 它的 public API 接收 `&Path` 或 staging 路徑，輸出 typed structs（`ParsedVisit`, `ParsedUrl`, `ParsedDownload`, `ParsedSearchTerm`）。

**讀先**：
- `docs/features/archive.md` (Section 3: 模塊化設計)
- `docs/plan/m0-foundation/backend-and-data-rearchitecture.md` (M0-BE-PR-001 到 PR-003)
- `src-tauri/Cargo.toml` — 了解現有 workspace 結構
- `src-tauri/crates/vault-core/src/chrome.rs` — 了解現有 Chromium 解析邏輯在哪（之後搬過來）

**要建立或修改的檔案**：
```
src-tauri/crates/browser-history-parser/
  Cargo.toml
  src/
    lib.rs           ← public API re-exports
    chromium/
      mod.rs         ← history parsing + provided-path inspection helper
    firefox/
      mod.rs         ← stub
    safari/
      mod.rs         ← stub
    takeout/
      mod.rs         ← stub
    types.rs         ← ParsedVisit, ParsedUrl, ParsedDownload, ParsedSearchTerm
    error.rs         ← ParseError enum
```

**修改**：
- `src-tauri/Cargo.toml` 的 `[workspace]` members 加入 `crates/browser-history-parser`

**Cargo.toml 依賴**：只允許 `rusqlite`、`serde`、`thiserror`、`chrono`。不能加 Tauri。

**驗收**：
```bash
cd src-tauri
cargo build -p browser-history-parser
cargo test -p browser-history-parser
```
兩個命令都要通過（即使 firefox/safari/takeout stub 是 `todo!()` 也沒關係，但 chromium 至少要有編譯通過的基礎 struct 定義）。

**Commit**：`feat(parser): add browser-history-parser crate skeleton`

---

> 做完了？→ 按 [AGENTS.md](../../AGENTS.md) 的收工流程更新本檔案，然後從 [BACKLOG.md](BACKLOG.md) 補充新任務。
> 歷史紀錄 → [CHANGELOG.md](CHANGELOG.md)
