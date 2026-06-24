//! Worker-side streaming chat orchestration (W-AI-1) + tool-executing agent harness (W-AI-7).
//!
//! ## Responsibilities
//! - resolve the configured LLM provider runtime and build a `RigLlmProvider`
//! - start a background streaming chat run, forwarding each chunk to the desktop emit sink
//!   as an `AiChatStreamEvent` until a terminal `Done`/`Error`
//! - when `toolsEnabled`, instead drive the durable W-AI-7 agent harness (probe tool capability,
//!   build the owned tool registry, journal every step to `derived/agent.sqlite`, finalize the run)
//! - request cooperative cancellation of a live run by id (shared by both paths)
//!
//! ## Not responsible for
//! - emitting Tauri events (the desktop command supplies the sink closure)
//! - the agent loop / tool execution itself (vault-core `agent_harness` / `agent_tools`)
//! - the agent.sqlite schema/CRUD (vault-core `agent_store`; this only writes through it)
//!
//! ## Performance notes
//! - both paths run on a dedicated worker thread with its own scoped runtime, so the UI thread is
//!   never blocked and the run can outlive the foreground `ai_chat_send` call. The interactive
//!   agent run is foreground (matching W-AI-1 transport + cancel) but every step is journaled so a
//!   resume = trace replay (never a model re-call); the lease-queue path stays for batch/insight.

use crate::context::{
    load_unlocked_config, selected_llm_provider_runtime, selected_optional_embedding_runtime,
    tokio_runtime,
};
use anyhow::Result;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};
use vault_core::{
    AgentCitationRecord, AgentJournal, AgentRunOutcome, AgentRunStatus, AgentToolContext,
    AiCapability, AiChatCancelResult, AiChatMessage, AiChatRole, AiChatSendAck, AiChatSendRequest,
    AiChatStreamChunk, AiChatStreamEvent, AiCitation, AiProviderRuntime, AiRunCancelled,
    AiRunControl, AppendAgentStep, BeginAgentRun, DEFAULT_MAX_ITERATIONS, DEFAULT_TOKEN_BUDGET,
    LlmChatRequest, LlmMessage, LlmRole, ProjectPaths, RigLlmProvider, ToolRegistry,
    append_agent_step, begin_agent_run, deregister_ai_chat_run, drive_agent_run,
    drive_ai_chat_stream, ensure_ai_capability_enabled, finalize_agent_run, probe_tool_capability,
    record_agent_citations, register_ai_chat_run, request_ai_chat_cancel,
};

/// Monotonic component of each run id, so ids stay unique within a process run.
static RUN_SEQUENCE: AtomicU64 = AtomicU64::new(0);

/// Generates a process-unique streaming chat run id.
///
/// Combines a millisecond timestamp with a monotonic counter so two sends in the same
/// millisecond still differ. The id is opaque to the front end (used only to subscribe + cancel).
fn next_run_id() -> String {
    let millis =
        SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_millis()).unwrap_or_default();
    let seq = RUN_SEQUENCE.fetch_add(1, Ordering::Relaxed);
    format!("chat-{millis}-{seq}")
}

/// Maps an IPC chat role onto the boundary `LlmRole`.
fn to_llm_role(role: &AiChatRole) -> LlmRole {
    match role {
        AiChatRole::System => LlmRole::System,
        AiChatRole::User => LlmRole::User,
        AiChatRole::Assistant => LlmRole::Assistant,
        AiChatRole::Tool => LlmRole::Tool,
    }
}

/// Builds the boundary chat request from the IPC send request.
///
/// W-AI-1 sends plain turns (no tool defs / no structured output); those additive request
/// fields default to empty so the agent harness (W-AI-7) can fill them later.
fn to_llm_request(request: &AiChatSendRequest) -> LlmChatRequest {
    let messages = request
        .messages
        .iter()
        .map(|AiChatMessage { role, content }| LlmMessage::new(to_llm_role(role), content.clone()))
        .collect();
    LlmChatRequest::new(messages, request.temperature, request.max_tokens)
}

/// Everything one agent run needs, resolved synchronously before the streaming thread starts.
///
/// Bundled so the (blocking) provider/config/registry resolution happens on the calling thread
/// while the run id is minted, then the whole bundle moves onto the background thread.
struct AgentRunSetup {
    llm_runtime: AiProviderRuntime,
    embedding_runtime: Option<AiProviderRuntime>,
    request: LlmChatRequest,
    conversation_id: Option<String>,
    message_id: Option<String>,
    paths: ProjectPaths,
    config: vault_core::AppConfig,
    session_database_key: Option<String>,
}

/// Starts a streaming chat run and returns its run id immediately.
///
/// Resolves the LLM provider, validates there is at least one message, registers the run for
/// cancellation, and spawns a worker thread that drives the stream — emitting one
/// `AiChatStreamEvent` per chunk through `emit` and a terminal `Done`/`Error` at the end. When
/// `request.tools_enabled` is set, the spawned thread drives the W-AI-7 agent harness instead of
/// plain streaming chat (probing tool capability, journaling every step, finalizing the run); the
/// IPC surface, run id, and cancellation are identical so the FE uses one engine. The `emit` sink
/// is the desktop `AppHandle::emit` wrapper; it must be `Send + Sync + 'static` so the background
/// thread can own it.
pub fn ai_chat_send<E>(
    session_database_key: Option<&str>,
    request: &AiChatSendRequest,
    emit: E,
) -> Result<AiChatSendAck>
where
    E: Fn(AiChatStreamEvent) + Send + Sync + 'static,
{
    if request.messages.is_empty() {
        anyhow::bail!("A chat request needs at least one message.");
    }
    let paths = vault_core::project_paths()?;
    let config = load_unlocked_config(&paths)?;
    // Consent gate at the firing site: a previously-configured provider+key must NOT let the
    // tool-executing agent run (or plain streaming chat) fire while the master AI switch or the
    // assistant sub-flag is off. This bails BEFORE any provider resolution / run registration /
    // spawn so no work, network egress, or run id is minted for a refused turn (H-1 / M-2).
    ensure_ai_capability_enabled(&config, AiCapability::Assistant)?;
    let runtime = selected_llm_provider_runtime(&config, request.provider_id.as_deref())?;

    let run_id = next_run_id();
    let cancel = register_ai_chat_run(&run_id);

    if request.tools_enabled {
        // The embedding provider is OPTIONAL: with none configured, search_bm25 still works and the
        // semantic planes degrade honestly. A resolution failure (e.g. missing key) is not fatal to
        // the agent run, so treat it as "no embedding provider" rather than aborting.
        let embedding_runtime = selected_optional_embedding_runtime(&config).ok().flatten();
        let setup = AgentRunSetup {
            llm_runtime: runtime,
            embedding_runtime,
            request: to_llm_request(request),
            conversation_id: request.conversation_id.clone(),
            message_id: request.message_id.clone(),
            paths,
            config,
            session_database_key: session_database_key.map(ToOwned::to_owned),
        };
        spawn_agent_run(run_id.clone(), setup, cancel, emit);
    } else {
        spawn_chat_run(run_id.clone(), runtime, to_llm_request(request), cancel, emit);
    }
    Ok(AiChatSendAck { run_id })
}

/// Unwind-safe deregistration of a live run.
///
/// Constructed by the code that actually drives a run; its `Drop` removes the run from the
/// cancellation registry on BOTH normal return and a panic in `run_chat_stream`. Without this a
/// panic would leak the `Arc<AtomicBool>` and leave a stale "live" run (so `request_ai_chat_cancel`
/// would report `true` forever). It must only be constructed once a run is being driven, so the
/// `spawn` failure path (which never drives the run) can deregister explicitly without a double
/// free.
struct RunGuard<'a> {
    run_id: &'a str,
}

impl Drop for RunGuard<'_> {
    fn drop(&mut self) {
        deregister_ai_chat_run(self.run_id);
    }
}

/// Spawns the dedicated thread that drives one streaming run to completion.
///
/// Split out so the spawn policy (own thread + scoped runtime + always-deregister) is in one
/// place and is reachable by the coverage stub below with the same shape. The run is deregistered
/// via a [`RunGuard`] constructed INSIDE the driven body, so it fires on both normal return and a
/// panic. If the thread cannot be spawned at all, the run was never driven: this path emits one
/// terminal `Error` event and deregisters explicitly (the guard never ran), so the front end —
/// which is subscribed by run id — sees a terminal chunk instead of hanging forever.
#[cfg(not(coverage))]
fn spawn_chat_run<E>(
    run_id: String,
    runtime: vault_core::AiProviderRuntime,
    request: LlmChatRequest,
    cancel: std::sync::Arc<std::sync::atomic::AtomicBool>,
    emit: E,
) where
    E: Fn(AiChatStreamEvent) + Send + Sync + 'static,
{
    // `emit` is shared (not cloned) so the spawn-failure path below can still reach it after the
    // closure takes ownership: `std::thread::Builder::spawn` does NOT hand the closure back on
    // error, so we keep an `Arc` handle out here rather than losing the sink.
    let emit = std::sync::Arc::new(emit);
    let thread_emit = emit.clone();
    let thread_run_id = run_id.clone();
    let spawn_result =
        std::thread::Builder::new().name("pathkeep-ai-chat".to_string()).spawn(move || {
            // The guard lives only with the driven run, so deregistration happens on normal
            // return AND on a panic inside `run_chat_stream`.
            let _guard = RunGuard { run_id: &thread_run_id };
            run_chat_stream(&thread_run_id, runtime, request, cancel, &|event| thread_emit(event));
        });
    if let Err(error) = spawn_result {
        // The thread never started, so no `RunGuard` exists for this run. Surface a terminal
        // error to the run's subscribers and clear the registry entry ourselves.
        emit(AiChatStreamEvent {
            run_id: run_id.clone(),
            chunk: AiChatStreamChunk::Error {
                message: format!("Failed to start chat stream worker: {error}"),
            },
        });
        deregister_ai_chat_run(&run_id);
    }
}

/// Coverage build: run the stream inline (deterministic stub provider, no real network) so the
/// emit + drive + deregister path is exercised by the unit test without thread/timing flakiness.
/// The same [`RunGuard`] makes deregistration unwind-safe here too.
#[cfg(coverage)]
fn spawn_chat_run<E>(
    run_id: String,
    runtime: vault_core::AiProviderRuntime,
    request: LlmChatRequest,
    cancel: std::sync::Arc<std::sync::atomic::AtomicBool>,
    emit: E,
) where
    E: Fn(AiChatStreamEvent) + Send + Sync + 'static,
{
    let _guard = RunGuard { run_id: &run_id };
    run_chat_stream(&run_id, runtime, request, cancel, &emit);
}

/// Builds the scoped Tokio runtime that hosts one streaming run.
///
/// In a normal build this is exactly `tokio_runtime()`. Under the coverage build it additionally
/// returns `Err` when the runtime config id contains `"runtime-error"`, so the `Err` arm of
/// `run_chat_stream`'s `match` (which emits a terminal `Error`) is exercised deterministically.
#[cfg(not(coverage))]
fn build_stream_runtime(
    _runtime: &vault_core::AiProviderRuntime,
) -> Result<tokio::runtime::Runtime> {
    tokio_runtime()
}

/// Coverage build of [`build_stream_runtime`]: forces a runtime-build failure for ids tagged
/// `"runtime-error"` so the terminal-error arm in `run_chat_stream` is covered.
#[cfg(coverage)]
fn build_stream_runtime(
    runtime: &vault_core::AiProviderRuntime,
) -> Result<tokio::runtime::Runtime> {
    if runtime.config.id.contains("runtime-error") {
        anyhow::bail!("forced coverage runtime-build error");
    }
    tokio_runtime()
}

/// Drives the boundary stream on a scoped runtime, emitting one event per chunk.
fn run_chat_stream<E>(
    run_id: &str,
    runtime: vault_core::AiProviderRuntime,
    request: LlmChatRequest,
    cancel: std::sync::Arc<std::sync::atomic::AtomicBool>,
    emit: &E,
) where
    E: Fn(AiChatStreamEvent) + Send + Sync,
{
    let stream_runtime = match build_stream_runtime(&runtime) {
        Ok(rt) => rt,
        Err(error) => {
            emit(AiChatStreamEvent {
                run_id: run_id.to_string(),
                chunk: AiChatStreamChunk::Error { message: error.to_string() },
            });
            return;
        }
    };
    let provider = RigLlmProvider::new(runtime);
    let driver = drive_ai_chat_stream(&provider, request, cancel, |chunk: AiChatStreamChunk| {
        emit(AiChatStreamEvent { run_id: run_id.to_string(), chunk });
    });
    stream_runtime.block_on(driver);
}

// ---------------------------------------------------------------------------
// W-AI-7 agent harness path.
// ---------------------------------------------------------------------------

/// Wraps the streaming run's `Arc<AtomicBool>` cancel token as an `AiRunControl` for the harness.
///
/// The harness checkpoints cancellation through `AiRunControl`; the existing chat path uses the
/// same `register_ai_chat_run` token, so this adapter lets `ai_chat_cancel` flip ONE token for both
/// paths. `checkpoint` returns `AiRunCancelled` (the harness routes that to a graceful `Done`).
struct WorkerCancelControl {
    cancel: Arc<AtomicBool>,
}

impl AiRunControl for WorkerCancelControl {
    fn checkpoint(&self, detail: &str) -> Result<()> {
        if self.cancel.load(Ordering::SeqCst) {
            return Err(AiRunCancelled::new(detail).into());
        }
        Ok(())
    }

    fn cancelled(&self) -> bool {
        self.cancel.load(Ordering::SeqCst)
    }
}

/// Durable journal that writes the harness's steps/citations to `derived/agent.sqlite` (W-AI-7).
///
/// JOURNAL-BEFORE-OBSERVE: the harness calls these BEFORE emitting the matching chunk, so a crash
/// mid-run leaves an interrupted partial trace and a resume = replay (never a model re-call). The
/// `run_id` anchors every write to the run header opened by [`spawn_agent_run`]. Citations are
/// converted to the canonical-url-keyed record (the W-STAR key) here, deriving the canonical url
/// from each citation's raw url so WU-6 can star a cited page.
struct AgentSqliteJournal {
    paths: ProjectPaths,
    run_id: String,
}

impl AgentJournal for AgentSqliteJournal {
    fn journal_step(
        &self,
        turn: u32,
        kind: &str,
        tool_name: Option<&str>,
        tool_call_id: Option<&str>,
        payload: &str,
    ) -> Result<()> {
        append_agent_step(
            &self.paths,
            &AppendAgentStep {
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

    fn record_citations(&self, citations: &[AiCitation]) -> Result<()> {
        // Dedup by history_id (the agent_citations primary key is (run_id, history_id)). The star
        // key (canonical_url) is resolved at the tool-call site (W-AI-7), so prefer the carried
        // value; only re-derive it from the raw url for a legacy citation that lacks one, and fall
        // back to the raw url so the citation is never dropped.
        let mut seen = std::collections::HashSet::new();
        let records: Vec<AgentCitationRecord> = citations
            .iter()
            .filter(|citation| seen.insert(citation.history_id))
            .map(|citation| AgentCitationRecord {
                history_id: citation.history_id,
                canonical_url: citation.canonical_url.clone().unwrap_or_else(|| {
                    vault_core::visit_taxonomy::normalize_visit_url(&citation.url)
                        .map(|normalized| normalized.canonical_url)
                        .unwrap_or_else(|| citation.url.clone())
                }),
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

/// Spawns the dedicated thread that drives one agent run to completion.
///
/// Mirrors [`spawn_chat_run`]'s spawn policy (own thread + scoped runtime + always-deregister via
/// [`RunGuard`]). The run header is opened here on the spawned thread (so a spawn failure never
/// leaves an orphan `running` row), then [`run_agent_stream`] drives the harness and finalizes the
/// header. A spawn failure emits one terminal `Error` and deregisters explicitly (the guard never
/// ran), so the FE — subscribed by run id — sees a terminal chunk instead of hanging.
#[cfg(not(coverage))]
fn spawn_agent_run<E>(run_id: String, setup: AgentRunSetup, cancel: Arc<AtomicBool>, emit: E)
where
    E: Fn(AiChatStreamEvent) + Send + Sync + 'static,
{
    let emit = Arc::new(emit);
    let thread_emit = emit.clone();
    let thread_run_id = run_id.clone();
    let spawn_result =
        std::thread::Builder::new().name("pathkeep-ai-agent".to_string()).spawn(move || {
            let _guard = RunGuard { run_id: &thread_run_id };
            run_agent_stream(&thread_run_id, setup, cancel, &|event| thread_emit(event));
        });
    if let Err(error) = spawn_result {
        emit(AiChatStreamEvent {
            run_id: run_id.clone(),
            chunk: AiChatStreamChunk::Error {
                message: format!("Failed to start agent run worker: {error}"),
            },
        });
        deregister_ai_chat_run(&run_id);
    }
}

/// Coverage build: drive the agent run inline (deterministic stub provider, no real network) so the
/// full harness + journal + finalize path is exercised by the unit test without thread flakiness.
#[cfg(coverage)]
fn spawn_agent_run<E>(run_id: String, setup: AgentRunSetup, cancel: Arc<AtomicBool>, emit: E)
where
    E: Fn(AiChatStreamEvent) + Send + Sync + 'static,
{
    let _guard = RunGuard { run_id: &run_id };
    run_agent_stream(&run_id, setup, cancel, &emit);
}

/// Drives the agent harness on a scoped runtime: open the run header, probe tool capability, build
/// the owned registry + context, run the loop journaling every step, then finalize the header.
///
/// The whole run is wrapped so a runtime-build failure, a header-open failure, or a probe failure
/// each surface as one terminal `Error` to the run's subscribers (the FE never hangs). The harness
/// itself emits the per-chunk events; this maps the harness `AgentRunOutcome` onto the persisted
/// `AgentRunStatus` so the trace header is honest after the run ends.
fn run_agent_stream<E>(run_id: &str, setup: AgentRunSetup, cancel: Arc<AtomicBool>, emit: &E)
where
    E: Fn(AiChatStreamEvent) + Send + Sync,
{
    let emit_error = |message: String| {
        emit(AiChatStreamEvent {
            run_id: run_id.to_string(),
            chunk: AiChatStreamChunk::Error { message },
        });
    };

    let stream_runtime = match build_stream_runtime(&setup.llm_runtime) {
        Ok(rt) => rt,
        Err(error) => return emit_error(error.to_string()),
    };

    // Open the durable run header BEFORE any model call so a crash leaves an interrupted trace.
    let begin = BeginAgentRun {
        id: run_id.to_string(),
        conversation_id: setup.conversation_id.clone(),
        message_id: setup.message_id.clone(),
        provider_id: Some(setup.llm_runtime.config.id.clone()),
        embedding_provider_id: setup.embedding_runtime.as_ref().map(|p| p.config.id.clone()),
    };
    if let Err(error) = begin_agent_run(&setup.paths, &begin) {
        return emit_error(error.to_string());
    }

    let provider = RigLlmProvider::new(setup.llm_runtime.clone());
    let registry = ToolRegistry::with_default_search_tools();
    let journal = AgentSqliteJournal { paths: setup.paths.clone(), run_id: run_id.to_string() };
    let control: Arc<dyn AiRunControl> = Arc::new(WorkerCancelControl { cancel });
    let tool_context = AgentToolContext {
        paths: setup.paths.clone(),
        config: setup.config.clone(),
        database_key: setup.session_database_key.clone(),
        embedding_provider: setup.embedding_runtime.clone(),
        default_profile_id: None,
        default_domain: None,
        default_limit: setup.config.ai.retrieval_top_k.max(1),
        // Thread the SAME cancel control the harness loop uses into the tool context so a `run_code`
        // sandbox traps promptly on a user cancel (the sandbox bumps its engine epoch from this hook).
        run_control: Some(control.clone()),
    };

    // Inject the host-computed system context (current date/time/timezone + archive span) as the
    // FIRST message of every agent run. The model has no clock inside the sandbox and only knows its
    // training cutoff, so without this it guesses the wrong year for "last Friday", searches empty
    // date ranges, and loops. The facts are resolved on the host (OS clock/timezone + a bounded,
    // cached archive read) and prepended ahead of any existing leading system message so it is part
    // of the first turn but never displaces the caller's own system guidance.
    let mut request = setup.request;
    let system_context =
        vault_core::build_agent_system_context(&vault_core::resolve_agent_system_context(
            &setup.paths,
            &setup.config,
            setup.session_database_key.as_deref(),
        ));
    request.messages.insert(0, LlmMessage::new(LlmRole::System, system_context));

    let outcome = stream_runtime.block_on(async {
        // Probe tool capability BEFORE attaching tools (02 §B). A transport/auth failure surfaces
        // as an error (honest), anything else degrades to tool-incapable (retrieve-then-answer).
        let can_use_tools = match probe_tool_capability(&provider).await {
            Ok(capable) => capable,
            Err(error) => {
                emit_error(error.to_string());
                return None;
            }
        };
        Some(
            drive_agent_run(
                &provider,
                &registry,
                &tool_context,
                can_use_tools,
                request,
                Some(control),
                DEFAULT_MAX_ITERATIONS,
                DEFAULT_TOKEN_BUDGET,
                &journal,
                |chunk: AiChatStreamChunk| {
                    emit(AiChatStreamEvent { run_id: run_id.to_string(), chunk });
                },
            )
            .await,
        )
    });

    // Finalize the durable header to match the harness outcome (the probe-error path already
    // emitted + returned None, but still records a failed header so the trace is honest).
    let (status, iterations, prompt_tokens, completion_tokens, error) = match outcome {
        Some(AgentRunOutcome::Completed { iterations, prompt_tokens, completion_tokens }) => {
            (AgentRunStatus::Completed, iterations, prompt_tokens, completion_tokens, None)
        }
        Some(AgentRunOutcome::Cancelled { iterations, prompt_tokens, completion_tokens }) => {
            (AgentRunStatus::Cancelled, iterations, prompt_tokens, completion_tokens, None)
        }
        Some(AgentRunOutcome::Failed { iterations, prompt_tokens, completion_tokens, message }) => {
            (AgentRunStatus::Failed, iterations, prompt_tokens, completion_tokens, Some(message))
        }
        None => {
            (AgentRunStatus::Failed, 0, 0, 0, Some("Tool capability probe failed.".to_string()))
        }
    };
    let _ = finalize_agent_run(
        &setup.paths,
        run_id,
        status,
        iterations as i64,
        prompt_tokens as i64,
        completion_tokens as i64,
        error.as_deref(),
    );
}

/// Requests cooperative cancellation of a live streaming chat run (plain chat OR agent run).
pub fn ai_chat_cancel(
    _session_database_key: Option<&str>,
    run_id: &str,
) -> Result<AiChatCancelResult> {
    Ok(AiChatCancelResult { cancelled: request_ai_chat_cancel(run_id) })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::{Arc, Mutex};
    #[cfg(coverage)]
    use tempfile::tempdir;
    use vault_core::{AiProviderConfig, AiProviderPurpose, AiRequestFormat};

    fn stub_runtime(id: &str) -> vault_core::AiProviderRuntime {
        vault_core::AiProviderRuntime {
            config: AiProviderConfig {
                id: id.to_string(),
                name: "Test".to_string(),
                purpose: AiProviderPurpose::Llm,
                request_format: AiRequestFormat::LmStudio,
                enabled: true,
                default_model: "stub-model".to_string(),
                ..AiProviderConfig::default()
            },
            api_key: Some(vault_core::SecretString::from("k".to_string())),
        }
    }

    #[test]
    fn next_run_id_is_unique_and_prefixed() {
        let first = next_run_id();
        let second = next_run_id();
        assert!(first.starts_with("chat-"));
        assert_ne!(first, second);
    }

    #[test]
    fn role_mapping_covers_all_variants() {
        assert_eq!(to_llm_role(&AiChatRole::System), LlmRole::System);
        assert_eq!(to_llm_role(&AiChatRole::User), LlmRole::User);
        assert_eq!(to_llm_role(&AiChatRole::Assistant), LlmRole::Assistant);
        assert_eq!(to_llm_role(&AiChatRole::Tool), LlmRole::Tool);
    }

    #[test]
    fn request_mapping_preserves_messages_and_sampling() {
        let request = AiChatSendRequest {
            provider_id: Some("llm".to_string()),
            messages: vec![
                AiChatMessage { role: AiChatRole::System, content: "sys".to_string() },
                AiChatMessage { role: AiChatRole::User, content: "hi".to_string() },
            ],
            temperature: Some(0.7),
            max_tokens: Some(64),
            ..Default::default()
        };
        let mapped = to_llm_request(&request);
        assert_eq!(mapped.messages.len(), 2);
        assert_eq!(mapped.messages[0].role, LlmRole::System);
        assert_eq!(mapped.messages[1].content, "hi");
        assert_eq!(mapped.temperature, Some(0.7));
        assert_eq!(mapped.max_tokens, Some(64));
        // W-AI-1 threads no tools / no structured output yet.
        assert!(mapped.tools.is_empty());
        assert_eq!(mapped.response_format, None);
    }

    #[test]
    fn empty_message_request_is_rejected_before_provider_resolution() {
        let request = AiChatSendRequest::default();
        let error = ai_chat_send(None, &request, |_| {}).expect_err("empty request");
        assert!(error.to_string().contains("at least one message"));
    }

    #[test]
    fn chat_send_refuses_when_master_ai_or_assistant_consent_is_off() {
        // A non-empty request with a configured provider+key must STILL refuse while the master AI
        // switch (or the assistant sub-flag) is off — a previously-configured provider must not let
        // the agent/streaming run fire with consent withdrawn (H-1 / M-2). The gate bails before any
        // provider resolution or run spawn, so no run id is minted and the sink is never touched.
        let _guard = crate::tests::lock_env();
        let dir = tempfile::tempdir().expect("tempdir");
        let keyring_root = dir.path().join("test-keyring");
        unsafe {
            std::env::set_var(crate::tests::PROJECT_ROOT_OVERRIDE_ENV, dir.path());
            std::env::set_var(crate::tests::TEST_KEYRING_OVERRIDE_ENV, &keyring_root);
        }
        // Configured providers + saved key, but the master AI switch is OFF.
        let mut config = crate::tests::configured_ai_config();
        config.ai.enabled = false;
        let paths = vault_core::project_paths().expect("project paths");
        vault_core::save_config(&paths, &config).expect("save config");
        vault_platform::keyring_set_provider_api_key("llm-primary", "llm-secret")
            .expect("store llm key");

        let request = AiChatSendRequest {
            provider_id: Some("llm-primary".to_string()),
            messages: vec![AiChatMessage {
                role: AiChatRole::User,
                content: "what did i read about example?".to_string(),
            }],
            tools_enabled: true,
            ..Default::default()
        };
        let error = ai_chat_send(None, &request, |_| panic!("sink must never be reached"))
            .expect_err("master AI off must refuse");
        let message = error.to_string();
        assert!(
            message.contains("Enable AI") && message.contains("assistant"),
            "honest, actionable refusal: {message}"
        );

        // Master ON but the assistant sub-flag OFF also refuses (sub-flag is the second half).
        config.ai.enabled = true;
        config.ai.assistant_enabled = false;
        vault_core::save_config(&paths, &config).expect("save config");
        let error = ai_chat_send(None, &request, |_| panic!("sink must never be reached"))
            .expect_err("assistant sub-flag off must refuse");
        assert!(error.to_string().contains("assistant"), "names the assistant capability");

        unsafe {
            std::env::remove_var(crate::tests::PROJECT_ROOT_OVERRIDE_ENV);
            std::env::remove_var(crate::tests::TEST_KEYRING_OVERRIDE_ENV);
        }
    }

    #[test]
    fn cancel_unknown_run_reports_not_cancelled() {
        let result = ai_chat_cancel(None, "definitely-not-a-live-run").expect("cancel");
        assert!(!result.cancelled);
    }

    #[test]
    fn agent_journal_record_citations_prefers_carried_canonical_url_then_falls_back() {
        // The journal pins one row per history_id. W-AI-7 resolves the W-STAR star key
        // (canonical_url) at the tool-call site, so record_citations must PREFER the carried value
        // and only re-derive it from the raw url for a citation that lacks one. This exercises both
        // arms of the `unwrap_or_else` plus the history_id dedup, against a real agent.sqlite.
        let dir = tempfile::tempdir().expect("tempdir");
        let paths = vault_core::config::project_paths_with_root(dir.path());
        let journal =
            AgentSqliteJournal { paths: paths.clone(), run_id: "run-cite-pref".to_string() };
        // begin a header so the citation FK has a parent row to cascade from.
        vault_core::begin_agent_run(
            &paths,
            &vault_core::BeginAgentRun {
                id: "run-cite-pref".to_string(),
                conversation_id: None,
                message_id: None,
                provider_id: None,
                embedding_provider_id: None,
            },
        )
        .expect("begin run");

        journal
            .record_citations(&[
                // Carried canonical_url is preferred verbatim (NOT re-normalized from the raw url).
                AiCitation {
                    history_id: 1,
                    profile_id: "p".to_string(),
                    url: "https://a.example/x?utm=1".to_string(),
                    title: Some("A".to_string()),
                    visited_at: "2026-01-01T00:00:00Z".to_string(),
                    score: Some(0.5),
                    canonical_url: Some("https://carried.example/star-key".to_string()),
                },
                // No carried value → fall back to normalizing the raw url.
                AiCitation {
                    history_id: 2,
                    profile_id: "p".to_string(),
                    url: "https://b.example/y".to_string(),
                    title: None,
                    visited_at: "2026-01-02T00:00:00Z".to_string(),
                    score: None,
                    canonical_url: None,
                },
                // Duplicate history_id is dropped (the (run_id, history_id) primary key).
                AiCitation {
                    history_id: 1,
                    profile_id: "p".to_string(),
                    url: "https://a.example/x?utm=2".to_string(),
                    title: Some("A2".to_string()),
                    visited_at: "2026-01-03T00:00:00Z".to_string(),
                    score: Some(0.1),
                    canonical_url: Some("https://other.example/".to_string()),
                },
            ])
            .expect("record citations");

        let trace = vault_core::load_agent_run(&paths, "run-cite-pref")
            .expect("load trace")
            .expect("trace present");
        assert_eq!(trace.citations.len(), 2, "history_id 1 was deduped");
        let first = trace.citations.iter().find(|c| c.history_id == 1).expect("row 1");
        assert_eq!(first.canonical_url, "https://carried.example/star-key");
        let second = trace.citations.iter().find(|c| c.history_id == 2).expect("row 2");
        assert_eq!(second.canonical_url, "https://b.example/y");
    }

    // Coverage-only: under a plain `cargo test` build `vault-core`'s `RigLlmProvider` compiles
    // its REAL network path (cfg(coverage) is workspace-wide only under cargo-llvm-cov), which
    // would attempt a live connection. Under coverage the stub stream runs, so this is where the
    // full drive path is meaningfully exercised.
    #[cfg(coverage)]
    #[test]
    fn run_chat_stream_drives_stub_provider_and_tags_run_id() {
        // Exercises the full drive path: scoped runtime → stub RigLlmProvider stream → emitted
        // events tagged with the run id, ending in a terminal Done.
        let events: Arc<Mutex<Vec<AiChatStreamEvent>>> = Arc::new(Mutex::new(Vec::new()));
        let sink = {
            let events = events.clone();
            move |event: AiChatStreamEvent| events.lock().expect("lock").push(event)
        };
        let request = to_llm_request(&AiChatSendRequest {
            provider_id: None,
            messages: vec![AiChatMessage { role: AiChatRole::User, content: "hi".to_string() }],
            temperature: Some(0.6),
            max_tokens: Some(32),
            ..Default::default()
        });
        let cancel = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
        run_chat_stream("run-drive", stub_runtime("llm-chat"), request, cancel, &sink);
        let captured = events.lock().expect("lock");
        assert!(!captured.is_empty());
        // Every event is tagged with the run id and the last is a terminal marker.
        assert!(captured.iter().all(|event| event.run_id == "run-drive"));
        let last = captured.last().expect("at least one event");
        assert!(matches!(last.chunk, AiChatStreamChunk::Done | AiChatStreamChunk::Error { .. }));
    }

    // Coverage-only: forces `build_stream_runtime` to fail (id contains "runtime-error") so the
    // terminal-error arm of `run_chat_stream`'s runtime match is exercised. Asserts exactly one
    // emitted `Error` chunk and no other events.
    #[cfg(coverage)]
    #[test]
    fn run_chat_stream_emits_error_when_runtime_build_fails() {
        let events: Arc<Mutex<Vec<AiChatStreamEvent>>> = Arc::new(Mutex::new(Vec::new()));
        let sink = {
            let events = events.clone();
            move |event: AiChatStreamEvent| events.lock().expect("lock").push(event)
        };
        let request = to_llm_request(&AiChatSendRequest {
            provider_id: None,
            messages: vec![AiChatMessage { role: AiChatRole::User, content: "hi".to_string() }],
            temperature: None,
            max_tokens: None,
            ..Default::default()
        });
        let cancel = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
        run_chat_stream("run-runtime-error", stub_runtime("runtime-error"), request, cancel, &sink);
        let captured = events.lock().expect("lock");
        assert_eq!(captured.len(), 1);
        assert!(matches!(captured[0].chunk, AiChatStreamChunk::Error { .. }));
        assert_eq!(captured[0].run_id, "run-runtime-error");
    }

    #[test]
    fn worker_cancel_control_flips_with_its_shared_token() {
        // The adapter mirrors the SAME `Arc<AtomicBool>` cancel token both chat paths share: while it
        // is unset, `checkpoint` is Ok and `cancelled()` is false; once the token flips, `checkpoint`
        // returns `AiRunCancelled` (which the harness routes to a graceful Done) and `cancelled()`
        // reports true.
        let token = Arc::new(AtomicBool::new(false));
        let control = WorkerCancelControl { cancel: token.clone() };
        control.checkpoint("before cancel").expect("ok while the token is unset");
        assert!(!control.cancelled());
        token.store(true, Ordering::SeqCst);
        let error = control.checkpoint("after cancel").expect_err("cancel surfaces an error");
        assert!(error.downcast_ref::<AiRunCancelled>().is_some(), "must be a cooperative cancel");
        assert!(control.cancelled());
    }

    #[test]
    fn pre_cancelled_run_emits_only_done() {
        let events: Arc<Mutex<Vec<AiChatStreamEvent>>> = Arc::new(Mutex::new(Vec::new()));
        let sink = {
            let events = events.clone();
            move |event: AiChatStreamEvent| events.lock().expect("lock").push(event)
        };
        let request = to_llm_request(&AiChatSendRequest {
            provider_id: None,
            messages: vec![AiChatMessage { role: AiChatRole::User, content: "hi".to_string() }],
            temperature: None,
            max_tokens: None,
            ..Default::default()
        });
        let cancel = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(true));
        run_chat_stream("run-cancelled", stub_runtime("llm-cancel"), request, cancel, &sink);
        let captured = events.lock().expect("lock");
        assert_eq!(captured.len(), 1);
        assert_eq!(captured[0].chunk, AiChatStreamChunk::Done);
    }

    // -----------------------------------------------------------------------
    // W-AI-7 agent harness worker path. These drive `run_agent_stream` directly
    // (the inline-spawn body) so the durable journal/finalize wiring + every
    // outcome→status mapping is exercised deterministically against the stub
    // RigLlmProvider, with no thread or network. Coverage-only because the real
    // network client otherwise compiles under a plain `cargo test`.
    // -----------------------------------------------------------------------

    /// Builds a self-contained agent run setup over a fresh temp project root + a tool-capable
    /// floor LLM runtime (`id`). The archive is not seeded: the `search_history` tool degrades
    /// honestly on the empty/uninitialized archive, and the run still completes — the point here is
    /// the worker wiring (journal → finalize), not retrieval recall (covered in vault-core).
    #[cfg(coverage)]
    fn agent_setup(root: &std::path::Path, id: &str) -> AgentRunSetup {
        let paths = vault_core::config::project_paths_with_root(root);
        let mut config = vault_core::AppConfig::default();
        config.ai.enabled = true;
        config.ai.assistant_enabled = true;
        AgentRunSetup {
            llm_runtime: stub_runtime(id),
            embedding_runtime: None,
            request: to_llm_request(&AiChatSendRequest {
                provider_id: None,
                messages: vec![AiChatMessage {
                    role: AiChatRole::User,
                    content: "what did i read about example?".to_string(),
                }],
                temperature: Some(0.2),
                max_tokens: Some(128),
                ..Default::default()
            }),
            // No conversation link here: the worker-wiring tests assert the durable run header +
            // journal/finalize, not the (FK-cascaded) conversation linkage, which `agent_store`'s
            // own tests cover by pre-saving the conversation row.
            conversation_id: None,
            message_id: Some("msg-agent".to_string()),
            paths,
            config,
            session_database_key: None,
        }
    }

    #[cfg(coverage)]
    fn agent_events_sink()
    -> (Arc<Mutex<Vec<AiChatStreamEvent>>>, impl Fn(AiChatStreamEvent) + Send + Sync) {
        let events: Arc<Mutex<Vec<AiChatStreamEvent>>> = Arc::new(Mutex::new(Vec::new()));
        let sink = {
            let events = events.clone();
            move |event: AiChatStreamEvent| events.lock().expect("lock").push(event)
        };
        (events, sink)
    }

    #[cfg(coverage)]
    #[test]
    fn coverage_agent_run_completes_and_finalizes_a_durable_trace() {
        // The full happy path: probe (floor → capable), drive the two-turn stub harness, journal each
        // step to agent.sqlite, then finalize the header as Completed. Asserts the emitted terminal
        // Done AND the durable trace the worker persisted (resume = replay reads exactly this).
        let dir = tempdir().expect("tempdir");
        let setup = agent_setup(dir.path(), "llm-agent-ok");
        let paths = setup.paths.clone();
        let (events, sink) = agent_events_sink();
        let cancel = Arc::new(AtomicBool::new(false));
        run_agent_stream("run-agent-ok", setup, cancel, &sink);

        let captured = events.lock().expect("lock");
        assert!(!captured.is_empty());
        assert!(captured.iter().all(|event| event.run_id == "run-agent-ok"));
        assert!(matches!(captured.last().expect("terminal chunk").chunk, AiChatStreamChunk::Done));
        // A ToolResult was emitted (the harness executed the stub's search_history call).
        assert!(
            captured
                .iter()
                .any(|event| matches!(event.chunk, AiChatStreamChunk::ToolResult { .. }))
        );
        drop(captured);

        // The durable header was finalized as Completed, linked to the chat turn, with the journaled
        // steps replayable in order (the worker's record of the run).
        let trace = vault_core::load_agent_run(&paths, "run-agent-ok")
            .expect("load trace")
            .expect("trace present");
        assert_eq!(trace.status, AgentRunStatus::Completed);
        assert_eq!(trace.message_id.as_deref(), Some("msg-agent"));
        assert!(trace.iterations >= 1, "at least one model turn was journaled");
        assert!(
            trace.steps.iter().any(|step| step.kind == "assistant-turn"),
            "the assistant turn must be journaled"
        );
    }

    #[cfg(coverage)]
    #[test]
    fn coverage_agent_run_self_heals_an_unsaved_conversation_link() {
        // Production path B1: a tools-enabled run opens BEFORE the FE persists its conversation (the
        // FE saves lazily on finalize), so `run_agent_stream` passes a `conversation_id` whose
        // `conversations` row does not exist yet. With the live agent_runs→conversations FK this would
        // trip `FOREIGN KEY constraint failed` inside begin_agent_run and emit a terminal Error before
        // any model call. begin_agent_run now self-heals (stub parent), so the run completes normally
        // and the durable header links to the (stubbed) conversation.
        let dir = tempdir().expect("tempdir");
        let mut setup = agent_setup(dir.path(), "llm-agent-lazy-conv");
        setup.conversation_id = Some("conv-unsaved".to_string());
        let paths = setup.paths.clone();
        let (events, sink) = agent_events_sink();
        let cancel = Arc::new(AtomicBool::new(false));
        run_agent_stream("run-agent-lazy", setup, cancel, &sink);

        let captured = events.lock().expect("lock");
        // The run does NOT die on the FK: it reaches a terminal Done, never an Error.
        assert!(matches!(captured.last().expect("terminal chunk").chunk, AiChatStreamChunk::Done));
        assert!(
            !captured.iter().any(|event| matches!(event.chunk, AiChatStreamChunk::Error { .. })),
            "a not-yet-saved conversation link must not kill the run on the FK"
        );
        drop(captured);

        // The durable header was finalized as Completed and links to the lazily-persisted id.
        let trace = vault_core::load_agent_run(&paths, "run-agent-lazy")
            .expect("load trace")
            .expect("trace present");
        assert_eq!(trace.status, AgentRunStatus::Completed);
        assert_eq!(trace.conversation_id.as_deref(), Some("conv-unsaved"));
    }

    #[cfg(coverage)]
    #[test]
    fn coverage_agent_run_maps_cancellation_to_a_cancelled_header() {
        // A pre-cancelled control: the harness sees `cancelled()` at its first checkpoint and ends
        // Cancelled, which the worker maps onto a Cancelled header (not Failed) — matching plain chat.
        let dir = tempdir().expect("tempdir");
        let setup = agent_setup(dir.path(), "llm-agent-cancel");
        let paths = setup.paths.clone();
        let (events, sink) = agent_events_sink();
        let cancel = Arc::new(AtomicBool::new(true));
        run_agent_stream("run-agent-cancel", setup, cancel, &sink);

        let captured = events.lock().expect("lock");
        assert!(matches!(captured.last().expect("terminal chunk").chunk, AiChatStreamChunk::Done));
        assert!(
            !captured.iter().any(|event| matches!(event.chunk, AiChatStreamChunk::Error { .. })),
            "a cooperative cancel never emits Error"
        );
        drop(captured);
        let trace = vault_core::load_agent_run(&paths, "run-agent-cancel")
            .expect("load trace")
            .expect("trace present");
        assert_eq!(trace.status, AgentRunStatus::Cancelled);
    }

    #[cfg(coverage)]
    #[test]
    fn coverage_agent_run_maps_a_stream_error_to_a_failed_header() {
        // The stub surfaces a mid-stream Err on turn 1 for ids containing "stream-error"; the harness
        // ends Failed and the worker records a Failed header carrying the error message.
        let dir = tempdir().expect("tempdir");
        let setup = agent_setup(dir.path(), "llm-agent-stream-error");
        let paths = setup.paths.clone();
        let (events, sink) = agent_events_sink();
        let cancel = Arc::new(AtomicBool::new(false));
        run_agent_stream("run-agent-failed", setup, cancel, &sink);

        let captured = events.lock().expect("lock");
        assert!(matches!(
            captured.last().expect("terminal chunk").chunk,
            AiChatStreamChunk::Error { .. }
        ));
        drop(captured);
        let trace = vault_core::load_agent_run(&paths, "run-agent-failed")
            .expect("load trace")
            .expect("trace present");
        assert_eq!(trace.status, AgentRunStatus::Failed);
        assert!(trace.error.is_some(), "a failed run records its error in the header");
    }

    #[cfg(coverage)]
    #[test]
    fn coverage_agent_run_surfaces_a_probe_transport_failure() {
        // A reachability failure during the tool-capability probe is surfaced as a terminal Error +
        // a Failed header noting the probe failure (None outcome arm). Every shipping format now
        // self-certifies, so the `probe-roundtrip` id opts this fixture out of the short-circuit to
        // force the real probe round-trip; `network-error` makes the stubbed probe chat fail.
        let dir = tempdir().expect("tempdir");
        let setup = agent_setup(dir.path(), "llm-agent-probe-roundtrip-network-error");
        let paths = setup.paths.clone();
        let (events, sink) = agent_events_sink();
        let cancel = Arc::new(AtomicBool::new(false));
        run_agent_stream("run-agent-probe", setup, cancel, &sink);

        let captured = events.lock().expect("lock");
        assert!(
            captured
                .iter()
                .any(|event| matches!(&event.chunk, AiChatStreamChunk::Error { message } if message.contains("refused")))
        );
        drop(captured);
        let trace = vault_core::load_agent_run(&paths, "run-agent-probe")
            .expect("load trace")
            .expect("trace present");
        assert_eq!(trace.status, AgentRunStatus::Failed);
        assert_eq!(trace.error.as_deref(), Some("Tool capability probe failed."));
    }

    #[cfg(coverage)]
    #[test]
    fn coverage_agent_run_emits_error_when_the_runtime_build_fails() {
        // A runtime-build failure (id "runtime-error") happens BEFORE any header is opened, so the
        // worker emits one terminal Error and never persists a run trace.
        let dir = tempdir().expect("tempdir");
        let setup = agent_setup(dir.path(), "llm-agent-runtime-error");
        let paths = setup.paths.clone();
        let (events, sink) = agent_events_sink();
        let cancel = Arc::new(AtomicBool::new(false));
        run_agent_stream("run-agent-runtime", setup, cancel, &sink);

        let captured = events.lock().expect("lock");
        assert_eq!(captured.len(), 1);
        assert!(matches!(captured[0].chunk, AiChatStreamChunk::Error { .. }));
        drop(captured);
        // No header was opened (the failure preceded begin_agent_run).
        assert!(
            vault_core::load_agent_run(&paths, "run-agent-runtime").expect("load trace").is_none()
        );
    }

    #[cfg(coverage)]
    #[test]
    fn coverage_agent_run_emits_error_when_the_header_open_fails() {
        // A begin_agent_run failure (forced by an unwritable agent.sqlite parent) must surface one
        // terminal Error and abort before any model call — the worker's header-open guard.
        let dir = tempdir().expect("tempdir");
        let mut setup = agent_setup(dir.path(), "llm-agent-header");
        // Point the agent database at a path whose parent is a FILE, so open/create cannot succeed.
        let blocker = dir.path().join("not-a-dir");
        std::fs::write(&blocker, b"x").expect("write blocker file");
        setup.paths.agent_database_path = blocker.join("agent.sqlite");
        let (events, sink) = agent_events_sink();
        let cancel = Arc::new(AtomicBool::new(false));
        run_agent_stream("run-agent-header", setup, cancel, &sink);

        let captured = events.lock().expect("lock");
        assert_eq!(captured.len(), 1);
        assert!(matches!(captured[0].chunk, AiChatStreamChunk::Error { .. }));
    }

    #[cfg(coverage)]
    #[test]
    fn coverage_agent_send_routes_tools_enabled_through_the_agent_path() {
        // End-to-end through `ai_chat_send` with toolsEnabled: resolves the provider, mints a run id,
        // and (under coverage) drives the agent harness inline so the tools_enabled branch + the
        // embedding-optional resolution are exercised. Requires a configured project root.
        let _guard = crate::tests::lock_env();
        let dir = tempdir().expect("tempdir");
        let keyring_root = dir.path().join("test-keyring");
        unsafe {
            std::env::set_var(crate::tests::PROJECT_ROOT_OVERRIDE_ENV, dir.path());
            std::env::set_var(crate::tests::TEST_KEYRING_OVERRIDE_ENV, &keyring_root);
        }
        let config = crate::tests::configured_ai_config();
        let paths = vault_core::project_paths().expect("project paths");
        vault_core::save_config(&paths, &config).expect("save config");
        vault_platform::keyring_set_provider_api_key("llm-primary", "llm-secret")
            .expect("store llm key");

        let (events, sink) = agent_events_sink();
        let ack = ai_chat_send(
            None,
            &AiChatSendRequest {
                provider_id: Some("llm-primary".to_string()),
                messages: vec![AiChatMessage {
                    role: AiChatRole::User,
                    content: "what did i read about example?".to_string(),
                }],
                temperature: Some(0.2),
                max_tokens: Some(64),
                tools_enabled: true,
                conversation_id: None,
                message_id: Some("msg-send".to_string()),
            },
            sink,
        )
        .expect("agent chat send");
        assert!(ack.run_id.starts_with("chat-"));
        let captured = events.lock().expect("lock");
        assert!(!captured.is_empty());
        assert!(captured.iter().all(|event| event.run_id == ack.run_id));
        assert!(matches!(
            captured.last().expect("terminal chunk").chunk,
            AiChatStreamChunk::Done | AiChatStreamChunk::Error { .. }
        ));
        drop(captured);
        unsafe {
            std::env::remove_var(crate::tests::PROJECT_ROOT_OVERRIDE_ENV);
            std::env::remove_var(crate::tests::TEST_KEYRING_OVERRIDE_ENV);
        }
    }
}
