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

- [!] **WORK-M4-C** — Secure App Lock And Profile Partitions `[!blocked: 先完成 PG-RD-PLAT-006，釐清 biometric / passcode / session-key security model]`
  - 讀先：
    - `docs/vision-and-requirements.md`
    - `docs/features/archive.md`
    - `docs/features/recall.md`
    - `docs/features/intelligence.md`
    - `docs/design/screens-and-nav.md`
    - `docs/plan/program/research-and-decisions.md`
  - 範圍：
    1. 定義 app lock 保護範圍：僅 UI session、資料庫解鎖、或兩者結合
    2. 研究 macOS / Windows / Linux 的 biometric / passcode / keyring fallback 與 recovery story
    3. 決定 shared profile scope 是否升級為真正的 per-profile partition，特別是 Insights / Dashboard 的 read model 邊界

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
