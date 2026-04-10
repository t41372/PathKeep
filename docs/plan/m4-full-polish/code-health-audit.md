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

| Surface                                          | Current truth                                                                                                                  | Next owner                                                                                                   |
| ------------------------------------------------ | ------------------------------------------------------------------------------------------------------------------------------ | ------------------------------------------------------------------------------------------------------------ |
| `src-tauri/crates/vault-core/src/archive/mod.rs` | Accepted mega-file for canonical archive ingest / query / doctor / export / rekey flows. Responsibility is right, size is not. | Future archive-plane split when a post-M5 work block touches canonical ingest or query boundaries.           |
| `src-tauri/crates/vault-core/src/chrome.rs`      | Browser discovery + staging + path heuristics are still mixed together.                                                        | Revisit when browser-discovery ownership is moved further into `vault-platform`.                             |
| `src-tauri/crates/vault-core/src/ai.rs`          | Still mixes provider, queue, index, and helper logic. M4 only truthfully closed the current shipping boundary.                 | `WORK-M5-A` / `WORK-M5-B`, alongside deterministic intelligence re-foundation.                               |
| `src-tauri/crates/vault-core/src/insights.rs`    | Still carries the older deterministic / insight logic while M5 proposal is pending.                                            | `WORK-M5-A` / `WORK-M5-B` after ADR-006 acceptance.                                                          |
| `src-tauri/crates/vault-worker/src/lib.rs`       | Accepted orchestration hotspot that still owns desktop bridge, MCP, schedule, lock, and derived-state wiring.                  | Keep stable during M5 unless a new work block explicitly funds worker-boundary extraction.                   |
| `src-tauri/crates/vault-platform/src/lib.rs`     | Scheduler / keyring / platform adapter surface keeps growing as release contracts mature.                                      | Keep as the platform integration layer; split only if a future milestone adds another major platform family. |

## Not M4 Blockers Anymore

- plugin execution sandbox
- dedicated enrichment queue family
- richer long-horizon topic / summary intelligence
- deterministic taxonomy / groups / threads / reference pages

These are not hidden debt inside M4. They are explicit M5 scope and belong to `WORK-M5-A` / `WORK-M5-B`.
