# BACKLOG — 後續工作塊佇列

> 這裡只放 **half-milestone 粒度**的 work blocks。  
> `STATUS.md` 清空或完成當前 block 後，才從這裡把下一個未被阻塞的 block 搬上去。

---

## 工作塊佇列（按順序）

- [ ] **WORK-CI-H** — Core Intelligence External Output Consumers And Host Integrations
  - 讀先：
    `docs/features/core-intelligence-ultimate-design.md`
    `docs/plan/core-intelligence-progress.md`
    `docs/plan/core-intelligence-handoff.md`
    `docs/design/screens-and-nav.md`
  - 目標：把目前只存在於 backend payload-provider commands 的 `embed/widget/public snapshot` surface，收斂成真正的 consumer / host integration contract，而不是停在「後端可回傳資料，但產品沒有地方用」。
  - 契約：先保持 `/intelligence` 主產品 surface 與 runtime truth 穩定，再擴到 widget / snippet / public snapshot；不得把 payload provider 冒充成完整 external integration。
  - 驗收：payload shape、consumer surface、權限 / honesty copy、source docs 與驗收路徑同步落地。
  - [!blocked: 需先完成 `WORK-CI-B` / `WORK-CI-F` 的主產品 finish-line truth pass]

- [ ] **WORK-CI-C** — Core Intelligence Legacy Cleanup And Long-Horizon Signoff
  - 讀先：
    `docs/plan/core-intelligence-progress.md`
    `docs/plan/core-intelligence-handoff.md`
    `docs/plan/program/research-and-decisions.md`
    `docs/architecture/data-model.md`
  - 目標：在主產品 cutover 穩定後，正式關掉 remaining legacy `vault-core::insights` 責任、完成 large-archive / low-RAM / queue-recovery signoff，並把後續 deterministic runtime complexity / resume strategy 收口成 accepted truth。
  - 契約：所有刪舊與性能收口都要以 current Core Intelligence contract 為中心，不可再為 legacy snapshot-first path 補 compatibility 層。
  - 驗收：source docs、benchmark artifact、cleanup diff、以及對應 quality / manual recipe 都存在。
  - [!blocked: 需先完成 `WORK-CI-B`，並把未提交的 runtime WIP 收口成可驗證事實]

---

## 依賴關係圖

```
WORK-M0-A ──┐
WORK-M0-B ──┴── WORK-M1-A → WORK-M1-B → WORK-M2-A → WORK-M2-B → WORK-M3-A → WORK-M3-B → WORK-QC-A → WORK-QC-B → WORK-M4-A → WORK-M4-B → WORK-M4-C / WORK-M4-D / WORK-M4-E / WORK-M4-F / WORK-M4-G / WORK-M4-H → WORK-QC-D → WORK-M4-J → WORK-M4-I → WORK-M4-K → WORK-M4-L → WORK-M5-A → WORK-M5-B
                     └──────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────→ WORK-QC-C → WORK-M1-C → WORK-M1-D
WORK-QC-T → WORK-CI-B / WORK-CI-F → WORK-CI-H → WORK-CI-C
```

---

## 維護規則

1. `STATUS.md` 清空時，從這裡頂部取最多 2 個未被阻塞的 work blocks
2. 新工作一律先放進這裡，除非使用者明確要求立刻調整 `STATUS.md`
3. 如果某個 block 需要再拆，只在 milestone 文檔的 WBS 裡拆，不把 `STATUS.md` / `BACKLOG.md` 再退回原子 task
4. 依賴解除後，直接更新行尾的 `[!blocked: ...]`
