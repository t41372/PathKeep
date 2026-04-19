# M6 — Shared Day And Domain Insights

> 目標：把 `day` 與 `domain` 升格成 Core Intelligence 的 first-class shared entity surface，讓 `/intelligence`、Dashboard、Explorer 與其他 active cards 不再各自重做完整 detail / route grammar。

---

## M6 的完成定義

- `/intelligence/day/:date` 成為正式的 exact-day insights route
- `/intelligence/domain/:domain` 正式收斂成 `Domain Insights` module，而不是孤立 deep-dive
- backend 有唯一完整的 `get_day_insights` typed read model
- frontend 有 shared day/domain href helpers，停止各頁面自己手搓 URL
- Dashboard、Intelligence、Explorer 至少在 active day/domain surfaces 上接入 shared entity entry
- 文檔與 route / IA 契約同步改成 `Insights first`

---

## 為什麼需要 M6

- Core Intelligence 現在最大的耦合不在計算層，而在 entity 沒有 first-class contract。
- `day` 長期只是 `Browsing Rhythm` 內的一段 page-local state。
- `domain` 雖然已有 route，但各處入口行為不一致，也沒有被視為 app-wide shared module。
- 如果這一輪不先把 `day` / `domain` 收斂，後面每張新卡都會繼續重複實作 drilldown。

---

## 本里程碑文檔

- [../../design/intelligence-entity-route-tradeoff.md](../../design/intelligence-entity-route-tradeoff.md)

---

## 里程碑檢查表

- [x] `M6-001` `get_day_insights` read model 與 typed desktop command 落地
- [x] `M6-002` `/intelligence/day/:date` route、shared href helpers、Insight Access strip 落地
- [x] `M6-003` Dashboard / Intelligence / Explorer 接上 day/domain entity-first navigation
- [x] `M6-004` source docs、route grammar、M7 seed plan 與 `TODO: M7` inventory 同步回寫

---

## 2026-04-19 closeout note

- `Browsing Rhythm` 的主互動現在已正式改成 navigation-first：點日格直接進 `/intelligence/day/:date`，不再把卡內 same-day detail 當主工作流。
- `/intelligence/domain/:domain` 雖然內部暫保留 `deep_dive` naming 過渡，但 user-facing IA 已正式視為 `Domain Insights`。
- Explorer detail rail 現在會沿用 shared entity grammar，提供 `Open day insights` / `Open domain insights` / exact-day evidence CTA。
- 這輪刻意 deferred 的 `TODO: M7` 已在下一個 milestone 完成收口：`day_insights.rs` 的 day-specific helper naming 已補齊、path-flow chips 已改成 shared entity resolution、Settings external-output review 也已接上 shared href contract。後續只剩更深的 aggregate identity / context focus gap，已改由 [M8](../m8-aggregate-entity-identity/README.md) 追蹤。
