# M5 — Intelligence Runtime & Extensions

> 目標：把從 M4 truthfully deferred 的 intelligence runtime 補成正式系統，讓 enrichment plugin、queue operations、deterministic insights 和 evidence controls 都可觀察、可配置、可重建。

---

## M5 的完成定義

- enrichment plugin runtime 不再藏在單一同步 pipeline 裡，而是有正式 contract、registry、enable / disable、rebuild / clear story。
- intelligence queue 至少對 enrichment / insight refresh 可觀察、可 retry、可 cancel，且有明確的 run / artifact trace。
- richer deterministic insights 可以在沒有 LLM 的情況下產生更高價值的 evidence-first 洞察。
- Settings / Insights 的操作面能誠實顯示 plugin、queue、degrade state、evidence boundary，而不是只露出「Run insights」單一按鈕。

---

## 為什麼需要 M5

- `M4` 已經把 release / polish / updater / namespace 等 closeout 集中處理，但這不代表 deferred 的 intelligence runtime 問題自然消失。
- 目前 repo 已經有可用的 insights 和部分 enrichment 邏輯，可是它們還長在同步巨型流程裡，plugin / queue / retry / evidence control 邊界都不夠清楚。
- 如果現在不把這些能力抽成正式 runtime，之後每加一個 plugin 或 deterministic insight，都會繼續把風險堆回 `insights.rs` 和 worker glue。

---

## 本里程碑文檔

- [enrichment-runtime-and-operations.md](enrichment-runtime-and-operations.md)
- [deterministic-insights-and-evidence-controls.md](deterministic-insights-and-evidence-controls.md)

---

## 里程碑檢查表

- [ ] `M5-001` enrichment runtime、plugin registry、job queue 和操作面完成第一個可驗收版本。
- [ ] `M5-002` deterministic insights 補齊一批高價值、無 LLM 也成立的洞察模塊。
- [ ] `M5-003` evidence controls、rebuild / clear / retry story 和相關 acceptance tests 完成。
- [ ] `M5-004` M3 / M4 / M5 邊界回寫到 docs，避免 roadmap 仍停在 M4。
