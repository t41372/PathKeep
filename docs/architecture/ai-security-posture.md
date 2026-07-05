# AI Security Posture & Threat Model

> Consolidated security artifact for PathKeep's shipped AI stack (W-AI-1..9, W-STAR, W-ENRICH).
> Written 2026-06-22 as the closeout for the AI redesign 2026 program.
> Every boundary below is grounded in the actual shipped code (file paths cited); this is a durable
> security record, not marketing. If you change one of the cited code paths, update the matching claim
> here.

This document describes **how PathKeep's optional AI surface is bounded**: what is off by default, what
can fire or egress only after explicit consent, how the code-mode sandbox and the outward MCP face are
constrained, and what is explicitly out of scope. It complements:

- [ADR-009](decisions/009-default-desktop-optional-intelligence-shipping.md) — default-desktop /
  optional-intelligence shipping boundary.
- [ADR-005](decisions/005-app-lock-session-boundary.md) — App Lock session boundary.
- [ADR-010](decisions/010-storage-plane-reset.md) — storage-plane truth model.
- [data-model.md](data-model.md) — storage planes and the export bundle contract.
- [tech-stack.md](tech-stack.md) — the AI framework / supply-chain posture.

All cited paths are relative to `src-tauri/crates/` unless noted.

---

## 0. One-paragraph summary

PathKeep's AI is **off by default and consent-gated**. A master `ai.enabled` flag gates every
capability; each capability has its own sub-flag, and a sub-flag does nothing unless the master flag is
on. Nothing reaches the network, the GPU, or an external tool until the user explicitly opts in. The two
surfaces that touch untrusted input — the **code-mode JS sandbox** (runs LLM-written JS over history) and
the **outward MCP face** (lets an external tool search history) — are constrained by construction: the
sandbox has zero ambient authority and a read-only host API; the MCP face is localhost stdio, read-only,
audited, and the SQLCipher key never crosses to the caller. Derived AI state (vectors, agent run traces,
chat transcripts) **never lives in the canonical SQLCipher archive** and is excluded from the portable
export.

---

## 1. Consent posture (off by default, master + sub-flag)

The AI surface is governed by `AiSettings` in `vault-core/src/models/intelligence.rs`. Every consent flag
is **hard-default-OFF** except the offline title-normalization plugin. From the `Default` impl
(`models/intelligence.rs`):

| Flag                     | Default | Capability gated                                                   |
| ------------------------ | ------- | ------------------------------------------------------------------ |
| `enabled`                | `false` | **Master AI flag** — gates everything below                        |
| `assistant_enabled`      | `false` | Streaming chat + agent harness                                     |
| `semantic_index_enabled` | `false` | Embedding backfill + semantic/hybrid retrieval                     |
| `mcp_enabled`            | `false` | Outward MCP server                                                 |
| `skill_enabled`          | `false` | MCP usage-guide (skill) payload                                    |
| `content_fetch_enabled`  | `false` | Site content fetch (network egress)                                |
| `gpu_enabled`            | `false` | Apple-Silicon Metal embedding tier (also requires a `metal` build) |
| `enrichment_enabled`     | `true`  | Offline title-normalization plugin only (no network, no egress)    |

### Master + sub-flag enforcement

A sub-capability is inert unless **both** the master flag and its own sub-flag are on. The gate lives at
each firing site, not just in the UI. One shared, pure, unit-tested guard —
`ensure_ai_capability_enabled(config, AiCapability)` in `vault-core/src/models/intelligence.rs` — is the
single enforcement point every consent-gated egress/compute firing site calls. It bails with an honest,
user-actionable "Enable AI and the &lt;capability&gt; in Settings." unless `config.ai.enabled` **and** the
matching sub-flag are on. Centralizing the predicate is deliberate: the five firing sites previously each
re-derived `enabled && sub_flag` by hand and drifted apart (some enforced only "provider configured /
unlocked"). The guard's `AiCapability` enum is the single vocabulary, and each firing site states which
capability it needs:

- **Assistant / agent** (`AiCapability::Assistant` ⇒ `enabled && assistant_enabled`): enforced at BOTH
  firing sites. The first-party assistant bails in `ai/search.rs` (`answer_history_question_with_control`),
  and the streaming-chat / tool-executing agent harness bails at the TOP of
  `vault-worker/src/intelligence/chat.rs::ai_chat_send` — before any provider resolution, run
  registration, or spawn — so a previously-configured provider+key cannot run a full agent (code-mode over
  the decrypted archive + LLM egress) with the master switch off.
- **Semantic index** (`AiCapability::SemanticIndex` ⇒ `enabled && semantic_index_enabled`): enforced at
  BOTH the auto-build-after-backup path (`vault-worker/src/archive_flows.rs`, additionally gated on
  `auto_index_after_backup`) and the explicit re-embed firing site
  (`vault-worker/src/intelligence/ai_queue.rs::build_ai_index_now`), so a user with the master on but Smart
  search deliberately off cannot trigger an embedding job (provider egress + the ~59 GB derived-vector
  tail). A `clear_only` cleanup job is exempt (it embeds nothing). The GPU re-embed UI
  (`src/pages/settings/ai-gpu-section.tsx`) mirrors the same gate: with Smart search off, both re-embed
  actions are disabled with an honest reason rather than offering an action the backend would refuse.
- **MCP** (`AiCapability::Mcp` ⇒ `enabled && mcp_enabled`): enforced at server start
  (`vault-worker/src/mcp.rs::run_mcp_stdio_server`, which additionally refuses if the app session is locked
  — see §3) AND re-checked on EVERY per-tool call (`mcp_search_result`, `mcp_archive_status_result`,
  `mcp_usage_guide_result`), since config is read fresh per call. A user who turns the MCP server off
  mid-session while an external tool still holds the stdio connection is refused on the next call rather
  than served.
- **Skill** (`AiCapability::Skill` ⇒ `enabled && skill_enabled`): the MCP usage-guide tool requires `Mcp`
  to serve at all (above), and the `skill_enabled` sub-flag then governs whether the body is returned —
  `build_mcp_usage_guide` returns an honest disabled notice (empty sections) when the skill toggle is off.
- **Content fetch** (`content_fetch_enabled`): `enrichment/content_fetch.rs::content_fetch_allowed`
  returns `false` unless `config.ai.content_fetch_enabled` (then per-extractor and per-domain gates). This
  one keeps its own dedicated chokepoint because it layers two further gates on top; the `AiCapability::ContentFetch`
  variant exists in the shared enum for parity and future callers.

The read-model state machine in `ai/read_model.rs` surfaces `disabled` first whenever `!config.ai.enabled`,
so the UI tells the honest story rather than implying a capability is live.

Note that chat-history CRUD (`vault-worker/src/intelligence/agent_store.rs`) is gated by the **App Lock**
boundary (`load_unlocked_config`), NOT by `ai.enabled`: the agent transcript plane is plaintext (no
SQLCipher key barrier), so every read/list/delete/rename refuses while the session is locked — but a user
may still read or delete existing transcripts with AI turned off, as long as they are unlocked (see §3 / §5).

The frontend mirror of these flags is `src/lib/types/intelligence.ts` (`AiSettings` interface). The newer
fields (`contentFetchEnabled`, `gpuEnabled`, the search-tuning knobs) are optional on the TS side so a
frozen snapshot without them deserializes as "off" — absent/false means off everywhere.

---

## 2. Code-mode sandbox (W-AI-8): zero ambient authority

Code-mode lets a capable LLM write a short JavaScript program that fans out searches and aggregates over
history. It is **default-enabled but sandboxed** (per the 2026-06 decision recorded in
[02-architecture-decisions.md §G](../plan/program/ai-redesign-2026/02-architecture-decisions.md)): the
Wasmtime sandbox _is_ the safety boundary, so there is no model-capability gate. The boundary was
independently security-reviewed this program (no escape/leak/bypass found). Implementation:
`vault-core/src/ai/code_mode.rs`; the `run_code` tool wrapper in `vault-core/src/ai/agent_tools.rs`; the
guest in `vault-core/src/ai/code_mode_guest/`.

### Zero dangerous ambient authority

The guest is a Javy (QuickJS → WASM) module run under Wasmtime with a deliberately empty capability set
(`code_mode.rs::build_scoped_wasi`): stdin/stdout are wired only to the host RPC channel, stderr is a sink,
and both the wall clock and monotonic clock are fixed at zero. There is **no preopened directory, no
inherited env, no args, no network sockets**.

Defense is layered:

1. **Import allowlist, fail-closed.** Before instantiation, `assert_no_dangerous_authority` introspects the
   compiled module's imports and rejects it if any import falls outside a 9-function allowlist of
   `wasi_snapshot_preview1` (`environ_get/sizes_get`, `clock_time_get` [fixed-zero], `fd_close/fdstat_get/
read/seek/write`, `proc_exit`). Absent by construction: `path_open`, any `sock_*`, `random_get`, any
   preopen. So **no filesystem, no network/sockets, no real clock, no host randomness, no environment**.
   `Math.random()` is QuickJS's own PRNG (deterministic), and `Date.now()` is 0.
2. **Hard limits.** `code_mode.rs` constants: 5 s wall-time budget enforced via Wasmtime epoch
   interruption (a 50 ms ticker on a dedicated OS thread); 64 MiB max linear memory via `StoreLimits`;
   `MAX_HOST_CALLS = 64`; `MAX_OUTPUT_BYTES = 256 KiB` (truncated on a UTF-8 char boundary); a 1 MiB RPC
   frame cap plus a raw-buffer cap as defense-in-depth.

### Hash-pinned guest

The committed `code_mode_guest/harness.wasm` is verified at load against a pinned SHA-256
(`GUEST_WASM_SHA256` in `code_mode.rs`); a mismatch yields an honest error, never a panic. A unit test
(`code_mode/tests.rs::guest_sha256_matches_pin`) fails the build if the guest changes without updating the
pin. Production never needs `javy` installed — only the committed `.wasm` is loaded.

### Capability-scoped, read-only host API

The only host surface is two synchronous RPC functions exposed to the guest, both **read-only**:

- `query_history({ query, plane?, limit?, profileId?, domain?, starredOnly? })` — runs the _same_ shared
  retrieval (`search_history_internal`) as the agent's search tools; `limit` is clamped to `[1, 50]`.
- `fetch_visits(ids: number[])` — resolves up to 50 visit ids against a bounded recent-window read model;
  silently drops unknown/reverted ids.

Neither performs INSERT/UPDATE/DELETE. There is **no DB handle, no SQL string, no `func_wrap` bridge** —
the linker only adds the scoped WASI context. The `run_code` tool description (`agent_tools.rs::
RUN_CODE_DESCRIPTION`) tells the model the program is read-only with no network/fs/clock/randomness.

### Threading

`run_code` runs on a blocking worker thread (`agent_tools.rs` uses `tokio::task::spawn_blocking`); the wasm
executes synchronously there and the epoch ticker runs on its own OS thread — **never on the UI thread**.
Host retrieval uses `block_in_place` so tokio keeps serving other tasks. User cancel bumps the epoch from
the ticker thread, trapping the guest at the next tick.

### Tested

`code_mode/tests.rs` pins the contract with ~37 functional + security tests, including: fs/net/socket
imports rejected before running; real JS reaching for `require('fs')`/`fetch()`/`process.env` fails safely;
fixed-zero clock; infinite loops trap on the wall-time deadline; allocation storms hit the memory limiter,
not a host panic; the host-call budget trips after 64; oversized output/frames are capped/poisoned;
cancel mid-script yields a clean outcome.

---

## 3. Outward MCP face (W-AI-9-B): localhost, read-only, audited

The MCP server lets an external AI tool query the user's history through the _same bounded retrieval_.
Implementation: `vault-worker/src/mcp.rs`, launched via the `mcp-server` CLI command
(`vault-worker/src/cli.rs`).

- **Transport: localhost stdio only.** The server is served over `rmcp::transport::io::stdio()` — there is
  **no TCP/network bind**. It speaks MCP over the process's stdin/stdout.
- **Opt-in, hard-default-OFF, unlock-gated.** Startup bails unless `config.ai.enabled &&
config.ai.mcp_enabled` (both default `false`) and additionally bails if the app session is locked
  (`resolved_app_lock_status(...).locked`). Every tool call re-checks the unlock state before reading the
  archive: `archive-status` returns a degraded snapshot with an honest "locked" warning, and
  `search-history` refuses (errors via `ensure_app_lock_unlocked`) — neither bypasses App Lock
  (aligns with [ADR-005](decisions/005-app-lock-session-boundary.md)).
- **Read-only surface — three tools, no mutation.** `search-history`, `archive-status`, and `usage-guide`.
  There is **no write/mutation tool**. The search limit is clamped to `[1, 50]` in `ai/search.rs`, so no
  external caller can pull an unbounded page.
- **The SQLCipher key never crosses to the caller.** The worker holds the database key
  (`read_database_key_from_keyring`, stored in the `BrowserHistoryMcpServer` struct) and uses it only to
  open a local connection. MCP responses carry query results, status metadata, and the usage-guide JSON —
  **never the key**.
- **Every external query is audited as `mcp_query`.** `record_mcp_query_run` writes a `runs` row with
  `run_type = 'mcp_query'`, `trigger = 'external'`, the profile scope, warnings, and a query summary —
  viewable in the Audit ledger. So the user sees every external touch, not just searches.

### Skills (W-AI-9-C)

The `usage-guide` MCP tool serves a JSON usage guide (granularity ladder / search-mode / citation
discipline / bounds) gated on `config.ai.skill_enabled` (default `false`). When disabled it returns
`enabled: false`, empty sections, and an honest notice. When enabled (and unlocked + initialized), the
fetch is audited like any other query. Built in `mcp.rs::build_mcp_usage_guide`.

### Known gap

The `archive-status` probe is **not** audited when the app is **locked** (`mcp.rs`, the `if !lock.locked`
guard around the audit write). The stated rationale: a locked status read touches no encrypted archive
content and holds no writable connection, so there is no archive access to record. This is an explicit,
documented carryover — see §7.

---

## 4. Egress (W-ENRICH content-fetch / og:image): off by default, SSRF-guarded

The only AI-side network egress is optional site-content / preview-image fetching. It is **hard-default-OFF
and decoupled from the offline title plugin**: `content_fetch_enabled` defaults `false`, and the job runner
is a no-op until the user opts in. The single egress chokepoint is `enrichment/content_fetch.rs`; the
og:image fetcher is `archive/history/og_images_fetch.rs`.

- **Triple consent gate.** `content_fetch_allowed` requires the master `content_fetch_enabled`, then a
  per-extractor toggle, then a per-domain rule — egress only happens for a URL that clears all three.
- **SSRF-guarded on every hop.** `archive/history/net_guard.rs::url_target_is_blocked` rejects non-http(s)
  schemes and any URL resolving to loopback, RFC1918 private, link-local (incl. the
  `169.254.169.254` metadata address), CGNAT, IPv6 unique/link-local, multicast, or reserved space. It is
  applied to the initial URL, **every redirect hop** (`redirect_hop_is_blocked` via the reqwest redirect
  policy), the **post-redirect final URL** (`guard_final_url`, defense-in-depth), and **every API
  sub-resource** (`guard_then_fetch_json`).
- **No cookies, no Referer, no fingerprinting.** The fetch client (`og_images_fetch.rs::
fetch_client_builder`) sets only a single static desktop Chrome UA, `Accept-Language`, and a fixed
  set of static, non-identifying headers (`sec-fetch-*`, `sec-ch-ua-*`, `Upgrade-Insecure-Requests`);
  it never sets a cookie or `Referer` header and carries nothing account- or session-specific.
- **Offline-first, per-host rate-limited.** No implicit egress on any operation; per-host token buckets
  (`enrichment/rate_limit.rs`) throttle requests and negative caching avoids retry storms.

---

## 5. Data sovereignty: derived AI state stays out of canonical, and off the export

The canonical archive is `archive/history-vault.sqlite` (SQLCipher) — the single source of truth, holding
**only** canonical facts. All AI-derived state is rebuildable and lives outside it:

| Plane                         | Location                              | In canonical? | In portable export?  |
| ----------------------------- | ------------------------------------- | ------------- | -------------------- |
| f32 vector store              | `derived/vectors/*.pkvec`             | No            | **No** (rebuild)     |
| visit→content map (W-AI-4c)   | `derived/vectors/*.pkmap`             | No            | **No**               |
| binary-recall plane (W-AI-5)  | `derived/vectors/*.pkbin`             | No            | **No**               |
| int8-rescore plane (W-AI-5)   | `derived/vectors/*.pki8`              | No            | **No**               |
| chat transcripts + run traces | `derived/agent.sqlite`                | No            | **No** (privacy)     |
| downloaded models             | `models/`                             | No            | **No** (re-download) |
| AI metadata / read model      | `derived/history-intelligence.sqlite` | No            | Yes (rebuildable)    |
| canonical facts               | `archive/history-vault.sqlite`        | **Yes**       | Yes                  |

- **Vectors never live in canonical SQLite.** The hand-rolled `FlatVectorIndex`
  (`ai/vector_index.rs`, planes in `ai/vector_planes.rs`, store in `ai/vector_store.rs`) writes only the
  `.pkvec/.pkmap/.pkbin/.pki8` sidecar files under `derived/vectors/`. The export bundle excludes the whole
  plane (`migration.rs::DERIVED_VECTOR_PLANE_EXTENSIONS = ["pkvec","pkmap","pkbin","pki8"]`) — the ~59 GB
  f32 tail is rebuildable derived state, re-embedded on the target rather than shipped.
- **Chat transcripts are excluded from the export.** Conversations, messages, and W-AI-7 agent run/step/
  citation traces persist only in `derived/agent.sqlite` (`agent_store.rs`). The export explicitly excludes
  it plus its WAL/SHM/journal siblings (`migration.rs::DERIVED_EXPORT_EXCLUDED_BASENAMES = ["agent.sqlite"]`);
  `EXPORT_EXCLUSIONS_DOC` states "Assistant chat transcripts stay on the source machine." Both exclusions
  are pinned by `migration.rs` tests (`export_excludes_agent_chat_transcripts_from_the_bundle`,
  `export_excludes_vector_sidecar_plane_from_the_bundle`).
- **The GPU tier is an opt-in reproducible-build feature.** Metal is an off-by-default `metal` cargo
  feature (`vault-core/Cargo.toml`); `gpu_enabled` is inert unless the binary was built with it. The
  default/CI build compiles out `Device::new_metal(0)` entirely (`ai/embedding_candle.rs::select_device`,
  cfg-gated `candle_device_for`), so a CPU-only build never touches GPU code. Toggling `gpu_enabled` does
  not change the embedding `model_id`/fingerprint (CPU and Metal produce the same vectors), so it never
  auto-invalidates the index.

---

## 6. Threat model

### Assets

The user's full local browsing history (canonical, in SQLCipher), the App Lock secret material (Stronghold,
local-only), and the user's configured provider API keys (in the OS keyring).

### Assumptions / trust boundaries

- **LLM output is untrusted.** A model may be steered by injected page titles/URLs in the context. Mitigation:
  the agent's tools are **read-only retrieval** over the user's own archive; the **code-mode sandbox** gives
  the model zero ambient authority (no fs/net/clock/random/env) and a read-only host API with hard limits;
  the context is bounded (recency-pruned, citation-pinned). A steered model cannot read the filesystem,
  reach the network, exfiltrate the SQLCipher key, or mutate the archive — the worst case is a wrong answer
  over the user's own data, which the citation contract surfaces for review.
- **An external MCP caller is untrusted.** Mitigation: the MCP face is localhost stdio, opt-in +
  default-OFF + unlock-gated, exposes only read-only search/status/guide tools, clamps the result limit to
  50, keeps the SQLCipher key in the worker, and audits every query as `mcp_query`.
- **The local network/machine is trusted.** PathKeep is a local-first desktop app; the MCP transport is
  stdio (no socket to attack), and content-fetch only reaches out to public hosts the user opted into,
  SSRF-guarded against internal targets.
- **Provider endpoints are user-chosen.** The user configures their own LLM/embedding providers (local
  Ollama/LM Studio or a cloud API); PathKeep does not host or proxy AI, and the user's history leaves the
  machine only to the provider the user explicitly configured for assistant/embedding calls.

### Explicitly out of scope

- A malicious LLM _provider endpoint_ the user deliberately configured (it sees the prompts/context the
  user sends it by design — that is the user's trust decision, not a PathKeep boundary).
- A compromised host OS / a local attacker with code execution as the user (can read the keyring and the
  archive key directly; PathKeep's in-process boundaries do not defend against local root).
- Cloud hosting, accounts, cross-machine sync (never built — see
  [03-implementation-plan.md §6](../plan/program/ai-redesign-2026/03-implementation-plan.md)).
- Replacing deterministic intelligence or lexical recall with AI, or weakening the canonical encryption
  boundary (all explicitly excluded from the program scope).

---

## 7. Residual risks & recommended follow-ups

These are flagged for later; none blocks the shipped surface, but each is a real edge the reviews surfaced:

- **Locked `archive-status` probe is not audited** (`mcp.rs`). A locked status read writes no audit row.
  Rationale is documented (no archive access to record), but a future hardening pass could record an
  external-touch trace even for the locked path.
- **MCP progress is global-queue, not per-job.** The MCP/queue progress surface reflects the global queue
  rather than per-job granularity; per-job MCP progress is a follow-up.
- **Full 14.4M "all-AI-on" profiling is recommended.** The fluidity envelope (no main-thread freeze on a
  14.4M archive with embedding/search/code-exec all active) should be backed by a React Profiler /
  flamegraph artifact, not assumed.
- **A full prompt-injection red-team is recommended.** Basic adversarial cases are covered by the sandbox
  contract and read-only tools; a dedicated red-team over injected page titles/URLs would harden the
  assumption that untrusted LLM steering cannot exfiltrate.
- **A Metal CI lane must re-run `cargo deny`.** The `metal` crates only enter the dependency graph under
  `--features metal`; a future Metal CI lane that builds with the feature MUST re-run `cargo deny check`,
  since the default/CI graph never sees those crates (noted in `vault-core/Cargo.toml`).
