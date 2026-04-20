# M13 — Broad Reuse Audit Across Support, Trust, And Workflow Surfaces

> 目標：延續 M12 的 support-action / diagnostics single-source 方法，把 reuse audit 從單一路由或單一 primitive 擴大到全 app 的 support、trust、workflow surface，而不是再讓 Settings mega-route 或 transport parity 單獨吃掉下一輪。
>
> **2026-04-20 pause note:** 使用者在 M13 開工後直接插單 `WORK-PERF-A`，要求先修 `/intelligence` large-archive 凍結與 route revisit 卡頓。M13 scope 沒被取消，但在 perf stop-ship block 收尾前不再往 reuse extraction 繼續推進。

---

## M13 的完成定義

- [ ] 盤點 app-wide support / trust / workflow surface 的 reusable grammar 與 owner map
- [ ] 決定哪些 Jobs / Settings / Import / Audit / Explorer follow-through 值得正式抽成 shared composition
- [ ] 收斂一輪高價值 shared support / workflow composition
- [ ] 把 transport parity 保持在 subordinate inventory，而不是重開 codegen / manifest 專案

---

## 首批範圍

- Jobs plugin / module summary rows與 runtime health composition
- Settings mega-route 的下一輪 owner split
- Import / Audit / Schedule / Security / Lock 的 workflow follow-through grammar
- Explorer export / support quick action 與其他 trust-repair affordance
- support / trust surface 的 code comments、`TODO: M13`、與 planning/source docs 對齊

---

## 建議工作塊

- `WORK-M13-A` — Broad Reuse Inventory Across Support / Trust / Workflow Surfaces
- `WORK-M13-B` — Shared Support / Workflow Composition Extraction

---

## 邊界

- 不回退 M6–M12 已 accepted 的 route / payload / review-shell / support-action boundary
- 不讓 Settings route split 壟斷整個 milestone；它只是一個 consumer，不是唯一主題
- transport parity 只在 inventory 證明 owner drift / maintenance cost 持續上升時才進下一步，不預設升格成主線
