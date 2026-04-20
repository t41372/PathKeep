# M10 — Workbench Reuse And Transport Hygiene

> 目標：在 M9 已把 route-level shared composition 收斂後，繼續清理尚未抽出的 workbench/review rows 與 route/desktop glue duplication，但不推翻既有 entity-first / focus / trusted-output contract。

---

## M10 的完成定義

- [ ] 收斂仍然 consumer-local 的 workbench/review row composition
- [ ] 決定哪些 intelligence route / desktop glue 值得正式拆分，哪些保留現狀
- [ ] 清理 M9 留下的 `TODO: M10`
- [ ] 把新的 deferred gap 改記後續 milestone，而不是讓 M10 無限膨脹

---

## 首批範圍

- `refind` overview/day/detail route 的 shared summary/detail chrome
- Explorer detail rail、session/trail grouped view 與 related route member rows 的 reuse
- Settings richer review surfaces（trusted outputs / local host / runtime chips）與 route CTA hierarchy 的剩餘 drift
- intelligence route file split 與 Tauri/worker/API pass-through glue inventory

---

## 當前待辦

- [ ] `M10-001` 盤點仍未抽成 shared primitive 的 workbench/review rows
- [ ] `M10-002` 決定哪些 route / glue split 真正值得做，哪些只記 inventory
- [ ] `M10-003` 先完成一輪高價值 workbench surface reuse
- [ ] `M10-004` 把 transport hygiene / decomposition 邊界回寫 source docs 與 backlog

---

## 建議工作塊

- `WORK-M10-A` — Shared Review Rows And Workbench Surface Reuse
- `WORK-M10-B` — Intelligence Route And Desktop Glue Decomposition

---

## 已知種子與參考

- [../m9-cross-app-reuse/README.md](../m9-cross-app-reuse/README.md)
- [../../design/intelligence-shared-route-composition-tradeoff.md](../../design/intelligence-shared-route-composition-tradeoff.md)
- [../../design/screens-and-nav.md](../../design/screens-and-nav.md)
- [../../features/intelligence-current-state.md](../../features/intelligence-current-state.md)
- [../../features/core-intelligence-ultimate-design.md](../../features/core-intelligence-ultimate-design.md)

---

## 邊界

- 不回退成 consumer-local route grammar 或 local state
- 不為了「拆大檔」而拆；只有能降低 drift / ownership 混亂的 split 才正式做
- 不把 M10 擴成新分析算法或大規模 desktop contract rewrite
