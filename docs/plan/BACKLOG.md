# BACKLOG — 後續工作塊佇列

> 這裡只放 **half-milestone 粒度**的 work blocks。  
> `STATUS.md` 清空或完成當前 block 後，才從這裡把下一個未被阻塞的 block 搬上去。

---

## 工作塊佇列（按順序）

### Program — Quality Closeout Before M4

- [ ] **WORK-QC-D** — Intelligence, Enrichment, And 60-Year Evidence Closeout
  - 讀先：
    `docs/features/intelligence.md`
    `docs/plan/m3-intelligence/providers-indexing-and-jobs.md`
    `docs/plan/m4-full-polish/enrichment-advanced-intelligence-and-remote.md`
    `docs/plan/m4-full-polish/large-archive-performance-runbook.md`
    `docs/plan/m4-full-polish/release-readiness-runbook.md`
    `docs/plan/program/research-and-decisions.md`
  - 目標：收斂剩餘的 M3 / M4 intelligence 開放項，包含 index invalidation、embedding / storage cost read model、MCP capability / consent / lock boundary、plugin sandbox / queue integration、revisit / resurfacing、privacy review 與 60-year baseline 的 artifact-backed honest support envelope。
  - 驗收：`bun run verify`

### M1 — Solid Archive

- [ ] **WORK-M1-C** — Archive Recoverability And Operations Truth Closure
  - 讀先：
    `docs/features/archive.md`
    `docs/architecture/desktop-command-surface.md`
    `docs/plan/m1-solid-archive/schema-backup-and-ledger.md`
    `docs/plan/m1-solid-archive/schedule-security-and-storage.md`
    `docs/plan/program/research-and-decisions.md`
  - 目標：關閉 M1 尚未誠實簽收的 restore / snapshot / doctor / audit summary / retention / rekey / schedule acceptance 條目；若現況仍是 partial support，補齊 source docs、acceptance artifact 與 deferred rationale，避免把 M1 寫成已完成但缺 recoverability contract。
  - 驗收：`bun run check && bun run build`

### M2 — Recall & Trust

### M3 — Intelligence

### M4 — Full Polish

---

## 依賴關係圖

```
WORK-M0-A ──┐
WORK-M0-B ──┴── WORK-M1-A → WORK-M1-B → WORK-M2-A → WORK-M2-B → WORK-M3-A → WORK-M3-B → WORK-QC-A → WORK-QC-B → WORK-M4-A → WORK-M4-B → WORK-M4-C / WORK-M4-D / WORK-M4-E
```

---

## 維護規則

1. `STATUS.md` 清空時，從這裡頂部取最多 2 個未被阻塞的 work blocks
2. 新工作一律先放進這裡，除非使用者明確要求立刻調整 `STATUS.md`
3. 如果某個 block 需要再拆，只在 milestone 文檔的 WBS 裡拆，不把 `STATUS.md` / `BACKLOG.md` 再退回原子 task
4. 依賴解除後，直接更新行尾的 `[!blocked: ...]`
