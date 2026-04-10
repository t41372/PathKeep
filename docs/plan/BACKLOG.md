# BACKLOG — 後續工作塊佇列

> 這裡只放 **half-milestone 粒度**的 work blocks。  
> `STATUS.md` 清空或完成當前 block 後，才從這裡把下一個未被阻塞的 block 搬上去。

---

## 工作塊佇列（按順序）

### Program — Quality Closeout Before M4

### M1 — Solid Archive

- [ ] **WORK-M1-D** — Snapshot Restore, Retention, And Rekey Audit Shipping
  - 讀先：
    `docs/features/archive.md`
    `docs/architecture/desktop-command-surface.md`
    `docs/plan/m1-solid-archive/schema-backup-and-ledger.md`
    `docs/plan/m1-solid-archive/schedule-security-and-storage.md`
    `docs/plan/program/research-and-decisions.md`
  - 目標：把目前只停在 deferred / partial support 的 snapshot restore preview / execute、retention / prune、以及 richer rekey audit summary 拉回真正可 shipping 的 recoverability contract，而不是永遠停在 truth-closeout 文檔上。
  - 驗收：`bun run verify`

### M2 — Recall & Trust

### M3 — Intelligence

### M4 — Full Polish

- [ ] **WORK-M4-K** — Security, Privacy, And Update Boundary
  - 讀先：
    `docs/architecture/decisions/005-app-lock-session-boundary.md`
    `docs/features/archive.md`
    `docs/design/screens-and-nav.md`
    `docs/standards.md`
    `RELEASE.md`
  - 目標：先以決策文檔誠實重開 macOS biometric 與 consented analytics 兩個 accepted-contract 變更，再落地 macOS-only biometric unlock、frontend-only analytics consent boundary，以及 Settings manual update check / release availability surface。
  - 驗收：`bun run verify`

- [ ] **WORK-M4-L** — Package Rename, Release Flow, Size Audit, And Code Health
  - 讀先：
    `AGENTS.md`
    `RELEASE.md`
    `docs/standards.md`
    `docs/plan/m4-full-polish/README.md`
    `docs/plan/program/repo-baseline.md`
  - 目標：把 bundle / keyring / data-root namespace 正式改成 `com.yi-ting.pathkeep`，建立真實 version bump / release runbook，並補上 artifact size attribution 與前後端 code health audit，避免發版準備只停在現有 private-release workflow。
  - 驗收：`bun run verify`

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
