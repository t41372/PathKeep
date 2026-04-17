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

> **2026-04-17 truth note:** incremental foundation 已落地，但 `WORK-CI-B` 仍然 open；剩下的 backend 真 blocker 是 `PG-RD-AI-011` 的 `10M / 14.4M / low-RAM / queue-recovery` signoff、legacy `vault-core::insights` cleanup，以及 P4 host/service integration。

---

## 完成度矩陣

| Slice                                               | 原規劃對應                | Backend       | Frontend                               | 目前 evidence                                                                                                                                                                                                                                                                                                                   | 現在還缺什麼                                                                                              |
| --------------------------------------------------- | ------------------------- | ------------- | -------------------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | --------------------------------------------------------------------------------------------------------- |
| Hard cutover / schema / queue foundation            | P1-1                      | `Done`        | `Done`                                 | [`CHANGELOG.md` `WORK-QC-T`](CHANGELOG.md), [`../../src-tauri/crates/vault-core/src/intelligence/mod.rs`](../../src-tauri/crates/vault-core/src/intelligence/mod.rs), [`../../src/app/router.tsx`](../../src/app/router.tsx), [`../../src/lib/core-intelligence/api.ts`](../../src/lib/core-intelligence/api.ts)                | 只剩 cutover cleanup，不是重新做 foundation                                                               |
| Explorer session / trail / navigation path          | P1-2, P1-3, P1-9          | `Done`        | `Done`                                 | [`../../src/pages/explorer/panels/session-group.tsx`](../../src/pages/explorer/panels/session-group.tsx), [`trail-group.tsx`](../../src/pages/explorer/panels/trail-group.tsx), [`navigation-tracer.tsx`](../../src/pages/explorer/panels/navigation-tracer.tsx), `get_sessions/get_search_trails/get_navigation_path` commands | 主要剩 manual truth pass、交互 polish、測試漂移修補                                                       |
| Intelligence overview sections                      | P1-4 ~ P1-8, P2-5, P2-8   | `Done`        | `Done`                                 | [`../../src/pages/intelligence/index.tsx`](../../src/pages/intelligence/index.tsx), [`sections.tsx`](../../src/pages/intelligence/sections.tsx), [`domain-deep-dive.tsx`](../../src/pages/intelligence/domain-deep-dive.tsx)                                                                                                    | 不缺主功能；剩 shared polish / truth gate                                                                 |
| Deep-analysis sections                              | P2-1 ~ P2-7               | `Done`        | `Done`                                 | `get_stable_sources/get_search_effectiveness/get_friction_signals/get_reopened_investigations/get_browsing_rhythm/get_discovery_trend/get_domain_deep_dive`, plus matching sections in [`../../src/pages/intelligence/sections.tsx`](../../src/pages/intelligence/sections.tsx)                                                 | 剩 domain drilldown / cross-link / manual evidence review 細節                                            |
| Advanced deterministic analysis                     | P3-1 ~ P4-2               | `Done`        | `Mostly done`                          | [`../../src-tauri/crates/vault-core/src/intelligence/phase_three.rs`](../../src-tauri/crates/vault-core/src/intelligence/phase_three.rs), [`phase_four.rs`](../../src-tauri/crates/vault-core/src/intelligence/phase_four.rs), `Breadth/Habits/PathFlows/CompareSets/MultiBrowserDiff/ObservedInteractions` sections            | 前台已 render 大部分 surface，但仍需要整體 truth pass 與 capability-gated UX review                       |
| Runtime controls / explainability                   | P1-10 + runtime follow-up | `Done`        | `Done`                                 | [`../../src/components/intelligence/explainability-panel.tsx`](../../src/components/intelligence/explainability-panel.tsx), Jobs / Settings runtime actions, `explain_entity`                                                                                                                                                   | 剩命名 / copy / docs / tests 一致性                                                                       |
| External output payload providers                   | P4-3 backend subset       | `Partial`     | `Not started as real consumer surface` | `get_intelligence_embed_cards`, `get_intelligence_widget_snapshot`, `get_intelligence_public_snapshot` commands exist in backend                                                                                                                                                                                                | 還沒做真正的 host integration / frontend consumer / snippet surface                                       |
| Large-archive proof / incremental runtime / cleanup | cross-phase closeout      | `In progress` | `Open follow-up only`                  | [`program/research-and-decisions.md`](program/research-and-decisions.md) `PG-RD-AI-011`, `core_intelligence_stage_checkpoints`, `artifacts/benchmarks/2026-04-17-intelligence-incremental-foundation/`                                                                                                                          | incremental foundation 已落地；剩餘是 `10M / 14.4M`、low-RAM chunking、queue recovery RSS 與更真實 replay |

---

## 現在還沒完成的任務

### Backend remaining

1. **`PG-RD-AI-011` 還是 open**
   - 60-year / 10M+ / low-RAM / queue-recovery 的性能與記憶體 envelope 仍未正式收口。
   - 目前只有 replayable synthetic benchmark artifact，不等於最終 signoff。
2. **Queue / rebuild 粒度仍要收斂**
   - `visit_derive` / `daily_rollup` / `structural_rebuild` / `full_rebuild` 已存在，append-only path 也已經真正接上 stage checkpoints。
   - 剩下的是把這條路再往 `10M+`、low-RAM、queue-recovery、更多 chunk/resume 策略收斂，而不是從頭發明 incremental queue。
3. **legacy `vault-core::insights` 還沒完全退場**
   - hard cutover 已完成，但舊 code 仍留作 enrichment / readable-content helper reuse。
   - 需要再決定哪些保留、哪些能刪、哪些要搬到新 module。
4. **P4 external host/service integration 還沒真正交付**
   - backend 現在只有 payload-provider commands，不是完整 external integration。

### Frontend remaining

1. **真正的 embed / widget / public snapshot consumer 還沒做**
   - backend 有 payload providers，TS API / routes / host consumer surface 仍未完成。
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
2. queue granularity / incremental resume cleanup
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
