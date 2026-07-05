# M17 AI-Redesign Milestone Review — Authoritative Final Report

Date: 2026-06-22
Scope: The full AI-redesign change set (W-AI-0..9 + W-ENRICH-1) on `feat/ai-redesign-2026`.
Method: 14 independent focus-area reviews, every finding adversarially re-verified, then deduped/clustered/prioritized here.

---

## 1. Executive Summary

**Verdict: the AI-redesign change set is solid and ships-worthy after a small, well-defined set of fixes.** This is high-quality, security-conscious, scale-aware work. The code-mode sandbox, the agent-harness durability contract, the embedding/vector retrieval stack, the enrichment egress chokepoint, the MCP outward surface, and the streaming chat FE are all genuinely well-engineered and honestly documented. No sandbox escape, no SQLCipher key leak, no silent-wrong-result, and no unconsented-egress-on-a-fresh-install was found. The hardest constraint — never freeze the main thread while streaming — is met by construction (ref-buffer + single-rAF flush).

**Risk profile.** The residual risk is concentrated in two clusters, both about _honesty of a boundary the project already claims to honor_, not about a broken core:

1. **Consent-gating drift (security/trust).** Several AI firing sites enforce "a provider is configured" but NOT the master `ai.enabled` / sub-flag the security-posture doc says gates them. The agent path, the streaming chat path, the GPU re-embed action, the MCP per-call path, and the agent-chat CRUD path each let AI read decrypted history and/or hit the network (or, for agent-chat, read transcripts while App Lock is engaged) in a state the user believes is off. None are exploitable on a fresh install; all require a previously-configured provider or an already-running process. But "the gate lives at each firing site, not just in the UI" is the project's own stated posture, and it is materially false for these sites.

2. **Status / copy honesty drift (trust).** Two FE build-status defects make the Smart-index callout lie (counts unrelated chat jobs as "Building…"; reverts to "nothing to rank yet" after a successful build), plus a family of English-only backend degradation notes rendered verbatim to zh-CN/zh-TW, plus stale "tracked for v0.3" copy on a surface now showing live counts.

A real performance defect (RA-SEARCH-1: domain/URL-star LIKE passes full-`SCAN urls`, contradicting the documented forward-seek invariant) rounds out the high-severity set.

**Headline numbers:**

- Areas reviewed: **14**
- Total findings raised across areas: **69**
- **Confirmed defects: 33** (verified, reproduced)
- **Trade-offs (real tension, defensible): 15**
- **Refuted: 12**

Confirmed severity breakdown: **0 critical · 3 high · 13 medium · 17 low.**

---

## 2. Confirmed Findings (deduped, by severity)

### HIGH (3)

#### H-1 · RA-CODEMODE-1 · `RA-AGENT-CODEMODE` · security

**Agent path is not gated on the master `ai.enabled` / `assistant_enabled` consent flags.**
File: `vault-worker/src/intelligence/chat.rs:103-141` (`ai_chat_send`); gate absent in `context.rs:166-174` and `ai/search.rs:448-458`.
Issue: `ai_chat_send` → `selected_llm_provider_runtime` → `spawn_agent_run` runs a full tool-executing agent (run_code over the decrypted archive + LLM network egress) with NO check of `config.ai.enabled`/`assistant_enabled`. The only de-facto gate is "a provider is configured + enabled + has a key." A user who configured a provider then toggled master AI OFF can still drive an agent run. The legacy `if !enabled { bail }` lives only in `answer_history_question_with_control` (the non-agent assistant path).
Why: Data Sovereignty + Intelligence-is-optional are core. `ai-security-posture.md §1` explicitly claims the gate "lives at each firing site, not just in the UI" — that claim is false for the shipped agent firing site. The UI hides the entry, but a stale/programmatic/dev-IPC call re-enables a network+archive-reading agent the user believes is off.
Fix: Gate at the agent firing site (in `ai_chat_send` / `run_agent_stream` preamble) — bail with the honest "Enable AI analysis and the assistant in Settings…" message unless `config.ai.enabled && config.ai.assistant_enabled`. Best enforced once centrally (a shared guard `search_history_internal` and both chat/agent paths call) so the two paths cannot drift.
**Corroboration:** independently found as RA-MODELS-BRIDGE-2 (streaming chat half) from the DTO/bridge angle. The streaming `ai_chat_send` and the agent path share the same ungated entry; fix once.

#### H-2 · RA-SEARCH-1 · `RA-SEARCH-STARS` · performance

**Domain-star and URL-star prefix-LIKE passes are full `SCAN urls`, not the claimed `idx_urls_url` SEARCH — breaks the 14.4M bounded-seek invariant.**
File: `vault-core/src/stars.rs:459-540, 616-659, 679-734`; migration `014_stars.sql:43-45`.
Issue: `idx_urls_url` is plain BINARY collation and there is no `PRAGMA case_sensitive_like=ON`, so SQLite's default case-insensitive LIKE cannot use the index for a range. `EXPLAIN QUERY PLAN` (sqlite 3.54) confirms: the prefix-LIKE and domain-LIKE passes are full `SCAN urls`; only `url = ?` is a true seek. `url_ids_for_canonical` pass 2 runs unconditionally (no `if ids.is_empty()` guard), so even a matched URL-star scans. Every `is:starred` facet query and every Starred-hub render performs one full covering-index scan of the entire `urls` table _per star_. The module docs, the migration comment, and `search.rs` perf notes all repeatedly assert "SEARCH …, never SCAN" — the opposite of what the planner does.
Why: This is exactly the O(corpus) interactive-path cost the §3 14.4M-row constraint forbids and that the forward-seek design exists to avoid. The code is _correct_ (the Rust re-check keeps results honest) but the central performance claim is false.
Fix: Either set `PRAGMA case_sensitive_like = ON` with byte-exact prefixes, or use explicit range bounds `url >= prefix AND url < prefix_upper` (verified to yield `SEARCH … USING COVERING INDEX`). For domain stars (no anchored prefix) resolve domain→urls via a persisted host/registrable-domain column+index. Gate `url_ids_for_canonical` pass 2 behind an exact-pass miss. Add EXPLAIN regressions for the prefix AND domain queries.
**Corroboration:** RA-SEARCH-2 (medium, below) is the test-side of the same defect — the boundedness regression test only EXPLAINs the exact-seek path, so it cannot catch this.

#### H-3 · RA-SEARCH-2 (FE) · `FE-EXPLORER-SEARCH` · ux

**After a build completes, the Smart-index callout reverts to "nothing to rank yet" — `indexedItems` is never refreshed by the poll.**
File: `src/pages/explorer/index.tsx:882-903` (bounded poll calls `refreshRuntimeStatus` only); `indexedItems` sourced at `866-870` from `snapshot.aiStatus`.
Issue: The 4s bounded poll runs only `refreshRuntimeStatus()` (loads `aiQueue` + `intelligence`), never `refreshAppData` (which carries `aiStatus.indexedItems`/`lastIndexedAt`). `handleIndexAction` calls `refreshAppData` once at enqueue, when `indexedItems` is still ~0. When the minutes-long build drains, the poll sees `running=0`/`queued=0` → clears the interval, but no `refreshAppData` ever fires. The callout flips from "Building…" straight back to "Smart search has nothing to rank yet" with a stale scope line — the build looks like it failed.
Why: `smart-index-status.tsx` explicitly promises ready ("N pages indexed") on completion. The actual completion state is a dishonest "nothing to rank yet" until an unrelated full refresh — the exact fabricated/stale-status problem the surface set out to avoid. Direct Trust & Transparency regression.
Fix: When the poll observes the active build transition true→false, fire one `refreshAppData(false)` (track previous `active` in a ref, or refresh in the cleanup when `shouldPollQueue` flips false). Add a route test asserting `indexedItems` updates after the queue drains.

---

### MEDIUM (13)

#### Consent / lock honesty

**M-1 · RA-MODELS-BRIDGE-1 · `RA-MODELS-BRIDGE` · security**
**Agent-chat CRUD commands bypass the App Lock boundary, leaking plaintext chat transcripts when locked.**
File: `vault-worker/src/intelligence/agent_store.rs:24-59`.
Issue: `save/list/load/delete/rename_conversation` resolve paths and call vault-core directly with NO lock gate — the only AI data path that does not funnel through `load_unlocked_config` (whose body is `ensure_app_lock_unlocked`). The agent plane is plaintext (no SQLCipher key), so there is no key barrier either. While PathKeep is locked, anyone at the machine can list/read/delete conversations via these commands or the dev-IPC mirror. `desktop-command-surface.md:50` states AI data read commands MUST return a refusal when locked.
Why: App Lock is a core trust feature; chat transcripts (the user's questions about their own browsing) are the single most sensitive plane and are explicitly excluded from export for sovereignty. The author conflated "no cipher key needed (derived sidecar)" with "no lock gate needed" — `context.rs:12` itself says App Lock is a session boundary, not archive encryption.
Fix: Route the five fns through `load_unlocked_config(&paths)?` (or an explicit `ensure_app_lock_unlocked`) before touching `agent_store`. Add a regression test asserting `list/load_conversation` refuse when locked.

**M-2 · RA-MODELS-BRIDGE-2 · `RA-MODELS-BRIDGE` · security**
**`ai_chat_send` (streaming chat + agent) does not enforce the master AI consent toggle (only provider-configured).**
File: `vault-worker/src/intelligence/chat.rs:103-141`.
Issue: Checks only `messages.is_empty()` then `selected_llm_provider_runtime`, which requires only a configured provider id + stored key — never `ai.enabled`/`assistant_enabled`/the provider's own `enabled`. A previously-configured provider + key lets a streaming LLM run fire with master AI OFF. (Verifier note: the `ask_ai_assistant` half is **refuted** — that path bails at `ai/search.rs:262` before any egress. The defect is confined to the streaming `ai_chat_send`, which is the same firing site as H-1.)
Why: Disabling AI in Settings is consent withdrawal; the backend must be the enforcement point. Bounded blast radius (needs prior config + key) → medium not high.
Fix: Same central guard as H-1. Also honor `AiProviderConfig.enabled` in `resolve_provider_runtime`. Test that `ai_chat_send` refuses when `assistant_enabled` is false despite a provider+key.

**M-3 · RA-SETTINGS-1 · `FE-SETTINGS-CONSENT` · security**
**GPU "Re-embed working set / full archive" bypasses the `semantic_index_enabled` consent sub-flag.**
File: `src/pages/settings/ai-gpu-section.tsx:274-294`; backend `vault-worker/src/intelligence/ai_queue.rs:494-544` (`build_ai_index_now`).
Issue: The re-embed actions are gated only on `saving || !aiOn` (the master), never `semanticIndexEnabled`. `startReembed` → `buildAiIndex` → `build_ai_index_now` enqueues an embedding job with NO `semantic_index_enabled` check (contrast `archive_flows.rs:111` which DOES gate auto-index on it). A user with master ON but Smart search deliberately OFF can trigger up to a full 14.4M-page re-embed — calling their embedding provider (token cost / egress) and writing ~59GB of derived vectors.
Why: `ai-security-posture.md §1` names `semantic_index_enabled` as the flag gating "Embedding backfill + semantic retrieval." Re-embedding IS embedding backfill. Tempered to medium because the action is explicit/user-initiated behind a PME cost estimate.
Fix: Disable the buttons with an honest `blockedReason` when `!semanticIndexEnabled`, AND add a backend guard in `build_ai_index_now` mirroring `archive_flows.rs`.

**M-4 · RA-MCP-1 · `RA-MCP-SKILL` · security**
**MCP tool calls never re-check `ai.enabled`/`mcp_enabled` — revoking MCP consent mid-session has no effect until the process is killed.**
File: `vault-worker/src/mcp.rs:279-331, 334-388, 470-516`.
Issue: The `mcp_enabled` gate is checked once at startup (`mcp.rs:473`). The three per-tool entry points re-check App Lock but never `mcp_enabled`. Config is re-read fresh per call, so the current flag IS available — the code just never consults it. A user who turns OFF the MCP server in Settings while an external tool holds the stdio connection keeps getting full history served and audited until they kill the worker.
Why: `ai-security-posture.md §3` frames `mcp_enabled` as THE control over the outward face and claims "every tool call re-checks." Unlock IS re-checked (the stronger control — locking cuts MCP off immediately), but the consent toggle silently does nothing mid-session. Medium given stdio-only/local threat model.
Fix: Add `if !config.ai.enabled || !config.ai.mcp_enabled { bail }` at the top of each of the three entry points (config is already loaded there). Add a test flipping `mcp_enabled` false and asserting an in-flight `mcp_search_result` is refused.

#### Status / copy honesty

**M-5 · RA-SEARCH-1 (FE) · `FE-EXPLORER-SEARCH` · correctness**
**Smart-index status counts assistant-chat jobs as index-build progress (false "Building…").**
File: `src/pages/explorer/paper-search-helpers.ts:334-368` (`deriveSmartIndexProgress`); consumes the aggregate `AiQueueStatus`.
Issue: `deriveSmartIndexProgress` keys phase off `queueStatus.queued/running`, but `load_ai_queue_status` counts `[IndexBuild, IndexClear, Assistant]`. An in-flight Assistant chat job makes the callout render "Building the smart-search index" + live counts. The backend ALREADY computes a correct index-only filtered count (`read_model.rs:58-61`, used for its own state field) but exposes the aggregate to the UI.
Why: Violates the file's own honesty contract and the milestone Trust principle. Narrow precondition conjunction (AI on + provider configured + Assistant job in flight while on Smart surface) → medium.
Fix: Surface index-specific queue counts to the UI (add `indexQueued`/`indexRunning` filtered to IndexBuild/IndexClear), or have `deriveSmartIndexProgress` confirm an Index\* job is present before declaring a build phase.

**M-6 · RA-SEARCH-3 (Rust) / XA-AI-1 (FE render) / RA-CODEMODE-2 (agent notes) · i18n**
**Backend AI degradation / harness notes are English-only, rendered verbatim to all locales.** _(Three areas converged here — merged.)_
Files: `vault-core/src/ai/search.rs:514-516, 749-751, 786-790, 826-828, 848-851, 1182-1185, 1202-1205` (search degradation notes); render at `src/components/explorer-paper/paper-search-view.tsx:530-542`; `vault-core/src/ai/agent_harness.rs:359/370/386, 627-629` and `agent_tools.rs:441/446` (harness control notes streamed into the transcript).
Issue: Full actionable sentences ("No embedding provider is selected, so results use lexical retrieval only.", "The semantic index has no vectors yet; run Build index…", "Reached the maximum number of agent steps…") are hard-coded English and rendered with no i18n key resolution. The project already localizes the same class of backend note elsewhere (`settings/helpers.ts:380-419` maps them to `t()` keys), proving the pattern exists — these paths were just missed.
Why: `AGENTS.md` principle 5 makes i18n a shipping contract for ALL user-visible copy including error/disabled/degraded states. zh-CN/zh-TW users see English guidance precisely on the trust-critical fallback path. (RA-CODEMODE-2 downgraded to low individually since it's confined to rare edge states, but it is the same root cause.)
Fix: Emit notes as stable enum/note codes from the backend; resolve to localized copy in the FE catalog with en/zh-CN/zh-TW. For the harness, split the model-facing English form from the user-facing localized chunk.

**M-7 · XA-AI-2 · `XA-PRODUCT-UX` · i18n**
**`ai-providers-section` maps a localized warning via English string-equality on backend output.**
File: `src/pages/settings/ai-providers-section.tsx:579-585`.
Issue: The only path to a translated index-health warning is an exact full-sentence English match; any other `aiStatus.warning` (and there are at least 8 distinct backend strings, several `format!`-interpolated with provider names and thus structurally unmatchable) falls through to raw English for all locales. A one-character drift in the Rust string silently regresses the single mapped case, with no test catching it.
Why: Same anti-pattern as M-6, on consent-relevant degraded-state copy.
Fix: Backend should return a stable warning code/enum; FE maps all variants by key, never on a full English sentence.

**M-8 · RA-FEMISC-2 · `FE-MISC` · i18n**
**Jobs "Assistant and embedding queue" copy still labels shipped M17 features as "tracked for v0.3".**
File: `src/lib/i18n/catalog/jobs.ts:69-70 (en), 238-239 (zh-CN), 403-404 (zh-TW)`.
Issue: `queueSummaryBody` = "Optional assistant and embedding work is tracked for v0.3…" renders unconditionally in JobsPage directly above live AI-queue counts (queued/running/failed). M17 shipped the assistant, chat history, and embedding queue; `optionalAiFeaturesAvailable` is now true. Telling users the queue is a future thing while showing its live counts is internally contradictory.
Why: Trust & Transparency on the exact surface users use to verify background work. Same stale-copy class the change set deliberately fixed for the contentFetch strings — this one was missed. All three locales.
Fix: Rewrite to the honest live off-by-default framing (mirror the contentFetch rewrite). Add an assertion that the copy contains no "v0.3"/"tracked for" string so the drift cannot return.

#### UX honesty

**M-9 · FE-ASSISTANT-1 / XA-AI-3 (behavior) + XA-AI-4 (test) · `FE-ASSISTANT` / `XA-PRODUCT-UX` · ux + tests**
**"Regenerate this answer" appends a duplicate question + second answer instead of replacing in place; tests only assert a mock fires.**
File: `src/pages/assistant/index.tsx:290-297, 507-508`; `src/components/assistant-chat/use-ai-chat-stream.ts:538`; tests `assistant-turn.test.tsx:489-578`, `active-chat.test.tsx:1075-1118`.
Issue: `onRegenerate` aliases `handleRetry` → `send(lastUserContent)`, and `send` unconditionally `[...current, userMessage, assistantMessage]`. Clicking Regenerate on a completed turn leaves the old answer and appends a duplicate question + new answer; the duplicate is persisted to `agent.sqlite` on finalize. The model transcript re-sends the question too. The i18n label ("Regenerate this answer") implies in-place replacement. The only test asserts a second `sendAiChat` fires and that the request's last message is the re-asked question — which _confirms_ the duplication rather than guarding against it (coverage theater for the semantics).
Why: Trust & Transparency / honest feedback — the control over-promises; the saved transcript is polluted.
Fix: (a) implement true regeneration — drop the trailing assistant turn, do NOT re-append the user turn, reuse the assistant message id so the answer replaces in place; OR (b) relabel to honest "Ask again" semantics. Add an integration test through the real `useAiChatStream` asserting the user-turn count is unchanged after Regenerate.

**M-10 · RA-FEMISC-1 / XA-AI-5 · `FE-MISC` / `XA-PRODUCT-UX` · ux**
**Onboarding "Set up AI in Settings" abandons the in-progress setup flow and loses the master-password draft.** _(Two areas; severity reconciled to medium per the fuller FE-MISC verification.)_
File: `src/pages/onboarding/index.tsx:269-272, 478-484` (AI step is `step === 5`, before `handleFinish`/`initializeArchive` at `step === 6`).
Issue: `handleAiSetUp` does `void navigate('/settings#settings-ai')` from the AI step, before the archive is initialized. `step` and `securityDraft` (the entered+confirmed master password) are local `useState`, not persisted; navigating unmounts OnboardingPage. Browser/storage/schedule choices survive (saved via `saveConfig`), but the step position and the password draft do not — the user must re-walk the flow and re-enter the password. (Verifier note: return paths ARE signposted — the assistant/dashboard zero-states deep-link back to `/onboarding`, and onboarding copy says choices are auto-saved — so the "no way back / silently abandoned" framing is softened; the real defect is the discarded confirmed master-password draft + lost step.)
Why: A user who picked Encrypted mode and typed+confirmed a password, then taps the prominent AI CTA, is dropped out of setup with the password gone. The AI step's own doc comment claims both actions "advance to review," contradicting the navigate-away behavior.
Fix: Prefer an in-flow affordance (mark intent to open AI settings after finish) so onboarding state + draft survive; or gate the deep-link to post-initialization; at minimum persist the step and warn the password draft will be lost. Test the `onSetUpAi` → navigate → return path.

#### Performance / correctness

**M-11 · XA-PERF-1 · `XA-PERF` · performance**
**Every semantic query does a full O(n) sequential scan of the entire `.pkmap` during hydration (unmeasured by the S2 latency benchmark).**
File: `vault-core/src/ai/visit_content_map.rs:164-178` (`history_ids_for_content_keys`); called from `ai/search.rs:883` (`hydrate_semantic_hits`).
Issue: `hydrate_semantic_hits` runs on EVERY semantic search (not just `is:starred`) and strides every 16-byte `.pkmap` record (~14.4M, ~230MB) with no early exit. The S2 benchmark only times `index.search(...)`, never touches the `.pkmap`, so the published p50=105ms/p95=130ms EXCLUDES this second corpus-sized pass. The `.pkmap` is per-visit, so it does NOT shrink with dedup. Docs claim "one remaining O(n) pass on the is:starred path" — the always-on hydration scan is not acknowledged.
Why: On the 4-core/8GB target a basic AI search pays the index sweep PLUS an unbudgeted full `.pkmap` scan per query, roughly doubling real interactive latency and evicting page cache. The benchmark used to declare the path "interactive" excludes a corpus-sized pass.
Fix: Build a keyed reverse `content_key→[history_id]` sidecar so hydration is O(k') seeks (the same fix the doc already promises for the starred facet TODO — serve both directions from it). At minimum extend the S2 benchmark to include the `.pkmap` hydration scan and correct the "one remaining O(n) pass" wording.
**Corroboration:** XA-PERF-4 (low/trade-off) is the `is:starred` forward-resolution scan — same `.pkmap` root cause; fix together.

**M-12 · RA-SEARCH-4 · `RA-SEARCH-STARS` · architecture**
**RRF fusion keys on per-visit id and relies on lexical sort=newest aligning with the semantic most-recent-visit pick — duplicates pages and partially loses the dual-list boost.**
File: `vault-core/src/ai/search.rs:558-619` (`fuse_ranked_lists`), `484-490`, `1094-1095`.
Issue: Fusion dedups on `item.id` (lexical visit id) vs `hit.history_id` (semantic's most-recent visible visit id). `lexical_history_results` hardcodes `sort=newest` and returns one row per VISIT with no page dedup. A frequently-visited page whose two newest matching visits both land in the top-`limit` window produces TWO entries — one "Lexical + semantic" (fused), one separate "Lexical match" — duplicating the row AND only the newest-visit row earns the RRF dual-list boost. (Verifier raised from low to medium: this is reproducible _today_ for a common pattern, not merely latent.) Every fusion test uses single-visit pages, so it's uncovered.
Why: The "page ranked in BOTH lists beats single-list" guarantee — the whole point of RRF here — is partially defeated, with duplicated result rows.
Fix: Fuse on a page-stable key (content_key / url_id), choosing the most-recent visible visit for the surviving entry. Add a multi-visit test where the lexical-matched visit differs from the semantic representative.

---

### LOW (17)

| ID                           | Area       | Dim            | One-line                                                                                                                                                                                                                     |
| ---------------------------- | ---------- | -------------- | ---------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| L-1 · RA-CODEMODE-4          | code-mode  | correctness    | `fetch_visits` resolves ids only within a 1,000-row recent window; silently drops valid older ids with no per-id "outside window" note (`code_mode.rs:607-650`).                                                             |
| L-2 · RA-EMBED-2 / XA-PERF-3 | embed/perf | performance    | `recall()` materializes a full n-element Vec (~345MB transient worst-case, ~24-72MB at realistic dedup) per query instead of a bounded top-k heap (`vector_index.rs:254-275`). _(Same defect, two areas.)_                   |
| L-3 · RA-ENRICH-2            | enrich     | performance    | Per-host rate-limit registry grows unbounded for process lifetime (~12MB realistic ceiling) (`enrichment/rate_limit.rs:133-156`).                                                                                            |
| L-4 · RA-ENRICH-3            | enrich     | correctness    | CORR-1: FTS `enrichment_text` refresh keys on a single (possibly-reverted cross-batch) visit, leaving live siblings un-findable until full rebuild (`content_fetch.rs:562-568`).                                             |
| L-5 · RA-ENRICH-4            | enrich     | security       | Blank-host rate-limit bypass returns allow; demonstrably reachable via malformed-but-parseable `https:///path` URLs (no flood since unresolvable, but the SSRF↔url_domain invariant doesn't hold) (`rate_limit.rs:143-149`). |
| L-6 · RA-MCP-4               | mcp        | correctness    | `database_key` snapshotted at server start; an archive rekey mid-session leaves a stale key and an opaque `internal_error` instead of an honest "key changed, restart" message (`mcp.rs:218-228, 482, 509`).                 |
| L-7 · RA-MODELS-BRIDGE-3     | models     | performance    | `job_queue_concurrency` is unclamped — a hand-edited config spawns that many OS threads + SQLite connections on the 4-core target (`models/app.rs:20-32`). _(retrieval_top_k half refuted — already clamped downstream.)_    |
| L-8 · FE-ASSISTANT-2         | assistant  | i18n/a11y      | Live-region milestone announces raw `run_code` to screen readers while sighted users see the humanized label (`assistant-turn.tsx:152-154`).                                                                                 |
| L-9 · FE-ASSISTANT-3         | assistant  | product        | No pagination past the backend's 50/200-conversation list cap; older chats unreachable from the UI (not data loss) (`chat-history-explorer.tsx`).                                                                            |
| L-10 · RA-SETTINGS-2         | settings   | ux             | CPU-only build renders a filled (checked) GPU checkbox for a persisted `gpuEnabled:true` (contextualized by badge+note; honesty-framing tension) (`ai-gpu-section.tsx:230-237`).                                             |
| L-11 · RA-SETTINGS-3         | settings   | correctness    | Content-fetch master/extractor toggles lack an in-flight `saving` guard → optimistic-on rollback race on the egress switch (`content-fetch-section.tsx`).                                                                    |
| L-12 · RA-SETTINGS-5         | settings   | i18n           | Integrations consent-artifact localizer falls through to raw English on backend wording drift (`settings/helpers.ts:366-434`).                                                                                               |
| L-13 · RA-SEARCH-3 (FE)      | explorer   | ux             | No-matches copy tells Smart users to "switch to semantic search" while already in it; stale "semantic" vocabulary (`catalog/explorer.ts:484-485`).                                                                           |
| L-14 · RA-SEARCH-4 (FE)      | explorer   | correctness    | `pendingAction` conflates Clear/Rebuild with Build → "build queued" shown during a clear; persists for the whole clear via aggregate counts (`index.tsx:866-870`).                                                           |
| L-15 · RA-FEMISC-3           | jobs       | ux             | Jobs `overviewBody` frames AI as "future AI features" after AI shipped (`catalog/jobs.ts:66-67`).                                                                                                                            |
| L-16 · XA-SEC-5              | security   | data-integrity | Raw LLM provider error strings journaled verbatim into `agent.sqlite` + streamed; no Bearer/userinfo redaction (`agent_harness.rs:208-216`).                                                                                 |
| L-17 · XA-AI-6               | product-ux | i18n           | ~40 legacy assistant i18n keys ×3 locales are dead after the redesign retired job-polling panels; `llm` en/zh value drift (`catalog/assistant.ts:47-107`).                                                                   |

Plus one test-quality low worth recording (XA-TESTS-5): explorer/smart-index status tests assert i18n keys + param-bags via echo translators rather than resolved copy, so a broken translation or mismatched `{queued}`/`{running}` placeholder passes silently — `ai-providers-section.test.tsx` (real I18nProvider) is the model to follow.

---

## 3. Cross-Cutting Patterns

**P1 — Consent gating enforced in the UI, not at the firing site.** The single strongest systemic signal. H-1, M-1, M-2, M-3, M-4 are the _same class_: a firing site checks "provider configured / unlocked" but not the master `ai.enabled` + sub-flag the security-posture doc claims gates it. Five independent surfaces (agent, streaming chat, GPU re-embed, MCP per-call, agent-chat CRUD) drifted the same way. The doc's "gates live at each firing site, not just in the UI" is currently aspirational. **Recommendation: introduce one shared backend guard** (`ensure_ai_capability_enabled(config, capability)`) that every AI firing site calls, and a test matrix asserting each capability refuses when its flag is off. This converts a recurring drift into a single enforced invariant.

**P2 — Honesty drift between backend prose and the i18n contract.** M-6, M-7, M-8, M-9, M-10, L-8, L-12, L-15, plus XA-TESTS-5 all stem from user-visible English (or stale) prose leaking past the localization/honesty boundary: backend degradation notes rendered raw, warning-by-string-equality, "tracked for v0.3" on a shipped surface, a label that over-promises, SR text diverging from visual. The codebase _already has_ the right pattern (note→key mappers, real-I18nProvider tests) but applied it unevenly. **Recommendation: backend emits stable note/warning codes, never prose; FE owns all localization; add a lint/test that no backend-origin string is rendered without key resolution.**

**P3 — Benchmarks measure the resident/build phase, not the full interactive query path.** M-11 (+ XA-PERF-2/3/4 trade-offs): the S2 latency number excludes the `.pkmap` hydration scan (always-on) and the forward-resolution scan (`is:starred`), and the "330MB build peak" excludes the embed-loop DedupTracker (~1.6-2GB worst-case). None are silent-wrong-results; they are envelope/honesty gaps where the published numbers under-state real cost on the 4-core/8GB target. **Recommendation: extend benchmarks to cover the full query path (index sweep + hydration) and document the embed-loop resident set; re-validate the 8GB envelope for the cold first build.**

**P4 — Test coverage that asserts the mechanism fired, not the resulting state.** XA-AI-4 (regenerate test confirms duplication instead of guarding it), RA-SEARCH-2 (boundedness test EXPLAINs only the path that already passes), XA-TESTS-5 (echo translators). The 100% gate is green over call-graphs that don't pin the behavior the user cares about. **Recommendation: for load-bearing properties (perf invariants, transcript shape, resolved copy), assert end-state through the real path.**

---

## 4. Trade-offs (for the maintainer to ratify or revisit)

These are real tensions, judged defensible. Listed so they are explicitly _decided_, not silently inherited.

- **RA-ENRICH-1** — DNS-rebinding TOCTOU residual now spans the whole working-set + GitHub-API fetch surface, not just og:image. Openly documented as a fail-open residual, but the BACKLOG/doc tracking lags the widened scope. **Action: re-record the residual as load-bearing for W-ENRICH (not just og:image); consider IP-pinning the validated socket.** _(The one trade-off worth actively revisiting — it widened.)_
- **RA-EMBED-3** — Deduped vector embeds only the first-seen visit's per-visit text (profile/visited_at). Low-signal leak; the `mod.rs` "exact" wording overstates (one-word doc fix). Deliberate dedup thesis.
- **RA-EMBED-5** — Int8 seek reader re-seeks per candidate with no buffer (thousands of small random reads). Documented v1 flat-engine choice; cheap cold-cache hardening available (sort candidate positions ascending). Re-measure cold-cache p95.
- **RA-CODEMODE-5** — `query_history` host call ignores the cancel hook mid-retrieval. Documented; epoch deadline bounds it; the proposed fix wouldn't reliably help (the slow `.pkmap` pass has no `.await` yield point).
- **RA-MCP-2** — `archive-status` serves AI-config flags + audits on the same un-re-gated surface. Derivative of M-4; fixed by M-4's per-call gate.
- **RA-SETTINGS-2/4/5/6** — CPU GPU checkbox framing; per-domain blocklist allow-rule drop (latent — no allow-rule emitter exists today); consent-artifact English fallthrough; estimate re-fetch on open (COUNT is over small `urls` table, not visits — cheap).
- **FE-ASSISTANT-4** — Connecting affordance + typing indicator both render during first-chunk wait. Documented as intentionally distinct regions; not dishonest.
- **XA-SEC-4 / XA-SEC-6** — code-mode `fetch_visits` profile scoping (no production caller sets a non-None default today); `net_guard.rs` header scoping (accurate within og:image scope).
- **XA-PERF-2 / XA-PERF-4** — DedupTracker resident set (document + optional u128 prefix); `is:starred` third `.pkmap` scan (same root cause as M-11, fix together).
- **XA-AI-7** — Assistant evidence deep-link searches by raw URL (the canonical-filter / visit-id deep-link the fix assumes does not exist in Explorer; raw-URL search is the most robust current option).
- **XA-TESTS-2 / XA-TESTS-4** — Real provider transport (rig/reqwest/candle) is `#[cfg(not(any(test,coverage)))]`; MCP stdio bind is excluded. Consistently-applied "rig pattern"; security-critical decode/encode/normalize IS tested via pure helpers. **Action: tighten `quality-matrix.md` to state the cfg-gated transport is structurally outside the coverage denominator** (the "no exclusions" framing is imprecise).

---

## 5. Prioritized Action List

**Fix before declaring M17 done (high + the consent cluster):**

1. **H-1 / M-2** — central AI-capability guard at the agent + streaming-chat firing sites (`ai.enabled && assistant_enabled`). _One fix covers both._
2. **M-1** — route agent-chat CRUD through the App Lock gate (transcript leak when locked).
3. **M-4** — per-call `mcp_enabled` re-check in the three MCP tool entry points.
4. **M-3** — gate GPU re-embed on `semantic_index_enabled` (UI + backend `build_ai_index_now`).
5. **H-2 / RA-SEARCH-2** — fix the domain/URL-star LIKE → real index range/seek; add the missing EXPLAIN regressions; until fixed, at minimum correct the false "never SCAN" doc/migration comments.
6. **H-3** — fire one `refreshAppData` when the Smart-index build drains (build-looks-failed regression).
7. **M-5** — Smart-index status must read index-only queue counts (stop counting chat jobs as "Building…").

**Medium polish (this milestone or the immediate follow-up):** 8. **M-6 / M-7 / M-8** — localize backend degradation/warning/jobs prose via stable codes; add the no-"v0.3" assertion. _(P2 cluster — do together.)_ 9. **M-9** — make Regenerate replace-in-place OR relabel to "Ask again"; fix the test to assert transcript shape. 10. **M-10** — stop dropping the master-password draft when onboarding deep-links to AI settings. 11. **M-11** — keyed reverse `.pkmap` sidecar for hydration (also closes XA-PERF-4); extend the S2 benchmark to the full query path. 12. **M-12** — fuse RRF on a page-stable key; add a multi-visit fusion test.

**Defer / record (low + trade-offs):** 13. The 17 lows — batch as hygiene: dead i18n keys (L-17), unbounded registries (L-3, L-7 clamp), error redaction (L-16), checkbox/toggle-guard polish (L-10, L-11), copy fixes (L-13, L-15), and the correctness edges (L-1, L-4, L-6, L-14). Each is bounded and self-contained. 14. Ratify the §4 trade-offs explicitly. The only one that _widened_ and deserves a fresh decision is **RA-ENRICH-1** (rebinding surface). Tighten `quality-matrix.md` honest-boundaries (XA-TESTS-2/4).

---

## 6. What Is Genuinely Strong

Credit where it is due — these are excellent and should not be touched defensively:

- **The code-mode sandbox.** Zero ambient authority, a fail-closed 9-function WASI allowlist enforced by real module introspection, SHA-256-pinned guest, read-only two-op host API that never crosses the DB key or SQL, hard limits (epoch wall-time, memory, host-call budget, output/frame caps), cancel via epoch bump on a dedicated thread. The security tests are mutation-resistant (real wasm importing `path_open`/`sock_accept` rejected; `require('fs')`/`fetch()`/`process.env` fail safely; fixed-zero clock; deterministic PRNG). No escape found.
- **The agent-harness durability contract.** Journal-before-observe, idempotent tool-result steps keyed by `run_id+tool_call_id`, atomic seq assignment, honest `interrupted=Running` marker, cancel-emits-Done — correct and well-covered. The async↔sync bridge is valid and proven by an in-runtime test. The agent migration ledger with checksum-drift guard and atomic per-migration transactions is excellent.
- **The embedding/vector stack.** Content-hash dedup ("the largest near-free lever"), the `.pkvec/.pkmap/.pkbin/.pki8` plane stores, the two-stage flat index (binary recall → int8 rescore), the streaming lockstep plane projection (bounded build RAM), and the resumable backfill with genuinely-tested no-dup/no-miss invariants. Consent-gated download with SHA-256 pinning, off-by-default GPU/Metal, correct export exclusions for all derived planes.
- **The enrichment egress chokepoint.** The most security-conscious surface in the change set: extractors pure-of-network, all fetching funnels through one shared client, a true triple consent gate re-checked at enqueue AND claim, layered SSRF guarding (page URL + every sub-resource + per-redirect-hop + post-redirect backstop), per-host token bucket charged after the gate. No consent bypass, no lying-success path.
- **The MCP outward surface.** stdio-only with no TCP bind, read-only (three tools, no mutation), result limit clamped `[1,50]`, the SQLCipher key verifiably absent from the audit trace (asserted by test), every touch audited, App Lock re-checked per call.
- **The streaming chat FE.** The ref-buffer + single-rAF-flush delivers the hardest constraint — streaming never touches the main thread. Generation guards drop superseded turns; teardown on cancel/reset/unmount is sound. The tool-use timeline is genuinely transparent: verbatim code source never truncated, host-call rows from structured fields via a `$`-safe template filler, limit chips matching the Rust enum exactly. Untrusted LLM markdown is sanitized (streamdown rehype-harden + sanitize). No XSS/SSRF.
- **The consent surfaces in Settings.** AI genuinely off-by-default (no cascade from the master, no auto-build on enable), the content-fetch master hard-default-OFF with an always-visible network-policy disclosure, clamp bounds that accurately mirror the Rust backend, and i18n parity structurally enforced via `satisfies TranslationCatalog` with real idiomatic translations on the new surfaces.
- **The test suite is load-bearing, not coverage theater.** The security-critical tests introspect the real committed Javy WASM module, drive a scripted multi-turn provider through the real ToolRegistry + journal, prove the SQLCipher key never lands in the MCP audit trace, and assert the FE rAF no-freeze contract behaviorally.

The defects found are about the _honesty of boundaries the project already set for itself_ — a sign the bar is high, not that the work is shaky. Close the consent-gate cluster (P1) and the two build-status lies (H-3, M-5), and this is a change set worthy of a product a user trusts for 20 years.
