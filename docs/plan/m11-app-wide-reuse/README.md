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

- [ ] `M11-001` 盤點全 app reusable review / PME / diagnostics surface
- [ ] `M11-002` 決定 mixed helper / dev mirror / transport glue 的下一輪 decomposition 邊界
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
