# M5 — Deterministic Intelligence

> 目標：把 PathKeep 的非 LLM / non-embedding intelligence 收斂成 evidence-first、cross-browser、可重建、可模組化的正式 baseline。  
> 這個 milestone 不是在現有 M3 / M4 上再堆 heuristic，而是要把 deterministic intelligence 的 vocabulary、evidence contract、taxonomy、invalidations 與 performance envelope 一次理順。
>
> **狀態：Proposed / blocked**  
> 在 [ADR-006](../../architecture/decisions/006-deterministic-intelligence-boundary.md) 被接受之前，M5 只能作為 proposal / planning artifact，不能直接宣稱已 supersede 現行 [features/intelligence.md](../../features/intelligence.md) 的 accepted contract。

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

- [ ] `M5-001` 凍結 deterministic evidence contract，正式移除 dwell / session-duration baseline 假設
- [ ] `M5-002` 建立 URL normalization、registrable domain extraction、search URL parsing、script-aware tokenization 基礎
- [ ] `M5-003` 建立 multi-dimensional taxonomy v2，具 user override、rule packs、lexicons、`unknown`
- [ ] `M5-004` 實作 query groups 與 query reformulation ladders v2
- [ ] `M5-005` 實作 cross-burst / cross-day thread merge、open loops、reference pages
- [ ] `M5-006` 建立 source role / source effectiveness 與 template summary modules
- [ ] `M5-007` 把 deterministic insights 拆出 internal module registry 與 explainability contract
- [ ] `M5-008` 補齊 rollback / restore / visibility invalidation、fixtures、acceptance 與 long-horizon benchmark
