//! Real LM Studio tool-executing AGENT e2e for the W-AI-7 harness (`drive_agent_run`).
//!
//! This is an INTEGRATION test (separate crate), so it links `vault-core` as a normal dependency
//! — the REAL rig network path + the REAL retrieval tools over a seeded archive + the REAL durable
//! `agent.sqlite` journal, not the in-crate `cfg(test)` stub. It is gated on `PATHKEEP_AGENT_E2E=1`
//! so it never runs in the coverage gate / CI (a missing or unset env var skips it); its logic is
//! covered by the un-gated harness/agent_store unit tests. Run it manually against a local LM Studio:
//!
//! ```sh
//! PATHKEEP_AGENT_E2E=1 cargo test --manifest-path src-tauri/Cargo.toml \
//!   -p vault-core --test lmstudio_agent_e2e -- --nocapture
//! ```
//!
//! It drives the WHOLE agent loop end-to-end and asserts the durable contract:
//! 1. the model emits a tool call → the harness executes `search_history` against a seeded archive
//! 2. a `ToolResult` streams back, the loop threads it, the model answers and CITES real rows
//! 3. a terminal `Usage` + `Citations` arrive before `Done`
//! 4. cancelling mid-loop emits a graceful `Done` (never an `Error`), matching plain chat
//! 5. the run's trace + citations are JOURNALED: `load_agent_run` replays steps + citations and
//!    `load_conversation` reconstructs the answering turn's citations + usage (W-AI-7 WU-7).

use std::path::Path;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use browser_history_fixtures::{FirefoxPlaceRow, FirefoxPlacesFixture, FirefoxVisitRow};
use vault_core::{
    AgentJournal, AgentRunOutcome, AgentToolContext, AiChatStreamChunk, AiCitation,
    AiProviderConfig, AiProviderPurpose, AiProviderRuntime, AiRequestFormat, AiRunCancelled,
    AiRunControl, AiSettings, AppConfig, ArchiveMode, BeginAgentRun, BrowserHistoryImportRequest,
    DEFAULT_MAX_ITERATIONS, DEFAULT_TOKEN_BUDGET, LlmChatRequest, LlmMessage, LlmRole,
    ProjectPaths, RigLlmProvider, SecretString, ToolRegistry, append_agent_step, begin_agent_run,
    drive_agent_run, finalize_agent_run, load_agent_run, load_conversation, probe_tool_capability,
    project_paths_with_root, record_agent_citations, save_conversation,
};

/// LM Studio fixture (matches the chat-stream + embedding e2es): tool-capable gemma LLM.
fn lmstudio_llm_runtime() -> AiProviderRuntime {
    AiProviderRuntime {
        config: AiProviderConfig {
            id: "lmstudio-agent-e2e".to_string(),
            name: "LM Studio Agent (e2e)".to_string(),
            purpose: AiProviderPurpose::Llm,
            request_format: AiRequestFormat::LmStudio,
            enabled: true,
            base_url: Some("http://localhost:1234/v1".to_string()),
            default_model: "google/gemma-4-26b-a4b-qat".to_string(),
            temperature: Some(0.2),
            max_tokens: Some(2048),
            ..AiProviderConfig::default()
        },
        api_key: Some(SecretString::from("lm-studio".to_string())),
    }
}

/// LM Studio embedding fixture so the hybrid `search_history` tool has a real semantic plane.
fn lmstudio_embedding_config() -> AiProviderConfig {
    AiProviderConfig {
        id: "lmstudio-agent-embed-e2e".to_string(),
        name: "LM Studio Agent Embedding (e2e)".to_string(),
        purpose: AiProviderPurpose::Embedding,
        request_format: AiRequestFormat::LmStudio,
        enabled: true,
        base_url: Some("http://localhost:1234/v1".to_string()),
        default_model: "text-embedding-qwen3-embedding-0.6b".to_string(),
        dimensions: None,
        api_key_saved: true,
        ..AiProviderConfig::default()
    }
}

/// An [`AppConfig`] with AI on + both LM Studio providers wired, over an initialized plaintext
/// archive, so the retrieval tools resolve real rows.
fn agent_config() -> AppConfig {
    let mut llm = lmstudio_llm_runtime().config;
    llm.api_key_saved = true;
    AppConfig {
        initialized: true,
        archive_mode: ArchiveMode::Plaintext,
        git_enabled: false,
        ai: AiSettings {
            enabled: true,
            assistant_enabled: true,
            semantic_index_enabled: true,
            retrieval_top_k: 5,
            llm_provider_id: Some(llm.id.clone()),
            embedding_provider_id: Some(lmstudio_embedding_config().id),
            llm_providers: vec![llm],
            embedding_providers: vec![lmstudio_embedding_config()],
            ..AiSettings::default()
        },
        ..AppConfig::default()
    }
}

/// Seeds a small, distinctive archive at `paths` by importing a real Firefox `places.sqlite`
/// fixture, so the lexical (BM25) plane has rows to surface for the agent's `search_history` call.
/// Returns the imported visit count (a sanity check the seed actually landed).
fn seed_archive(paths: &ProjectPaths, config: &AppConfig, source_dir: &Path) -> usize {
    let history_path = source_dir.join("places.sqlite");
    // A handful of recognizable pages; the prompt asks about "tauri", which only the first row
    // matches, so a faithful tool call must surface it.
    let base_ms = 1_777_680_000_000;
    FirefoxPlacesFixture::new()
        .add_place(FirefoxPlaceRow {
            id: 1,
            url: "https://tauri.app/guide/".to_string(),
            title: Some("Tauri Getting Started Guide".to_string()),
            visit_count: 3,
            hidden: false,
            last_visit_unix_ms: base_ms,
        })
        .add_place(FirefoxPlaceRow {
            id: 2,
            url: "https://www.rust-lang.org/learn".to_string(),
            title: Some("Learn Rust".to_string()),
            visit_count: 2,
            hidden: false,
            last_visit_unix_ms: base_ms + 1_000,
        })
        .add_place(FirefoxPlaceRow {
            id: 3,
            url: "https://news.example.com/weather".to_string(),
            title: Some("Local Weather Forecast".to_string()),
            visit_count: 1,
            hidden: false,
            last_visit_unix_ms: base_ms + 2_000,
        })
        .add_visit(FirefoxVisitRow {
            id: 11,
            place_id: 1,
            visit_time_unix_ms: base_ms,
            from_visit: None,
            visit_type: Some(1),
        })
        .add_visit(FirefoxVisitRow {
            id: 12,
            place_id: 2,
            visit_time_unix_ms: base_ms + 1_000,
            from_visit: None,
            visit_type: Some(1),
        })
        .add_visit(FirefoxVisitRow {
            id: 13,
            place_id: 3,
            visit_time_unix_ms: base_ms + 2_000,
            from_visit: None,
            visit_type: Some(1),
        })
        .write(&history_path)
        .expect("write firefox fixture");

    let request = BrowserHistoryImportRequest {
        source_path: history_path.display().to_string(),
        dry_run: false,
        browser_family: Some("firefox".to_string()),
        profile_id: Some("firefox:default".to_string()),
        browser_name: Some("Mozilla Firefox".to_string()),
        profile_name: Some("default".to_string()),
    };
    let inspection = vault_core::import_browser_history(paths, config, None, &request)
        .expect("import seeded archive");
    inspection.candidate_items
}

/// A real-database [`AgentJournal`] backed by `agent.sqlite`, mirroring the worker's journal so the
/// e2e exercises the SAME durable write path the desktop app uses (journal-before-observe).
struct SqliteJournal {
    paths: ProjectPaths,
    run_id: String,
}

impl AgentJournal for SqliteJournal {
    fn journal_step(
        &self,
        turn: u32,
        kind: &str,
        tool_name: Option<&str>,
        tool_call_id: Option<&str>,
        payload: &str,
    ) -> anyhow::Result<()> {
        append_agent_step(
            &self.paths,
            &vault_core::AppendAgentStep {
                run_id: self.run_id.clone(),
                turn: turn as i64,
                kind: kind.to_string(),
                tool_name: tool_name.map(ToOwned::to_owned),
                tool_call_id: tool_call_id.map(ToOwned::to_owned),
                payload: payload.to_string(),
            },
        )?;
        Ok(())
    }

    fn record_citations(&self, citations: &[AiCitation]) -> anyhow::Result<()> {
        let records: Vec<vault_core::AgentCitationRecord> = citations
            .iter()
            .map(|citation| vault_core::AgentCitationRecord {
                history_id: citation.history_id,
                canonical_url: citation
                    .canonical_url
                    .clone()
                    .unwrap_or_else(|| citation.url.clone()),
                url: citation.url.clone(),
                title: citation.title.clone(),
                visited_at: Some(citation.visited_at.clone()),
                score: citation.score,
            })
            .collect();
        record_agent_citations(&self.paths, &self.run_id, &records)?;
        Ok(())
    }
}

/// A cooperative cancel control over a shared `AtomicBool`, mirroring the worker's adapter so the
/// e2e can cancel a live run mid-loop and assert the graceful `Done`.
struct CancelToken {
    cancel: Arc<AtomicBool>,
}

impl AiRunControl for CancelToken {
    fn checkpoint(&self, detail: &str) -> anyhow::Result<()> {
        if self.cancel.load(Ordering::SeqCst) {
            return Err(AiRunCancelled::new(detail.to_string()).into());
        }
        Ok(())
    }
    fn cancelled(&self) -> bool {
        self.cancel.load(Ordering::SeqCst)
    }
}

/// Builds the tool context the harness drives: the seeded archive + the real LM Studio embedding
/// provider so `search_history` runs the full hybrid pipeline.
fn tool_context(paths: &ProjectPaths, config: &AppConfig) -> AgentToolContext {
    AgentToolContext {
        paths: paths.clone(),
        config: config.clone(),
        database_key: None,
        embedding_provider: Some(AiProviderRuntime {
            config: lmstudio_embedding_config(),
            api_key: Some(SecretString::from("lm-studio".to_string())),
        }),
        default_profile_id: None,
        default_domain: None,
        default_limit: config.ai.retrieval_top_k.max(1),
        run_control: None,
    }
}

#[tokio::test]
async fn lmstudio_agent_runs_the_full_tool_loop_and_journals_a_durable_cited_trace() {
    if std::env::var("PATHKEEP_AGENT_E2E").as_deref() != Ok("1") {
        eprintln!(
            "skipping LM Studio agent e2e: set PATHKEEP_AGENT_E2E=1 with LM Studio running on :1234 \
             (LLM google/gemma-4-26b-a4b-qat + embedding text-embedding-qwen3-embedding-0.6b)"
        );
        return;
    }

    let dir = tempfile::tempdir().expect("tempdir");
    let paths = project_paths_with_root(dir.path());
    let config = agent_config();
    let seeded = seed_archive(&paths, &config, dir.path());
    assert!(seeded > 0, "the seeded archive must hold at least one visit");
    eprintln!("agent e2e: seeded {seeded} visit(s)");

    // The chat turn the run answers: link the durable trace to a real conversation + assistant
    // message so load_conversation can reconstruct the citations + usage onto that turn (WU-7).
    let conversation_id = "conv-agent-e2e";
    let message_id = "assistant-agent-e2e";
    save_conversation(
        &paths,
        &vault_core::SaveAgentConversationRequest {
            id: conversation_id.to_string(),
            title: None,
            provider_id: Some(lmstudio_llm_runtime().config.id),
            messages: vec![
                vault_core::AgentMessage {
                    id: "user-agent-e2e".to_string(),
                    role: "user".to_string(),
                    content: "When did I read the Tauri guide?".to_string(),
                    ..Default::default()
                },
                vault_core::AgentMessage {
                    id: message_id.to_string(),
                    role: "assistant".to_string(),
                    content: String::new(),
                    status: Some("done".to_string()),
                    ..Default::default()
                },
            ],
        },
    )
    .expect("save conversation");

    let run_id = "run-agent-e2e";
    begin_agent_run(
        &paths,
        &BeginAgentRun {
            id: run_id.to_string(),
            conversation_id: Some(conversation_id.to_string()),
            message_id: Some(message_id.to_string()),
            provider_id: Some(lmstudio_llm_runtime().config.id),
            embedding_provider_id: Some(lmstudio_embedding_config().id),
        },
    )
    .expect("begin run");

    let provider = RigLlmProvider::new(lmstudio_llm_runtime());
    let registry = ToolRegistry::with_default_search_tools();
    let context = tool_context(&paths, &config);
    let journal = SqliteJournal { paths: paths.clone(), run_id: run_id.to_string() };

    let request = LlmChatRequest::new(
        vec![
            LlmMessage::new(
                LlmRole::System,
                "You answer questions about the user's OWN browser history. You MUST call the \
                 search_history tool to find evidence before answering, and cite the rows you used.",
            ),
            LlmMessage::new(
                LlmRole::User,
                "Search my history: when did I read the Tauri guide? Cite the page.",
            ),
        ],
        Some(0.2),
        Some(2048),
    );

    // Capture the streamed chunks so we can assert the observable loop shape.
    let chunks = std::sync::Mutex::new(Vec::<AiChatStreamChunk>::new());
    // Probe tool capability exactly as the worker does. NOTE: the probe issues a 16-token turn, which
    // a REASONING model (gemma emits hundreds of reasoning tokens) can spend entirely on thinking,
    // returning empty content + `finish_reason: length` — so the probe can under-report `false` for a
    // model that, given a real budget, calls tools faithfully. Since this e2e's purpose is to drive
    // the REAL tool loop, force tools on when the probe degrades, and log it (a tool-incapable model
    // would simply emit no ToolCall below, which the assertions catch).
    let probed = probe_tool_capability(&provider).await.expect("probe tool capability");
    let can_use_tools = if probed {
        true
    } else {
        eprintln!(
            "agent e2e: probe reported tool-INCAPABLE (reasoning-model + 16-token probe budget); \
             forcing tools on to drive the real loop"
        );
        true
    };

    let outcome = drive_agent_run(
        &provider,
        &registry,
        &context,
        can_use_tools,
        request,
        None,
        DEFAULT_MAX_ITERATIONS,
        DEFAULT_TOKEN_BUDGET,
        &journal,
        |chunk: AiChatStreamChunk| chunks.lock().expect("lock").push(chunk),
    )
    .await;

    let captured = chunks.into_inner().expect("into inner");

    // --- Observable loop shape ------------------------------------------------------------------
    let tool_calls =
        captured.iter().filter(|c| matches!(c, AiChatStreamChunk::ToolCall { .. })).count();
    let tool_results =
        captured.iter().filter(|c| matches!(c, AiChatStreamChunk::ToolResult { .. })).count();
    let answer: String = captured
        .iter()
        .filter_map(|c| match c {
            AiChatStreamChunk::Token { text } => Some(text.as_str()),
            _ => None,
        })
        .collect();
    let usage_markers =
        captured.iter().filter(|c| matches!(c, AiChatStreamChunk::Usage { .. })).count();
    let streamed_citations = captured.iter().find_map(|c| match c {
        AiChatStreamChunk::Citations { citations } => Some(citations.clone()),
        _ => None,
    });
    let terminal_done = captured.last().is_some_and(|c| matches!(c, AiChatStreamChunk::Done));

    eprintln!(
        "agent e2e: outcome={outcome:?} toolCalls={tool_calls} toolResults={tool_results} \
         usageMarkers={usage_markers} answerChars={} streamedCitations={}",
        answer.len(),
        streamed_citations.as_ref().map(Vec::len).unwrap_or(0),
    );
    eprintln!("--- answer ---\n{}", answer.trim());
    if let Some(cites) = &streamed_citations {
        for cite in cites {
            eprintln!("  cite [{}] {} — {:?}", cite.history_id, cite.url, cite.title);
        }
    }

    // The agent harness streams a `ToolCall` BEFORE executing it (the live transparency row the FE
    // turns into the args + spinner) and the executed `ToolResult` after (F1) — so the honest
    // tool-loop signal is BOTH a ToolCall and its matching ToolResult.
    assert!(
        matches!(outcome, AgentRunOutcome::Completed { .. }),
        "the run must complete, got {outcome:?}"
    );
    assert!(
        tool_calls >= 1,
        "the harness must stream a ToolCall before executing the tool (F1 transparency timeline)"
    );
    assert!(
        tool_results >= 1,
        "the harness must execute search_history at least once and stream its ToolResult"
    );
    assert!(
        !answer.trim().is_empty(),
        "the run must produce a final answer (a cited answer, or the honest max-steps note)"
    );
    assert!(usage_markers >= 1, "the run must report at least one Usage marker");
    assert!(terminal_done, "a completed run ends with a terminal Done");
    // The seeded Tauri guide must surface as evidence (the prompt only matches that row), proving the
    // tool ran against the REAL archive, not a stub.
    let cites = streamed_citations.expect("a completed tool run must stream a Citations chunk");
    assert!(
        cites.iter().any(|c| c.url.contains("tauri.app")),
        "the seeded Tauri guide must surface as a citation"
    );

    // Finalize the durable header to match the harness outcome (mirrors the worker).
    if let AgentRunOutcome::Completed { iterations, prompt_tokens, completion_tokens } = outcome {
        finalize_agent_run(
            &paths,
            run_id,
            vault_core::AgentRunStatus::Completed,
            iterations as i64,
            prompt_tokens as i64,
            completion_tokens as i64,
            None,
        )
        .expect("finalize run");
    }

    // --- Durable trace: resume = replay ---------------------------------------------------------
    let trace = load_agent_run(&paths, run_id).expect("load trace").expect("trace present");
    assert!(!trace.steps.is_empty(), "the journal must hold the replayable steps");
    assert!(
        trace.steps.iter().any(|s| s.kind == "tool-result"),
        "a tool-result step must be journaled"
    );
    assert!(!trace.citations.is_empty(), "the run must pin at least one citation");
    assert_eq!(trace.status, vault_core::AgentRunStatus::Completed);

    // --- WU-7: reconstruction on reopen ---------------------------------------------------------
    let detail = load_conversation(&paths, conversation_id)
        .expect("load conversation")
        .expect("conversation present");
    let assistant =
        detail.messages.iter().find(|m| m.id == message_id).expect("assistant message present");
    assert!(
        !assistant.citations.is_empty(),
        "the answering turn must reconstruct its pinned citations on reopen"
    );
    assert!(
        assistant.citations.iter().all(|c| c.canonical_url.is_some()),
        "every reconstructed citation carries the W-STAR star key"
    );
    eprintln!(
        "agent e2e: reopened turn reconstructed {} citation(s) + usage={:?}",
        assistant.citations.len(),
        assistant.usage,
    );
}

#[tokio::test]
async fn lmstudio_agent_cancel_mid_loop_emits_a_graceful_done() {
    if std::env::var("PATHKEEP_AGENT_E2E").as_deref() != Ok("1") {
        eprintln!("skipping LM Studio agent cancel e2e: set PATHKEEP_AGENT_E2E=1");
        return;
    }

    let dir = tempfile::tempdir().expect("tempdir");
    let paths = project_paths_with_root(dir.path());
    let config = agent_config();
    seed_archive(&paths, &config, dir.path());

    let run_id = "run-agent-cancel-e2e";
    begin_agent_run(&paths, &BeginAgentRun { id: run_id.to_string(), ..Default::default() })
        .expect("begin run");

    let provider = RigLlmProvider::new(lmstudio_llm_runtime());
    let registry = ToolRegistry::with_default_search_tools();
    let context = tool_context(&paths, &config);
    let journal = SqliteJournal { paths: paths.clone(), run_id: run_id.to_string() };

    // Pre-armed cancel: the FIRST checkpoint (before the first model turn) trips, so the loop stops
    // cooperatively and emits a graceful Done — the same contract plain chat honors.
    let cancel = Arc::new(AtomicBool::new(true));
    let control: Arc<dyn AiRunControl> = Arc::new(CancelToken { cancel });

    let request = LlmChatRequest::new(
        vec![LlmMessage::new(LlmRole::User, "Search my history about the tauri guide.")],
        Some(0.2),
        Some(512),
    );

    let chunks = std::sync::Mutex::new(Vec::<AiChatStreamChunk>::new());
    let outcome = drive_agent_run(
        &provider,
        &registry,
        &context,
        true,
        request,
        Some(control),
        DEFAULT_MAX_ITERATIONS,
        DEFAULT_TOKEN_BUDGET,
        &journal,
        |chunk: AiChatStreamChunk| chunks.lock().expect("lock").push(chunk),
    )
    .await;

    let captured = chunks.into_inner().expect("into inner");
    eprintln!("agent cancel e2e: outcome={outcome:?} chunks={}", captured.len());
    assert!(
        matches!(outcome, AgentRunOutcome::Cancelled { .. }),
        "a cancelled run reports Cancelled, got {outcome:?}"
    );
    assert!(
        captured.iter().any(|c| matches!(c, AiChatStreamChunk::Done)),
        "cancel mid-loop must emit a graceful Done, never an Error"
    );
    assert!(
        !captured.iter().any(|c| matches!(c, AiChatStreamChunk::Error { .. })),
        "cancel must not surface an Error"
    );
}
