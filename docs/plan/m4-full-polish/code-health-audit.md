# M4 — Code Health Audit

> `WORK-M4-L` closeout artifact. This file records the remaining hot spots that are real, accepted, and explicitly deferred, so M4 does not end with debt hidden in chat history.

## Scope

- Focus on source modules that are still larger or more cross-cutting than we want.
- Only list debt that remains after M4 closeout.
- Every item must point to its next owner or backlog destination.

## Frontend Hotspots

| Surface                        | Current truth                                                                                                                                              | Next owner                                                                                                                    |
| ------------------------------ | ---------------------------------------------------------------------------------------------------------------------------------------------------------- | ----------------------------------------------------------------------------------------------------------------------------- |
| `src/lib/backend.ts`           | Browser-preview fixture + legacy compatibility helper. Still too large and too easy to accidentally grow.                                                  | Keep reference-only during M5; split preview fixtures / legacy helpers only when a new work block touches the shell contract. |
| `src/pages/settings/index.tsx` | Settings is now the control tower for archive, app lock, analytics, updater, derived state, and diagnostics. The route is shipping, but the file is large. | Follow-up split by panel after M5 foundation work stabilizes the next Settings surfaces.                                      |
| `src/lib/app-context.tsx`      | Reference-only legacy global state. No longer the live shell contract, but still present in tests / compatibility paths.                                   | Retire once remaining preview / legacy consumers are removed in a future cleanup block.                                       |

## Rust / Desktop Hotspots

| Surface                                          | Current truth                                                                                                                                                                                                                            | Next owner                                                                                                                                         |
| ------------------------------------------------ | ---------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | -------------------------------------------------------------------------------------------------------------------------------------------------- |
| `src-tauri/crates/vault-core/src/archive/mod.rs` | Accepted mega-file for canonical archive ingest / query / doctor / export / rekey flows. Responsibility is right, size is not.                                                                                                           | Future archive-plane split when a post-M5 work block touches canonical ingest or query boundaries.                                                 |
| `src-tauri/crates/vault-core/src/chrome.rs`      | Browser discovery + staging + path heuristics are still mixed together.                                                                                                                                                                  | Revisit when browser-discovery ownership is moved further into `vault-platform`.                                                                   |
| `src-tauri/crates/vault-core/src/ai.rs`          | Still mixes provider, queue, index, and helper logic. M5 已把 deterministic baseline 與 first-party enrichment runtime 收斂清楚，但 AI plane 本身仍然集中在同一個 mega surface。                                                         | 保持 shipping 穩定；只有在新的 post-M5 work block 明確資助 AI-plane extraction 時才再拆。                                                          |
| `src-tauri/crates/vault-core/src/insights.rs`    | 2026-04-11 已先把 M5-B 的 grouping / topics / surfaces / storage 子責任拆到 `src/insights/` 子模組，不再用 milestone 檔名承載 shipping logic；但主入口仍同時擁有 orchestration、snapshot loading、explainability 與 stale / clear flow。 | 下一個 post-M5 cleanup block 若再碰 deterministic insight plane，優先把 orchestration 與 snapshot / explain helpers 往更清楚的子模組續拆。         |
| `src-tauri/crates/vault-worker/src/lib.rs`       | Accepted orchestration hotspot that still owns desktop bridge, MCP, schedule, lock, and derived-state wiring.                                                                                                                            | Keep stable during M5 unless a new work block explicitly funds worker-boundary extraction.                                                         |
| `src-tauri/crates/vault-platform/src/lib.rs`     | Scheduler / keyring / platform adapter surface keeps growing as release contracts mature. 2026-04-10 audit already slimmed macOS keyring wiring back to native stores and removed an accidental `turso` dependency chain.                | Keep as the platform integration layer; split only if a future milestone adds another major platform family.                                       |
| Rust dependency graph for desktop builds         | Default desktop release 仍把 optional intelligence runtime (`lancedb` / `lance` / `datafusion` / `rig-core`) 與 archive / shell-critical flows 一起放進同一個 binary；2026-04-10 已取得明確 product / packaging sign-off。               | 這現在是 accepted trade-off，而不是 active blocker；持續用 `bun run release:size-audit` 監測，只有在產品方向改變時才重新打開 build-boundary 決策。 |

## Not M4 Blockers Anymore

- plugin execution sandbox
- dedicated enrichment queue family
- richer long-horizon topic / summary intelligence
- deterministic taxonomy / groups / threads / reference pages

These are not hidden debt inside M4. They are explicit M5 scope and belong to `WORK-M5-A` / `WORK-M5-B`.
