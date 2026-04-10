# BACKLOG — 後續工作塊佇列

> 這裡只放 **half-milestone 粒度**的 work blocks。  
> `STATUS.md` 清空或完成當前 block 後，才從這裡把下一個未被阻塞的 block 搬上去。

---

## 工作塊佇列（按順序）

### Program — Quality Closeout Before M4

### M1 — Solid Archive

### M2 — Recall & Trust

### M3 — Intelligence

### M4 — Full Polish

- [ ] **WORK-M4-L** — Package Rename, Release Flow, Size Audit, And Code Health
  - 讀先：
    `AGENTS.md`
    `RELEASE.md`
    `docs/standards.md`
    `docs/plan/m4-full-polish/README.md`
    `docs/plan/program/repo-baseline.md`
  - 目標：把 bundle / keyring / data-root namespace 正式改成 `com.yi-ting.pathkeep`，建立真實 version bump / release runbook，並補上 artifact size attribution 與前後端 code health audit，避免發版準備只停在現有 private-release workflow。
  - 驗收：`bun run verify`

### M5 — Deterministic Intelligence

- [ ] **WORK-M5-A** — Deterministic Evidence Contract, Foundation, And Taxonomy [!blocked: requires ADR-006 acceptance before replacing current session/dwell-centric intelligence contract]
  - 讀先：
    `docs/architecture/decisions/006-deterministic-intelligence-boundary.md`
    `docs/features/deterministic-intelligence.md`
    `docs/features/intelligence.md`
    `docs/architecture/data-model.md`
    `docs/architecture/module-boundary-map.md`
    `docs/architecture/tech-stack.md`
  - 目標：凍結 honest evidence contract、URL normalization / registrable-domain / search-parser baseline、多維 taxonomy precedence、region rule packs、script-aware tokenization 與 unknown / override governance，避免 deterministic intelligence 再建立在 estimated dwell 或 session-duration 假設上。
  - 驗收：`bun run check && bun run build`

- [ ] **WORK-M5-B** — Query Groups, Threads, Reference Pages, And Module Registry [!blocked: depends on WORK-M5-A]
  - 讀先：
    `docs/features/deterministic-intelligence.md`
    `docs/plan/m5-deterministic-intelligence/README.md`
    `docs/plan/m5-deterministic-intelligence/groups-threads-and-surfaces.md`
    `docs/plan/m4-full-polish/intelligence-60-year-envelope.md`
    `docs/design/screens-and-nav.md`
  - 目標：把 query groups、query ladders、cross-day thread merge、open loops、source effectiveness、reference pages 與 deterministic module registry 做成 explainable、可重建、可 profile-scope、可 invalidate 的正式 shipping surface。
  - 驗收：`bun run check && bun run build`

---

## 依賴關係圖

```
WORK-M0-A ──┐
WORK-M0-B ──┴── WORK-M1-A → WORK-M1-B → WORK-M2-A → WORK-M2-B → WORK-M3-A → WORK-M3-B → WORK-QC-A → WORK-QC-B → WORK-M4-A → WORK-M4-B → WORK-M4-C / WORK-M4-D / WORK-M4-E / WORK-M4-F / WORK-M4-G / WORK-M4-H → WORK-QC-D → WORK-M4-J → WORK-M4-I → WORK-M4-K → WORK-M4-L
                     └──────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────→ WORK-QC-C → WORK-M1-C → WORK-M1-D
```

---

## 維護規則

1. `STATUS.md` 清空時，從這裡頂部取最多 2 個未被阻塞的 work blocks
2. 新工作一律先放進這裡，除非使用者明確要求立刻調整 `STATUS.md`
3. 如果某個 block 需要再拆，只在 milestone 文檔的 WBS 裡拆，不把 `STATUS.md` / `BACKLOG.md` 再退回原子 task
4. 依賴解除後，直接更新行尾的 `[!blocked: ...]`
