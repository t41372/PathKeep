# BACKLOG — 後續工作塊佇列

> 這裡只放 **half-milestone 粒度**的 work blocks。  
> `STATUS.md` 清空或完成當前 block 後，才從這裡把下一個未被阻塞的 block 搬上去。

---

## 工作塊佇列（按順序）

> 2026-04-18 update：使用者已明確把第二台主機 benchmark parity 從當前計劃移除。current-host desktop truth audit 之後，新增一個 follow-up block，但目前仍屬 blocked。
> 2026-04-19 note：使用者直接插單的 `WORK-UI-D`（dashboard yearly rhythm + Intelligence IA cleanup）已從當輪暫時拉進 `STATUS.md` 完成並 append 到 `CHANGELOG.md`；BACKLOG 仍維持只有 blocked 的 `WORK-CI-N`。
> 2026-04-19 M6 follow-up：`WORK-M6-A` 已完成，並把下一輪 `WORK-M7-A — Cross-App Reuse Audit And Insight Entity Consolidation` 正式立項。依照 work-block 流程，`WORK-M7-A` 已從 BACKLOG 提升到 `STATUS.md` 作為當前 active block；BACKLOG 目前因此仍只剩 blocked 的 `WORK-CI-N`。
> 2026-04-19 M7 follow-up：`WORK-M7-A` 已完成，並把下一輪 `WORK-M8-A — Aggregate Entity Identity And Context Reuse` 正式立項、提升到 `STATUS.md`。BACKLOG 目前仍只剩 blocked 的 `WORK-CI-N`；M8 的 active plan 則改在 `STATUS.md` 與 `docs/plan/m8-aggregate-entity-identity/README.md` 維護。
> 2026-04-19 M8 follow-up：`WORK-M8-A` 已完成，並把下一輪 `WORK-M9-A — Remaining Reuse Inventory And Single-Source Map` 與 `WORK-M9-B — Shared Digest / CTA / Evidence Composition Extraction` 正式立項、提升到 `STATUS.md`。BACKLOG 目前再次只剩 blocked 的 `WORK-CI-N`；M9 的 active plan 改在 `STATUS.md` 與 `docs/plan/m9-cross-app-reuse/README.md` 維護。
> 2026-04-19 M9 follow-up：`WORK-M9-A` 與 `WORK-M9-B` 已完成，並把下一輪 `WORK-M10-A — Shared Review Rows And Workbench Surface Reuse` 與 `WORK-M10-B — Intelligence Route And Desktop Glue Decomposition` 正式立項、提升到 `STATUS.md`。BACKLOG 目前再次只剩 blocked 的 `WORK-CI-N`；M10 的 active plan 改在 `STATUS.md` 與 `docs/plan/m10-workbench-reuse/README.md` 維護。
> 2026-04-19 M10 follow-up：`WORK-M10-A` 與 `WORK-M10-B` 已完成，並把下一輪 `WORK-M11-A — App-Wide Reuse Inventory And Single-Source Map` 與 `WORK-M11-B — Shared Review / PME / Diagnostics Surface Extraction` 正式立項、提升到 `STATUS.md`。BACKLOG 目前再次只剩 blocked 的 `WORK-CI-N`；M11 的 active plan 改在 `STATUS.md` 與 `docs/plan/m11-app-wide-reuse/README.md` 維護。
> 2026-04-19 M11-A follow-up：`WORK-M11-A` 已完成。repo 現在已有 app-wide review grammar single-source map、`PG-RD-UX-012`、以及 M12 seed；`WORK-M11-B` 仍留在 `STATUS.md` 作為 active block。下一輪 seed 已新增 `WORK-M12-A` / `WORK-M12-B`，但在 M11-B 完成前不提前提升。

- [ ] **WORK-M12-A** — Shared Support Actions And Diagnostics Inventory
  - 讀先：
    `docs/plan/m12-support-actions-and-diagnostics/README.md`
    `docs/plan/m11-app-wide-reuse/README.md`
    `docs/design/app-wide-review-grammar-tradeoff.md`
    `docs/design/screens-and-nav.md`
    `docs/features/intelligence-current-state.md`
  - 目標：盤點全 app 還未進 shared review grammar 的 support actions、diagnostics rows、以及 Settings mega-route 的下一輪拆分機會，建立 single-source map 與 extraction boundary。
  - 契約：延續 M11 已 accepted 的 neutral review primitive / route / trusted-output 邊界；這一輪先做 inventory 與 owner map，不直接擴成大規模 route rewrite 或 transport automation 專案。
  - 驗收：source docs、inventory map、後續 `TODO: M12` 與 extraction strategy 存在

- [ ] **WORK-M12-B** — Support Action / Diagnostics Primitive Extraction
  - 讀先：
    `docs/plan/m12-support-actions-and-diagnostics/README.md`
    `docs/design/screens-and-nav.md`
    `docs/design/ux-principles.md`
    `docs/features/intelligence-current-state.md`
    `docs/plan/e2e-workflow-tests.md`
  - 目標：根據 `WORK-M12-A` 的 inventory，把至少一輪高價值的 shared support action / diagnostics primitive 抽離，優先處理 open-path / copy action、general diagnostics rows、以及 Settings / Import / Audit / Jobs 的 support summary drift。
  - 契約：只抽明確跨 consumer 重複且能降低 drift 的 grammar；不得為了抽象而重開 M11 已收斂的 route / payload / review shell contract。
  - 驗收：`bun run check && bun run build`

- [!] **WORK-CI-N** — Full Desktop Truth Pass After Locked-Archive Bootstrap Recovery
  - 讀先：
    `docs/plan/core-intelligence-desktop-truth-audit.md`
    `docs/plan/core-intelligence-progress.md`
    `docs/plan/core-intelligence-handoff.md`
    `docs/plan/e2e-workflow-tests.md`
    `docs/plan/m4-full-polish/large-archive-performance-runbook.md`
  - 目標：在 current-host shell bootstrap / Security unlock 可以穩定進入 unlocked state 之後，完成原本要求的 real-data import + Core Intelligence + 全 app Computer Use truth pass，並補齊 post-unlock profiling bundle。
  - 契約：使用 Computer Use 跑 Chrome `yi-ting` profile、archive encryption `000000`（不寫入鑰匙圈）、Import / Intelligence / Domain Deep Dive / Explorer session-trail / Settings external outputs / Jobs / Audit / Schedule / Assistant 全路由；若現有 app root 仍不符合「已清資料」前提，必須先取得使用者明確確認後才能做 destructive reset。 [!blocked: 2026-04-19 current-host live desktop 已能顯示最新 short-SHA build label、staged `/intelligence` surface 與 shared runtime status，source-level bundle drift 不再是主要 blocker；剩下的 full truth pass 阻塞點是 onboarding / import 若要重做真實 re-import，仍需要先對當前 app root 做 destructive reset，而這一步必須取得使用者明確同意]

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
WORK-M5-C → WORK-M6-A → WORK-M7-A → WORK-M8-A → WORK-M9-A → WORK-M9-B → WORK-M10-A → WORK-M10-B → WORK-M11-A → WORK-M11-B → WORK-M12-A → WORK-M12-B
```

---

## 維護規則

1. `STATUS.md` 清空時，從這裡頂部取最多 2 個未被阻塞的 work blocks
2. 新工作一律先放進這裡，除非使用者明確要求立刻調整 `STATUS.md`
3. 如果某個 block 需要再拆，只在 milestone 文檔的 WBS 裡拆，不把 `STATUS.md` / `BACKLOG.md` 再退回原子 task
4. 依賴解除後，直接更新行尾的 `[!blocked: ...]`
