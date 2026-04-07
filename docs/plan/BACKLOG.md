# BACKLOG — 後續工作塊佇列

> 這裡只放 **half-milestone 粒度**的 work blocks。  
> `STATUS.md` 清空或完成當前 block 後，才從這裡把下一個未被阻塞的 block 搬上去。

---

## 工作塊佇列（按順序）

### M1 — Solid Archive

### M2 — Recall & Trust

- [ ] **WORK-M2-A** — Imports, Rollback, And Multi-Browser `[!blocked: 需要 WORK-M1-B]`
  - 讀先：`docs/features/archive.md`、`docs/features/recall.md`、`docs/plan/m2-recall-and-trust/imports-browsers-and-rollback.md`
  - 目標：完成 Google Takeout、Firefox、rollback / un-revert、Doctor 核心能力

- [ ] **WORK-M2-B** — Trust UX, I18n, And Platforms `[!blocked: 需要 WORK-M2-A]`
  - 讀先：`docs/design/ux-principles.md`、`docs/standards.md`、`docs/plan/m2-recall-and-trust/trust-ux-i18n-and-platforms.md`
  - 目標：把 PME、i18n、跨平台排程、accessibility、trust UX 做到可驗收

### M3 — Intelligence

- [ ] **WORK-M3-A** — Providers, Queue, And Indexing `[!blocked: 需要 WORK-M2-B]`
  - 讀先：`docs/features/intelligence.md`、`docs/plan/m3-intelligence/providers-indexing-and-jobs.md`
  - 目標：完成 AI provider 設定、job queue、embedding / semantic index foundation

- [ ] **WORK-M3-B** — Search, Assistant, And Insights `[!blocked: 需要 WORK-M3-A]`
  - 讀先：`docs/features/intelligence.md`、`docs/design/screens-and-nav.md`、`docs/plan/m3-intelligence/search-assistant-and-insights.md`
  - 目標：完成 semantic search、assistant、insights v1 與 evidence UX

### M4 — Full Polish

- [ ] **WORK-M4-A** — Enrichment And Remote Backup `[!blocked: 需要 WORK-M3-B]`
  - 讀先：`docs/features/intelligence.md`、`docs/features/archive.md`、`docs/plan/m4-full-polish/enrichment-advanced-intelligence-and-remote.md`
  - 目標：完成 enrichment plugin story、進階 intelligence、remote backup

- [ ] **WORK-M4-B** — Release Readiness And Platform Polish `[!blocked: 需要 WORK-M4-A]`
  - 讀先：`docs/standards.md`、`docs/plan/m4-full-polish/platform-release-and-polish.md`
  - 目標：完成 release engineering、多平台真機驗收、performance / accessibility / docs polish

---

## 依賴關係圖

```
WORK-M0-A ──┐
WORK-M0-B ──┴── WORK-M1-A → WORK-M1-B → WORK-M2-A → WORK-M2-B → WORK-M3-A → WORK-M3-B → WORK-M4-A → WORK-M4-B
```

---

## 維護規則

1. `STATUS.md` 清空時，從這裡頂部取最多 2 個未被阻塞的 work blocks
2. 新工作一律先放進這裡，除非使用者明確要求立刻調整 `STATUS.md`
3. 如果某個 block 需要再拆，只在 milestone 文檔的 WBS 裡拆，不把 `STATUS.md` / `BACKLOG.md` 再退回原子 task
4. 依賴解除後，直接更新行尾的 `[!blocked: ...]`
