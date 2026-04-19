# BACKLOG — 後續工作塊佇列

> 這裡只放 **half-milestone 粒度**的 work blocks。  
> `STATUS.md` 清空或完成當前 block 後，才從這裡把下一個未被阻塞的 block 搬上去。

---

## 工作塊佇列（按順序）

> 2026-04-18 update：使用者已明確把第二台主機 benchmark parity 從當前計劃移除。current-host desktop truth audit 之後，新增一個 follow-up block，但目前仍屬 blocked。

- [!] **WORK-CI-N** — Full Desktop Truth Pass After Locked-Archive Bootstrap Recovery
  - 讀先：
    `docs/plan/core-intelligence-desktop-truth-audit.md`
    `docs/plan/core-intelligence-progress.md`
    `docs/plan/core-intelligence-handoff.md`
    `docs/plan/e2e-workflow-tests.md`
    `docs/plan/m4-full-polish/large-archive-performance-runbook.md`
  - 目標：在 current-host shell bootstrap / Security unlock 可以穩定進入 unlocked state 之後，完成原本要求的 real-data import + Core Intelligence + 全 app Computer Use truth pass，並補齊 post-unlock profiling bundle。
  - 契約：使用 Computer Use 跑 Chrome `yi-ting` profile、archive encryption `000000`（不寫入鑰匙圈）、Import / Intelligence / Domain Deep Dive / Explorer session-trail / Settings external outputs / Jobs / Audit / Schedule / Assistant 全路由；若現有 app root 仍不符合「已清資料」前提，必須先取得使用者明確確認後才能做 destructive reset。 [!blocked: source 已補上 locked-snapshot degradation、unlock fail-fast 與 short-SHA build label，但 current-host fresh desktop relaunch 仍顯示舊的 generic dashboard copy 與不帶 SHA 的 shell chrome；這代表 host-side stale WebView / bundle cache drift 仍攔在 full truth pass 前面，必要時還需要使用者明確同意 reset 當前 app root]

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
```

---

## 維護規則

1. `STATUS.md` 清空時，從這裡頂部取最多 2 個未被阻塞的 work blocks
2. 新工作一律先放進這裡，除非使用者明確要求立刻調整 `STATUS.md`
3. 如果某個 block 需要再拆，只在 milestone 文檔的 WBS 裡拆，不把 `STATUS.md` / `BACKLOG.md` 再退回原子 task
4. 依賴解除後，直接更新行尾的 `[!blocked: ...]`
