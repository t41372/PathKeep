# M5 — Deterministic Intelligence

> 目標：把 PathKeep 的非 LLM / non-embedding intelligence 收斂成 evidence-first、cross-browser、可重建、可模組化的正式 baseline。  
> 這個 milestone 不是在現有 M3 / M4 上再堆 heuristic，而是要把 deterministic intelligence 的 vocabulary、evidence contract、taxonomy、invalidations 與 performance envelope 一次理順。
>
> **狀態：Active**  
> 2026-04-10 起，[ADR-006](../../architecture/decisions/006-deterministic-intelligence-boundary.md) 已接受，M5 可正式 supersede 現行 [features/intelligence.md](../../features/intelligence.md) 裡的 session / dwell / embedding-first deterministic baseline。
>
> **2026-04-15 reset note:** 這個 milestone 交出的 evidence-first foundation 已被 [features/core-intelligence-ultimate-design.md](../../features/core-intelligence-ultimate-design.md) 升級成更激進的 Core Intelligence hard-reset baseline。這份 README 保留作為 pre-reset M5 closeout archive，不再單獨定義最新 deterministic product contract。

---

## Source Inputs

- [../../features/deterministic-intelligence.md](../../features/deterministic-intelligence.md)
- [../../features/intelligence.md](../../features/intelligence.md)
- [../../architecture/decisions/006-deterministic-intelligence-boundary.md](../../architecture/decisions/006-deterministic-intelligence-boundary.md)
- [../../architecture/data-model.md](../../architecture/data-model.md)
- [../../architecture/module-boundary-map.md](../../architecture/module-boundary-map.md)
- [../../architecture/tech-stack.md](../../architecture/tech-stack.md)
- [../m4-full-polish/intelligence-60-year-envelope.md](../m4-full-polish/intelligence-60-year-envelope.md)

---

## M5 的完成定義

- deterministic intelligence baseline 不再依賴 estimated dwell / session duration / focus proxies
- burst / query group / thread 的 vocabulary、evidence tier、confidence 與 invalidation contract 都已正式落地
- taxonomy v2 具備 multi-dimensional classifier、versioned rule packs、user override 與 `unknown` fallback
- no-AI mode 仍可提供高價值 insights：query ladders、reference pages、source role / effectiveness、open loops、template summaries
- deterministic modules 具備 internal registry、enable / disable、rebuild / clear、explainability 與測試夾具
- deterministic pipeline 有 replayable benchmark，足以支撐 long-horizon / heavy-user envelope 的成本審查

---

## 本里程碑文檔

- [foundation-and-taxonomy.md](foundation-and-taxonomy.md)
- [groups-threads-and-surfaces.md](groups-threads-and-surfaces.md)

---

## 里程碑檢查表

- [x] `M5-001` 凍結 deterministic evidence contract，正式移除 dwell / session-duration baseline 假設
- [x] `M5-002` 建立 URL normalization、registrable domain extraction、search URL parsing、script-aware tokenization 基礎
- [x] `M5-003` 建立 multi-dimensional taxonomy v2，具 user override、rule packs、lexicons、`unknown`
- [x] `M5-004` 實作 query groups 與 query reformulation ladders v2
- [x] `M5-005` 實作 cross-burst / cross-day thread merge、open loops、reference pages
- [x] `M5-006` 建立 source role / source effectiveness 與 template summary modules
- [x] `M5-007` 把 deterministic insights 拆出 internal module registry 與 explainability contract
- [x] `M5-008` 補齊 rollback / restore / visibility invalidation、fixtures、acceptance 與 long-horizon benchmark

### 2026-04-10 foundation progress

- `vault-core::visit_taxonomy` 現在已提供 URL normalization、registrable-domain / subdomain extraction、search parser、script-aware tokenization、evidence tiers、taxonomy v2 precedence、China Mainland / US core packs 與 user override baseline。
- `vault-core::insights` 已開始把 taxonomy / evidence trace 持久化到 `visit_insight_features`，並把 deterministic importance score 從 `duration_ms` 移開。
- `WORK-M5-A` closeout 同步把 first-party-only enrichment runtime、dual built-in plugin defaults、Settings / Insights queue review surface、retry / cancel guard 與 browser preview/runtime fixture 一起簽收；下一個 focus 轉到 `WORK-M5-B` 的 query groups / threads / reference pages。
- `PG-RD-AI-010` 尚未完成，因此 runtime 仍只允許 checked-in heuristic packs；任何 external tokenizer / registrable-domain / language-ID / optional model asset 仍不得進 shipping bundle。

### 2026-04-10 groups / surfaces closeout

- `vault-core::insights` 現在已正式按 `visit features -> burst -> query group -> thread -> reference/source/summaries` 的 deterministic pipeline 執行，並把 `burst_id` / `query_group_id`、`query_group_count`、thread confidence / evidence tier 一起持久化到 shipping derived state。
- 新的 derived tables / runtime trace 已落地：`insight_bursts`、`insight_query_groups`、`insight_query_group_members`、`insight_reference_pages`、`insight_source_effectiveness`、`deterministic_module_runtime`。
- Settings / Insights 現在可 review deterministic module registry，顯示 `ready` / `stale` / `disabled` / `idle`、dependencies、derived tables、last built、stale reason，且模組 enable-state 已回寫到 `AppConfig.deterministic.modules`。
- Dashboard / Insights summary surface 現在優先消費 deterministic `templateSummaries`，query groups、reference pages 與 source effectiveness 也都變成正式、可 profile-scope、可 explain 的 shipping surface。

### 2026-04-12 runtime / envelope progress

- deterministic pipeline 現在把 search terms、feature rows、enrichments 都改成 bounded joins，只讀當前 visit set；thread merge 也改成 incremental accumulator，不再在 nested loop 裡重建整份 token / domain / anchor union。
- backend deterministic registry 現在直接產出 `periodic-summary` 與 `contrastive-summary` cards；`Important but Unsaved` 仍明確 deferred，直到 canonical archive 真正 ingest bookmark / saved-page facts。
- scoped rebuild persistence 現在按 `profile_scope + window_days` 分區，30-day / 365-day、single-profile / all-profile snapshot 不再互相覆蓋。
- `src-tauri/crates/vault-core/examples/intelligence-benchmark.rs` 與 `artifacts/benchmarks/2026-04-12-intelligence-rewrite/` 現在提供可重跑的 100k / 1M synthetic benchmark evidence；這讓 M5 已有 replayable perf artifact，但仍**不等於**完成 10M / low-RAM / 真實 large-profile signoff。
