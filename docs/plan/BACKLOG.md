# BACKLOG — 後續工作塊佇列

> 這裡只放 **half-milestone 粒度**的 work blocks。  
> `STATUS.md` 清空或完成當前 block 後，才從這裡把下一個未被阻塞的 block 搬上去。

---

## 工作塊佇列（按順序）

- [ ] **WORK-CI-J** — Alternate-Host Intelligence Evidence And Recovery Parity
  - 讀先：
    `docs/plan/core-intelligence-progress.md`
    `docs/plan/core-intelligence-handoff.md`
    `docs/plan/program/research-and-decisions.md`
    `docs/features/intelligence-current-state.md`
  - 目標：把 current-host 已完成的 `14.4M / 60y` long-horizon replay、expired-lease recovery、與 queue/runtime evidence，在 alternate host 上重跑成可 review 的 parity artifact，而不是只沿用 current-host 結論。
  - 契約：不得把 `WORK-CI-C` 已完成的 cleanup / signoff 重新打開；若 alternate host 的 keychain、filesystem 或 desktop capability 造成新差異，必須以新 artifact / runbook 誠實描述，而不是補 compatibility 敘事。
  - 驗收：alternate-host benchmark / recovery artifact bundle、對應 runbook / docs 回寫，以及明確的 pass / fail 結論

---

## 依賴關係圖

```
WORK-M0-A ──┐
WORK-M0-B ──┴── WORK-M1-A → WORK-M1-B → WORK-M2-A → WORK-M2-B → WORK-M3-A → WORK-M3-B → WORK-QC-A → WORK-QC-B → WORK-M4-A → WORK-M4-B → WORK-M4-C / WORK-M4-D / WORK-M4-E / WORK-M4-F / WORK-M4-G / WORK-M4-H → WORK-QC-D → WORK-M4-J → WORK-M4-I → WORK-M4-K → WORK-M4-L → WORK-M5-A → WORK-M5-B
                     └──────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────→ WORK-QC-C → WORK-M1-C → WORK-M1-D
WORK-QC-T → WORK-CI-B / WORK-CI-F
WORK-CI-F → WORK-CI-H
WORK-CI-B → WORK-CI-C
WORK-CI-H → WORK-CI-I
WORK-CI-C → WORK-CI-J
```

---

## 維護規則

1. `STATUS.md` 清空時，從這裡頂部取最多 2 個未被阻塞的 work blocks
2. 新工作一律先放進這裡，除非使用者明確要求立刻調整 `STATUS.md`
3. 如果某個 block 需要再拆，只在 milestone 文檔的 WBS 裡拆，不把 `STATUS.md` / `BACKLOG.md` 再退回原子 task
4. 依賴解除後，直接更新行尾的 `[!blocked: ...]`
