# M5-DI-B — Query Groups, Threads, And Surfaces

> 讀這份文檔的時機：Foundation / taxonomy 定案後，你要把 deterministic intelligence 真正變成使用者看得到的 grouping、cards、explainability 與 rebuild contract。

## Source Inputs

- [../../features/deterministic-intelligence.md](../../features/deterministic-intelligence.md)
- [../../architecture/decisions/006-deterministic-intelligence-boundary.md](../../architecture/decisions/006-deterministic-intelligence-boundary.md)
- [../../design/screens-and-nav.md](../../design/screens-and-nav.md)
- [../m4-full-polish/intelligence-60-year-envelope.md](../m4-full-polish/intelligence-60-year-envelope.md)

## 本工作包要交付什麼

- burst / query group / thread / open-loop vocabulary 與 persistence strategy
- query reformulation ladders v2
- source role / source effectiveness
- reference pages / resurfacing cards
- template summaries
- internal module registry、rebuild / clear / explainability contract

## WBS

- [ ] `M5-DI-B-001` 把 `burst` 定義為低風險時間容器，不再作 task / duration 代理
- [ ] `M5-DI-B-002` 建立 query-group builder：start / extend / terminate rules、reformulation handling、evidence tiers
- [ ] `M5-DI-B-003` 建立 cross-burst / cross-day thread merge，明確 confidence 與 reopen semantics
- [ ] `M5-DI-B-004` 實作 source role / source effectiveness，不得使用 dwell / foreground proxies
- [ ] `M5-DI-B-005` 實作 open loops、reference pages、resurfacing 與 template summaries
- [ ] `M5-DI-B-006` 定義 deterministic module registry：enable / disable、rebuild / clear、explainability、artifact trace
- [ ] `M5-DI-B-007` 建立 rollback / restore / visibility invalidation 與 stale-read honesty
- [ ] `M5-DI-B-008` 建立 acceptance / perf artifact：query-group fixtures、restore-after-rollback fixtures、heavy-window rebuild benchmark

## Exit Artifacts

- deterministic grouping contract
- explainable cards / ladders / reference-page surfaces
- rebuild / invalidate / clear contract
- replayable acceptance + perf bundle

## 2026-04-12 progress note

- shipping runtime 現在已補上 profile/window partitioned persistence、backend-owned periodic / contrastive summaries、bounded joins，以及 thread accumulator rewrite。
- replayable perf bundle 現在由 `src-tauri/crates/vault-core/examples/intelligence-benchmark.rs` 生成，artifact 會落在 `artifacts/benchmarks/2026-04-12-intelligence-rewrite/`。
- 這代表 `M5-DI-B-005` / `M5-DI-B-006` / `M5-DI-B-008` 的核心 runtime 已收斂，但 final signoff 仍保留在真實 large-profile replay、10M-scale proof、以及 bookmark facts 尚未進 canonical archive 前的 `Important but Unsaved` deferred truth。
