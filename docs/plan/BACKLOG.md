# BACKLOG — 待辦任務佇列

> 這裡是所有尚未進入 STATUS.md 的任務。Agent 只在 STATUS.md 清空後才來這裡取任務。
> 取任務規則：從頂部開始，跳過帶有 inline `[!blocked: ...]` 標記的任務，取最多 5 個搬到 STATUS.md。
> 本檔**沒有獨立 BLOCKED 表**；blocked 狀態只用任務行尾的 `[!blocked: ...]` 標記表示。依賴解除後，直接移除該標記。

---

## 任務佇列（按優先順序）

### Phase 0 收尾

- [ ] **TASK-006** — 凍結 timestamp contract
  - 讀先：`docs/architecture/data-model.md` Section 1
  - 建：`docs/architecture/decisions/004-timestamp-contract.md`
  - Commit：`docs(adr): add ADR-004 timestamp contract`

### Phase 1 — M0 Backend

- [ ] **TASK-007** — 寫 canonical schema v1 migration 檔案 `[!blocked: 需要 TASK-002,003,006]`
  - 讀先：`docs/architecture/data-model.md`, ADR-001 ~ ADR-004
  - 建：`src-tauri/crates/vault-core/src/migrations/001_init.sql`
  - Commit：`feat(schema): add canonical schema v1 migration 001_init.sql`

- [ ] **TASK-008** — 實作 migration executor `[!blocked: 需要 TASK-007]`
  - 讀先：`docs/architecture/data-model.md` Section 2, ADR-001
  - 建：`src-tauri/crates/vault-core/src/migrations/mod.rs`
  - 驗收：`cargo test -p vault-core -- migrations`
  - Commit：`feat(schema): implement migration executor with checksum ledger`

- [ ] **TASK-009** — 拆分 archive.rs：抽出 schema bootstrapping `[!blocked: 需要 TASK-008]`
  - 讀先：`src-tauri/crates/vault-core/src/archive.rs`, `docs/plan/m0-foundation/backend-and-data-rearchitecture.md` (M0-BE-MD-001)
  - 驗收：`cargo test -p vault-core`
  - Commit：`refactor(vault-core): replace ad-hoc schema bootstrap with migration executor`

- [ ] **TASK-010** — 把 Chromium 解析邏輯搬到 browser-history-parser `[!blocked: 需要 TASK-005]`
  - 讀先：`src-tauri/crates/vault-core/src/chrome.rs`, `docs/plan/m0-foundation/backend-and-data-rearchitecture.md` (M0-BE-PR-004)
  - 驗收：`cargo test --workspace`
  - Commit：`refactor(parser): move chromium parsing logic to browser-history-parser crate`

- [ ] **TASK-011** — 定義新 Tauri command surface (草案)
  - 讀先：`src-tauri/src/lib.rs`, `docs/design/screens-and-nav.md`
  - 建：`docs/plan/m0-foundation/command-surface-v1.md`
  - Commit：`docs(m0): draft new Tauri command surface for new UI`

### Phase 2 — M0 Frontend

- [ ] **TASK-012** — 刪除舊 UI 入口，建立新 shell skeleton `[!blocked: 需要 TASK-004]`
  - 讀先：`src/main.tsx`, `src/AppNew.tsx`, `docs/design/design-tokens.md`
  - 建：`src/app/shell.tsx`, `src/app/router.tsx`
  - 驗收：`bun run typecheck && bun run test:unit`
  - Commit：`feat(shell): bootstrap new app shell with route tree`

- [ ] **TASK-013** — 建立 CSS token layer `[!blocked: 需要 TASK-004]`
  - 讀先：`docs/design/design-tokens.md`
  - 建：`src/styles/tokens.css`, `src/styles/reset.css`
  - 驗收：`bun run typecheck`
  - Commit：`feat(design): add CSS token layer from design tokens`

- [ ] **TASK-014** — 建立所有 page skeletons
  - 讀先：`src/app/router.tsx`, `docs/design/screens-and-nav.md`
  - 驗收：`bun run typecheck && bun run test:unit`
  - Commit：`feat(pages): add all page skeletons for new shell`

- [ ] **TASK-015** — 重寫 Playwright e2e smoke target
  - 讀先：`tests/e2e/shell.spec.ts`, `docs/plan/m0-foundation/rename-quality-and-cutover.md` (M0-CO-QA-003)
  - 驗收：`bun run test:e2e`
  - Commit：`test(e2e): rewrite shell smoke spec for new app shell`

### Phase 3 — M0 Cutover & Naming

- [ ] **TASK-016** — 命名遷移：package.json + Tauri config
  - 讀先：`package.json`, `src-tauri/tauri.conf.json`
  - 驗收：`bun run typecheck && cargo build --manifest-path src-tauri/Cargo.toml`
  - Commit：`chore(rename): update package.json and tauri.conf.json to PathKeep`

- [ ] **TASK-017** — 重寫 README
  - 讀先：`README.md`, `docs/vision-and-requirements.md`
  - Commit：`docs(readme): rewrite README for PathKeep pivot`

- [ ] **TASK-018** — M0 完成驗收 checklist
  - 讀先：`docs/plan/m0-foundation/README.md`
  - 驗收：`bun run typecheck`, `bun run test:unit`, `bun run test:e2e`, `cargo test --workspace`
  - Commit：`chore(m0): M0 milestone complete — update STATUS.md`

---

## 依賴關係圖

```
TASK-001 (ADR reset) ──┐
TASK-002 (ADR run)   ──┤
TASK-003 (ADR rollback)┤── TASK-007 (schema v1) → TASK-008 (migration) → TASK-009 (split archive.rs)
TASK-006 (ADR timestamp)┘
TASK-004 (design tokens) ── TASK-012 (new shell) + TASK-013 (CSS tokens)
TASK-005 (parser crate) ── TASK-010 (move chromium logic)
M0 全部完成 → M1 開始
```

---

## 如何加新任務

1. 新任務加在佇列的適當位置（按 Phase 和依賴順序）
2. 如果有依賴，在任務行尾加 `[!blocked: 需要 TASK-XXX]`
3. 如果依賴已被解決，移除 `[!blocked: ...]` 標記
4. 任務編號持續遞增，不重用已完成的編號
