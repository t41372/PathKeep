//! Responsibilities: preserve journal replay and message evidence behaviour.
//! Not responsible for: provider execution or schema migrations.
//! Failure modes: duplicate steps, missing parents, lost citations and wrong answering run.

use super::*;

fn citation(history_id: i64, canonical: &str) -> AgentCitationRecord {
    AgentCitationRecord {
        history_id,
        canonical_url: canonical.to_string(),
        url: format!("{canonical}?utm=x"),
        title: Some(format!("page {history_id}")),
        visited_at: Some("2026-06-21T00:00:00Z".to_string()),
        score: Some(0.5),
        profile_id: Some("chrome:Default".to_string()),
    }
}

#[test]
fn run_trace_round_trips_header_steps_and_citations_in_order() {
    let vault = Vault::open_temp().unwrap();
    // The run links to a real conversation so the cascade FK is exercised.
    save_conversation(
        &vault,
        &SaveAgentConversationRequest {
            id: "conv-run".to_string(),
            title: Some("run".to_string()),
            provider_id: None,
            messages: vec![user_message("m1", "what did i read?")],
        },
    )
    .expect("save conversation");

    begin_agent_run(
        &vault,
        &BeginAgentRun {
            id: "run-1".to_string(),
            conversation_id: Some("conv-run".to_string()),
            message_id: Some("m1".to_string()),
            provider_id: Some("llm-local".to_string()),
            embedding_provider_id: Some("embed-local".to_string()),
        },
    )
    .expect("begin run");

    // Journal-before-observe: the harness appends each step; seq is assigned monotonically.
    let s0 = append_agent_step(
        &vault,
        &AppendAgentStep {
            run_id: "run-1".to_string(),
            turn: 1,
            kind: "assistant-turn".to_string(),
            tool_name: None,
            tool_call_id: None,
            payload: r#"{"text":"thinking"}"#.to_string(),
        },
    )
    .expect("append step 0");
    let s1 = append_agent_step(
        &vault,
        &AppendAgentStep {
            run_id: "run-1".to_string(),
            turn: 1,
            kind: "tool-result".to_string(),
            tool_name: Some("search_history".to_string()),
            tool_call_id: Some("call-1".to_string()),
            payload: r#"{"rows":3}"#.to_string(),
        },
    )
    .expect("append step 1");
    assert_eq!((s0, s1), (0, 1), "seq is 0-based monotonic");

    record_agent_citations(
        &vault,
        "run-1",
        &[citation(20, "https://b.example/"), citation(10, "https://a.example/")],
    )
    .expect("record citations");
    finalize_agent_run(&vault, "run-1", AgentRunStatus::Completed, 2, 42, 17, None)
        .expect("finalize");

    let trace = load_agent_run(&vault, "run-1").expect("load").expect("present");
    assert_eq!(trace.status, AgentRunStatus::Completed);
    assert_eq!(trace.iterations, 2);
    assert_eq!((trace.prompt_tokens, trace.completion_tokens), (42, 17));
    assert_eq!(trace.error, None);
    assert_eq!(trace.message_id.as_deref(), Some("m1"));
    // Steps replay in seq order.
    assert_eq!(trace.steps.len(), 2);
    assert_eq!(trace.steps[0].seq, 0);
    assert_eq!(trace.steps[0].kind, "assistant-turn");
    assert_eq!(trace.steps[1].tool_call_id.as_deref(), Some("call-1"));
    // Citations sorted by history_id, canonical_url carried for the W-STAR key.
    assert_eq!(trace.citations.len(), 2);
    assert_eq!(trace.citations[0].history_id, 10);
    assert_eq!(trace.citations[0].canonical_url, "https://a.example/");
}

#[test]
fn duplicate_tool_result_step_is_idempotent_on_call_id() {
    let vault = Vault::open_temp().unwrap();
    begin_agent_run(&vault, &BeginAgentRun { id: "run-idem".to_string(), ..Default::default() })
        .expect("begin");
    let first = append_agent_step(
        &vault,
        &AppendAgentStep {
            run_id: "run-idem".to_string(),
            turn: 1,
            kind: "tool-result".to_string(),
            tool_name: Some("search_bm25".to_string()),
            tool_call_id: Some("call-7".to_string()),
            payload: r#"{"rows":1}"#.to_string(),
        },
    )
    .expect("first append");
    // Re-journaling the SAME (run_id, tool_call_id) returns the existing seq, no second row.
    let again = append_agent_step(
        &vault,
        &AppendAgentStep {
            run_id: "run-idem".to_string(),
            turn: 1,
            kind: "tool-result".to_string(),
            tool_name: Some("search_bm25".to_string()),
            tool_call_id: Some("call-7".to_string()),
            payload: r#"{"rows":1}"#.to_string(),
        },
    )
    .expect("dup append");
    assert_eq!(first, again, "duplicate returns the same seq");

    let trace = load_agent_run(&vault, "run-idem").expect("load").expect("present");
    assert_eq!(trace.steps.len(), 1, "idempotency key prevents a second row");
}

#[test]
fn finalize_failed_run_records_error_and_load_missing_is_none() {
    let vault = Vault::open_temp().unwrap();
    begin_agent_run(&vault, &BeginAgentRun { id: "run-fail".to_string(), ..Default::default() })
        .expect("begin");
    finalize_agent_run(&vault, "run-fail", AgentRunStatus::Failed, 1, 5, 0, Some("boom"))
        .expect("finalize failed");
    let trace = load_agent_run(&vault, "run-fail").expect("load").expect("present");
    assert_eq!(trace.status, AgentRunStatus::Failed);
    assert_eq!(trace.error.as_deref(), Some("boom"));
    assert!(load_agent_run(&vault, "missing-run").expect("load missing").is_none());
}

#[test]
fn begin_agent_run_rejects_blank_id_and_reopen_resets_accounting() {
    let vault = Vault::open_temp().unwrap();
    let blank =
        begin_agent_run(&vault, &BeginAgentRun { id: "  ".to_string(), ..Default::default() })
            .expect_err("blank id");
    assert!(blank.to_string().contains("agent run id is required"));

    begin_agent_run(&vault, &BeginAgentRun { id: "run-reopen".to_string(), ..Default::default() })
        .expect("begin");
    finalize_agent_run(&vault, "run-reopen", AgentRunStatus::Completed, 3, 9, 9, None)
        .expect("finalize");
    // Re-begin resets status to running and zeroes accounting (clean restart of the same id).
    begin_agent_run(&vault, &BeginAgentRun { id: "run-reopen".to_string(), ..Default::default() })
        .expect("re-begin");
    let trace = load_agent_run(&vault, "run-reopen").expect("load").expect("present");
    assert_eq!(trace.status, AgentRunStatus::Running);
    assert_eq!((trace.iterations, trace.prompt_tokens, trace.completion_tokens), (0, 0, 0));
}

#[test]
fn record_agent_citations_replaces_prior_set() {
    let vault = Vault::open_temp().unwrap();
    begin_agent_run(&vault, &BeginAgentRun { id: "run-cite".to_string(), ..Default::default() })
        .expect("begin");
    record_agent_citations(&vault, "run-cite", &[citation(1, "https://one.example/")])
        .expect("first citations");
    record_agent_citations(
        &vault,
        "run-cite",
        &[citation(2, "https://two.example/"), citation(3, "https://three.example/")],
    )
    .expect("replace citations");
    let trace = load_agent_run(&vault, "run-cite").expect("load").expect("present");
    assert_eq!(trace.citations.len(), 2, "prior set replaced, not appended");
    assert_eq!(trace.citations[0].history_id, 2);
}

#[test]
fn deleting_conversation_cascades_run_trace() {
    // The run trace is keyed off the conversation; deleting the conversation must cascade the
    // run, its steps, and citations (the trace is rebuildable derived state).
    let vault = Vault::open_temp().unwrap();
    save_conversation(
        &vault,
        &SaveAgentConversationRequest {
            id: "conv-casc".to_string(),
            title: None,
            provider_id: None,
            messages: vec![user_message("m1", "hi")],
        },
    )
    .expect("save");
    begin_agent_run(
        &vault,
        &BeginAgentRun {
            id: "run-casc".to_string(),
            conversation_id: Some("conv-casc".to_string()),
            ..Default::default()
        },
    )
    .expect("begin");
    append_agent_step(
        &vault,
        &AppendAgentStep {
            run_id: "run-casc".to_string(),
            turn: 1,
            kind: "assistant-turn".to_string(),
            tool_name: None,
            tool_call_id: None,
            payload: "{}".to_string(),
        },
    )
    .expect("step");
    record_agent_citations(&vault, "run-casc", &[citation(1, "https://x.example/")])
        .expect("citations");

    assert!(delete_conversation(&vault, "conv-casc").expect("delete"));
    assert!(load_agent_run(&vault, "run-casc").expect("load after cascade").is_none());
    let orphan_steps: i64 =
        query(&vault, "SELECT COUNT(*) FROM agent_steps WHERE run_id = 'run-casc'", (), |row| {
            Ok(row.get(0)?)
        })
        .expect("orphan steps");
    let orphan_citations: i64 = query(
        &vault,
        "SELECT COUNT(*) FROM agent_citations WHERE run_id = 'run-casc'",
        (),
        |row| Ok(row.get(0)?),
    )
    .expect("orphan citations");
    assert_eq!((orphan_steps, orphan_citations), (0, 0), "cascade removes children");
}

#[test]
fn begin_agent_run_self_heals_missing_conversation_parent() {
    // Regression: the FE persists a conversation lazily ON FINALIZE (after the turn), so the
    // FIRST tools-enabled turn of a NEW conversation opens a run whose `conversation_id` has no
    // `conversations` row yet. With the live FK (+ foreign_keys=ON) the agent_runs INSERT would
    // trip `FOREIGN KEY constraint failed` and kill the run before any model call. begin_agent_run
    // must self-heal by upserting a stub parent so the run opens cleanly.
    let vault = Vault::open_temp().unwrap();
    begin_agent_run(
        &vault,
        &BeginAgentRun {
            id: "run-orphan".to_string(),
            conversation_id: Some("conv-not-yet-saved".to_string()),
            message_id: Some("m1".to_string()),
            provider_id: Some("llm-local".to_string()),
            ..Default::default()
        },
    )
    .expect("begin must succeed even though the conversation row does not exist yet");

    // The run header is persisted and round-trips, linked to the (now stubbed) conversation.
    let trace = load_agent_run(&vault, "run-orphan").expect("load").expect("present");
    assert_eq!(trace.conversation_id.as_deref(), Some("conv-not-yet-saved"));
    assert_eq!(trace.status, AgentRunStatus::Running);

    // A stub parent conversations row now exists (placeholder title) so the FK is satisfied.
    let (title, message_count): (String, i64) = query(
        &vault,
        "SELECT title, message_count FROM conversations WHERE id = 'conv-not-yet-saved'",
        (),
        |row| Ok((row.get(0)?, row.get(1)?)),
    )
    .expect("stub conversation row exists");
    assert_eq!(title, UNTITLED_CONVERSATION_TITLE, "stub carries a placeholder title");
    assert_eq!(message_count, 0, "stub carries no messages");
}

#[test]
fn save_conversation_overwrites_a_begin_agent_run_stub() {
    // The stub begin_agent_run plants must be a true placeholder: a later save_conversation for
    // the same id lands the real title + transcript (wholesale replace) and the run still links.
    let vault = Vault::open_temp().unwrap();
    begin_agent_run(
        &vault,
        &BeginAgentRun {
            id: "run-stub".to_string(),
            conversation_id: Some("conv-lazy".to_string()),
            message_id: Some("m1".to_string()),
            ..Default::default()
        },
    )
    .expect("begin plants a stub conversation");

    // Capture the stub's created_at so we can prove the real save preserves the birth time.
    let stub_created_at: String = {
        query(&vault, "SELECT created_at FROM conversations WHERE id = 'conv-lazy'", (), |row| {
            Ok(row.get(0)?)
        })
        .expect("stub created_at")
    };

    // The FE finalizes the turn: the real save overwrites the stub with the genuine transcript.
    std::thread::sleep(StdDuration::from_millis(5));
    let saved = save_conversation(
        &vault,
        &SaveAgentConversationRequest {
            id: "conv-lazy".to_string(),
            title: None,
            provider_id: Some("llm-local".to_string()),
            messages: vec![
                user_message("m1", "what did i read about tauri?"),
                assistant_message("m2", "You read about Tauri in April."),
            ],
        },
    )
    .expect("save overwrites the stub");
    assert_eq!(saved.title, "what did i read about tauri?", "real title replaces the stub");
    assert_eq!(saved.message_count, 2, "real transcript replaces the empty stub");
    assert_eq!(saved.created_at, stub_created_at, "created_at preserved across the overwrite");

    // The transcript landed and the run still links to the (now real) conversation.
    let detail = load_conversation(&vault, "conv-lazy").expect("load").expect("present");
    assert_eq!(detail.messages.len(), 2);
    assert_eq!(detail.messages[0].id, "m1");
    let trace = load_agent_run(&vault, "run-stub").expect("load").expect("present");
    assert_eq!(trace.conversation_id.as_deref(), Some("conv-lazy"));
}

#[test]
fn begin_agent_run_does_not_clobber_an_existing_saved_conversation() {
    // When the conversation already exists (e.g. a follow-up turn), the stub upsert must be a
    // no-op: ON CONFLICT DO NOTHING keeps the real title, created_at, and message_count intact.
    let vault = Vault::open_temp().unwrap();
    let saved = save_conversation(
        &vault,
        &SaveAgentConversationRequest {
            id: "conv-existing".to_string(),
            title: Some("Real title".to_string()),
            provider_id: None,
            messages: vec![user_message("m1", "hi"), assistant_message("m2", "yo")],
        },
    )
    .expect("save real conversation first");

    std::thread::sleep(StdDuration::from_millis(5));
    begin_agent_run(
        &vault,
        &BeginAgentRun {
            id: "run-followup".to_string(),
            conversation_id: Some("conv-existing".to_string()),
            ..Default::default()
        },
    )
    .expect("begin against an existing conversation");
    let (title, created_at, message_count): (String, String, i64) = query(
        &vault,
        "SELECT title, created_at, message_count FROM conversations WHERE id = 'conv-existing'",
        (),
        |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
    )
    .expect("existing conversation row");
    assert_eq!(title, "Real title", "begin must not overwrite the real title with a stub");
    assert_eq!(created_at, saved.created_at, "begin must not restamp created_at");
    assert_eq!(message_count, 2, "begin must not zero the real message_count");
}

#[test]
fn agent_run_status_tags_round_trip() {
    for status in [
        AgentRunStatus::Running,
        AgentRunStatus::Completed,
        AgentRunStatus::Cancelled,
        AgentRunStatus::Failed,
    ] {
        assert_eq!(AgentRunStatus::from_tag(status.as_str()), status);
    }
    // An unknown tag reads as the honest "interrupted" Running state.
    assert_eq!(AgentRunStatus::from_tag("nonsense"), AgentRunStatus::Running);
}

// -----------------------------------------------------------------------
// W-AI-7 WU-7: citations + usage reconstructed on reopen (load_conversation).
// -----------------------------------------------------------------------

#[test]
fn load_conversation_reconstructs_citations_and_usage_for_the_answering_turn() {
    // The durable trace must survive a reopen: a run linked to an assistant message (by
    // message_id) pins citations + tallies tokens, and a later load_conversation stitches BOTH
    // onto that assistant turn (and only that one) — so a reopened conversation renders the same
    // evidence rows + star keys + token footer the live turn streamed.
    let vault = Vault::open_temp().unwrap();
    save_conversation(
        &vault,
        &SaveAgentConversationRequest {
            id: "conv-trace".to_string(),
            title: None,
            provider_id: Some("llm-local".to_string()),
            messages: vec![
                user_message("u1", "what did i read about tauri?"),
                assistant_message("a1", "You read the Tauri guide in April."),
            ],
        },
    )
    .expect("save conversation");

    // The agent run that ANSWERED message a1 (the FE passes the assistant message id as the run's
    // message_id), pins two citations, and finalizes with a token tally.
    begin_agent_run(
        &vault,
        &BeginAgentRun {
            id: "run-trace".to_string(),
            conversation_id: Some("conv-trace".to_string()),
            message_id: Some("a1".to_string()),
            provider_id: Some("llm-local".to_string()),
            embedding_provider_id: None,
        },
    )
    .expect("begin run");
    record_agent_citations(
        &vault,
        "run-trace",
        &[citation(20, "https://tauri.app/guide/"), citation(10, "https://tauri.app/")],
    )
    .expect("record citations");
    finalize_agent_run(&vault, "run-trace", AgentRunStatus::Completed, 2, 128, 64, None)
        .expect("finalize");

    let detail = load_conversation(&vault, "conv-trace").expect("load").expect("present");
    assert_eq!(detail.messages.len(), 2);
    // The user turn carries NO reconstructed trace.
    let user = &detail.messages[0];
    assert_eq!(user.id, "u1");
    assert!(user.citations.is_empty(), "user turn must not carry citations");
    assert!(user.usage.is_none(), "user turn must not carry usage");
    // The assistant turn carries the run's usage + citations (ordered by history_id, with the
    // canonical_url star key preserved).
    let assistant = &detail.messages[1];
    assert_eq!(assistant.id, "a1");
    assert_eq!(
        assistant.usage,
        Some(AgentUsage { prompt_tokens: 128, completion_tokens: 64 }),
        "assistant turn must carry the run's reconstructed usage"
    );
    assert_eq!(assistant.citations.len(), 2, "both pinned citations are reconstructed");
    assert_eq!(assistant.citations[0].history_id, 10);
    assert_eq!(
        assistant.citations[0].canonical_url.as_deref(),
        Some("https://tauri.app/"),
        "the W-STAR star key survives the reopen"
    );
    assert_eq!(assistant.citations[1].history_id, 20);
    assert_eq!(assistant.citations[0].profile_id, "chrome:Default", "the profile survives too");
}

#[test]
fn load_conversation_without_a_run_leaves_turns_trace_free() {
    // A plain conversation (no agent run journaled) must reopen with no citations + no usage on
    // any turn — the reconstruction is purely additive and never fabricates a trace.
    let vault = Vault::open_temp().unwrap();
    save_conversation(
        &vault,
        &SaveAgentConversationRequest {
            id: "conv-plain".to_string(),
            title: None,
            provider_id: None,
            messages: vec![user_message("u1", "hi"), assistant_message("a1", "hello")],
        },
    )
    .expect("save");

    let detail = load_conversation(&vault, "conv-plain").expect("load").expect("present");
    for message in &detail.messages {
        assert!(message.citations.is_empty(), "no run → no citations");
        assert!(message.usage.is_none(), "no run → no usage");
    }
}

#[test]
fn load_conversation_uses_the_latest_run_per_message_and_zero_tokens_yield_no_usage() {
    // Two runs answer the SAME assistant message (e.g. a re-asked turn reusing the id): the LATEST
    // run's trace wins on reopen, and a run that tallied zero tokens surfaces no usage footer (the
    // honest "no accounting" state), while its citations still reconstruct.
    let vault = Vault::open_temp().unwrap();
    save_conversation(
        &vault,
        &SaveAgentConversationRequest {
            id: "conv-latest".to_string(),
            title: None,
            provider_id: None,
            messages: vec![user_message("u1", "ask"), assistant_message("a1", "answer")],
        },
    )
    .expect("save");

    // The earlier run pins one citation + a token tally.
    begin_agent_run(
        &vault,
        &BeginAgentRun {
            id: "run-old".to_string(),
            conversation_id: Some("conv-latest".to_string()),
            message_id: Some("a1".to_string()),
            ..Default::default()
        },
    )
    .expect("begin old");
    record_agent_citations(&vault, "run-old", &[citation(1, "https://old.example/")])
        .expect("old citations");
    finalize_agent_run(&vault, "run-old", AgentRunStatus::Completed, 1, 50, 20, None)
        .expect("finalize old");

    // A later run for the same message tallies ZERO tokens and pins a different citation. Force a
    // strictly later updated_at so the "latest run wins" tie-break is meaningful.
    std::thread::sleep(StdDuration::from_millis(5));
    begin_agent_run(
        &vault,
        &BeginAgentRun {
            id: "run-new".to_string(),
            conversation_id: Some("conv-latest".to_string()),
            message_id: Some("a1".to_string()),
            ..Default::default()
        },
    )
    .expect("begin new");
    record_agent_citations(&vault, "run-new", &[citation(2, "https://new.example/")])
        .expect("new citations");
    finalize_agent_run(&vault, "run-new", AgentRunStatus::Completed, 1, 0, 0, None)
        .expect("finalize new");

    let detail = load_conversation(&vault, "conv-latest").expect("load").expect("present");
    let assistant = &detail.messages[1];
    assert!(assistant.usage.is_none(), "a zero-token run surfaces no usage footer");
    assert_eq!(assistant.citations.len(), 1, "only the latest run's citation is shown");
    assert_eq!(assistant.citations[0].history_id, 2, "latest run's evidence wins on reopen");
}
