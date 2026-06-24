//! Code-mode sandbox core + capability-scoped read-only host API (W-AI-8 WU-1, D7 / 02 §G Layer 2).
//!
//! ## Responsibilities
//! - run an LLM-authored **JavaScript** program inside a **Wasmtime** sandbox whose only authority
//!   is a host-controlled stdin/stdout RPC channel — **no DANGEROUS ambient authority** (no
//!   filesystem, no network/sockets, no environment, no real wall clock; the JS engine's `Date.now()`
//!   reads a fixed zero clock and `Math.random()` is the QuickJS deterministic PRNG). The guest is a
//!   pre-compiled **Javy** (QuickJS→WASM) module; it reaches the host ONLY through WASI fd I/O, which
//!   the scoped [`WasiP1Ctx`] binds to in-memory pipes the host owns (verified by import
//!   introspection: only `wasi_snapshot_preview1`, and only the read/write/clock/exit set Javy uses).
//! - enforce HARD resource limits host-side (never by trusting the guest): an epoch wall-time
//!   deadline (→ [`LimitsHit::Time`]), a [`wasmtime::StoreLimits`] memory/instance cap
//!   (→ [`LimitsHit::Memory`]), a per-script host-call budget (→ [`LimitsHit::HostCalls`]), and a
//!   result OUTPUT cap in bytes (→ [`LimitsHit::Output`]). Any limit/trap yields a clean
//!   [`CodeOutcome`] with the partial output, NEVER a host panic.
//! - expose the capability-scoped host API (ADR J.6 freeze) by REUSING the W-AI-5/6 retrieval
//!   (`search_history_internal`) verbatim and resolving each row's `canonical_url` exactly as
//!   `agent_tools.rs` does (the W-STAR star key, so distilled output stays citable/starrable).
//! - tie [`AiRunControl`] cancellation to an epoch bump so a user cancel traps the guest promptly,
//!   and run the wasm synchronously on the caller's worker thread (never the UI).
//!
//! ## Not responsible for
//! - wrapping this as a `run_code` [`AgentTool`](super::agent_tools::AgentTool) (that is WU-2) or
//!   the agent loop / journaling / PME (that is `agent_harness.rs`)
//! - opening a DB handle, holding the SQLCipher key, or building SQL — the host fns receive only an
//!   [`AgentToolContext`] and call the EXISTING retrieval; no connection/SQL/key crosses the wasm
//!   boundary (the guest can name only the WASI fd channel, and the channel speaks only the two
//!   read-only host ops)
//!
//! ## The guest engine (the engineering decision — D7)
//! The production guest is **Javy** (Bytecode Alliance, the accepted D7 engine): the `javy` CLI
//! statically links QuickJS into a self-contained `harness.wasm` whose ONLY imports are nine
//! `wasi_snapshot_preview1` functions (environ get/sizes, `clock_time_get`, fd close/fdstat/read/
//! seek/write, `proc_exit`) — there is NO fs preopen, NO sockets, NO env, and (crucially) NO
//! `random_get` import. The host links those WASI fns through a SCOPED [`WasiP1Ctx`] that grants
//! exactly four capabilities and NOTHING else: stdin = an in-memory pipe carrying the host's RPC
//! replies; stdout = an in-memory sink capturing the guest's RPC requests (serviced synchronously);
//! stderr = a discard sink; and a fixed zero wall + monotonic clock (so `Date.now()` cannot read
//! real time). No preopen, no env, no args, no socket capability.
//!
//! That is the §G "no dangerous ambient authority" contract: the guest physically cannot touch the
//! filesystem, the network, the environment, or the real clock. `harness.js` (committed alongside
//! `harness.wasm`) is the JS the CLI compiled; the `.wasm` is committed as a build artifact and its
//! SHA-256 is pinned + asserted at load ([`GUEST_WASM_SHA256`]) so a tampered guest fails closed.
//! Production does NOT need `javy` installed — only the committed `.wasm` is loaded.
//!
//! ### The host<->guest RPC channel (how source-in / result-out / query host fns are wired)
//! Javy static modules expose no general host-import bridge; the only conduit is WASI fd I/O. So the
//! channel is a length-prefixed JSON protocol over stdio (4-byte LE length + UTF-8 JSON body),
//! serviced SYNCHRONOUSLY by the host: the guest WRITES a request to stdout and READS the reply from
//! stdin; the host computes the reply during that read. The ops are `source` →
//! `{ "source": <the LLM JS> }` (the harness fetches the program this way; stdin is the reply
//! channel, not a one-shot source feed); `query_history` / `fetch_visits` →
//! `{ "rows": [...], "notes": [...] }` (the read-only retrieval / bounded id lookup); `result` →
//! `{ "ok": true }` (the distilled output the model sees); and `error` → `{ "ok": true }` (an honest
//! JS syntax/runtime error).
//!
//! `harness.js` exposes `query_history(argsObj)` / `fetch_visits(ids)` as JS globals over the
//! `query_history` / `fetch_visits` ops, and `eval`s the source as a function body (so a top-level
//! `return` distills a value). The host refuses an over-budget / cancelled call by CLOSING the reply
//! channel (a zero-length read), which the harness turns into a thrown Error → a clean wind-down.
//!
//! ## Performance notes
//! - the wasm runs on the caller's worker thread via `block_in_place`, so the async retrieval host
//!   fn can `Handle::block_on` without blocking a tokio worker; the UI thread is never touched.
//! - the epoch deadline + StoreLimits + host-call budget bound CPU, RAM, and retrieval fan-out so a
//!   runaway script can never spin the known O(n) `is:starred` `.pkmap` pass or exhaust memory.

use super::AiRunControl;
use super::agent_tools::AgentToolContext;
use super::search::search_history_internal;
use crate::models::{AiCitation, AiSearchRequest};
use anyhow::Result;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;
use wasmtime::{
    Config, Engine, Linker, Module, ResourceLimiter, Store, StoreLimits, StoreLimitsBuilder,
};
use wasmtime_wasi::WasiCtxBuilder;
use wasmtime_wasi::cli::{IsTerminal, StdinStream, StdoutStream};
use wasmtime_wasi::clocks::{HostMonotonicClock, HostWallClock};
use wasmtime_wasi::p1::{WasiP1Ctx, add_to_linker_sync};
use wasmtime_wasi::p2::{InputStream, OutputStream, Pollable, StreamError, StreamResult};

/// The committed Javy (QuickJS→WASM) guest, loaded as bytes and parsed by `Module::new`.
///
/// Build-time-only artifact: produced once by the `javy` CLI from [`harness.js`] (see the dev task
/// in "## The guest engine" above). Loaded via `include_bytes!` so production never needs `javy`.
///
/// [`harness.js`]: ../ai/code_mode_guest/harness.js
const GUEST_WASM: &[u8] = include_bytes!("code_mode_guest/harness.wasm");

/// SHA-256 of [`GUEST_WASM`], pinned so a tampered/swapped guest fails closed at load.
///
/// Regeneration (dev task): edit `code_mode_guest/harness.js`, rebuild with
/// `javy build harness.js -o harness.wasm -C source=omitted -C deterministic` (Javy ≥ 9; the
/// `deterministic` flag makes the artifact byte-reproducible), then run the unit test
/// `guest_sha256_matches_pin` — it prints the new digest on mismatch; paste it here.
const GUEST_WASM_SHA256: &str = "85c423ecf4ea7fe7aabedd2b55bc001c240aa2a6e3f6cd29dda9d4036c885803";

/// The Javy module's entry export (WASI command convention).
const GUEST_ENTRY: &str = "_start";

/// The single WASI module the Javy guest is allowed to import. Any other import module (fs/net/etc.)
/// is a dangerous-authority violation rejected before instantiation.
const ALLOWED_IMPORT_MODULE: &str = "wasi_snapshot_preview1";

/// The exact `wasi_snapshot_preview1` functions a statically-linked Javy guest imports. NONE of
/// these grants dangerous authority once the host binds them to the scoped ctx: the fd fns operate
/// only on the in-memory stdin/stdout/stderr pipes (no preopen → no real file is reachable),
/// `clock_time_get` reads a fixed zero clock, `environ_*` see an empty environment, and `proc_exit`
/// just ends the run. A guest naming any fn OUTSIDE this set (e.g. `path_open`, `sock_*`,
/// `random_get`) is rejected — that is the host-side proof the import surface did not widen.
const ALLOWED_WASI_FNS: [&str; 9] = [
    "environ_get",
    "environ_sizes_get",
    "clock_time_get",
    "fd_close",
    "fd_fdstat_get",
    "fd_read",
    "fd_seek",
    "fd_write",
    "proc_exit",
];

/// Per-script wall-time budget. The epoch ticker bumps the engine epoch every [`EPOCH_TICK`]; the
/// store's deadline is `WALL_TIME_BUDGET / EPOCH_TICK` ticks, so the guest traps within one tick of
/// the budget. Bounds CPU on the 4-core target without precise instruction counting.
pub const WALL_TIME_BUDGET: Duration = Duration::from_secs(5);

/// Epoch ticker period (see [`WALL_TIME_BUDGET`]). Coarse enough to cost ~nothing, fine enough that
/// a trap lands well within a second of the deadline (and a cancel traps within one tick).
pub const EPOCH_TICK: Duration = Duration::from_millis(50);

/// Max linear-memory bytes the guest may allocate (StoreLimits). Over-alloc traps as
/// [`LimitsHit::Memory`]. 64 MiB is ample for QuickJS + bounded retrieval distillation and far under
/// the 8 GB target envelope. (Javy's runtime base footprint is a few MiB, well within this.)
pub const MAX_GUEST_MEMORY_BYTES: usize = 64 * 1024 * 1024;

/// Max host-API calls one script may make (query_history + fetch_visits combined). Caps retrieval
/// fan-out so a `while(true) query_history()` loop trips [`LimitsHit::HostCalls`] rather than
/// spinning the bounded (but non-free) retrieval pipeline.
pub const MAX_HOST_CALLS: u32 = 64;

/// Max bytes the host will return as the script's output. A larger result is truncated and the
/// [`LimitsHit::Output`] marker is recorded (the model never receives an unbounded blob — 02 §F).
pub const MAX_OUTPUT_BYTES: usize = 256 * 1024;

/// Max rows any single host-API call returns (mirrors the `limit.clamp(1, 50)` retrieval discipline).
pub const MAX_ROWS_PER_CALL: u32 = 50;

/// Max ids a single `fetch_visits` call may request (hard cap on `ids.len()`).
pub const MAX_FETCH_IDS: usize = 50;

/// Hard cap on a single RPC request frame the host will read from the guest (defends the host from a
/// guest claiming an enormous frame length). 1 MiB is far more than any legitimate query request.
const MAX_REQUEST_FRAME_BYTES: usize = 1024 * 1024;

/// Independent host-side cap on the partial request buffer, enforced on the RAW bytes the guest has
/// written so far — BEFORE any declared frame length is trusted. The declared-length check
/// ([`MAX_REQUEST_FRAME_BYTES`]) only fires once 4 length bytes are buffered, so a single huge
/// `fd_write` with no valid small frame prefix could otherwise transiently buffer up to the guest's
/// whole linear memory (~64 MiB). This bound makes the host's buffering independent of the guest's
/// StoreLimits: one legitimate frame is the 4-byte prefix + a body ≤ [`MAX_REQUEST_FRAME_BYTES`], so
/// a slack of a few bytes over that is ample headroom; anything larger is a hostile/malformed guest.
const MAX_REQUEST_BUFFER_BYTES: usize = MAX_REQUEST_FRAME_BYTES + 16;

/// Which hard limit (if any) stopped a script — recorded host-side, surfaced to the user/journal.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum LimitsHit {
    /// The epoch wall-time deadline expired (infinite loop / too-slow script) → trap.
    Time,
    /// The guest tried to allocate past [`MAX_GUEST_MEMORY_BYTES`] → StoreLimits trap.
    Memory,
    /// The script exhausted its host-call budget ([`MAX_HOST_CALLS`]); further calls were refused.
    HostCalls,
    /// The accumulated output crossed [`MAX_OUTPUT_BYTES`] and was truncated.
    Output,
    /// The user cancelled the run; the cancel bumped the epoch so the guest trapped promptly.
    Cancelled,
}

/// A summary of one host-API call the script made (for WU-4 journaling + WU-5 transparency display).
///
/// Carries the fn name, the STRUCTURED call arguments (so the WU-5 FE can localize + render across
/// en/zh-CN/zh-TW without parsing a Rust-debug string), and the row count returned — enough for the
/// user to SEE what the script queried (02 §G transparency) without re-deriving it.
///
/// The structured fields are populated per function: `query_history` carries `query`/`plane`/`limit`;
/// `fetch_visits` carries `requested_ids` (the count of ids asked for). Each is omitted from the wire
/// when absent (`skip_serializing_if`), so a record only ever advertises the args its function used.
/// `args_summary` is kept as a non-localized human/debug fallback — it is the SAME string a Rust
/// reader sees in a log, and it lets any consumer that does NOT localize (trace replay, a raw journal
/// dump) still render the call without re-deriving it from the structured fields.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HostCallRecord {
    /// The host fn invoked (`query_history` or `fetch_visits`).
    pub function: String,
    /// The query string (`query_history` only) — verbatim, so the FE can render + translate around it.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub query: Option<String>,
    /// The retrieval plane as a stable lowercase token (`hybrid`/`vector`/`bm25`; `query_history` only).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub plane: Option<String>,
    /// The effective row limit after clamping (`query_history` only).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub limit: Option<u32>,
    /// How many visit ids were requested (`fetch_visits` only; the call is capped at [`MAX_FETCH_IDS`]).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub requested_ids: Option<u32>,
    /// A bounded, non-localized human/debug fallback summary of the call arguments. Authoritative
    /// rendering is the structured fields above; this stays for consumers that do not localize.
    pub args_summary: String,
    /// How many rows the call returned (0 on a refused/degraded call).
    pub row_count: u32,
}

/// The result of running one code-mode script (carries enough for WU-4 journal + WU-5 display).
///
/// `model_text` is the distilled, bounded output the script returned (what the model would see);
/// `citations` are the canonical_url-keyed evidence rows the script's queries surfaced (so the
/// distilled answer stays citable/starrable — the W-STAR contract). `host_calls` is the transparent
/// timeline; `source` is the script verbatim (so the user sees exactly what ran); `error` is an
/// honest message on a host/guest failure; `limits_hit` records which hard limit (if any) fired.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CodeOutcome {
    /// The script's distilled output (bounded by [`MAX_OUTPUT_BYTES`]); empty on a hard failure.
    pub model_text: String,
    /// Evidence rows the script's queries surfaced, each keyed by `canonical_url` for W-STAR.
    pub citations: Vec<AiCitation>,
    /// The transparent host-call timeline (fn, args summary, row count) — 02 §G.
    pub host_calls: Vec<HostCallRecord>,
    /// The script source verbatim (transparency: the user sees exactly what ran).
    pub source: String,
    /// An honest error message when the run failed (compile/instantiate/trap, or a thrown JS error).
    pub error: Option<String>,
    /// Which hard limit stopped the run, if any.
    pub limits_hit: Option<LimitsHit>,
}

/// Arguments for a `query_history` host call (a JSON subset of [`AiSearchRequest`] + the plane).
#[derive(Debug, Clone, Default, Deserialize)]
struct QueryHistoryArgs {
    /// The search query. OPTIONAL: an empty/omitted query returns the most recent visits.
    #[serde(default)]
    query: String,
    /// Which recall plane to drive: `hybrid` (default), `vector`, or `bm25`.
    #[serde(default)]
    plane: Option<String>,
    /// Optional browser-profile filter.
    #[serde(default)]
    profile_id: Option<String>,
    /// Optional domain filter.
    #[serde(default)]
    domain: Option<String>,
    /// Max rows (clamped to [`MAX_ROWS_PER_CALL`]).
    #[serde(default)]
    limit: Option<u32>,
    /// Restrict recall to starred pages only.
    #[serde(default)]
    starred_only: Option<bool>,
}

/// Arguments for a `fetch_visits` host call: specific visit ids to look up (capped at [`MAX_FETCH_IDS`]).
#[derive(Debug, Clone, Default, Deserialize)]
struct FetchVisitsArgs {
    /// The visit ids to resolve (read model only; hard-capped).
    #[serde(default)]
    ids: Vec<i64>,
}

/// Which retrieval plane a `query_history` call drives (mirrors `agent_tools::SearchPlane`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum CodeSearchPlane {
    /// Lexical BM25 only (works with NO embedding provider).
    Bm25,
    /// Semantic vector recall (embedding provider threaded; degrades to lexical with a note).
    Vector,
    /// Full hybrid lexical + semantic RRF (the default).
    Hybrid,
}

impl CodeSearchPlane {
    /// Parses the optional `plane` string, defaulting to hybrid; an unknown value falls back to
    /// hybrid (honest, never an error — the model may mis-spell the plane).
    fn parse(plane: Option<&str>) -> Self {
        match plane.map(str::trim).map(str::to_ascii_lowercase).as_deref() {
            Some("bm25") | Some("lexical") | Some("keyword") => Self::Bm25,
            Some("vector") | Some("semantic") => Self::Vector,
            _ => Self::Hybrid,
        }
    }

    /// The stable lowercase token for this plane — the SAME spelling the model uses + the WU-5 FE keys
    /// its localized label off (never the Rust enum `Debug`, which would leak `Bm25`/`Hybrid` casing).
    fn as_token(self) -> &'static str {
        match self {
            Self::Bm25 => "bm25",
            Self::Vector => "vector",
            Self::Hybrid => "hybrid",
        }
    }
}

/// A [`ResourceLimiter`] wrapping [`StoreLimits`] that also RECORDS when a memory growth is refused.
///
/// Wasmtime surfaces both an epoch-deadline and an out-of-memory as opaque traps, so distinguishing
/// [`LimitsHit::Memory`] from [`LimitsHit::Time`] by the trap message alone is brittle. This limiter
/// records the refusal at the source (the `memory_growing` callback returns `Ok(false)`), so the
/// outcome can classify a memory-bound trap deterministically. Delegates all enforcement to the
/// inner `StoreLimits` (the configured memory/instance/table caps).
struct BoundedLimiter {
    inner: StoreLimits,
    /// Set true the first time a memory growth is refused by the cap (the deterministic Memory signal).
    memory_refused: bool,
}

impl ResourceLimiter for BoundedLimiter {
    fn memory_growing(
        &mut self,
        current: usize,
        desired: usize,
        maximum: Option<usize>,
    ) -> wasmtime::Result<bool> {
        let allowed = self.inner.memory_growing(current, desired, maximum)?;
        if !allowed {
            self.memory_refused = true;
        }
        Ok(allowed)
    }

    fn table_growing(
        &mut self,
        current: usize,
        desired: usize,
        maximum: Option<usize>,
    ) -> wasmtime::Result<bool> {
        self.inner.table_growing(current, desired, maximum)
    }

    fn instances(&self) -> usize {
        self.inner.instances()
    }

    fn tables(&self) -> usize {
        self.inner.tables()
    }

    fn memories(&self) -> usize {
        self.inner.memories()
    }
}

/// The retrieval/limits/transparency half of the sandbox run — everything the host needs to service
/// the guest's RPC ops, owned by the wasm `Store` (alongside the [`WasiP1Ctx`]).
///
/// The retrieval context + tokio handle let the (sync) RPC service run the async
/// `search_history_internal` via `Handle::block_on` under `block_in_place`. The counters/accumulators
/// enforce the host-call budget + output cap and collect the transparent timeline + citations. NO DB
/// handle/SQL/key lives here — only the `AgentToolContext` the existing retrieval already accepts.
struct HostState {
    /// Retrieval context (paths/config/key/embedding provider + scope defaults) — same as the tools.
    context: AgentToolContext,
    /// Tokio handle to drive the async retrieval from inside the sync RPC service.
    runtime: tokio::runtime::Handle,
    /// Cooperative cancel hook; checked at each host call (the epoch bump handles in-loop cancel).
    control: Option<Arc<dyn AiRunControl>>,
    /// The script source the guest fetches via the `source` op.
    source: String,
    /// Host calls made so far (budget = [`MAX_HOST_CALLS`]).
    host_calls_made: u32,
    /// The script's final output bytes (set by the guest's `result` op, output-capped).
    output: Vec<u8>,
    /// The transparent host-call timeline.
    records: Vec<HostCallRecord>,
    /// Evidence citations accumulated across the script's queries (canonical_url keyed).
    citations: Vec<AiCitation>,
    /// Which hard limit fired during execution, if any (output/host-call/cancel caps set this).
    limits_hit: Option<LimitsHit>,
    /// An honest error the guest reported via the `error` op (a JS syntax/runtime error).
    guest_error: Option<String>,
}

impl HostState {
    /// Services one RPC request frame from the guest, returning the JSON reply bytes — or `None` to
    /// REFUSE the call (the host closes the reply channel, which the harness turns into a thrown
    /// Error → clean wind-down). Refusal happens on the host-call budget, a cancel, or a malformed /
    /// over-cap request. This is the single choke point through which all guest↔host data flows.
    fn service_request(&mut self, body: &[u8]) -> Option<Vec<u8>> {
        let request: Value = serde_json::from_slice(body).ok()?;
        match request.get("op").and_then(Value::as_str) {
            Some("source") => {
                // The harness always asks for the source first; this op does NOT consume the budget.
                Some(serde_json::to_vec(&json!({ "source": self.source })).ok()?)
            }
            Some("result") => {
                self.capture_result(request.get("value"));
                Some(serde_json::to_vec(&json!({ "ok": true })).ok()?)
            }
            Some("error") => {
                let message = request.get("message").and_then(Value::as_str).unwrap_or("");
                self.guest_error = Some(message.to_string());
                Some(serde_json::to_vec(&json!({ "ok": true })).ok()?)
            }
            Some("query_history") => self.service_host_call(HostRequest::QueryHistory(
                serde_json::from_value(args_of(&request)).ok()?,
            )),
            Some("fetch_visits") => self.service_host_call(HostRequest::FetchVisits(
                serde_json::from_value(args_of(&request)).ok()?,
            )),
            // An unknown op is refused (the host never trusts the guest's bytes).
            _ => None,
        }
    }

    /// Captures the guest's `result` value as the model-facing output, applying the output cap.
    ///
    /// `serde_json::to_vec` always yields valid UTF-8, so an over-cap result is truncated on the
    /// largest char boundary ≤ [`MAX_OUTPUT_BYTES`] (never mid-codepoint) — a raw byte cut would split
    /// a multibyte CJK/emoji glyph and leave `model_text` ragged (the lossy decode would emit U+FFFD).
    fn capture_result(&mut self, value: Option<&Value>) {
        let bytes = match value {
            Some(value) => serde_json::to_vec(value).unwrap_or_default(),
            None => Vec::new(),
        };
        if bytes.len() > MAX_OUTPUT_BYTES {
            self.output = bytes[..char_boundary_floor(&bytes, MAX_OUTPUT_BYTES)].to_vec();
            self.limits_hit.get_or_insert(LimitsHit::Output);
        } else {
            self.output = bytes;
        }
    }

    /// Services a parsed `query_history` / `fetch_visits` request: budget check → cancel check → run
    /// the read-only retrieval.
    ///
    /// Returns the reply bytes, or `None` to refuse (budget exhausted / cancelled / retrieval or
    /// validation error). A refusal records the relevant limit but NEVER traps the host.
    fn service_host_call(&mut self, request: HostRequest) -> Option<Vec<u8>> {
        // Host-call budget: refuse past MAX_HOST_CALLS and record the limit (the trip is host-side,
        // not trusted to the guest). A while(true) query loop trips this rather than spinning.
        if self.host_calls_made >= MAX_HOST_CALLS {
            self.limits_hit.get_or_insert(LimitsHit::HostCalls);
            return None;
        }
        // Cooperative cancel at the host-call boundary (the epoch bump covers in-loop cancel; this
        // covers a cancel observed exactly at a call). A refused-by-cancel call closes the channel.
        if self.control.as_ref().is_some_and(|control| control.cancelled()) {
            self.limits_hit.get_or_insert(LimitsHit::Cancelled);
            return None;
        }
        self.host_calls_made += 1;

        // A retrieval / validation error is reported as a refused call (the script can wind down);
        // the host never propagates a retrieval failure into a host trap.
        let reply = self.run_host_request(request).ok()?;
        Some(reply)
    }

    /// Runs one parsed host request against the existing retrieval, returning the JSON reply bytes.
    ///
    /// This is the ONE place the host API touches PathKeep data, and it does so ONLY through the
    /// shared `search_history_internal` (so code-mode and the plain tools never drift). It resolves
    /// `canonical_url` exactly as `agent_tools.rs` does so distilled output stays starrable.
    fn run_host_request(&mut self, request: HostRequest) -> Result<Vec<u8>> {
        let (function, call_args, response) = match request {
            HostRequest::QueryHistory(args) => self.run_query_history(args)?,
            HostRequest::FetchVisits(args) => self.run_fetch_visits(args)?,
        };
        let row_count = response.items.len() as u32;
        // Resolve canonical_url for every row (the W-STAR key) and accrue citations.
        let mut rows = Vec::with_capacity(response.items.len());
        for item in &response.items {
            let canonical_url = crate::visit_taxonomy::normalize_visit_url(&item.url)
                .map(|normalized| normalized.canonical_url);
            self.citations.push(AiCitation {
                history_id: item.history_id,
                profile_id: item.profile_id.clone(),
                url: item.url.clone(),
                title: item.title.clone(),
                visited_at: item.visited_at.clone(),
                score: Some(item.score),
                canonical_url: canonical_url.clone(),
            });
            rows.push(json!({
                "id": item.history_id,
                "url": item.url,
                "title": item.title,
                "domain": item.domain,
                "visitedAt": item.visited_at,
                "score": item.score,
                "matchReason": item.match_reason,
                "canonicalUrl": canonical_url,
            }));
        }
        self.records.push(HostCallRecord {
            function: function.to_string(),
            query: call_args.query,
            plane: call_args.plane,
            limit: call_args.limit,
            requested_ids: call_args.requested_ids,
            args_summary: call_args.args_summary,
            row_count,
        });
        let reply = json!({ "rows": rows, "notes": response.notes });
        Ok(serde_json::to_vec(&reply)?)
    }

    /// Services a `query_history` call: clamps the limit, picks the plane, runs the shared retrieval.
    fn run_query_history(
        &mut self,
        args: QueryHistoryArgs,
    ) -> Result<(&'static str, HostCallArgs, crate::models::AiSearchResponse)> {
        // An empty/blank query is NOT refused: it flows through to `search_history_internal`, which
        // returns the most recent visits (browse-by-recency). This is the agent's only way to
        // ENUMERATE history — to find the date range or list "what did I do recently" — since
        // semantic recall is keyword-driven. Refusing it left date-range questions with no entry
        // point and made the model loop.
        let query = args.query.trim().to_string();
        let plane = CodeSearchPlane::parse(args.plane.as_deref());
        let limit = args.limit.unwrap_or(self.context.default_limit).clamp(1, MAX_ROWS_PER_CALL);
        // Structured args for the localizable FE timeline, plus a non-localized debug fallback. The
        // `plane` token is the stable lowercase form the model/FE understand (not the Rust enum Debug).
        let call_args = HostCallArgs {
            query: Some(query.clone()),
            plane: Some(plane.as_token().to_string()),
            limit: Some(limit),
            requested_ids: None,
            args_summary: format!("query={query:?} plane={} limit={limit}", plane.as_token()),
        };
        let request = AiSearchRequest {
            query,
            profile_id: args.profile_id.or_else(|| self.context.default_profile_id.clone()),
            domain: args.domain.or_else(|| self.context.default_domain.clone()),
            limit: Some(limit),
            cursor: None,
            starred_only: args.starred_only,
        };
        // Bm25 drops the embedding provider (lexical only → works with NO provider); the semantic
        // planes thread it (degrading to lexical with a note when none is configured).
        let provider = match plane {
            CodeSearchPlane::Bm25 => None,
            CodeSearchPlane::Vector | CodeSearchPlane::Hybrid => {
                self.context.embedding_provider.as_ref()
            }
        };
        let response = self.block_on_search(&request, provider)?;
        Ok(("query_history", call_args, response))
    }

    /// Services a `fetch_visits` call: bounded lookup of specific visit ids (read model only).
    ///
    /// Reuses the lexical retrieval as the read model by resolving each id against a bounded query;
    /// WU-1 keeps this honest + bounded (cap on `ids.len()`) without a second SQL path. The ids that
    /// resolve to a visible visit are returned; unknown/reverted ids are silently dropped (honest).
    fn run_fetch_visits(
        &mut self,
        args: FetchVisitsArgs,
    ) -> Result<(&'static str, HostCallArgs, crate::models::AiSearchResponse)> {
        if args.ids.is_empty() {
            anyhow::bail!("fetch_visits requires a non-empty `ids` array");
        }
        if args.ids.len() > MAX_FETCH_IDS {
            anyhow::bail!(
                "fetch_visits accepts at most {MAX_FETCH_IDS} ids (got {})",
                args.ids.len()
            );
        }
        let requested_ids = args.ids.len() as u32;
        let call_args = HostCallArgs {
            requested_ids: Some(requested_ids),
            args_summary: format!("ids={requested_ids} (capped at {MAX_FETCH_IDS})"),
            ..HostCallArgs::default()
        };
        let response = self.block_on_fetch(&args.ids)?;
        Ok(("fetch_visits", call_args, response))
    }

    /// Drives the async `search_history_internal` from the sync RPC service via the tokio handle.
    ///
    /// `block_in_place` (the wasm runs inside a multi-thread runtime worker via `block_on` at the
    /// entry) lets us `Handle::block_on` the retrieval without "runtime within a runtime". This is
    /// the settled resolution of the async knot (vs wasmtime async host fns) — simpler, and the
    /// epoch deadline already bounds wall time so a slow retrieval cannot hang the script forever.
    fn block_on_search(
        &self,
        request: &AiSearchRequest,
        provider: Option<&super::AiProviderRuntime>,
    ) -> Result<crate::models::AiSearchResponse> {
        let context = &self.context;
        let runtime = self.runtime.clone();
        tokio::task::block_in_place(|| {
            runtime.block_on(search_history_internal(
                &context.paths,
                &context.config,
                context.database_key.as_deref(),
                provider,
                request,
            ))
        })
    }

    /// Bounded id lookup: resolves the requested visit ids to visible rows through the canonical
    /// read model (`list_history` with no query → recent visits), filtered to the wanted ids.
    ///
    /// Reuses the EXISTING read path (no new SQL crosses into code-mode), is bounded (a capped recent
    /// pool, never a corpus scan), and is honest: an id outside the recent pool / a reverted id simply
    /// drops out (the script sees only the ids that resolve to a visible visit). WU-1 keeps this a
    /// bounded recent-window resolver; a future keyed id→row index can widen the window without
    /// changing the host-API contract.
    fn block_on_fetch(&self, ids: &[i64]) -> Result<crate::models::AiSearchResponse> {
        let wanted: std::collections::HashSet<i64> = ids.iter().copied().collect();
        let listing = crate::archive::list_history(
            &self.context.paths,
            &self.context.config,
            self.context.database_key.as_deref(),
            crate::models::HistoryQuery {
                q: None,
                profile_id: None,
                browser_kind: None,
                domain: None,
                start_time_ms: None,
                end_time_ms: None,
                sort: Some("newest".to_string()),
                // A bounded recent pool (the largest `list_history` allows), filtered to the ids.
                limit: Some(1_000),
                page: None,
                cursor: None,
                regex_mode: Some(false),
            },
        )?;
        let items: Vec<crate::models::AiSearchEntry> = listing
            .items
            .iter()
            .filter(|item| wanted.contains(&item.id))
            .map(|item| super::search::history_entry_to_search_entry(item, 1.0, "Fetched by id"))
            .collect();
        Ok(crate::models::AiSearchResponse {
            total: items.len(),
            provider_id: "fetch-visits".to_string(),
            model: "none".to_string(),
            items,
            notes: Vec::new(),
            note_codes: Vec::new(),
            next_cursor: None,
        })
    }
}

/// Extracts the `args` object from a request envelope, defaulting to an empty object when absent (so
/// a call with omitted args parses into the per-fn struct's `#[serde(default)]` fields).
fn args_of(request: &Value) -> Value {
    request.get("args").cloned().unwrap_or_else(|| json!({}))
}

/// The largest index `≤ cap` that lands on a UTF-8 char boundary of `bytes` (which must be valid
/// UTF-8). Walking back from `cap` crosses at most 3 continuation bytes (a codepoint is ≤ 4 bytes),
/// so the truncated slice stays valid UTF-8 instead of splitting a multibyte CJK/emoji glyph. At/past
/// the end (and at 0) `cap` is already a boundary, so the walk-back only runs strictly inside.
fn char_boundary_floor(bytes: &[u8], cap: usize) -> usize {
    let mut end = cap.min(bytes.len());
    while end > 0 && end < bytes.len() && (bytes[end] & 0b1100_0000) == 0b1000_0000 {
        end -= 1;
    }
    end
}

/// A single parsed host-API request the guest relays.
#[derive(Debug, Clone)]
enum HostRequest {
    /// `query_history(args)` → bounded `AiSearchEntry` rows + canonical_url (plane selectable).
    QueryHistory(QueryHistoryArgs),
    /// `fetch_visits(ids)` → bounded lookup of specific visit rows by id.
    FetchVisits(FetchVisitsArgs),
}

/// The STRUCTURED, effective arguments of one serviced host call — the localizable fields the WU-5 FE
/// renders + translates, plus the non-localized `args_summary` debug fallback. Built INSIDE the
/// `run_*` services (so they reflect the clamped/parsed values, not the raw guest input) and folded
/// into the [`HostCallRecord`] alongside the row count. Each per-function field is `None` when the
/// other function ran, so the record only ever advertises the args its function actually used.
#[derive(Debug, Clone, Default)]
struct HostCallArgs {
    /// `query_history`: the query string verbatim.
    query: Option<String>,
    /// `query_history`: the resolved plane as a stable lowercase token (`hybrid`/`vector`/`bm25`).
    plane: Option<String>,
    /// `query_history`: the effective (clamped) row limit.
    limit: Option<u32>,
    /// `fetch_visits`: how many ids were requested (post-validation, pre-resolution).
    requested_ids: Option<u32>,
    /// A non-localized human/debug fallback summary (the same string a Rust log would show).
    args_summary: String,
}

/// The full `Store` data: the scoped WASI ctx + the retrieval/limits state, plus the limiter the
/// `Store` enforces. `add_to_linker_sync` reaches the WASI ctx via the `|state| &mut state.wasi`
/// closure; the RPC streams reach `HostState` through their shared `Arc<Mutex<_>>` (see below), so
/// this struct just OWNS the pieces and keeps them alive for the run.
struct CodeModeStore {
    /// The scoped preview1 WASI ctx (stdin/stdout/stderr pipes + fixed clock; nothing else).
    wasi: WasiP1Ctx,
    /// The resource limiter the `Store` enforces (records memory refusals).
    limits: BoundedLimiter,
}

/// The RPC channel shared between the stdin (reply) and stdout (request) streams and serviced by the
/// host. A guest write to stdout APPENDS to `request_buf`; once a full frame is buffered it is handed
/// to [`HostState::service_request`] and the reply is queued for the stdin stream to drain. A refused
/// call leaves no reply queued, so the next stdin read returns 0 bytes (channel closed for the guest).
///
/// Wrapped in `Arc<Mutex<_>>` because wasmtime-wasi's stdin/stdout streams are separate trait objects
/// that must share this state; the sync linker calls their `read`/`write` serially on one thread, so
/// the mutex is never contended (it exists only to satisfy the `'static + Send` stream bounds).
struct RpcChannel {
    /// The retrieval/limits/transparency state (services each request frame).
    host: HostState,
    /// Partial request bytes the guest has written so far (frames are length-prefixed; see below).
    request_buf: Vec<u8>,
    /// Reply bytes queued for the stdin stream to deliver (length-prefixed frames).
    reply_buf: Vec<u8>,
    /// How many reply bytes have already been delivered to the guest.
    reply_pos: usize,
    /// Set when a request frame claimed an over-cap length (the host refuses to read it) — the run
    /// is poisoned so the guest gets EOF on every subsequent read rather than partial garbage.
    poisoned: bool,
}

impl RpcChannel {
    /// Appends guest-written bytes and drains every COMPLETE request frame (4-byte LE length + body),
    /// servicing each and queuing its reply. A refused request queues NO reply (channel closes).
    fn push_request_bytes(&mut self, bytes: &[u8]) {
        if self.poisoned {
            return;
        }
        self.request_buf.extend_from_slice(bytes);
        // Defense in depth: cap the RAW buffered bytes independently of any declared frame length, so
        // the host bound never relies on the guest's StoreLimits. A single oversized write (with no
        // valid small frame to drain) is caught here even before its 4-byte length prefix is trusted.
        if self.request_buf.len() > MAX_REQUEST_BUFFER_BYTES {
            self.poisoned = true;
            self.request_buf.clear();
            return;
        }
        loop {
            if self.request_buf.len() < 4 {
                return;
            }
            let len = u32::from_le_bytes([
                self.request_buf[0],
                self.request_buf[1],
                self.request_buf[2],
                self.request_buf[3],
            ]) as usize;
            // A frame larger than the cap is a malformed/hostile guest: poison the channel so the
            // guest reads EOF and winds down, rather than the host buffering an unbounded request.
            if len > MAX_REQUEST_FRAME_BYTES {
                self.poisoned = true;
                self.request_buf.clear();
                return;
            }
            if self.request_buf.len() < 4 + len {
                return;
            }
            let body = self.request_buf[4..4 + len].to_vec();
            self.request_buf.drain(0..4 + len);
            // A serviced request queues its reply; a refused one (None) queues nothing, so the next
            // stdin read returns 0 → the harness throws and the host returns the partial result + the
            // recorded limit marker.
            if let Some(reply) = self.host.service_request(&body) {
                self.queue_reply(&reply);
            }
        }
    }

    /// Queues a reply as a length-prefixed frame for the stdin stream to deliver.
    fn queue_reply(&mut self, reply: &[u8]) {
        self.reply_buf.extend_from_slice(&(reply.len() as u32).to_le_bytes());
        self.reply_buf.extend_from_slice(reply);
    }

    /// Delivers up to `size` queued reply bytes to the guest (the stdin stream's `read`). An empty
    /// return signals the channel is closed (no reply pending) → the guest's read returns 0.
    fn drain_reply(&mut self, size: usize) -> Vec<u8> {
        let end = (self.reply_pos + size).min(self.reply_buf.len());
        let chunk = self.reply_buf[self.reply_pos..end].to_vec();
        self.reply_pos = end;
        chunk
    }
}

/// A fixed zero wall clock: `Date.now()` in the guest reads 0 (no real-time leak, reproducible).
struct ZeroWallClock;
impl HostWallClock for ZeroWallClock {
    fn resolution(&self) -> Duration {
        Duration::from_secs(1)
    }
    fn now(&self) -> Duration {
        Duration::ZERO
    }
}

/// A fixed zero monotonic clock (same posture as the wall clock).
struct ZeroMonotonicClock;
impl HostMonotonicClock for ZeroMonotonicClock {
    fn resolution(&self) -> u64 {
        1
    }
    fn now(&self) -> u64 {
        0
    }
}

/// The stdin stream = the host's RPC REPLY channel. Reading it delivers queued reply bytes; an empty
/// read means the host refused/finished (the guest sees EOF).
#[derive(Clone)]
struct ReplyStdin(Arc<std::sync::Mutex<RpcChannel>>);
impl IsTerminal for ReplyStdin {
    fn is_terminal(&self) -> bool {
        false
    }
}
impl StdinStream for ReplyStdin {
    fn async_stream(&self) -> Box<dyn tokio::io::AsyncRead + Send + Sync> {
        // Never used: the sync linker calls `p2_stream().read` directly. A dummy keeps the trait
        // satisfied without an async runtime in the loop.
        Box::new(tokio::io::empty())
    }
    fn p2_stream(&self) -> Box<dyn InputStream> {
        Box::new(ReplyInput(self.0.clone()))
    }
}
struct ReplyInput(Arc<std::sync::Mutex<RpcChannel>>);
#[wasmtime_wasi::async_trait]
impl Pollable for ReplyInput {
    async fn ready(&mut self) {}
}
impl InputStream for ReplyInput {
    fn read(&mut self, size: usize) -> StreamResult<bytes::Bytes> {
        let chunk =
            self.0.lock().unwrap_or_else(std::sync::PoisonError::into_inner).drain_reply(size);
        if chunk.is_empty() {
            // No reply queued → the channel is closed for the guest (a refused call or run end).
            Err(StreamError::Closed)
        } else {
            Ok(bytes::Bytes::from(chunk))
        }
    }
}

/// The stdout stream = the host's RPC REQUEST channel. Each guest write appends request bytes; a
/// complete frame is serviced synchronously and its reply queued for the stdin stream.
#[derive(Clone)]
struct RequestStdout(Arc<std::sync::Mutex<RpcChannel>>);
impl IsTerminal for RequestStdout {
    fn is_terminal(&self) -> bool {
        false
    }
}
impl StdoutStream for RequestStdout {
    fn async_stream(&self) -> Box<dyn tokio::io::AsyncWrite + Send + Sync> {
        Box::new(tokio::io::sink())
    }
    fn p2_stream(&self) -> Box<dyn OutputStream> {
        Box::new(RequestOutput(self.0.clone()))
    }
}
struct RequestOutput(Arc<std::sync::Mutex<RpcChannel>>);
impl OutputStream for RequestOutput {
    fn write(&mut self, bytes: bytes::Bytes) -> StreamResult<()> {
        self.0.lock().unwrap_or_else(std::sync::PoisonError::into_inner).push_request_bytes(&bytes);
        Ok(())
    }
    fn flush(&mut self) -> StreamResult<()> {
        Ok(())
    }
    fn check_write(&mut self) -> StreamResult<usize> {
        Ok(usize::MAX)
    }
}
#[wasmtime_wasi::async_trait]
impl Pollable for RequestOutput {
    async fn ready(&mut self) {}
}

/// A discard sink for the guest's stderr (Javy writes diagnostics there; the host ignores them).
#[derive(Clone)]
struct SinkStderr;
impl IsTerminal for SinkStderr {
    fn is_terminal(&self) -> bool {
        false
    }
}
impl StdoutStream for SinkStderr {
    fn async_stream(&self) -> Box<dyn tokio::io::AsyncWrite + Send + Sync> {
        Box::new(tokio::io::sink())
    }
    fn p2_stream(&self) -> Box<dyn OutputStream> {
        Box::new(NullSink)
    }
}
struct NullSink;
impl OutputStream for NullSink {
    fn write(&mut self, _bytes: bytes::Bytes) -> StreamResult<()> {
        Ok(())
    }
    fn flush(&mut self) -> StreamResult<()> {
        Ok(())
    }
    fn check_write(&mut self) -> StreamResult<usize> {
        Ok(usize::MAX)
    }
}
#[wasmtime_wasi::async_trait]
impl Pollable for NullSink {
    async fn ready(&mut self) {}
}

/// Builds the SCOPED preview1 [`WasiP1Ctx`]: stdin = RPC replies, stdout = RPC requests, stderr =
/// sink, a fixed zero clock, and NOTHING else (no preopen / env / args / network). This is the §G
/// "no dangerous ambient authority" boundary expressed in code: by NEVER calling `preopened_dir`,
/// `env`/`inherit_env`, `args`, or any `inherit_network`/`allow_tcp`/`allow_udp`, the guest's WASI
/// has no filesystem, no environment, no process args, and no sockets. (Javy imports no `random_get`,
/// so WASI randomness is unreachable regardless; `Math.random()` is the QuickJS deterministic PRNG.)
fn build_scoped_wasi(channel: Arc<std::sync::Mutex<RpcChannel>>) -> WasiP1Ctx {
    let mut builder = WasiCtxBuilder::new();
    builder.stdin(ReplyStdin(channel.clone()));
    builder.stdout(RequestStdout(channel));
    builder.stderr(SinkStderr);
    builder.wall_clock(ZeroWallClock);
    builder.monotonic_clock(ZeroMonotonicClock);
    builder.build_p1()
}

/// Builds the locked-down [`Engine`]: epoch interruption + fuel ON, plus the `async`/component-model
/// support `wasmtime-wasi` preview1 requires. NO pooling/gc.
///
/// The Config is the security contract in code: epoch interruption gives the wall-time deadline +
/// the cancel-trap; fuel gives a deterministic CPU variant for tests. We use the SYNC linker path
/// (`add_to_linker_sync`) — the WASI fd calls run synchronously on the worker thread, so a host RPC
/// can `block_in_place` the retrieval (async support stays off, the wasmtime 45 default). A fresh
/// engine per call keeps runs isolated (no shared store/epoch state).
fn build_engine() -> Result<Engine> {
    let mut config = Config::new();
    config.epoch_interruption(true);
    config.consume_fuel(true);
    // `?` converts wasmtime::Error → anyhow::Error (the two are distinct types but `From` exists).
    Ok(Engine::new(&config)?)
}

/// Asserts the compiled guest imports ONLY the scoped-WASI surface — no fs/net/env/random fn beyond
/// the read/write/clock/exit set Javy uses (the "no dangerous ambient authority" contract, checked by
/// construction before instantiation). A guest naming any other import is rejected fail-closed.
fn assert_no_dangerous_authority(module: &Module) -> Result<()> {
    for import in module.imports() {
        if import.module() != ALLOWED_IMPORT_MODULE {
            anyhow::bail!(
                "guest imports a non-WASI module `{}` (dangerous-authority violation)",
                import.module()
            );
        }
        if !ALLOWED_WASI_FNS.contains(&import.name()) {
            anyhow::bail!(
                "guest imports an unexpected WASI fn `{}::{}` (dangerous-authority violation)",
                import.module(),
                import.name()
            );
        }
    }
    Ok(())
}

/// Runs one code-mode script inside the sandbox, returning a [`CodeOutcome`] (never panics).
///
/// This is the WU-1 callable entry (WU-2 wraps it as a `run_code` tool). The flow:
/// 1. build the locked-down engine + compile the pinned guest; assert no dangerous authority.
/// 2. build the scoped WASI ctx (stdin/stdout RPC channel + fixed clock) over a [`HostState`].
/// 3. start the epoch ticker (→ wall-time deadline) + a cancel watcher (cancel bumps the epoch).
/// 4. set the StoreLimits + epoch deadline + fuel, instantiate, and call `_start`.
/// 5. classify any trap into [`LimitsHit`], thread the captured result/citations/error through.
///
/// `runtime` is the tokio handle the host fn drives async retrieval on; it MUST be a multi-thread
/// runtime handle (so `block_in_place` is valid). Runs synchronously on the caller's worker thread.
pub fn run_code_in_sandbox(
    source: &str,
    context: &AgentToolContext,
    runtime: tokio::runtime::Handle,
    control: Option<Arc<dyn AiRunControl>>,
) -> CodeOutcome {
    run_guest_or_outcome(source, context, runtime, control, GUEST_WASM, GUEST_WASM_SHA256)
}

/// Runs a pinned guest, mapping ANY setup failure to an honest [`CodeOutcome`] (never a panic).
///
/// Parameterized by the guest bytes + its expected SHA-256 so a test can drive the integrity-failure
/// path (a wrong pin) through the exact same error→outcome mapping the public entry uses, without a
/// tampered committed artifact. The public entry passes the real constants.
fn run_guest_or_outcome(
    source: &str,
    context: &AgentToolContext,
    runtime: tokio::runtime::Handle,
    control: Option<Arc<dyn AiRunControl>>,
    wasm: &[u8],
    pin: &str,
) -> CodeOutcome {
    match run_guest_with_pin(source, context, runtime, control, wasm, pin) {
        Ok(outcome) => outcome,
        // Any setup failure (integrity/compile/instantiate) becomes an honest CodeOutcome, never a
        // panic — mapped by the pure [`outcome_from_setup_error`] so the contract is unit-tested.
        Err(error) => outcome_from_setup_error(source, &error),
    }
}

/// Maps a setup failure into an honest [`CodeOutcome`] (empty output, the error string, no limit).
///
/// Pure so the "a setup failure never panics, it yields an honest outcome" contract is tested
/// directly rather than only through the rare real failure paths.
fn outcome_from_setup_error(source: &str, error: &anyhow::Error) -> CodeOutcome {
    CodeOutcome {
        source: source.to_string(),
        error: Some(error.to_string()),
        ..CodeOutcome::default()
    }
}

/// The fallible body: verify the pin (fail closed), then run the guest with unbounded fuel.
///
/// `pin` guards against a tampered/swapped guest (defense in depth). Delegates to [`execute_guest`]
/// with the `_start` entry and an effectively unbounded fuel (the epoch deadline is the wall-time
/// bound; fuel is the deterministic test lever).
fn run_guest_with_pin(
    source: &str,
    context: &AgentToolContext,
    runtime: tokio::runtime::Handle,
    control: Option<Arc<dyn AiRunControl>>,
    wasm: &[u8],
    pin: &str,
) -> Result<CodeOutcome> {
    if !guest_integrity_ok(wasm, pin) {
        anyhow::bail!("committed code-mode guest failed its SHA-256 integrity pin");
    }
    execute_guest(wasm, GUEST_ENTRY, source, context, runtime, control, u64::MAX)
}

/// Whether `wasm`'s SHA-256 equals `expected_sha` (the committed-guest integrity check, testable).
fn guest_integrity_ok(wasm: &[u8], expected_sha: &str) -> bool {
    use sha2::{Digest, Sha256};
    hex::encode(Sha256::digest(wasm)) == expected_sha
}

#[cfg(test)]
thread_local! {
    /// Test-only override of the wall-time budget (so a budget/host-call test that must complete many
    /// real retrievals is not racing the production 5s clock under the instrumented parallel sweep).
    static TEST_WALL_TIME_BUDGET: std::cell::Cell<Option<Duration>> = const { std::cell::Cell::new(None) };
}

/// Test-only PROCESS-GLOBAL wall-time-budget override in milliseconds (0 = unset), for the rare test
/// whose sandbox call runs on a DIFFERENT thread than the test (e.g. the `run_code` tool's
/// `spawn_blocking` worker, where the thread-local override cannot reach). Set under a serialized
/// guard so cross-thread tests don't race; the thread-local takes precedence when set.
#[cfg(test)]
pub(crate) static TEST_WALL_TIME_BUDGET_MS_GLOBAL: std::sync::atomic::AtomicU64 =
    std::sync::atomic::AtomicU64::new(0);

/// The effective wall-time budget for this run (the production [`WALL_TIME_BUDGET`], or a test override).
fn wall_time_budget() -> Duration {
    #[cfg(test)]
    {
        if let Some(budget) = TEST_WALL_TIME_BUDGET.with(std::cell::Cell::get) {
            return budget;
        }
        let global_ms = TEST_WALL_TIME_BUDGET_MS_GLOBAL.load(std::sync::atomic::Ordering::Relaxed);
        if global_ms != 0 {
            return Duration::from_millis(global_ms);
        }
    }
    WALL_TIME_BUDGET
}

/// The epoch-deadline tick count for the current wall-time budget (≥1 so a tiny budget still arms).
fn deadline_ticks() -> u64 {
    (wall_time_budget().as_nanos() / EPOCH_TICK.as_nanos()).max(1) as u64
}

/// Compiles + runs ONE guest module under the locked-down config, returning the assembled outcome.
///
/// The single place the engine/StoreLimits/epoch-deadline/fuel/scoped-WASI are wired, so the public
/// entry and the security tests share the EXACT same security configuration — a test that traps a
/// memory or fuel limit is proving the production config, not a stand-in. Parameterized by the guest
/// bytes, the entry fn, and the fuel budget so a test can drive a purpose-built guest (memory-grow
/// loop, fuel-bounded CPU loop) through the real runtime.
#[allow(clippy::too_many_arguments)]
fn execute_guest(
    guest_wasm: &[u8],
    entry: &str,
    source: &str,
    context: &AgentToolContext,
    runtime: tokio::runtime::Handle,
    control: Option<Arc<dyn AiRunControl>>,
    fuel: u64,
) -> Result<CodeOutcome> {
    let engine = build_engine()?;
    let module = Module::new(&engine, guest_wasm)?;
    assert_no_dangerous_authority(&module)?;

    let host = HostState {
        context: context.clone(),
        runtime,
        control: control.clone(),
        source: source.to_string(),
        host_calls_made: 0,
        output: Vec::new(),
        records: Vec::new(),
        citations: Vec::new(),
        limits_hit: None,
        guest_error: None,
    };
    let channel = Arc::new(std::sync::Mutex::new(RpcChannel {
        host,
        request_buf: Vec::new(),
        reply_buf: Vec::new(),
        reply_pos: 0,
        poisoned: false,
    }));

    let store_data = CodeModeStore {
        wasi: build_scoped_wasi(channel.clone()),
        limits: BoundedLimiter {
            inner: StoreLimitsBuilder::new()
                .memory_size(MAX_GUEST_MEMORY_BYTES)
                .instances(1)
                .tables(1)
                .build(),
            memory_refused: false,
        },
    };
    let mut store = Store::new(&engine, store_data);
    store.limiter(|state| &mut state.limits);
    // The deadline is N ticks; the background ticker bumps the epoch every EPOCH_TICK so the guest
    // traps within one tick of the wall-time budget. epoch_deadline_trap is the default, set here.
    store.set_epoch_deadline(deadline_ticks());
    store.epoch_deadline_trap();
    // Fuel is a deterministic CPU safety net + the test lever; the epoch deadline is the primary
    // wall-time bound. A generous default so normal scripts never run out (tests set it low).
    store.set_fuel(fuel).ok();

    let mut linker: Linker<CodeModeStore> = Linker::new(&engine);
    add_to_linker_sync(&mut linker, |state: &mut CodeModeStore| &mut state.wasi)?;

    // Start the epoch ticker + cancel watcher; both bump the engine epoch. The ticker advances the
    // deadline; a cancel jumps the epoch past the deadline so the guest traps promptly.
    let ticker = EpochTicker::start(engine.clone(), control);

    let instance = linker.instantiate(&mut store, &module)?;
    let run = instance.get_typed_func::<(), ()>(&mut store, entry)?;
    // Normalize the wasmtime::Result into anyhow::Result so the outcome assembler is vendor-free.
    let call_result = run.call(&mut store, ()).map_err(anyhow::Error::from);

    let cancelled = ticker.cancelled();
    ticker.stop();

    // Pull the host state back out of the shared channel (the store + its WASI streams are dropped
    // here, releasing the last Arc clones so the unwrap succeeds).
    let memory_refused = store.data().limits.memory_refused;
    drop(store);
    drop(linker);
    let channel = Arc::try_unwrap(channel)
        .map_err(|_| anyhow::anyhow!("code-mode channel still shared after run"))?
        .into_inner()
        .unwrap_or_else(std::sync::PoisonError::into_inner);

    Ok(assemble_outcome(source, channel.host, call_result, cancelled, memory_refused))
}

/// Assembles the final [`CodeOutcome`] from the post-run host state + the `_start` call result.
///
/// Classifies a trap into the right [`LimitsHit`] (a fuel/epoch trap vs a cancel vs a real error)
/// and threads the output / records / citations / limit / guest error through. Cancellation takes
/// precedence so a user cancel reads as `Cancelled` even though it surfaced as an epoch trap. A
/// guest-reported JS error (the `error` op) becomes the honest `CodeOutcome.error` when the run was
/// otherwise clean.
fn assemble_outcome(
    source: &str,
    state: HostState,
    call_result: Result<()>,
    cancelled: bool,
    memory_refused: bool,
) -> CodeOutcome {
    let HostState { output, records, citations, limits_hit, guest_error, .. } = state;
    let model_text = String::from_utf8_lossy(&output).into_owned();

    let (limits_hit, error) = match call_result {
        // A clean run: surface any guest-reported JS error (syntax/runtime) as the honest error.
        Ok(()) => (limits_hit, guest_error),
        Err(trap) => {
            // A trap on a cancelled run is a cancel; a trap after the limiter refused a memory
            // growth is the memory bound (recorded at the source, not string-matched); otherwise it
            // is the wall-time/fuel bound (or a genuine guest fault). Prefer the most specific signal.
            if cancelled {
                (Some(LimitsHit::Cancelled), None)
            } else if memory_refused {
                (Some(LimitsHit::Memory), None)
            } else if let Some(limit) = limits_hit {
                (Some(limit), None)
            } else {
                // No host-recorded limit: classify the wasm trap precisely. A resource-limit trap
                // (epoch deadline / out-of-fuel / memory OOB) is a clean limit (error stays None); a
                // genuine guest fault (unreachable, table OOB, …) is an honest error.
                match classify_trap(&trap) {
                    Some(limit) => (Some(limit), None),
                    None => (None, Some(trap.to_string())),
                }
            }
        }
    };

    CodeOutcome {
        model_text,
        citations,
        host_calls: records,
        source: source.to_string(),
        error,
        limits_hit,
    }
}

/// Classifies a wasm trap into a hard limit, or `None` for a genuine guest fault.
///
/// Uses the precise wasmtime [`Trap`] variant (not a string match): an epoch-deadline
/// [`Trap::Interrupt`] or [`Trap::OutOfFuel`] is the wall-time/CPU bound ([`LimitsHit::Time`]); a
/// [`Trap::MemoryOutOfBounds`] is the memory bound ([`LimitsHit::Memory`]). Any other trap
/// (unreachable, table OOB, a non-Trap error) is a genuine fault → `None` so the outcome carries an
/// honest error instead of mislabeling a bug as a limit. (The deterministic memory-cap signal is the
/// limiter's `memory_refused`, checked first by the caller; this is the fallback classifier.)
fn classify_trap(trap: &anyhow::Error) -> Option<LimitsHit> {
    match trap.downcast_ref::<wasmtime::Trap>() {
        Some(wasmtime::Trap::Interrupt | wasmtime::Trap::OutOfFuel) => Some(LimitsHit::Time),
        Some(wasmtime::Trap::MemoryOutOfBounds) => Some(LimitsHit::Memory),
        _ => None,
    }
}

/// A background epoch ticker: advances the engine epoch every [`EPOCH_TICK`] (driving the store's
/// wall-time deadline) and, on cancel, jumps the epoch hard so the guest traps promptly.
///
/// Runs on its own OS thread (NOT a tokio worker) so it ticks even while the wasm `run.call` blocks
/// the calling thread. Dropped/stopped after the call returns. Records whether a cancel fired so the
/// outcome can read a cancel-induced trap as [`LimitsHit::Cancelled`] rather than `Time`.
struct EpochTicker {
    stop: Arc<AtomicBool>,
    cancelled: Arc<AtomicBool>,
    handle: Option<std::thread::JoinHandle<()>>,
}

impl EpochTicker {
    /// Starts the ticker thread bumping `engine`'s epoch until [`stop`](Self::stop) is called.
    fn start(engine: Engine, control: Option<Arc<dyn AiRunControl>>) -> Self {
        let stop = Arc::new(AtomicBool::new(false));
        let cancelled = Arc::new(AtomicBool::new(false));
        let stop_thread = stop.clone();
        let cancelled_thread = cancelled.clone();
        let handle = std::thread::spawn(move || {
            while !stop_thread.load(Ordering::Relaxed) {
                std::thread::sleep(EPOCH_TICK);
                if stop_thread.load(Ordering::Relaxed) {
                    break;
                }
                // A user cancel: jump the epoch well past the deadline so the guest traps now.
                if control.as_ref().is_some_and(|control| control.cancelled()) {
                    cancelled_thread.store(true, Ordering::Relaxed);
                    for _ in 0..deadline_ticks().saturating_add(1) {
                        engine.increment_epoch();
                    }
                    break;
                }
                engine.increment_epoch();
            }
        });
        Self { stop, cancelled, handle: Some(handle) }
    }

    /// Whether a cancel was observed (so the outcome can read a trap as `Cancelled`).
    fn cancelled(&self) -> bool {
        self.cancelled.load(Ordering::Relaxed)
    }

    /// Stops the ticker thread and joins it.
    fn stop(self) {}
}

impl Drop for EpochTicker {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Relaxed);
        if let Some(handle) = self.handle.take() {
            let _ = handle.join();
        }
    }
}

#[cfg(test)]
mod tests;
