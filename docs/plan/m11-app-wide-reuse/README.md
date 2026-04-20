# M11 — App-Wide Reuse Audit And Shared Review Grammar

> 目標：在 M10 已把 intelligence subtree 的 shared workbench 與 transport hygiene 收斂後，繼續從全 app 角度盤點仍然重複造輪子的 review / PME / diagnostics surface，避免 reusable grammar 只停留在 `/intelligence` 與 Settings external outputs。

---

## M11 的完成定義

- [ ] 盤點 app-wide reusable review / PME / diagnostics grammar，建立 single-source map
- [ ] 決定哪些剩餘 mixed helper / dev mirror / pass-through glue 值得正式拆分，哪些只保留 inventory
- [ ] 至少完成一輪跨 route 的 shared review / PME / diagnostics primitive extraction
- [ ] 把新的 deferred gap 誠實改記後續 milestone，而不是讓 M11 無限膨脹

---

## 首批範圍

- `src/lib/intelligence.ts` 仍混雜的 route grammar / evidence / assistant helper
- `src-tauri/src/dev_ipc_bridge.rs` intelligence mirror drift
- `vault-worker` / worker bridge 剩餘 purely mechanical pass-through inventory
- Settings / Jobs / Import / Audit 之間的 shared review row / code preview / PME grammar
- 全 app diagnostics / copy-export / verify surface 的 reusable target-link / status-row extraction

---

## 當前待辦

- [x] `M11-001` 盤點全 app reusable review / PME / diagnostics surface
- [x] `M11-002` 決定 mixed helper / dev mirror / transport glue 的下一輪 decomposition 邊界
- [ ] `M11-003` 抽出一輪跨 route shared review grammar
- [ ] `M11-004` 回寫 source docs、status、backlog 與 `TODO: M11` inventory

---

## 建議工作塊

- `WORK-M11-A` — App-Wide Reuse Inventory And Single-Source Map
- `WORK-M11-B` — Shared Review / PME / Diagnostics Surface Extraction

---

## 已知種子與參考

- [../m10-workbench-reuse/README.md](../m10-workbench-reuse/README.md)
- [../../design/intelligence-workbench-transport-hygiene-tradeoff.md](../../design/intelligence-workbench-transport-hygiene-tradeoff.md)
- [../../design/screens-and-nav.md](../../design/screens-and-nav.md)
- [../../features/intelligence-current-state.md](../../features/intelligence-current-state.md)
- [../../features/core-intelligence-ultimate-design.md](../../features/core-intelligence-ultimate-design.md)

---

## 邊界

- 不回退已 accepted 的 route grammar、command names、payload shape
- 不為了「檔案看起來小一點」而做沒有 owner payoff 的 mechanical split
- 優先處理跨 consumer 明確重複、且能降低 drift 的 review / PME / diagnostics grammar
- 若要重開更深的 transport generation / manifest / codegen，需要先單獨立 trade-off

---

## Single-Source Map

| 契約 / 能力 | canonical owner | 目前狀態 |
| --- | --- | --- |
| entity route / search-param grammar | `src/lib/core-intelligence/routes.ts` | accepted；M11 明確禁止再回 `src/lib/intelligence.ts` 混寫 |
| status / tone / label policy | `src/lib/trust-review.ts`、`src/lib/intelligence-runtime.ts`、`src/lib/intelligence-presentation.ts` | accepted |
| app-level diagnostics capture | `src/lib/runtime-diagnostics.ts` | accepted |
| neutral review shell | `src/components/review/` | M11-B 正式升格；原 `src/components/intelligence/workbench/review-surface.tsx` 只留 compatibility re-export |
| transport chain | front-end client → IPC bridge → Tauri command / dev bridge → worker bridge → `vault-worker` | M11 只做 inventory / boundary，不做 codegen |

---

## Consumer-Local Drift Inventory

### M11-B 立即抽取

- Settings remote backup PME、AI integration preview、external-output local host
- Schedule 的 PME tab、generated-file preview、verify result rows
- Audit 的 artifact rows
- Jobs 的 recent AI/runtime job rows與 action footer

### 留到 M12

- Settings general diagnostics path rows / support actions
- Import 的 browser-profile review cards、batch review / doctor follow-through
- Audit restore preview / related import review deeper extraction
- Jobs plugin / module summary rows
- copy / open-path action reuse 與 transport parity automation feasibility

---

## 2026-04-19 WORK-M11-A Closeout

- 新增 [`../../design/app-wide-review-grammar-tradeoff.md`](../../design/app-wide-review-grammar-tradeoff.md)，正式接受「先做 single-source map，再只抽第一輪 neutral review primitive」的 M11 邊界。
- `src/lib/intelligence.ts` 的結論已定案：route href / entity label helper 回 `src/lib/core-intelligence/routes.ts`；AI/provider/assistant presentation 進 `src/lib/intelligence-ai-presentation.ts`；evidence / assistant link 與 citation dedupe 進 `src/lib/intelligence-links.ts`；barrel 只留 compatibility role。
- `src-tauri/src/dev_ipc_bridge.rs` 與 `src-tauri/src/worker_bridge/intelligence.rs` / `vault-worker` 的後續也已定案：M11 不再機械拆 transport；只有在 M12 inventory 證明 owner drift / parity cost 持續存在時才重開。
- 下一輪 seed 已整理到 [`../m12-support-actions-and-diagnostics/README.md`](../m12-support-actions-and-diagnostics/README.md)，主題固定為 support actions、diagnostics rows、Settings mega-route further split，以及更輕量的 transport parity automation 評估。
