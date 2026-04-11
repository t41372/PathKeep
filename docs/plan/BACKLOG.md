# BACKLOG — 後續工作塊佇列

> 這裡只放 **half-milestone 粒度**的 work blocks。  
> `STATUS.md` 清空或完成當前 block 後，才從這裡把下一個未被阻塞的 block 搬上去。

---

## 工作塊佇列（按順序）

### Program — Quality Closeout Before M4

- [!] **WORK-QC-F** — Optional Intelligence Runtime Boundary And Bundle Size Follow-Up [!blocked: needs explicit product / packaging sign-off if the default desktop build stops shipping optional AI / MCP / semantic runtime in-process]
  - 讀先：
    `docs/architecture/tech-stack.md`
    `docs/features/intelligence.md`
    `docs/features/deterministic-intelligence.md`
    `docs/plan/m4-full-polish/release-size-audit.md`
    `docs/plan/m4-full-polish/code-health-audit.md`
  - 目標：釐清 `lancedb` / `lance` / `datafusion` / `rig-core` 這條 optional intelligence stack 是否應繼續和 archive / shell-critical desktop runtime 同 binary shipping，或改成可選 sidecar / helper / feature-gated build boundary。
  - 驗收：產出有 trade-off 的設計決策與可重跑的 size / packaging evidence；若涉及改變 default shipping surface，必須先取得使用者明確 sign-off。

### M1 — Solid Archive

### M2 — Recall & Trust

### M3 — Intelligence

### M4 — Full Polish

### M5 — Deterministic Intelligence

---

## 依賴關係圖

```
WORK-M0-A ──┐
WORK-M0-B ──┴── WORK-M1-A → WORK-M1-B → WORK-M2-A → WORK-M2-B → WORK-M3-A → WORK-M3-B → WORK-QC-A → WORK-QC-B → WORK-M4-A → WORK-M4-B → WORK-M4-C / WORK-M4-D / WORK-M4-E / WORK-M4-F / WORK-M4-G / WORK-M4-H → WORK-QC-D → WORK-M4-J → WORK-M4-I → WORK-M4-K → WORK-M4-L → WORK-M5-A → WORK-M5-B
                     └──────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────→ WORK-QC-C → WORK-M1-C → WORK-M1-D
```

---

## 維護規則

1. `STATUS.md` 清空時，從這裡頂部取最多 2 個未被阻塞的 work blocks
2. 新工作一律先放進這裡，除非使用者明確要求立刻調整 `STATUS.md`
3. 如果某個 block 需要再拆，只在 milestone 文檔的 WBS 裡拆，不把 `STATUS.md` / `BACKLOG.md` 再退回原子 task
4. 依賴解除後，直接更新行尾的 `[!blocked: ...]`
