//! Responsibilities: retain the parent module's behavior regressions on native storage.
//! Not responsible for: product implementation or external network execution.
//! Failure modes: incorrect boundaries, lost records, replay drift and invalid state transitions.
use super::*;
// The match-reason CODE vocabulary the search pipeline stamps (see `ai::search`); the fixtures
// below carry codes, not prose, so they stay honest about the shape the summarizer really sees.
use super::super::search::{MATCH_REASON_LEXICAL, MATCH_REASON_LEXICAL_DATE_ORDERED};
use crate::ai::tests::native::fixtures::{SeedVisit, seed_visits};
use crate::config::project_paths_with_root;
use crate::models::{AiProviderConfig, AiProviderPurpose, AiRequestFormat};
use secrecy::SecretString;

/// The UTC offset the tests render LOCAL times with — deterministic, host-clock-independent.
fn utc() -> chrono_tz::Tz {
    chrono_tz::UTC
}

fn embedding_runtime() -> AiProviderRuntime {
    AiProviderRuntime {
        config: AiProviderConfig {
            id: "embed-1".to_string(),
            name: "Embed".to_string(),
            purpose: AiProviderPurpose::Embedding,
            request_format: AiRequestFormat::OpenAi,
            enabled: true,
            default_model: "embed-model".to_string(),
            dimensions: Some(8),
            ..AiProviderConfig::default()
        },
        api_key: Some(SecretString::from("k".to_string())),
    }
}

fn context_with(embedding: Option<AiProviderRuntime>) -> (tempfile::TempDir, AgentToolContext) {
    let vault = crate::Vault::open_temp().unwrap();
    let dir = tempfile::tempdir().expect("tempdir");
    let paths = project_paths_with_root(dir.path());
    let mut config = AppConfig::default();
    config.ai.enabled = true;
    config.ai.assistant_enabled = true;
    let context = AgentToolContext {
        vault: vault.clone(),
        paths,
        config,
        database_key: None,
        embedding_provider: embedding,
        default_profile_id: None,
        default_domain: None,
        default_limit: 8,
        // The harness tests build a context with no run control; the search tools never read it
        // and `run_code` threads `None` into the sandbox (no cancel hook) as a safe default.
        run_control: None,
        // Tests pin UTC so the LOCAL-time rendering stays deterministic regardless of the host clock.
        time_zone: chrono_tz::UTC,
    };
    (dir, context)
}

/// Seeds `count` history visits all matching a shared lexical `token`, so a `search_history`
/// dispatch over them can prove the MODEL-FACING tool still caps its returned page at 50 rows even
/// when the caller asks for more (context protection), distinct from the sandbox's larger ceiling.
fn seed_matching_visits(context: &AgentToolContext, token: &str, count: i64) {
    let base_ms = 13_300_000_000_000_i64;
    let visits = (0..count)
        .map(|index| {
            SeedVisit::unix_ms(
                50_000 + index,
                "chrome:Default",
                &format!("https://site.example/{token}/page-{index}"),
                Some(&format!("{token} entry {index}")),
                base_ms + index * 60_000,
            )
        })
        .collect();
    seed_visits(&context.vault, visits);
}

#[tokio::test]
async fn search_history_tool_caps_returned_rows_at_fifty_even_when_more_requested() {
    // CONTEXT PROTECTION: the MODEL-FACING `search_history` tool returns rows that enter the
    // model's context, so it clamps its own `limit` to [`MAX_TOOL_ROWS`] (50) BEFORE retrieval —
    // even when the model asks for 200 over a >50-row set. This is the COUNTERPART to the sandbox's
    // raised ceiling: the two paths deliberately differ. One row → one citation in the outcome, so
    // the citation count is the returned-row count. Removing the `.min(MAX_TOOL_ROWS)` cap (so the
    // tool inherits the larger retrieval ceiling) returns >50 and fails this.
    let (_dir, context) = context_with(None);
    seed_matching_visits(&context, "needle", 120);
    let registry = ToolRegistry::with_default_search_tools();
    let outcome = registry
        .dispatch("search_history", json!({ "query": "needle", "limit": 200 }), &context)
        .await
        .expect("search_history dispatch");
    // The model-facing tool stays capped at exactly the cap (50), not the raised internal ceiling:
    // 120 rows match + limit:200 requested, so the page fills to exactly the cap. Reverting the
    // 50-cap lets the 1000 ceiling through and returns 120 here.
    assert_eq!(
        outcome.citations.len(),
        MAX_TOOL_ROWS as usize,
        "the 50-row cap is the binding limit over a 120-row match set even when 200 are requested"
    );
    // The summary reports the clamped applied_limit, so the model knows the page was bounded.
    assert!(
        outcome.model_text.contains("applied_limit: 50"),
        "applied_limit reflects the 50-row cap: {}",
        outcome.model_text
    );
}

#[test]
fn registry_advertises_all_default_tools_in_stable_order() {
    let registry = ToolRegistry::with_default_search_tools();
    assert!(!registry.is_empty());
    let names: Vec<String> = registry.definitions().into_iter().map(|def| def.name).collect();
    // BTreeMap keeps the order deterministic (alphabetical), so the prompt is stable. W-PKG-A
    // collapsed the three escalation planes into one `search_history`; W-AI-9 WPB adds the
    // intelligence, stars, and annotations tools.
    assert_eq!(
        names,
        vec!["intelligence_report", "list_annotations", "list_stars", "run_code", "search_history",]
    );
}

#[test]
fn empty_registry_reports_empty_and_has_no_definitions() {
    let registry = ToolRegistry::new();
    assert!(registry.is_empty());
    assert!(registry.definitions().is_empty());
}

#[tokio::test]
async fn dispatch_unknown_tool_is_a_recoverable_error() {
    let (_dir, context) = context_with(None);
    let registry = ToolRegistry::with_default_search_tools();
    let error = registry
        .dispatch("does_not_exist", json!({ "query": "x" }), &context)
        .await
        .expect_err("unknown tool");
    assert!(error.to_string().contains("unknown tool"));
}

#[tokio::test]
async fn search_history_works_with_no_embedding_provider() {
    // Hybrid degrades to lexical-only when no embedding provider is configured (honest degradation).
    let (_dir, context) = context_with(None);
    let registry = ToolRegistry::with_default_search_tools();
    let outcome = registry
        .dispatch("search_history", json!({ "query": "tauri" }), &context)
        .await
        .expect("search_history runs without an embedding provider (degrades to lexical)");
    // No rows in an empty fixture archive, but the call succeeds and summarizes honestly.
    assert!(outcome.model_text.contains("search_history"));
}

#[tokio::test]
async fn search_history_date_filter_args_are_deserialized() {
    // W-PKG-A: start_date and end_date must be accepted and threaded through to the request.
    let (_dir, context) = context_with(None);
    let registry = ToolRegistry::with_default_search_tools();
    let outcome = registry
        .dispatch(
            "search_history",
            json!({ "query": "test", "start_date": "2026-06-01", "end_date": "2026-06-30" }),
            &context,
        )
        .await
        .expect("date-filtered search_history must succeed");
    // Empty archive → no rows, but the call succeeds (no deserialization error, no panic).
    assert!(outcome.model_text.contains("search_history"));
}

#[tokio::test]
async fn search_history_sort_and_cursor_args_are_deserialized_and_threaded() {
    let (_dir, context) = context_with(None);
    seed_matching_visits(&context, "mlx", 6);
    let registry = ToolRegistry::with_default_search_tools();
    let first = registry
        .dispatch("search_history", json!({"query":"mlx", "sort":"oldest", "limit":5}), &context)
        .await
        .unwrap();
    let cursor = first
        .model_text
        .split_once("next_cursor: ")
        .map(|(_, rest)| rest.split(')').next().unwrap())
        .expect("continuation advertised");
    let next = registry
        .dispatch(
            "search_history",
            json!({"query":"mlx", "sort":"oldest", "cursor":cursor, "limit":5}),
            &context,
        )
        .await
        .unwrap();
    assert_eq!(first.citations.len(), 5);
    assert_eq!(next.citations.len(), 1);
    assert!(!first.citations.iter().any(|item| item.history_id == next.citations[0].history_id));
}

#[test]
fn search_history_schema_and_description_document_sort_and_pagination() {
    // The model can only USE sort/pagination if the schema advertises them and the description
    // teaches the "oldest = first occurrence" + cursor-paging contract (this is how the agent learns
    // to answer "when did I first browse X?" instead of sampling the recent month).
    let def = HistorySearchTool::search_history().definition();
    let props = &def.parameters["properties"];
    assert_eq!(props["sort"]["type"], "string");
    let sort_enum = props["sort"]["enum"].as_array().expect("sort enum");
    assert!(sort_enum.contains(&json!("oldest")));
    assert!(sort_enum.contains(&json!("newest")));
    assert!(sort_enum.contains(&json!("relevance")));
    assert_eq!(props["cursor"]["type"], "string");
    assert!(
        def.description.contains("oldest") && def.description.contains("first"),
        "the description teaches oldest=first occurrence: {}",
        def.description
    );
    assert!(
        def.description.contains("next_cursor") && def.description.contains("cursor"),
        "the description teaches cursor pagination: {}",
        def.description
    );
}

#[test]
fn search_summary_surfaces_next_cursor_when_present() {
    use crate::models::{AiSearchEntry, AiSearchResponse};
    let response = AiSearchResponse {
        search_state: None,
        total: 3,
        provider_id: "date-ordered".to_string(),
        model: "none".to_string(),
        items: vec![AiSearchEntry {
            history_id: 1,
            profile_id: "p".to_string(),
            url: "https://a.example/".to_string(),
            title: Some("A".to_string()),
            domain: "a.example".to_string(),
            visited_at: "2025-03-04T12:00:00Z".to_string(),
            score: 0.0,
            match_reason: MATCH_REASON_LEXICAL_DATE_ORDERED.to_string(),
            enrichment_excerpt: None,
        }],
        notes: Vec::new(),
        note_codes: Vec::new(),
        next_cursor: Some("1".to_string()),
        applied_limit: Some(1),
        has_more: true,
    };
    let text = summarize_search_for_model("search_history", &response, utc());
    assert!(text.contains("has_more: true"), "has_more surfaced: {text}");
    assert!(text.contains("next_cursor: 1"), "next_cursor surfaced so the model can page: {text}");

    // No cursor → no next_cursor segment.
    let last = AiSearchResponse { next_cursor: None, has_more: false, ..response };
    let last_text = summarize_search_for_model("search_history", &last, utc());
    assert!(!last_text.contains("next_cursor"), "no cursor segment on the last page: {last_text}");
}

#[tokio::test]
async fn search_history_with_embedding_provider_runs_hybrid() {
    // With a provider configured the search tool runs the full hybrid pipeline; the empty fixture
    // archive yields no rows, but the call succeeds (no panic, no error).
    let (_dir, context) = context_with(Some(embedding_runtime()));
    let registry = ToolRegistry::with_default_search_tools();
    let outcome = registry
        .dispatch("search_history", json!({ "query": "rust", "limit": 5 }), &context)
        .await
        .expect("search_history runs hybrid with a provider");
    assert!(outcome.model_text.contains("search_history"));
}

#[tokio::test]
async fn empty_or_omitted_query_returns_recent_visits() {
    // Browse-by-recency: a blank query (or an omitted one) is no longer rejected — it returns
    // the most recent visits so the model can enumerate history. The empty fixture archive holds
    // no visits, so the call succeeds with zero rows (no error, no panic).
    let (_dir, context) = context_with(None);
    let registry = ToolRegistry::with_default_search_tools();
    for args in [json!({ "query": "   " }), json!({})] {
        let outcome = registry
            .dispatch("search_history", args.clone(), &context)
            .await
            .unwrap_or_else(|error| panic!("empty query {args} must succeed, got {error}"));
        assert!(outcome.model_text.contains("search_history"));
        assert!(outcome.citations.is_empty());
    }
}

#[tokio::test]
async fn invalid_arguments_are_rejected_without_panicking() {
    let (_dir, context) = context_with(None);
    let registry = ToolRegistry::with_default_search_tools();
    // `query` is required and must be a string; a number is a parse error, not a panic.
    let error = registry
        .dispatch("search_history", json!({ "query": 42 }), &context)
        .await
        .expect_err("invalid args");
    assert!(error.to_string().contains("invalid search arguments"));
}

#[test]
fn search_plane_provider_selection_drops_provider_for_bm25() {
    let (_dir, context) = context_with(Some(embedding_runtime()));
    // Bm25 never threads the provider; the semantic planes do.
    assert!(SearchPlane::Bm25.provider_for(&context).is_none());
    assert!(SearchPlane::Vector.provider_for(&context).is_some());
    assert!(SearchPlane::Hybrid.provider_for(&context).is_some());
}

#[test]
fn summarize_search_lists_rows_and_notes() {
    use crate::models::{AiSearchEntry, AiSearchResponse};
    let response = AiSearchResponse {
        search_state: None,
        total: 1,
        provider_id: "p1".to_string(),
        model: "m1".to_string(),
        items: vec![AiSearchEntry {
            history_id: 7,
            profile_id: "default".to_string(),
            url: "https://example.com/".to_string(),
            title: Some("Example".to_string()),
            domain: "example.com".to_string(),
            visited_at: "2026-06-21T00:00:00Z".to_string(),
            score: 0.42,
            match_reason: MATCH_REASON_LEXICAL.to_string(),
            enrichment_excerpt: None,
        }],
        notes: vec!["lexical only".to_string()],
        note_codes: Vec::new(),
        next_cursor: None,
        applied_limit: Some(50),
        has_more: false,
    };
    let text = summarize_search_for_model("search_hybrid", &response, utc());
    assert!(text.contains("[7]"));
    assert!(text.contains("https://example.com/"));
    assert!(text.contains("Note: lexical only"));

    // The empty case still names the tool and surfaces notes.
    let empty = AiSearchResponse {
        search_state: None,
        total: 0,
        provider_id: "p1".to_string(),
        model: "m1".to_string(),
        items: Vec::new(),
        notes: vec!["nothing".to_string()],
        note_codes: Vec::new(),
        next_cursor: None,
        applied_limit: None,
        has_more: false,
    };
    let empty_text = summarize_search_for_model("search_bm25", &empty, utc());
    assert!(empty_text.contains("no matching history rows"));
    assert!(empty_text.contains("Note: nothing"));
}

#[test]
fn search_summary_includes_applied_limit_and_has_more() {
    use crate::models::{AiSearchEntry, AiSearchResponse};
    let response = AiSearchResponse {
        search_state: None,
        total: 2,
        provider_id: "hybrid".to_string(),
        model: "m1".to_string(),
        items: vec![AiSearchEntry {
            history_id: 1,
            profile_id: "p".to_string(),
            url: "https://a.example/".to_string(),
            title: Some("A".to_string()),
            domain: "a.example".to_string(),
            visited_at: "2026-06-21T00:00:00Z".to_string(),
            score: 0.9,
            match_reason: MATCH_REASON_LEXICAL.to_string(),
            enrichment_excerpt: None,
        }],
        notes: Vec::new(),
        note_codes: Vec::new(),
        next_cursor: None,
        applied_limit: Some(50),
        has_more: true,
    };
    let text = summarize_search_for_model("search_history", &response, utc());
    assert!(text.contains("applied_limit: 50"), "applied_limit in summary: {text}");
    assert!(text.contains("has_more: true"), "has_more in summary: {text}");

    // When has_more is false it should not appear.
    let response_no_more =
        AiSearchResponse { applied_limit: Some(8), has_more: false, ..response.clone() };
    let text2 = summarize_search_for_model("search_history", &response_no_more, utc());
    assert!(text2.contains("applied_limit: 8"), "applied_limit in summary: {text2}");
    assert!(!text2.contains("has_more"), "has_more absent when false: {text2}");
}

#[test]
fn run_code_description_includes_start_end_date_and_examples() {
    let def = RunCodeTool::new().definition();
    assert!(def.description.contains("startDate"), "run_code description includes startDate");
    assert!(def.description.contains("endDate"), "run_code description includes endDate");
    assert!(
        def.description.contains("2026-06-19"),
        "run_code description includes date-range example"
    );
    assert!(
        def.description.contains("machine learning"),
        "run_code description includes no-answer example"
    );
    assert!(
        def.description.contains("{ found: false }"),
        "run_code description includes explicit no-answer shape"
    );
    // Defect A — the description teaches the LOCAL row fields and the byDay example groups by
    // `r.localDate` (not the old UTC `r.visitedAt.slice(0, 10)`).
    assert!(
        def.description.contains("visitedAtLocal") && def.description.contains("localDate"),
        "run_code description documents the LOCAL row fields"
    );
    assert!(def.description.contains("r.localDate"), "the byDay example groups by the LOCAL date");
    assert!(
        !def.description.contains("r.visitedAt.slice(0, 10)"),
        "the old UTC-slice day bucket is gone"
    );
}

// ---- W-AI-8 WU-2: the `run_code` tool ----------------------------------------------------

use super::super::code_mode::CodeOutcome;

#[test]
fn run_code_is_registered_by_default() {
    // Default-enabled (2026-06 decision): every agent run advertises `run_code` alongside search.
    let registry = ToolRegistry::with_default_search_tools();
    let names: Vec<String> = registry.definitions().into_iter().map(|def| def.name).collect();
    assert!(names.contains(&"run_code".to_string()), "run_code is a default tool: {names:?}");
}

#[test]
fn run_code_default_matches_new() {
    // `RunCodeTool` is a unit tool; `Default` and `new` are interchangeable (both name `run_code`).
    // The `Default` impl is exercised via the fully-qualified call so the `default_constructed_
    // unit_structs` lint stays clean (`RunCodeTool::default()` on a unit struct trips it).
    let default_tool = <RunCodeTool as Default>::default();
    assert_eq!(default_tool.name(), RunCodeTool::new().name());
    assert_eq!(default_tool.name(), "run_code");
}

#[test]
fn run_code_definition_teaches_the_host_api_and_requires_source() {
    // The definition advertised to the model must be honest about the sandbox (read-only, no
    // net/fs/clock/random) and name the two globals; the schema requires exactly `source`.
    let def = RunCodeTool::new().definition();
    assert_eq!(def.name, "run_code");
    assert!(def.description.contains("query_history"));
    assert!(def.description.contains("fetch_visits"));
    assert!(def.description.contains("READ-ONLY"));
    assert!(def.description.contains("NO network"));
    // F2: the description teaches the `notes` degradation signal and tells the model to honor it.
    assert!(def.description.contains("notes"), "the notes degradation signal is documented");
    assert!(
        def.description.contains("fell back to keyword"),
        "the description gives the concrete semantic→lexical degradation example"
    );
    // F1: the description steers the model to ALWAYS return a value (even a negative one), and no
    // longer tells it to return nothing.
    assert!(
        def.description.contains("Always end the program with `return"),
        "the description steers the model to always return a value"
    );
    assert!(
        def.description.contains("{ found: false }"),
        "the description shows a negative return shape for the no-answer case"
    );
    assert!(
        !def.description.to_lowercase().contains("return nothing if there is no answer"),
        "the misleading 'return nothing' guidance is dropped"
    );
    assert_eq!(def.parameters["required"], json!(["source"]));
    assert_eq!(def.parameters["properties"]["source"]["type"], "string");
}

#[test]
fn maps_a_clean_code_outcome_to_a_populated_tool_outcome() {
    // A clean CodeOutcome (distilled text + one host call + a citation, no error/limit) maps onto
    // a ToolOutcome carrying the code-mode transparency fields verbatim.
    let outcome = CodeOutcome {
        model_text: "{\"count\":1}".to_string(),
        citations: vec![AiCitation {
            history_id: 101,
            profile_id: "p".to_string(),
            url: "https://rust-lang.org/".to_string(),
            title: Some("Rust".to_string()),
            visited_at: "2026-01-01T00:00:00Z".to_string(),
            score: Some(0.9),
            canonical_url: Some("https://rust-lang.org/".to_string()),
        }],
        host_calls: vec![HostCallRecord {
            function: "query_history".to_string(),
            query: Some("rust".to_string()),
            plane: Some("hybrid".to_string()),
            limit: Some(8),
            requested_ids: None,
            args_summary: "query=\"rust\" plane=hybrid limit=8".to_string(),
            row_count: 1,
        }],
        source: "return query_history({query:'rust'});".to_string(),
        error: None,
        limits_hit: None,
        coverage_incomplete: false,
    };
    let tool = code_outcome_to_tool_outcome(outcome).expect("clean outcome maps to Ok");
    assert_eq!(tool.model_text, "{\"count\":1}");
    assert_eq!(tool.code_source.as_deref(), Some("return query_history({query:'rust'});"));
    assert_eq!(tool.host_calls.len(), 1);
    assert_eq!(tool.host_calls[0].function, "query_history");
    assert_eq!(tool.host_calls[0].query.as_deref(), Some("rust"));
    assert_eq!(tool.host_calls[0].plane.as_deref(), Some("hybrid"));
    assert_eq!(tool.host_calls[0].limit, Some(8));
    assert_eq!(tool.citations.len(), 1);
    assert_eq!(tool.limits_hit, None);
}

#[test]
fn maps_a_hard_failure_with_no_output_to_a_recoverable_error() {
    // A thrown-JS-error CodeOutcome (error set, empty output) becomes an Err so the harness
    // threads a recoverable is_error tool result (same discipline as the search tools).
    let outcome = CodeOutcome {
        model_text: String::new(),
        source: "return notDefined.boom;".to_string(),
        error: Some("ReferenceError: notDefined is not defined".to_string()),
        ..CodeOutcome::default()
    };
    let error = code_outcome_to_tool_outcome(outcome).expect_err("a hard failure is an Err");
    assert!(error.to_string().contains("run_code failed"));
    assert!(error.to_string().contains("notDefined"));
}

#[test]
fn an_empty_no_error_outcome_is_normalized_to_an_honest_sentinel() {
    // F1: a script that returned nothing AND did not throw is NOT a hard error, but an empty
    // result is silently ambiguous to the model. It is normalized to an honest sentinel (NOT a
    // fabricated answer) and stays a non-error Ok the harness threads back.
    let outcome = CodeOutcome {
        model_text: String::new(),
        source: "let x = 1;".to_string(),
        error: None,
        ..CodeOutcome::default()
    };
    let tool = code_outcome_to_tool_outcome(outcome).expect("an empty no-error run is Ok");
    assert_eq!(tool.model_text, "[run_code completed but returned no value]");

    // Whitespace-only output is treated the same (an honest "no value", not a blank string).
    let whitespace = CodeOutcome {
        model_text: "  \n\t ".to_string(),
        source: "return '   ';".to_string(),
        error: None,
        ..CodeOutcome::default()
    };
    let tool = code_outcome_to_tool_outcome(whitespace).expect("whitespace output is Ok");
    assert_eq!(tool.model_text, "[run_code completed but returned no value]");
}

#[test]
fn an_empty_bounded_outcome_keeps_the_sentinel_and_appends_the_limit_note() {
    // F1 + the limit note compose: a bounded run that ALSO returned nothing reports the honest
    // "no value" sentinel AND the bounded-run note (so the model is never handed a bare string).
    let outcome = CodeOutcome {
        model_text: String::new(),
        source: "while (true) {}".to_string(),
        error: None,
        limits_hit: Some(LimitsHit::Time),
        ..CodeOutcome::default()
    };
    let tool = code_outcome_to_tool_outcome(outcome).expect("a bounded empty run is Ok");
    assert!(tool.model_text.starts_with("[run_code completed but returned no value]"));
    assert!(tool.model_text.contains("bounded by a hard limit"));
    assert!(tool.model_text.contains("wall-time limit"));
}

#[test]
fn an_error_with_partial_output_is_still_returned_not_failed() {
    // If the script produced SOME distilled output, a trailing error does not discard it — the
    // model gets the partial result (Ok), not a recoverable error (only an empty+error is Err).
    let outcome = CodeOutcome {
        model_text: "{\"partial\":true}".to_string(),
        source: "...".to_string(),
        error: Some("late error".to_string()),
        ..CodeOutcome::default()
    };
    let tool = code_outcome_to_tool_outcome(outcome).expect("partial output is kept (Ok)");
    assert_eq!(tool.model_text, "{\"partial\":true}");
}

#[test]
fn a_limited_outcome_appends_an_honest_note_and_is_ok() {
    // A bounded run (output cap hit) returns Ok, carries the limit, and appends an honest note so
    // the model does not treat the truncated result as complete (never fabricated).
    let outcome = CodeOutcome {
        model_text: "truncated...".to_string(),
        source: "return big;".to_string(),
        limits_hit: Some(LimitsHit::Output),
        ..CodeOutcome::default()
    };
    let tool = code_outcome_to_tool_outcome(outcome).expect("a limited run is Ok");
    assert_eq!(tool.limits_hit, Some(LimitsHit::Output));
    assert!(tool.model_text.contains("truncated..."));
    assert!(
        tool.model_text.contains("bounded by a hard limit")
            && tool.model_text.contains("output size cap"),
        "an honest bounded-run note is appended: {}",
        tool.model_text
    );
}

#[test]
fn limits_hit_label_covers_every_variant() {
    // The model-facing label maps every hard-limit variant (keeps the note honest + non-empty).
    assert_eq!(limits_hit_label(LimitsHit::Time), "wall-time limit");
    assert_eq!(limits_hit_label(LimitsHit::Memory), "memory limit");
    assert_eq!(limits_hit_label(LimitsHit::HostCalls), "host-call budget");
    assert_eq!(limits_hit_label(LimitsHit::Output), "output size cap");
    assert_eq!(limits_hit_label(LimitsHit::Cancelled), "cancelled");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn run_code_dispatch_runs_real_js_and_returns_a_distilled_result() {
    // End-to-end through the tool path (the async↔sync `spawn_blocking` bridge): real JS runs in
    // the sandbox and returns a distilled value, carrying the script source on the outcome.
    let (_dir, context) = context_with(None);
    let registry = ToolRegistry::with_default_search_tools();
    let outcome = registry
        .dispatch(
            "run_code",
            json!({ "source": "return { sum: [1,2,3].reduce((a,b)=>a+b,0) };" }),
            &context,
        )
        .await
        .expect("run_code runs through the tool path");
    let distilled: Value = serde_json::from_str(&outcome.model_text).expect("valid JSON output");
    assert_eq!(distilled["sum"], 6);
    assert!(outcome.code_source.is_some(), "the script source is carried for transparency");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn run_code_dispatch_threads_a_thrown_js_error_as_recoverable() {
    // A thrown JS error in the sandbox becomes an Err the harness threads as a recoverable
    // is_error tool result — never a panic, never a hang.
    let (_dir, context) = context_with(None);
    let registry = ToolRegistry::with_default_search_tools();
    let error = registry
        .dispatch("run_code", json!({ "source": "return notDefinedAnywhere.boom;" }), &context)
        .await
        .expect_err("a thrown JS error is a recoverable tool error");
    assert!(error.to_string().contains("run_code failed"));
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn run_code_rejects_empty_or_invalid_source_without_panicking() {
    let (_dir, context) = context_with(None);
    let registry = ToolRegistry::with_default_search_tools();
    // An empty/whitespace source is rejected before any sandbox run.
    let empty = registry
        .dispatch("run_code", json!({ "source": "   " }), &context)
        .await
        .expect_err("empty source rejected");
    assert!(empty.to_string().contains("must not be empty"));
    // A non-string `source` is a parse error, not a panic.
    let invalid = registry
        .dispatch("run_code", json!({ "source": 42 }), &context)
        .await
        .expect_err("invalid source rejected");
    assert!(invalid.to_string().contains("invalid run_code arguments"));
    // A missing `source` is likewise rejected (the field is required).
    let missing = registry
        .dispatch("run_code", json!({}), &context)
        .await
        .expect_err("missing source rejected");
    assert!(missing.to_string().contains("invalid run_code arguments"));
}

// ---- W-AI-9 WPB: intelligence_report, list_stars, list_annotations ---------------------

#[test]
fn intelligence_report_definition_names_report_enum_and_key_phrases() {
    let tool = IntelligenceReportTool;
    let def = tool.definition();
    assert_eq!(def.name, "intelligence_report");
    assert!(def.description.contains("pre-computed intelligence report"));
    assert!(def.description.contains("patterns"));
    assert!(def.description.contains("trends"));
    // The schema requires `report` and lists the enum.
    assert_eq!(def.parameters["required"], json!(["report"]));
    let report_prop = &def.parameters["properties"]["report"];
    assert_eq!(report_prop["type"], "string");
    let enum_values = report_prop["enum"].as_array().expect("enum array");
    assert!(enum_values.len() >= 8, "all 8 report variants: {enum_values:?}");
    assert!(enum_values.contains(&json!("top_sites")));
    assert!(enum_values.contains(&json!("overview")));
}

#[tokio::test]
async fn intelligence_report_top_sites_on_empty_archive_returns_not_ready() {
    // Against a fresh tempdir with no intelligence data, the tool returns the honest
    // "not ready" outcome rather than panicking.
    let (_dir, context) = context_with(None);
    let tool = IntelligenceReportTool;
    let outcome = tool
        .call(json!({ "report": "top_sites" }), &context)
        .await
        .expect("should not panic on empty archive");
    assert!(outcome.model_text.contains("intelligence index has not been built"));
}

#[tokio::test]
async fn intelligence_report_overview_on_empty_archive_returns_not_ready() {
    let (_dir, context) = context_with(None);
    let tool = IntelligenceReportTool;
    let outcome = tool
        .call(json!({ "report": "overview" }), &context)
        .await
        .expect("should not panic on empty archive");
    assert!(outcome.model_text.contains("intelligence index has not been built"));
}

#[tokio::test]
async fn intelligence_report_sessions_on_empty_archive_returns_not_ready() {
    let (_dir, context) = context_with(None);
    let tool = IntelligenceReportTool;
    let outcome = tool
        .call(json!({ "report": "sessions" }), &context)
        .await
        .expect("should not panic on empty archive");
    assert!(outcome.model_text.contains("intelligence index has not been built"));
}

#[tokio::test]
async fn intelligence_report_unknown_variant_is_an_error() {
    let (_dir, context) = context_with(None);
    let tool = IntelligenceReportTool;
    let error = tool
        .call(json!({ "report": "unicorn" }), &context)
        .await
        .expect_err("unknown variant should error");
    assert!(error.to_string().contains("unknown report variant"));
}

#[tokio::test]
async fn intelligence_report_empty_report_is_rejected() {
    let (_dir, context) = context_with(None);
    let tool = IntelligenceReportTool;
    let error = tool
        .call(json!({ "report": "" }), &context)
        .await
        .expect_err("empty report should be rejected");
    assert!(error.to_string().contains("must not be empty"));
}

#[tokio::test]
async fn intelligence_report_invalid_args_are_rejected() {
    let (_dir, context) = context_with(None);
    let tool = IntelligenceReportTool;
    let error = tool
        .call(json!({ "report": 42 }), &context)
        .await
        .expect_err("numeric report should fail parse");
    assert!(error.to_string().contains("invalid intelligence_report arguments"));
}

#[tokio::test]
async fn intelligence_report_domain_trend_requires_domain() {
    // L2 — the `domain` required-arg check must run BEFORE the readiness gate, so an omitted
    // domain is an actionable arg ERROR (not the generic "not built" note) even on an empty plane.
    let (_dir, context) = context_with(None);
    let tool = IntelligenceReportTool;
    let error = tool
        .call(json!({ "report": "domain_trend" }), &context)
        .await
        .expect_err("domain_trend without `domain` must bail with a required-arg error");
    let message = error.to_string();
    assert!(message.contains("domain"), "got: {message}");
    assert!(
        !message.contains("not been built"),
        "the arg error must pre-empt the readiness note: {message}"
    );
}

#[tokio::test]
async fn intelligence_report_day_insights_requires_start_date() {
    // L2 — the `start_date` required-arg check must run BEFORE the readiness gate (same as above).
    let (_dir, context) = context_with(None);
    let tool = IntelligenceReportTool;
    let error = tool
        .call(json!({ "report": "day_insights" }), &context)
        .await
        .expect_err("day_insights without `start_date` must bail with a required-arg error");
    let message = error.to_string();
    assert!(message.contains("start_date"), "got: {message}");
    assert!(
        !message.contains("not been built"),
        "the arg error must pre-empt the readiness note: {message}"
    );
}

// ---- list_stars tool tests ----

#[test]
fn list_stars_definition_has_correct_name_and_key_phrases() {
    let tool = ListStarsTool;
    let def = tool.definition();
    assert_eq!(def.name, "list_stars");
    assert!(def.description.contains("starred"));
    assert!(def.description.contains("favorited"));
    assert!(def.description.contains("bookmarks"));
    let sort_prop = &def.parameters["properties"]["sort"];
    assert_eq!(sort_prop["type"], "string");
    let enum_values = sort_prop["enum"].as_array().expect("enum array");
    assert!(enum_values.contains(&json!("recent")));
    assert!(enum_values.contains(&json!("most_revisited")));
}

#[tokio::test]
async fn list_stars_on_empty_archive_returns_no_stars() {
    let (_dir, context) = context_with(None);
    let tool = ListStarsTool;
    let outcome = tool.call(json!({}), &context).await.expect("should not panic on empty archive");
    assert!(outcome.model_text.contains("no starred pages found"));
}

#[tokio::test]
async fn list_stars_with_recent_sort_on_empty_archive() {
    let (_dir, context) = context_with(None);
    let tool = ListStarsTool;
    let outcome = tool.call(json!({ "sort": "recent" }), &context).await.expect("should succeed");
    assert!(outcome.model_text.contains("no starred pages found"));
}

#[tokio::test]
async fn list_stars_with_most_revisited_sort_on_empty_archive() {
    let (_dir, context) = context_with(None);
    let tool = ListStarsTool;
    let outcome =
        tool.call(json!({ "sort": "most_revisited" }), &context).await.expect("should succeed");
    assert!(outcome.model_text.contains("no starred pages found"));
}

#[tokio::test]
async fn list_stars_invalid_args_are_rejected() {
    let (_dir, context) = context_with(None);
    let tool = ListStarsTool;
    let error = tool
        .call(json!({ "sort": 42 }), &context)
        .await
        .expect_err("numeric sort should fail parse");
    assert!(error.to_string().contains("invalid list_stars arguments"));
}

// ---- list_annotations tool tests ----

#[test]
fn list_annotations_definition_has_correct_name_and_key_phrases() {
    let tool = ListAnnotationsTool;
    let def = tool.definition();
    assert_eq!(def.name, "list_annotations");
    assert!(def.description.contains("annotations"));
    assert!(def.description.contains("notes"));
    assert!(def.description.contains("tags"));
    let query_prop = &def.parameters["properties"]["query"];
    assert_eq!(query_prop["type"], "string");
}

#[tokio::test]
async fn list_annotations_on_empty_archive_returns_no_annotations() {
    let (_dir, context) = context_with(None);
    let tool = ListAnnotationsTool;
    let outcome = tool.call(json!({}), &context).await.expect("should not panic on empty archive");
    assert!(outcome.model_text.contains("no annotations found"));
}

#[tokio::test]
async fn list_annotations_with_query_on_empty_archive() {
    let (_dir, context) = context_with(None);
    let tool = ListAnnotationsTool;
    let outcome = tool.call(json!({ "query": "rust" }), &context).await.expect("should succeed");
    assert!(outcome.model_text.contains("no annotations found"));
    assert!(outcome.model_text.contains("rust"));
}

#[tokio::test]
async fn list_annotations_without_query_on_empty_archive() {
    let (_dir, context) = context_with(None);
    let tool = ListAnnotationsTool;
    let outcome = tool.call(json!({ "limit": 10 }), &context).await.expect("should succeed");
    assert!(outcome.model_text.contains("no annotations found"));
}

#[tokio::test]
async fn list_annotations_invalid_args_are_rejected() {
    let (_dir, context) = context_with(None);
    let tool = ListAnnotationsTool;
    let error = tool
        .call(json!({ "query": 42 }), &context)
        .await
        .expect_err("numeric query should fail parse");
    assert!(error.to_string().contains("invalid list_annotations arguments"));
}

// ---- W-AI-9 WPB registry integration ----

#[test]
fn new_tools_are_registered_by_default() {
    let registry = ToolRegistry::with_default_search_tools();
    let names: Vec<String> = registry.definitions().into_iter().map(|def| def.name).collect();
    assert!(
        names.contains(&"intelligence_report".to_string()),
        "intelligence_report is a default tool: {names:?}"
    );
    assert!(names.contains(&"list_stars".to_string()), "list_stars is a default tool: {names:?}");
    assert!(
        names.contains(&"list_annotations".to_string()),
        "list_annotations is a default tool: {names:?}"
    );
}

#[tokio::test]
async fn dispatch_intelligence_report_via_registry() {
    let (_dir, context) = context_with(None);
    let registry = ToolRegistry::with_default_search_tools();
    let outcome = registry
        .dispatch("intelligence_report", json!({ "report": "top_sites" }), &context)
        .await
        .expect("dispatch should succeed");
    // On an empty archive, intelligence is not ready.
    assert!(outcome.model_text.contains("intelligence"));
}

#[tokio::test]
async fn dispatch_list_stars_via_registry() {
    let (_dir, context) = context_with(None);
    let registry = ToolRegistry::with_default_search_tools();
    let outcome = registry
        .dispatch("list_stars", json!({}), &context)
        .await
        .expect("dispatch should succeed");
    assert!(outcome.model_text.contains("list_stars"));
}

#[tokio::test]
async fn dispatch_list_annotations_via_registry() {
    let (_dir, context) = context_with(None);
    let registry = ToolRegistry::with_default_search_tools();
    let outcome = registry
        .dispatch("list_annotations", json!({}), &context)
        .await
        .expect("dispatch should succeed");
    assert!(outcome.model_text.contains("list_annotations"));
}

// ============================================================================================
// LETHAL tests: char-boundary truncation (H2), intelligence_report success arms (B1/M3/L1/L2),
// and list_stars / list_annotations over SEEDED data (H2 note truncation). Each asserts a real
// result that a regression of the fix would break.
// ============================================================================================

#[test]
fn output_pages_keep_multibyte_characters_and_reject_partial_character_cursors() {
    let raw = "你好🦀".repeat(2000);
    let first: Value =
        serde_json::from_str(&super::super::report_pages::output(&raw, None).unwrap()).unwrap();
    let fragment = first["jsonFragment"].as_str().unwrap();
    assert!(raw.starts_with(fragment));
    assert!(fragment.len() < raw.len());
    let bad = serde_json::to_string(&(crate::utils::sha256_hex(raw.as_bytes()), 1_usize)).unwrap();
    assert!(super::super::report_pages::output(&raw, Some(&bad)).is_err());
}

/// Initializes the archive + Core Intelligence schema and seeds enough rows to make
/// `intelligence_status().ready == true` and the read models return the seeded data.
///
/// Replicates the relevant INSERTs from `intelligence::tests::fixtures::seed_core_intelligence_fixture`
/// (`pub(super)` there, so not nameable here): one `sessions` row (so `ready` is true and `sessions`
/// returns data) and one `domain_daily_rollups` row for `example.com` (so `top_sites` / `domain_trend`
/// return the seeded domain). Dates are in 2024 so they fall inside any all-time window.
fn seed_ready_intelligence(context: &AgentToolContext) {
    use vault_store::{Db, Priority};
    context.vault.write(Db::Archive,Priority::Batch,|s| {
        s.execute("INSERT INTO source_profiles(id,browser_kind,profile_name,profile_path,profile_key,discovered_at) VALUES(1,'chrome','Default','/fixture','chrome:Default','')",())?;Ok(())
    }).unwrap();
    context.vault.write(Db::Intelligence,Priority::Batch,|s| {
        s.execute("INSERT INTO intelligence_generations(profile_id) VALUES('chrome:Default')",())?;
        s.execute("INSERT INTO sessions_rows(session_id,profile_id,first_visit_ms,last_visit_ms,visit_count,search_count,domain_count,is_deep_dive,auto_title,computed_at) VALUES('s-1','chrome:Default',1711929600000,1711929660000,3,1,2,1,'Researching SQLite','')",())?;
        s.execute("INSERT INTO domain_daily_rollups_rows(date_key,profile_id,registrable_domain,domain_category,visit_count,search_count,new_domain_visits,unique_urls) VALUES('2024-04-01','chrome:Default','example.com','reference',9,0,1,4)",())?;
        s.execute("INSERT INTO intelligence_counts(generation,profile_id,table_name,value) VALUES(0,'chrome:Default','sessions',1)",())?;Ok(())
    }).unwrap();
}
fn seed_session_visit(context: &AgentToolContext, session_id: &str, visit_id: i64, visit_ms: i64) {
    use vault_store::{Db, Priority, params_owned as params};
    context.vault.write(Db::Archive,Priority::Batch,move |s| {
        let page_id=s.query_one("INSERT INTO pages(url,canonical_url,registrable_domain) VALUES('https://example.com/page','https://example.com/page','example.com') ON CONFLICT(url) DO UPDATE SET url=excluded.url RETURNING id",(),|r|Ok(r.get::<i64>(0)?))?;
        s.execute("INSERT INTO urls(id,page_id,url,title,source_profile_id,created_by_run_id,first_visit_ms,first_visit_iso,last_visit_ms,last_visit_iso) VALUES(?1,?2,'https://example.com/page','Example',1,0,?3,'',?3,'')",params![visit_id,page_id,visit_ms])?;
        s.execute("INSERT INTO visits(id,url_id,source_visit_id,source_profile_id,created_by_run_id,visit_time_ms,visit_time_iso,transition_type) VALUES(?1,?1,?2,1,0,?3,'',805306368)",params![visit_id,visit_id.to_string(),visit_ms])?;Ok(())
    }).unwrap();
    let session_id = session_id.to_owned();
    context.vault.write(Db::Intelligence,Priority::Batch,move |s| {
        s.execute("INSERT INTO visit_derived_facts_rows(visit_id,profile_id,registrable_domain,canonical_url,domain_category,page_category,evidence_tier,taxonomy_source,computed_at) VALUES(?1,'chrome:Default','example.com','https://example.com/page','reference','docs_page','tier-a','test','')",[visit_id])?;
        s.execute("INSERT INTO visit_assignments(visit_id,profile_id,session_id) VALUES(?1,'chrome:Default',?2)",params![visit_id,session_id])?;Ok(())
    }).unwrap();
}

#[tokio::test]
async fn intelligence_report_top_sites_returns_seeded_domain_all_time() {
    // Test 3 — top_sites over a READY plane returns the seeded domain. Omitted dates exercise the
    // B1 all-time path (the rollup date_key 2024-04-01 must fall inside the substituted
    // [1900-01-01, today] window; before B1 the `""` start made `date_key <= ""` match nothing).
    let (_dir, context) = context_with(None);
    seed_ready_intelligence(&context);
    let tool = IntelligenceReportTool;
    let outcome = tool
        .call(json!({ "report": "top_sites" }), &context)
        .await
        .expect("top_sites over a ready plane returns data");
    assert!(
        !outcome.model_text.contains("not been built"),
        "a ready plane must NOT return the not-ready note: {}",
        outcome.model_text
    );
    assert!(
        outcome.model_text.contains("example.com"),
        "the seeded domain must appear in the all-time top_sites: {}",
        outcome.model_text
    );
}

#[tokio::test]
async fn intelligence_report_sessions_returns_seeded_session_all_time() {
    // Test 3 — sessions over a READY plane returns the seeded session, all-time (omitted dates).
    let (_dir, context) = context_with(None);
    seed_ready_intelligence(&context);
    let tool = IntelligenceReportTool;
    let outcome = tool
        .call(json!({ "report": "sessions" }), &context)
        .await
        .expect("sessions over a ready plane returns data");
    assert!(!outcome.model_text.contains("not been built"), "got: {}", outcome.model_text);
    assert!(
        outcome.model_text.contains("Researching SQLite"),
        "the seeded session auto_title must appear: {}",
        outcome.model_text
    );
}

#[tokio::test]
async fn intelligence_report_overview_returns_data_all_time() {
    // Test 3 — overview over a READY plane returns its all-time snapshot WITHOUT error (B1: the
    // all-time path needs start == "1900-01-01"; the omitted dates substitute exactly that, and
    // `is_all_time_range` recognizes it to load/build the snapshot rather than erroring).
    let (_dir, context) = context_with(None);
    seed_ready_intelligence(&context);
    let tool = IntelligenceReportTool;
    let outcome = tool
        .call(json!({ "report": "overview" }), &context)
        .await
        .expect("overview over a ready plane (all-time) returns data, not an error");
    assert!(outcome.model_text.contains("intelligence_report(overview)"));
    assert!(!outcome.model_text.contains("not been built"), "got: {}", outcome.model_text);
}

#[tokio::test]
async fn intelligence_report_day_insights_returns_data_for_a_seeded_day() {
    // Test 3 — day_insights for the seeded day returns data (the rollup is on 2024-04-01). The RAW
    // start_date is the target day (not the all-time sentinel substitution).
    let (_dir, context) = context_with(None);
    seed_ready_intelligence(&context);
    let tool = IntelligenceReportTool;
    let outcome = tool
        .call(json!({ "report": "day_insights", "start_date": "2024-04-01" }), &context)
        .await
        .expect("day_insights for a seeded day returns data");
    assert!(outcome.model_text.contains("intelligence_report(day_insights)"));
    assert!(!outcome.model_text.contains("not been built"), "got: {}", outcome.model_text);
    // The day's top_sites (built from the rollup) name the seeded domain.
    assert!(
        outcome.model_text.contains("example.com"),
        "the seeded day must surface the domain: {}",
        outcome.model_text
    );
}

#[tokio::test]
async fn intelligence_report_domain_trend_returns_seeded_points() {
    // Test 3 — domain_trend (which reads domain_daily_rollups) returns the seeded point, all-time.
    let (_dir, context) = context_with(None);
    seed_ready_intelligence(&context);
    let tool = IntelligenceReportTool;
    let outcome = tool
        .call(json!({ "report": "domain_trend", "domain": "example.com" }), &context)
        .await
        .expect("domain_trend over a ready plane returns data");
    assert!(!outcome.model_text.contains("not been built"), "got: {}", outcome.model_text);
    assert!(
        outcome.model_text.contains("2024-04-01"),
        "the seeded rollup date_key must appear in the trend: {}",
        outcome.model_text
    );
}

#[tokio::test]
async fn intelligence_report_remaining_arms_run_on_a_ready_plane() {
    // Test 3 — the arms NOT covered by the per-report success tests above (search_trails /
    // activity_mix / browsing_rhythm) must each dispatch over a READY plane without error,
    // returning their (possibly empty) result labeled by report — exercising every remaining
    // dispatch arm. `ensure_core_intelligence_schema` (via the seed) creates their tables, so a
    // sparse plane yields empty-but-valid results, never a "no such table" error.
    let (_dir, context) = context_with(None);
    seed_ready_intelligence(&context);
    let tool = IntelligenceReportTool;
    for report in ["search_trails", "activity_mix", "browsing_rhythm"] {
        let outcome = tool
            .call(json!({ "report": report }), &context)
            .await
            .unwrap_or_else(|error| panic!("{report} over a ready plane must succeed: {error}"));
        assert!(
            outcome.model_text.contains(&format!("intelligence_report({report})")),
            "the {report} arm ran and labeled its result: {}",
            outcome.model_text
        );
        assert!(
            !outcome.model_text.contains("not been built"),
            "a ready plane must not return the not-ready note for {report}: {}",
            outcome.model_text
        );
    }
}

#[tokio::test]
async fn intelligence_report_not_ready_when_plane_is_empty() {
    // Test 3 (keep a not-ready arm) — an initialized-but-empty plane returns the honest note.
    let (_dir, context) = context_with(None);
    let tool = IntelligenceReportTool;
    let outcome = tool
        .call(json!({ "report": "top_sites" }), &context)
        .await
        .expect("empty plane returns the honest not-ready note, never an error");
    assert!(outcome.model_text.contains("not been built"), "got: {}", outcome.model_text);
}

#[tokio::test]
async fn intelligence_report_pages_over_budget_json_without_losing_unicode() {
    let (_dir, context) = context_with(None);
    seed_ready_intelligence(&context);
    context.vault.write(vault_store::Db::Intelligence,vault_store::Priority::Batch,|s| {
        s.execute("DELETE FROM domain_daily_rollups_rows",())?;
        for index in 0..300_i64 {
            s.execute("INSERT INTO domain_daily_rollups_rows(date_key,profile_id,registrable_domain,domain_category,visit_count,search_count,new_domain_visits,unique_urls) VALUES('2024-04-01','chrome:Default',?1,?2,?3,0,0,1)",vault_store::params_owned![format!("例子{index}.example.com"),"参考资料分类".repeat(4),index+1])?;
        }Ok(())
    }).unwrap();
    let mut cursor = None::<String>;
    let mut joined = String::new();
    let mut pages = 0;
    loop {
        let outcome = IntelligenceReportTool
            .call(json!({"report":"top_sites","limit":300,"output_cursor":cursor}), &context)
            .await
            .unwrap();
        assert!(outcome.model_text.len() <= INTELLIGENCE_REPORT_MAX_BYTES);
        let body = outcome.model_text.split_once("result follows.\n").unwrap().1;
        let value: serde_json::Value = serde_json::from_str(body).unwrap();
        joined.push_str(value["jsonFragment"].as_str().unwrap());
        pages += 1;
        cursor = value["nextOutputCursor"].as_str().map(str::to_owned);
        if cursor.is_none() {
            break;
        }
    }
    assert!(pages > 1);
    let report: serde_json::Value = serde_json::from_str(&joined).unwrap();
    assert_eq!(report["items"].as_array().unwrap().len(), 300);
}

#[tokio::test]
async fn list_stars_returns_seeded_star_details() {
    // Test 4 — list_stars over a SEEDED archive returns the starred page's domain/title/visit-count.
    let (_dir, context) = context_with(None);
    // Star a URL through the public API (the same canonicalization the tool resolves).
    crate::stars::set_star(
        &context.vault,
        crate::models::SetStarRequest {
            entity_kind: crate::models::StarEntityKind::Url,
            entity_key: "https://example.com/page".to_string(),
            source_profile: None,
        },
    )
    .expect("star a url");
    let tool = ListStarsTool;
    let outcome = tool.call(json!({}), &context).await.expect("list_stars returns the seeded star");
    assert!(outcome.model_text.contains("1 starred item"), "got: {}", outcome.model_text);
    assert!(
        outcome.model_text.contains("example.com"),
        "the starred domain must appear: {}",
        outcome.model_text
    );
}

#[tokio::test]
async fn list_stars_tool_exposes_a_reusable_cursor_and_canonical_annotations() {
    let (_dir, context) = context_with(None);
    for id in 0..2 {
        crate::set_star(
            &context.vault,
            crate::SetStarRequest {
                entity_kind: crate::StarEntityKind::Url,
                entity_key: format!("https://star.test/{id}"),
                source_profile: None,
            },
        )
        .unwrap();
    }
    crate::set_notes(
        &context.vault,
        crate::SetNotesRequest {
            url: "https://star.test/1?utm_source=notes".into(),
            notes: "Favorite".into(),
            source_profile: None,
        },
    )
    .unwrap();
    let first = ListStarsTool.call(json!({"limit":1}), &context).await.unwrap();
    assert!(first.model_text.contains("More stars exist"));
    let page: crate::StarPage =
        serde_json::from_str(first.model_text.lines().last().unwrap()).unwrap();
    assert_eq!(page.items[0].notes, "Favorite");
    let second =
        ListStarsTool.call(json!({"limit":1,"cursor":page.next_cursor}), &context).await.unwrap();
    let last: crate::StarPage =
        serde_json::from_str(second.model_text.lines().last().unwrap()).unwrap();
    assert_ne!(page.items[0].entity_key, last.items[0].entity_key);
    assert!(last.next_cursor.is_none());
}

#[tokio::test]
async fn list_stars_renders_untitled_branch() {
    // Test 4 — the "(untitled)" formatting branch: a starred URL with no title row renders the
    // sentinel rather than an empty title. A regression dropping the branch fails this.
    let (_dir, context) = context_with(None);
    crate::stars::set_star(
        &context.vault,
        crate::models::SetStarRequest {
            entity_kind: crate::models::StarEntityKind::Url,
            entity_key: "https://untitled.example/x".to_string(),
            source_profile: None,
        },
    )
    .expect("star a url");
    let tool = ListStarsTool;
    let outcome = tool.call(json!({}), &context).await.expect("list_stars succeeds");
    assert!(
        outcome.model_text.contains("(untitled)"),
        "a star with no title row renders the untitled sentinel: {}",
        outcome.model_text
    );
}

#[tokio::test]
async fn list_annotations_returns_notes_and_tags_branch() {
    // Test 4 — list_annotations over SEEDED annotations returns the note + the tags-present branch.
    let (_dir, context) = context_with(None);
    let url = "https://example.com/tagged";
    crate::annotations::set_notes(
        &context.vault,
        crate::models::SetNotesRequest {
            url: url.to_string(),
            notes: "Short note about Rust".to_string(),
            source_profile: None,
        },
    )
    .expect("set notes");
    crate::annotations::replace_tags(
        &context.vault,
        crate::models::ReplaceTagsRequest {
            url: url.to_string(),
            tags: vec!["rust".to_string(), "reference".to_string()],
            source_profile: None,
        },
    )
    .expect("set tags");
    let tool = ListAnnotationsTool;
    let outcome = tool.call(json!({}), &context).await.expect("list_annotations returns data");
    assert!(outcome.model_text.contains("1 annotation"), "got: {}", outcome.model_text);
    assert!(outcome.model_text.contains("Short note about Rust"), "note: {}", outcome.model_text);
    // The tags-present branch renders the bracketed list (tags are normalized + sorted, so
    // alphabetical order, not insertion order).
    assert!(outcome.model_text.contains("[reference, rust]"), "tags: {}", outcome.model_text);
}

#[tokio::test]
async fn list_annotations_renders_no_tags_branch() {
    // Test 4 — the no-tags branch: a note with no tags renders without a "[...]" segment.
    let (_dir, context) = context_with(None);
    crate::annotations::set_notes(
        &context.vault,
        crate::models::SetNotesRequest {
            url: "https://example.com/plain".to_string(),
            notes: "A note with no tags".to_string(),
            source_profile: None,
        },
    )
    .expect("set notes");
    let tool = ListAnnotationsTool;
    let outcome = tool.call(json!({}), &context).await.expect("list_annotations returns data");
    assert!(outcome.model_text.contains("A note with no tags"), "got: {}", outcome.model_text);
    assert!(
        !outcome.model_text.contains('['),
        "no-tag note must not render a bracket: {}",
        outcome.model_text
    );
}

#[tokio::test]
async fn list_annotations_pages_every_byte_of_a_long_cjk_note() {
    let (_dir, context) = context_with(None);
    let note = format!("x{}", "记录笔记内容".repeat(700));
    crate::annotations::set_notes(
        &context.vault,
        crate::models::SetNotesRequest {
            url: "https://example.com/cjk".into(),
            notes: note.clone(),
            source_profile: None,
        },
    )
    .unwrap();
    let mut cursor = None;
    let mut text = String::new();
    let mut pages = 0;
    loop {
        let result =
            ListAnnotationsTool.call(json!({"output_cursor":cursor}), &context).await.unwrap();
        assert!(result.model_text.len() <= 8000);
        let page: Value = serde_json::from_str(&result.model_text).unwrap();
        text.push_str(page["jsonFragment"].as_str().unwrap());
        pages += 1;
        cursor = page["nextOutputCursor"].as_str().map(str::to_owned);
        if cursor.is_none() {
            break;
        }
    }
    assert!(pages > 1);
    assert!(text.contains(&note));
}

// ============================================================================================
// LOCAL-time rendering (Defect A) + session_detail (Defect C) + the sessions-semantics note (B).
// ============================================================================================

#[test]
fn format_local_ms_renders_local_and_handles_an_invalid_instant() {
    // Success: an epoch-ms instant renders as `YYYY-MM-DD HH:MM` in the offset. 1_700_000_000_000ms
    // is 2023-11-14T22:13:20Z; in +08:00 that is the 15th, 06:13 — proving the offset is applied.
    let plus8 = chrono_tz::Asia::Taipei;
    assert_eq!(format_local_ms(1_700_000_000_000, plus8), "2023-11-15 06:13");
    assert_eq!(format_local_ms(1_700_000_000_000, utc()), "2023-11-14 22:13");
    // Each instant gets its own date's offset: noon in New York in winter (EST) and in summer (EDT).
    let new_york = chrono_tz::America::New_York;
    assert_eq!(format_local_ms(1_772_384_400_000, new_york), "2026-03-01 12:00");
    assert_eq!(format_local_ms(1_782_921_600_000, new_york), "2026-07-01 12:00");
    // Parse-failure branch: `from_timestamp_millis` returns None for an out-of-range instant, so the
    // helper yields the empty string (never panics). i64::MAX ms is far past the representable range.
    assert_eq!(format_local_ms(i64::MAX, utc()), "");
}

#[test]
fn format_local_rfc3339_renders_local_and_passes_through_unparseable_input() {
    // Success: an RFC-3339 UTC instant renders as LOCAL `YYYY-MM-DD HH:MM` in the offset.
    let plus8 = chrono_tz::Asia::Taipei;
    assert_eq!(format_local_rfc3339("2026-06-21T22:30:00Z", plus8), "2026-06-22 06:30");
    assert_eq!(format_local_rfc3339("2026-06-21T22:30:00Z", utc()), "2026-06-21 22:30");
    // Parse-failure branch: an unparseable value is returned UNCHANGED (never panics, never empty).
    assert_eq!(format_local_rfc3339("not-a-date", utc()), "not-a-date");
}

#[test]
fn local_date_rfc3339_renders_the_local_day_and_is_empty_on_parse_failure() {
    // Success: the LOCAL calendar date can roll over relative to UTC when the offset crosses midnight.
    let plus8 = chrono_tz::Asia::Taipei;
    assert_eq!(local_date_rfc3339("2026-06-21T22:30:00Z", plus8), "2026-06-22");
    assert_eq!(local_date_rfc3339("2026-06-21T22:30:00Z", utc()), "2026-06-21");
    // Parse-failure branch: an unparseable value yields the empty string (never panics).
    assert_eq!(local_date_rfc3339("garbage", utc()), "");
}

#[test]
fn enrich_first_last_visit_local_adds_local_siblings_and_skips_missing_ms() {
    // Both ms fields present → both LOCAL siblings inserted, raw ms kept.
    let mut both =
        json!({ "firstVisitMs": 1_700_000_000_000_i64, "lastVisitMs": 1_700_000_060_000_i64 });
    enrich_first_last_visit_local(&mut both, utc());
    assert_eq!(both["firstVisitLocal"], "2023-11-14 22:13");
    assert_eq!(both["lastVisitLocal"], "2023-11-14 22:14");
    assert_eq!(both["firstVisitMs"], 1_700_000_000_000_i64, "the raw ms field is kept");
    // Only one ms field present → only that LOCAL sibling is added; the missing one stays absent.
    let mut first_only = json!({ "firstVisitMs": 1_700_000_000_000_i64 });
    enrich_first_last_visit_local(&mut first_only, utc());
    assert_eq!(first_only["firstVisitLocal"], "2023-11-14 22:13");
    assert!(first_only.get("lastVisitLocal").is_none(), "no sibling for an absent ms field");
    // A non-integer ms (e.g. null) is skipped — no LOCAL sibling fabricated.
    let mut not_int = json!({ "firstVisitMs": null });
    enrich_first_last_visit_local(&mut not_int, utc());
    assert!(not_int.get("firstVisitLocal").is_none());
}

#[test]
fn enrich_session_like_local_walks_an_array_and_no_ops_on_a_non_array() {
    // Array branch: every element gets its LOCAL siblings.
    let mut array = json!([
        { "firstVisitMs": 1_700_000_000_000_i64, "lastVisitMs": 1_700_000_060_000_i64 },
        { "firstVisitMs": 1_700_000_120_000_i64, "lastVisitMs": 1_700_000_180_000_i64 },
    ]);
    enrich_session_like_local(&mut array, utc());
    assert_eq!(array[0]["firstVisitLocal"], "2023-11-14 22:13");
    assert_eq!(array[1]["lastVisitLocal"], "2023-11-14 22:16");
    // Non-array (a torn/empty report) is a no-op — never panics, never mutates.
    let mut not_array = json!({ "unexpected": true });
    enrich_session_like_local(&mut not_array, utc());
    assert_eq!(not_array, json!({ "unexpected": true }), "a non-array input is left untouched");
}

#[test]
fn intelligence_report_definition_advertises_session_detail_and_session_id() {
    // Defect C — the schema must list `session_detail` in the report enum and offer `session_id`,
    // and the description must teach the sessions-semantics caveat + the drill-down path (Defect B).
    let def = IntelligenceReportTool.definition();
    let enum_values =
        def.parameters["properties"]["report"]["enum"].as_array().expect("enum array");
    assert!(enum_values.contains(&json!("session_detail")), "session_detail in enum");
    assert_eq!(def.parameters["properties"]["session_id"]["type"], "string");
    assert!(
        def.description.contains("autoTitle is a HEURISTIC")
            && def.description.contains("session_detail"),
        "the sessions-semantics note + drill-down hint is present: {}",
        def.description
    );
    // The tool is honest about its epistemic limit: STATISTICS/STRUCTURE only, NOT semantic
    // content — for what the user actually did, the model must read the actual visits.
    assert!(
        def.description.contains("STATISTICAL")
            && def.description.contains("CANNOT tell you WHAT")
            && def.description.contains("NO semantic understanding"),
        "the statistics-only / no-semantics caveat is present: {}",
        def.description
    );
}

#[test]
fn intel_report_parse_maps_session_detail_and_lists_it_on_an_unknown_variant() {
    // The parse maps the new name, and the unknown-variant error lists `session_detail` so the
    // model is told the full valid set.
    assert_eq!(IntelReport::parse("session_detail").expect("parses"), IntelReport::SessionDetail);
    let err = IntelReport::parse("nope").expect_err("unknown variant errors");
    assert!(
        err.to_string().contains("session_detail"),
        "the valid-variant list names session_detail: {err}"
    );
}

#[tokio::test]
async fn intelligence_report_session_detail_requires_session_id() {
    // Defect C — the `session_id` required-arg check runs BEFORE the readiness gate, so an omitted
    // id is an actionable arg ERROR (not the generic "not built" note) even on an empty plane.
    let (_dir, context) = context_with(None);
    let error = IntelligenceReportTool
        .call(json!({ "report": "session_detail" }), &context)
        .await
        .expect_err("session_detail without session_id must bail with a required-arg error");
    let message = error.to_string();
    assert!(message.contains("session_id"), "got: {message}");
    assert!(
        !message.contains("not been built"),
        "the arg error must pre-empt the readiness note: {message}"
    );
}

#[tokio::test]
async fn intelligence_report_session_detail_returns_the_seeded_session_with_local_times() {
    // Defect C + A — session_detail over a READY plane returns the seeded session AND its one visit,
    // rendering LOCAL times on BOTH the session (firstVisitLocal/lastVisitLocal) and each visit
    // (visitTimeLocal). The tests pin UTC, so the local string equals the UTC wall clock — which
    // makes the loop-body `visitTimeLocal` insertion (Defect C) exercised, not just the session row.
    let (_dir, context) = context_with(None);
    seed_ready_intelligence(&context);
    seed_session_visit(&context, "s-1", 70_001, 1_711_929_630_000);
    let outcome = IntelligenceReportTool
        .call(json!({ "report": "session_detail", "session_id": "s-1" }), &context)
        .await
        .expect("session_detail for a seeded session returns data");
    assert!(!outcome.model_text.contains("not been built"), "got: {}", outcome.model_text);
    assert!(
        outcome.model_text.contains("intelligence_report(session_detail)"),
        "the arm labels its result: {}",
        outcome.model_text
    );
    // The seeded session's first_visit_ms 1711929600000 = 2024-04-01T00:00:00Z → LOCAL (UTC) string.
    assert!(
        outcome.model_text.contains("firstVisitLocal")
            && outcome.model_text.contains("2024-04-01 00:00"),
        "the session carries a LOCAL first-visit time: {}",
        outcome.model_text
    );
    // The visits loop adds visitTimeLocal: visit_time_ms 1711929630000 = 2024-04-01T00:00:30Z.
    assert!(
        outcome.model_text.contains("visitTimeLocal")
            && outcome.model_text.contains("2024-04-01 00:00"),
        "the visit row carries a LOCAL visit time (the loop body ran): {}",
        outcome.model_text
    );
}

#[tokio::test]
async fn intelligence_report_session_detail_unknown_id_propagates_an_error() {
    // A non-existent session id is `Err(not found)` from the read model, which propagates as a
    // recoverable tool error (never a panic).
    let (_dir, context) = context_with(None);
    seed_ready_intelligence(&context);
    let error = IntelligenceReportTool
        .call(json!({ "report": "session_detail", "session_id": "no-such-session" }), &context)
        .await
        .expect_err("an unknown session id errors");
    assert!(error.to_string().contains("not found"), "got: {error}");
}

#[tokio::test]
async fn intelligence_report_sessions_enriches_rows_with_local_times() {
    // Defect A — the sessions report's rows carry the LOCAL first/last visit siblings (UTC-pinned).
    let (_dir, context) = context_with(None);
    seed_ready_intelligence(&context);
    let outcome = IntelligenceReportTool
        .call(json!({ "report": "sessions" }), &context)
        .await
        .expect("sessions over a ready plane returns data");
    assert!(
        outcome.model_text.contains("firstVisitLocal")
            && outcome.model_text.contains("2024-04-01 00:00"),
        "session rows carry a LOCAL first-visit time: {}",
        outcome.model_text
    );
}

#[tokio::test]
async fn search_history_renders_local_visit_times_in_the_summary() {
    // Defect A — the search tool's per-row `visited` is rendered as LOCAL time (UTC-pinned context),
    // not the raw RFC-3339 instant. The seed uses RFC-3339 `now`, so prove the format is `HH:MM`,
    // not the `T...Z` instant. We seed under a known token and assert the LOCAL shape appears.
    use crate::models::{AiSearchEntry, AiSearchResponse};
    let response = AiSearchResponse {
        search_state: None,
        total: 1,
        provider_id: "p".to_string(),
        model: "m".to_string(),
        items: vec![AiSearchEntry {
            history_id: 9,
            profile_id: "p".to_string(),
            url: "https://x.example/".to_string(),
            title: Some("X".to_string()),
            domain: "x.example".to_string(),
            visited_at: "2026-06-21T22:30:00Z".to_string(),
            score: 0.1,
            match_reason: MATCH_REASON_LEXICAL.to_string(),
            enrichment_excerpt: None,
        }],
        notes: Vec::new(),
        note_codes: Vec::new(),
        next_cursor: None,
        applied_limit: Some(50),
        has_more: false,
    };
    let plus8 = chrono_tz::Asia::Taipei;
    let text = summarize_search_for_model("search_history", &response, plus8);
    assert!(
        text.contains("2026-06-22 06:30"),
        "the row renders LOCAL time (offset applied, no T...Z): {text}"
    );
    assert!(!text.contains("22:30:00Z"), "the raw RFC-3339 instant is not used: {text}");
}

#[test]
fn a_partial_index_is_explicit_in_the_model_facing_search_result() {
    let vault = crate::Vault::open_temp().unwrap();
    crate::test_support::search::seed_pages(&vault, 1);
    let _paused = vault.search().pause();
    crate::set_notes(
        &vault,
        crate::SetNotesRequest {
            url: "https://transport.test/page/1".into(),
            notes: "unpublishedwordtoken".into(),
            source_profile: None,
        },
    )
    .unwrap();
    let result = crate::ai::search_pages::lexical_page(
        &vault,
        &crate::AiSearchRequest { query: "unpublishedwordtoken".into(), ..Default::default() },
    )
    .unwrap();
    assert!(result.items.is_empty());
    assert!(!result.search_state.as_ref().unwrap().coverage_complete);
    let summary = summarize_search_for_model("search_history", &result, chrono_tz::UTC);
    assert!(summary.contains("coverageComplete\":false"), "{summary}");
    assert!(summary.contains("incomplete"), "{summary}");
    assert!(!summary.contains("no matching history rows"), "{summary}");
}
