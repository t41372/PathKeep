# M7 — Cross-App Reuse Audit And Insight Entity Consolidation

> 目標：在 M6 把 `day` / `domain` 升格成 first-class route 之後，全面盤點 PathKeep 裡仍然各自為政的 intelligence entity、drilldown、digest 與 review surface，抽出可全局復用的 navigation / component / read-model contract。

---

## M7 的完成定義

- [x] 完整盤點 app 內仍然重複造輪子的 intelligence entity surface
- [x] 產出 generic insight-entity navigation / digest component / route grammar 設計
- [x] 清掉 M6 留下的 `TODO: M7`
- [x] 把新的 entity-first contract 回寫到 feature / design / architecture / plan docs

---

## 首批盤點範圍

- query family
- refind page
- source / stable source / domain role
- session / trail / navigation path
- category mix
- compare set / reopened investigation / habit entity
- external-output review surfaces

---

## 當前待辦

- [x] `M7-001` 掃描 active `src/` surface，列出仍存在 consumer-local composition 的 entity 與 deep-link grammar
- [x] `M7-002` 設計 generic insight-entity navigation contract：shared href、shared CTA、shared digest 以及 primary/secondary action hierarchy
- [x] `M7-003` 收斂 route / read-model boundary，決定哪些 entity 需要 first-class route、哪些只需 shared drawer / digest
- [x] `M7-004` 逐一清理 `TODO: M7`，並把 accepted decisions 回寫到 source docs

---

## 已知種子與參考

- [../m6-shared-insight-surfaces/README.md](../m6-shared-insight-surfaces/README.md)
- [../../design/intelligence-entity-route-tradeoff.md](../../design/intelligence-entity-route-tradeoff.md)
- [../../design/intelligence-generic-entity-navigation-tradeoff.md](../../design/intelligence-generic-entity-navigation-tradeoff.md)
- `src-tauri/crates/vault-core/src/intelligence/day_insights.rs`
- `src/pages/intelligence/sections/secondary-sections.tsx`
- `src/pages/settings/external-outputs-panel.tsx`

---

## 邊界

- M7 的重點是「抽象與復用」，不是再擴一輪大型新 intelligence feature list。
- 不可推翻 M6 已接受的 `Insights first` / entity-first route baseline；若要改，必須先產出新的 trade-off 文檔並取得使用者確認。

---

## 2026-04-19 closeout note

- `src/lib/intelligence.ts` 現在正式提供 generic `InsightEntityTarget` / `insightEntityHref()` contract；`day` / `domain` helpers 已退成 thin wrapper，並新增 `query family`、`refind page`、`session`、`trail` shared href grammar。
- `query family`、`refind page`、`session`、`trail` 已升格成 first-class insights route；`reopened investigation`、`habit`、`stable source`、`friction`、`compare set`、`multi-browser diff`、external-output chips 等 active surface 也都改成先解析到 shared destination，而不是各自拼 `/explorer`。
- 共享 entity chrome 現在統一承接 header / meta / primary CTA / secondary evidence CTA / explainability slot；`/intelligence`、`/intelligence/day/:date`、Explorer grouped/detail、以及 Settings external outputs 不再各自手寫 action hierarchy。
- M6 遺留的 `TODO: M7` 已全部收口；剩餘的 stable identity / context focus 缺口正式移交給 [M8 — Aggregate Entity Identity And Context Reuse](../m8-aggregate-entity-identity/README.md)。
