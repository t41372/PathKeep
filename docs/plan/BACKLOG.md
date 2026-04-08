# BACKLOG — 後續工作塊佇列

> 這裡只放 **half-milestone 粒度**的 work blocks。  
> `STATUS.md` 清空或完成當前 block 後，才從這裡把下一個未被阻塞的 block 搬上去。

---

## 工作塊佇列（按順序）

### Program — Quality Closeout Before M4

- [ ] **WORK-QC-B** — Close Remaining M0-M3 Product And Doc Debt `[!blocked: 需要 WORK-QC-A]`
  - 讀先：`docs/vision-and-requirements.md`、`docs/design/ux-principles.md`、`docs/design/screens-and-nav.md`、`docs/plan/m0-foundation/README.md`、`docs/plan/m1-solid-archive/README.md`、`docs/plan/m3-intelligence/README.md`
  - 目標：把 M0-M3 還沒真正簽收的產品與文檔債收乾淨，包含 prototype / design gap、trust-critical i18n、desktop-vs-preview 驗收邊界、On This Day / evidence / timezone 等與設計或需求不一致的實作

### M1 — Solid Archive

### M2 — Recall & Trust

### M3 — Intelligence

### M4 — Full Polish

- [ ] **WORK-M4-A** — Enrichment And Remote Backup `[!blocked: 需要 WORK-QC-A、WORK-QC-B]`
  - 讀先：`docs/features/intelligence.md`、`docs/features/archive.md`、`docs/plan/m4-full-polish/enrichment-advanced-intelligence-and-remote.md`
  - 目標：在已完成 quality closeout 的 archive + intelligence v1 之上，補齊 enrichment plugin system、advanced intelligence 與 remote backup 的第一個可驗收 slice

- [ ] **WORK-M4-B** — Release Readiness And Platform Polish `[!blocked: 需要 WORK-M4-A，且 WORK-QC-A / WORK-QC-B 不能回退]`
  - 讀先：`docs/standards.md`、`docs/plan/m4-full-polish/platform-release-and-polish.md`
  - 目標：完成 release engineering、多平台真機驗收、performance / accessibility / docs polish

---

## 依賴關係圖

```
WORK-M0-A ──┐
WORK-M0-B ──┴── WORK-M1-A → WORK-M1-B → WORK-M2-A → WORK-M2-B → WORK-M3-A → WORK-M3-B → WORK-QC-A → WORK-QC-B → WORK-M4-A → WORK-M4-B
```

---

## 維護規則

1. `STATUS.md` 清空時，從這裡頂部取最多 2 個未被阻塞的 work blocks
2. 新工作一律先放進這裡，除非使用者明確要求立刻調整 `STATUS.md`
3. 如果某個 block 需要再拆，只在 milestone 文檔的 WBS 裡拆，不把 `STATUS.md` / `BACKLOG.md` 再退回原子 task
4. 依賴解除後，直接更新行尾的 `[!blocked: ...]`
