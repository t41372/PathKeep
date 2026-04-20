# M9 — Cross-App Reuse Audit And Shared Composition

> 目標：在 M6–M8 已把 shared insights entity、aggregate identity 與 context focus 收斂後，全面盤點整個 app 內仍然重複造輪子的 UI composition、helper、read-model glue 與 consumer-local review chrome，決定哪些抽象應正式升格成 shared primitives，哪些只保留在局部。

---

## M9 的完成定義

- [x] 完整盤點 app 內剩餘的 consumer-local composition 與 duplicated helper / glue code
- [x] 產出共享 digest / CTA / evidence / focus / review composition 的抽象策略
- [x] 至少把一輪高價值 shared composition 從多個 consumer 中抽離
- [x] 把新的 deferred reuse gap 改記 `TODO: M9` / `TODO: M10`，並回寫 source docs / status / backlog

---

## 首批範圍

- shared digest / meta / stat-row composition
- shared CTA hierarchy / target-link derivation / focus carry-through helpers
- evidence / freshness / explainability review chrome
- Dashboard / Intelligence / Explorer / Settings 之間的 duplicated section scaffolding
- frontend / backend typed contract glue 的重複邏輯

---

## 當前待辦

- [x] `M9-001` 掃描 active `src/` / `src-tauri/` surface，列出仍存在 consumer-local composition、duplicated helper 與 near-duplicate read-model glue 的位置
- [x] `M9-002` 決定哪些 shared composition 應抽成 primitives / helpers / envelopes，哪些保持 route-local
- [x] `M9-003` 先抽離一輪高價值 shared composition，覆蓋至少兩個以上 consumer
- [x] `M9-004` 把新的 reuse debt 與下一輪 milestone seed 寫回 `STATUS.md`、`BACKLOG.md`、feature/design docs 與 code TODO

---

## 建議工作塊

- `WORK-M9-A` — Remaining Reuse Inventory And Single-Source Map
- `WORK-M9-B` — Shared Digest / CTA / Evidence Composition Extraction

---

## 已知種子與參考

- [../m8-aggregate-entity-identity/README.md](../m8-aggregate-entity-identity/README.md)
- [../../design/intelligence-aggregate-entity-focus-tradeoff.md](../../design/intelligence-aggregate-entity-focus-tradeoff.md)
- [../../design/screens-and-nav.md](../../design/screens-and-nav.md)
- [../../features/intelligence-current-state.md](../../features/intelligence-current-state.md)
- [../../features/core-intelligence-ultimate-design.md](../../features/core-intelligence-ultimate-design.md)

---

## 邊界

- M9 的重點是 reuse inventory、shared composition 與 single-source extraction，不是再擴一輪新的分析功能
- 不可推翻 M6–M8 已接受的 entity-first / focus / trusted-output boundary；若要改，必須先重開 trade-off 並取得使用者確認
- 不為了抽象而抽象；只有跨 consumer 明確重複、且能降低 drift 風險的 composition 才升格成 shared contract

---

## 2026-04-19 closeout note

- inventory 結論正式接受：route grammar、focus grammar、`InsightEntityActions`、`InsightEntityHero`、`IntelligenceSectionMeta` 與 `BrowsingRhythmCard` 已是 shared truth；M9-B 真正高價值的重複點則是 route-level metric strip、query-family card、compare-set page list、structured target label 與 section header chrome。
- 這輪 shared extraction 採 route-level frontend primitive 優先，不把 scope 擴成 Tauri/worker/API pass-through refactor。接受的 trade-off 見 [../../design/intelligence-shared-route-composition-tradeoff.md](../../design/intelligence-shared-route-composition-tradeoff.md)。
- code 現在新增 shared route composition primitives，並把 `/intelligence` overview、promoted routes 與 Settings trusted outputs 接到同一批 metric/card/list/label helper；section-level `證據與新鮮度` badge 也改回 inline-end header chrome，不再佔整行或吃掉過大的 hover hitbox。
- 本輪明確留給下一輪的 gap 改以 `TODO: M10` 與 [../m10-workbench-reuse/README.md](../m10-workbench-reuse/README.md) 追蹤：尤其 `refind` summary/detail chrome、Explorer review rows、以及 route / desktop glue decomposition。
