# Core Intelligence Progress

> **Status:** Active continuation tracker  
> **Last audited:** 2026-04-17  
> **Purpose:** 把 [`core-intelligence-ultimate-design.md`](../features/core-intelligence-ultimate-design.md) 自 2026-04-15 hard reset 之後的**實際完成度**、**剩餘工作**、以及 frontend/backend split ownership 收斂成一份 planning-side source of truth。

---

## 這份文件是做什麼的

這不是重新抄一遍設計稿。

這份文件回答三件事：

1. 2026-04-15 那份 Core Intelligence 規劃，現在到底做到哪裡了？
2. 前端 / 後端各自還剩什麼？
3. 如果一個沒有聊天上下文的 agent 被叫來「繼續前端」或「繼續後端」，應該先讀哪些東西？

---

## 先讀這裡

### 不分前後端都要先讀

1. [`../features/core-intelligence-ultimate-design.md`](../features/core-intelligence-ultimate-design.md)
2. 這份文件
3. [`core-intelligence-handoff.md`](core-intelligence-handoff.md)
4. [`../features/intelligence-current-state.md`](../features/intelligence-current-state.md)
5. [`../design/screens-and-nav.md`](../design/screens-and-nav.md)

### 繼續後端時再讀

1. [`program/research-and-decisions.md`](program/research-and-decisions.md) 的 `PG-RD-AI-011`
2. [`../architecture/data-model.md`](../architecture/data-model.md)
3. [`../architecture/desktop-command-surface.md`](../architecture/desktop-command-surface.md)
4. [`CHANGELOG.md`](CHANGELOG.md) 的 `WORK-QC-T`

### 繼續前端時再讀

1. [`../design/screens-and-nav.md`](../design/screens-and-nav.md)
2. [`../features/intelligence-current-state.md`](../features/intelligence-current-state.md)
3. [`../../src/lib/core-intelligence/types.ts`](../../src/lib/core-intelligence/types.ts)
4. [`../../src/lib/core-intelligence/api.ts`](../../src/lib/core-intelligence/api.ts)

---

## 2026-04-17 實際真相

- Core Intelligence 的實作進度，**已經明顯超過**最初「backend 做到 P1/P2、frontend 剛做到 P1/P2」的口頭分工狀態。
- backend 現在不只做完 Phase 1 / 2；repo 內已經有 **Phase 3 / 4 deterministic query APIs**，以及 `embed/widget/public snapshot` payload provider commands。
- frontend 也不只做到最初的 `/intelligence` 骨架；repo 內已經有：
  - `/intelligence`
  - `/intelligence/domain/:domain`
  - Explorer 的 session / trail 分組與 navigation tracer
  - Dashboard 的 On This Day / intelligence 入口接線
  - Jobs / Settings 的 Core Intelligence runtime controls
  - `/intelligence` 內大量 P1-P4 deterministic sections
- 真正還沒完成的，已經不是「先把第一版頁面做出來」，而是：
  - large-archive / low-RAM / chunked incremental runtime signoff
  - legacy `vault-core::insights` cutover cleanup
  - external snippet / widget / public snapshot 的真正 consumer / host integration
  - `/insights` 殘留命名、測試、truth-pass 漂移
- 2026-04-17 backend follow-up 現在已補上 **incremental foundation**：
  - per-profile `core_intelligence_stage_checkpoints`
  - append-only `visit-derive` / `daily-rollup` / `structural-rebuild`
  - runtime artifact 的 `executionMode / dirtyVisitCount / dirtyDateKeys / fallbackReason`
  - `path_flows` 4-step contract
  - replayable incremental benchmark artifacts
- 2026-04-17 backend follow-up（late）又往 finish-line 推進了一格：
  - structural stage 的 profile-wide aggregates 現在改成 batch scan：query families 以 search-event batches 建構，refind/path-flow/habit 以 derived-visit batches 聚合，不再為這些 aggregates 額外 materialize whole-profile `search_events` / `visit_derived_facts`
  - `artifacts/benchmarks/2026-04-17-intelligence-incremental-foundation/` 現在多了 `expired-lease-recovery-100k-60y.json`，並且所有 `100k / 60y` artifacts 都會附 corpus stats、peak RSS、以及 follow-up scenario metadata
- 2026-04-17 backend finish-line follow-up：
  - `visit-derive` / `daily-rollup` 的 `fallback-full` 現在已經 chunk 化：visit-derived facts 會 batch scan canonical visits 並逐批持久化，daily rollups 會 batch scan derived visits 並用 accumulator 合成，再整批 replace dirty/full rollups
  - repo 現在新增 `artifacts/benchmarks/2026-04-17-intelligence-finish-line/`，留下 `100k / 60y` 的 low-RAM fallback 與 expired-lease recovery evidence，並在 README 裡固定 synthetic / existing-archive replay command contract
  - benchmark harness 現在支援 `--app-root` / `--session-key` 的 existing-archive replay，但本機可見的真實 app root 目前是 encrypted archive、且沒有 benchmark session key，所以 real-replay artifact 仍然缺席
- 2026-04-17 backend signoff attempt：
  - live queued enrichment execution 已正式從 legacy `vault-core::insights` 搬到 [`../../src-tauri/crates/vault-core/src/enrichment.rs`](../../src-tauri/crates/vault-core/src/enrichment.rs)，worker-facing call shape 維持不變
  - benchmark harness 現在新增 `--persist-app-root`，可把 large synthetic corpus 保留下來給下一個 scenario / replay 使用；`/tmp/pathkeep-benchmark-smoke.json` 的 2k smoke 已驗證 replay command 與 persisted-root 路徑都正常
  - 這輪 `10M / 60y` 嘗試先暴露 `daily-rollup` full fallback，再暴露 structural `build_sessions` 複雜度；repo 因此又補了 SQLite-side daily rollup aggregation、prepared `visit_derived_facts` persistence、移除 structural `tail_visits.clone()`，以及 one-pass session aggregate build
  - 這些修正明顯改善了 large-host shape：`10M` structural-stage RSS 約從 `4.8 GiB` 降到 `3.0 GiB` 等級，release attempt 的 sample header 也曾量到 physical footprint 約 `2.1 GiB`、peak 約 `2.6 GiB`
  - 但 release `10M` persisted-root attempt 在超過 `30m` 後仍未產出 final JSON artifact，而 real app root replay 仍因 keychain CLI 讀 `database-key` 回 `128` 而 blocked；因此最新 truth 不是「finish line 已完成」，而是 blocker 已經縮小到 structural full rebuild cost + real-archive unlockability。詳見 [`../../artifacts/benchmarks/2026-04-17-intelligence-signoff/README.md`](../../artifacts/benchmarks/2026-04-17-intelligence-signoff/README.md)
- 2026-04-17 backend signoff attempt（post-streaming）：
  - structural tail rebuild 現在已改成 batch stream：不再把整段 tail materialize 成 `Vec<VisitRecord>`，range delete 也不再先收集 giant `visit_id` list；新的 regression 已覆蓋 batch-boundary session/trail stability、range delete boundary 與 batched `source_effectiveness`
  - benchmark harness 另外補上 `--skip-baseline-rebuild`，只允許搭配 existing `--app-root` replay，並在 target 沒有既有 Core Intelligence read model 時誠實拒絕
  - [`../../artifacts/benchmarks/2026-04-17-intelligence-signoff/full-1m-60y-post-streaming.json`](../../artifacts/benchmarks/2026-04-17-intelligence-signoff/full-1m-60y-post-streaming.json) 已落地：`1M / 60y` debug full rebuild 約 `383s`、query surfaces 約 `403ms`、peak RSS 約 `118 MiB`
  - release `10M / 60y` persisted-root attempt 在超過 `31m` 時仍未產出 final JSON artifact，但 `ps` 看到的 RSS 約只在 `440 MiB` 等級，顯示這輪真正收掉的是 structural RSS cliff；現在剩的是真正的 wall-clock completion / final artifact blocker，而不是先前的 multi-GiB memory spike
- 2026-04-17 backend contract follow-up：
  - app snapshot / worker-facing runtime readiness 現在以 `intelligenceStatus` / `IntelligenceStatus` 為 canonical 命名；`InsightStatus` 只剩 legacy in-repo alias，不能再被當成新 contract 擴寫
  - `src/lib/core-intelligence/types.ts` 與 `src/lib/core-intelligence/api.ts` 現在已正式補上 `get_intelligence_embed_cards`、`get_intelligence_widget_snapshot`、`get_intelligence_public_snapshot` 的 typed IPC draft；剩下缺的是 consumer / host integration，不是「前端完全沒有 command contract」
- 2026-04-17 synthetic signoff closeout（real replay still pending）：
  - `stageTimingsMs` 現在已修正為跨 profile 累加，不再只反映單一 profile 的 rebuild 時間
  - [`../../artifacts/benchmarks/2026-04-17-intelligence-signoff/full-2k-smoke-signoff.json`](../../artifacts/benchmarks/2026-04-17-intelligence-signoff/full-2k-smoke-signoff.json) 已重生：baseline rebuild 約 `158 ms`、query surfaces 約 `11 ms`、stage totals 約 `146 ms`、peak RSS 約 `22.8 MiB`
  - [`../../artifacts/benchmarks/2026-04-17-intelligence-signoff/full-1m-60y-signoff.json`](../../artifacts/benchmarks/2026-04-17-intelligence-signoff/full-1m-60y-signoff.json) 已重生：baseline rebuild 約 `106,503 ms`、query surfaces 約 `375 ms`、stage totals 約 `105,218 ms`、peak RSS 約 `780.7 MiB`
  - [`../../artifacts/benchmarks/2026-04-17-intelligence-signoff/full-10m-60y-signoff.json`](../../artifacts/benchmarks/2026-04-17-intelligence-signoff/full-10m-60y-signoff.json) 現在已是 corrected rebuild-only replay artifact：baseline rebuild 約 `2,078,480 ms`、query surfaces 約 `1,250 ms`、`visit-derive=469,343 ms`、`daily-rollup=19,351 ms`、`structural-rebuild=1,579,679 ms`、peak RSS 約 `1.44 GiB`
  - [`../../artifacts/benchmarks/2026-04-17-intelligence-signoff/expired-lease-recovery-10m-signoff.json`](../../artifacts/benchmarks/2026-04-17-intelligence-signoff/expired-lease-recovery-10m-signoff.json) 已驗證 durable `10m-signoff` root 的 `--skip-baseline-rebuild` queue recovery；one expired queued lease 會被 requeue、one cancelled lease 維持 cancelled，peak RSS 約 `598 MiB`
  - disposable real-app-root copy 已準備在 `~/Library/Caches/pathkeep-benchmarks/real-replay-copy`；`WORK-CI-B` 現在唯一還沒收口的 blocker 是 encrypted archive session key 尚未取得

> **2026-04-17 truth note:** incremental foundation、structural aggregate batching、`visit-derive` / `daily-rollup` full-fallback chunking、以及 synthetic `2k / 1m / 10m / queue-recovery` evidence 都已落地；`WORK-CI-B` 仍然 open，但現在唯一缺的 backend finish-line artifact 是 encrypted real-app-root replay。若之後還要補 `14.4M` 或更長 horizon，應視為 `WORK-CI-C` 的 residual scope，而不是這個 block 的當前 blocker。

---

## 完成度矩陣

| Slice                                               | 原規劃對應                | Backend       | Frontend                               | 目前 evidence                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                              | 現在還缺什麼                                                                                                                                        |
| --------------------------------------------------- | ------------------------- | ------------- | -------------------------------------- | ---------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | --------------------------------------------------------------------------------------------------------------------------------------------------- |
| Hard cutover / schema / queue foundation            | P1-1                      | `Done`        | `Done`                                 | [`CHANGELOG.md` `WORK-QC-T`](CHANGELOG.md), [`../../src-tauri/crates/vault-core/src/intelligence/mod.rs`](../../src-tauri/crates/vault-core/src/intelligence/mod.rs), [`../../src/app/router.tsx`](../../src/app/router.tsx), [`../../src/lib/core-intelligence/api.ts`](../../src/lib/core-intelligence/api.ts)                                                                                                                                                                                                                                                                                                                                           | 只剩 cutover cleanup，不是重新做 foundation                                                                                                         |
| Explorer session / trail / navigation path          | P1-2, P1-3, P1-9          | `Done`        | `Done`                                 | [`../../src/pages/explorer/panels/session-group.tsx`](../../src/pages/explorer/panels/session-group.tsx), [`trail-group.tsx`](../../src/pages/explorer/panels/trail-group.tsx), [`navigation-tracer.tsx`](../../src/pages/explorer/panels/navigation-tracer.tsx), `get_sessions/get_search_trails/get_navigation_path` commands                                                                                                                                                                                                                                                                                                                            | 主要剩 manual truth pass、交互 polish、測試漂移修補                                                                                                 |
| Intelligence overview sections                      | P1-4 ~ P1-8, P2-5, P2-8   | `Done`        | `Done`                                 | [`../../src/pages/intelligence/index.tsx`](../../src/pages/intelligence/index.tsx), [`sections.tsx`](../../src/pages/intelligence/sections.tsx), [`domain-deep-dive.tsx`](../../src/pages/intelligence/domain-deep-dive.tsx)                                                                                                                                                                                                                                                                                                                                                                                                                               | 不缺主功能；剩 shared polish / truth gate                                                                                                           |
| Deep-analysis sections                              | P2-1 ~ P2-7               | `Done`        | `Done`                                 | `get_stable_sources/get_search_effectiveness/get_friction_signals/get_reopened_investigations/get_browsing_rhythm/get_discovery_trend/get_domain_deep_dive`, plus matching sections in [`../../src/pages/intelligence/sections.tsx`](../../src/pages/intelligence/sections.tsx)                                                                                                                                                                                                                                                                                                                                                                            | 剩 domain drilldown / cross-link / manual evidence review 細節                                                                                      |
| Advanced deterministic analysis                     | P3-1 ~ P4-2               | `Done`        | `Mostly done`                          | [`../../src-tauri/crates/vault-core/src/intelligence/phase_three.rs`](../../src-tauri/crates/vault-core/src/intelligence/phase_three.rs), [`phase_four.rs`](../../src-tauri/crates/vault-core/src/intelligence/phase_four.rs), `Breadth/Habits/PathFlows/CompareSets/MultiBrowserDiff/ObservedInteractions` sections                                                                                                                                                                                                                                                                                                                                       | 前台已 render 大部分 surface，但仍需要整體 truth pass 與 capability-gated UX review                                                                 |
| Runtime controls / explainability                   | P1-10 + runtime follow-up | `Done`        | `Done`                                 | [`../../src/components/intelligence/explainability-panel.tsx`](../../src/components/intelligence/explainability-panel.tsx), Jobs / Settings runtime actions, `explain_entity`                                                                                                                                                                                                                                                                                                                                                                                                                                                                              | 剩命名 / copy / docs / tests 一致性                                                                                                                 |
| External output payload providers                   | P4-3 backend subset       | `Partial`     | `TS contract done, no real consumer surface` | `get_intelligence_embed_cards`, `get_intelligence_widget_snapshot`, `get_intelligence_public_snapshot` commands now exist in backend, and `src/lib/core-intelligence/{types,api}.ts` already type them end-to-end                                                                                                                                                                                                                                                                                                                                                                                                                                          | 還沒做真正的 host integration / frontend consumer / snippet surface                                                                                 |
| Large-archive proof / incremental runtime / cleanup | cross-phase closeout      | `Mostly done` | `Open follow-up only`                  | [`program/research-and-decisions.md`](program/research-and-decisions.md) `PG-RD-AI-011`, `core_intelligence_stage_checkpoints`, structural tail streaming, structural aggregate batch scan, chunked `visit-derive` / `daily-rollup` fallback, [`artifacts/benchmarks/2026-04-17-intelligence-incremental-foundation/`](../../artifacts/benchmarks/2026-04-17-intelligence-incremental-foundation/), [`artifacts/benchmarks/2026-04-17-intelligence-finish-line/`](../../artifacts/benchmarks/2026-04-17-intelligence-finish-line/), [`artifacts/benchmarks/2026-04-17-intelligence-signoff/`](../../artifacts/benchmarks/2026-04-17-intelligence-signoff/) | synthetic `10M` / queue-recovery 已完成；剩餘 blocker 只剩 session-key real-archive replay，若還要補 `14.4M+` 應回收到 residual benchmark follow-up |

---

## 現在還沒完成的任務

### Backend remaining

1. **`PG-RD-AI-011` 還是 open**
   - synthetic `2k / 1m / 10m / queue-recovery` envelope 現在都已存在於 signoff artifacts，且 `stageTimingsMs` 已修正為跨 profile 總和。
   - 本機 real-replay artifact 仍因 encrypted archive 缺 session key 而尚未產出；disposable copy 已備好，但還沒有解鎖條件。
   - 如果後續還要補 `14.4M+` 或更長 horizon benchmark，這應該視為 residual follow-up，而不是目前 finish-line 的唯一 blocker。
2. **Queue / rebuild 粒度仍要收斂**
   - `visit_derive` / `daily_rollup` / `structural_rebuild` / `full_rebuild` 已存在，append-only path 也已經真正接上 stage checkpoints。
   - structural stage 的 profile-wide aggregate pass 已經 batch 化，`visit-derive` / `daily-rollup` full fallback 也已 chunk 化；這一輪 synthetic queue-recovery 也已經用 durable `10m-signoff` root 驗證 `--skip-baseline-rebuild` replay。
   - 目前最重的 remaining hotspot 不再是 `daily-rollup` full fallback，而是 structural full rebuild；corrected `10M` artifact 顯示 structural stage 約占 `1,579,679 ms / 2,068,373 ms` 的 rebuild total。
3. **legacy `vault-core::insights` 還沒完全退場**
   - hard cutover 已完成，但舊 code 仍留作 enrichment / readable-content helper reuse。
   - `preferred_embedding_content` 這類 shared readable-content helper 已開始回收到 `enrichment` 邊界；這輪也已把 `execute_enrichment_job_by_id` 周邊移出 `insights`。剩下待收縮的是更多 inert snapshot-era helper，而不是 worker 仍依賴 `insights` 執行 queued enrichment。
4. **P4 external host/service integration 還沒真正交付**
   - backend 和 TS draft 現在都有 payload-provider contract，但仍不是完整 external integration。

### Frontend remaining

1. **真正的 embed / widget / public snapshot consumer 還沒做**
   - backend 有 payload providers，TS API draft 也已接上；但 routes / host consumer surface 仍未完成。
2. **`/intelligence` hard cutover 還有測試 / copy 漂移**
   - repo 內仍可找到 `/insights` / `Insights` 殘留，例如 [`../../tests/e2e/shell.spec.ts`](../../tests/e2e/shell.spec.ts)。
   - 這些不能再當成 product contract，但需要整理回一致。
3. **需要一次完整 truth pass**
   - `/intelligence`
   - `/intelligence/domain/:domain`
   - Explorer session / trail view
   - Jobs / Settings runtime review
   - capability-gated observed-interactions / external-output related copy

### Planning / docs note

- 2026-04-17 起，Core Intelligence continuation 已經正式接進：
  - `STATUS.md`
  - `BACKLOG.md`
  - `README.md`
  - `program/traceability-map.md`
  - `core-intelligence-handoff.md`
- 後續只要 frontend/backend 的完成度再變動，必須先更新這份文件和 handoff，再更新 code-closeout 敘事；不要讓 planning truth 再次落後於實作。

---

## 對 fresh agent 的直接指令

### 如果使用者說「繼續後端」

先讀：

1. [`../features/core-intelligence-ultimate-design.md`](../features/core-intelligence-ultimate-design.md)
2. 這份文件
3. [`core-intelligence-handoff.md`](core-intelligence-handoff.md)

然後直接從這三條 remaining work 挑第一個未完成的真 blocker：

1. `PG-RD-AI-011` large-archive / low-RAM / chunked runtime signoff
2. larger-host queue-recovery RSS + real-archive replay evidence
3. legacy `vault-core::insights` cleanup or external host integration

### 如果使用者說「繼續前端」

先讀：

1. [`../features/core-intelligence-ultimate-design.md`](../features/core-intelligence-ultimate-design.md)
2. 這份文件
3. [`core-intelligence-handoff.md`](core-intelligence-handoff.md)
4. [`../design/screens-and-nav.md`](../design/screens-and-nav.md)

然後優先做：

1. `/intelligence` / Explorer / Domain Deep Dive truth pass
2. `/insights` 殘留 route / copy / tests 清理
3. external output payload consumer / host surface

---

## 驗收提醒

只要有代碼變更，最少跑：

```bash
bun run check
bun run build
```

backend 有實作改動時，再加：

```bash
cargo test --manifest-path src-tauri/Cargo.toml -p vault-core --lib
```
