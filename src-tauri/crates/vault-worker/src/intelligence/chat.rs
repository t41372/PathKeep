//! Failure modes: chat loads another root, loses cancellation or skips the durable journal before exposing output.
//! Worker-side streaming chat orchestration (W-AI-1) + tool-executing agent harness (W-AI-7).
//!
//! ## Responsibilities
//! - resolve the configured LLM provider runtime and build a `RigLlmProvider`
//! - start a background streaming chat run, forwarding each chunk to the desktop emit sink
//!   as an `AiChatStreamEvent` until a terminal `Done`/`Error`
//! - when `toolsEnabled`, instead drive the durable W-AI-7 agent harness (probe tool capability,
//!   build the owned tool registry, journal every step to `derived/agent.db`, finalize the run)
//! - request cooperative cancellation of a live run by id (shared by both paths)
//!
//! ## Not responsible for
//! - emitting Tauri events (the desktop command supplies the sink closure)
//! - the agent loop / tool execution itself (vault-core `agent_harness` / `agent_tools`)
//! - the agent.db schema/CRUD (vault-core `agent_store`; this only writes through it)
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
    vault: vault_core::Vault,
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
    vault: &vault_core::Vault,
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
    // spawn so no work, network egress, or run id is minted for a refused turn.
    ensure_ai_capability_enabled(&config, AiCapability::Assistant)?;
    let runtime = selected_llm_provider_runtime(&config, request.provider_id.as_deref())?;

    let run_id = next_run_id();
    let cancel = register_ai_chat_run(&run_id);

    if request.tools_enabled {
        // The embedding provider is OPTIONAL: with none configured, search_history still works
        // (its hybrid plane degrades to lexical honestly). A resolution failure (e.g. missing key)
        // is not fatal to the agent run, so treat it as "no embedding provider" rather than aborting.
        let embedding_runtime = selected_optional_embedding_runtime(&config).ok().flatten();
        let setup = AgentRunSetup {
            vault: vault.clone(),
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
/// place. The run is deregistered
/// via a [`RunGuard`] constructed INSIDE the driven body, so it fires on both normal return and a
/// panic. If the thread cannot be spawned at all, the run was never driven: this path emits one
/// terminal `Error` event and deregisters explicitly (the guard never ran), so the front end —
/// which is subscribed by run id — sees a terminal chunk instead of hanging forever.
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
    let stream_runtime = match tokio_runtime() {
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

/// Durable journal that writes the harness's steps/citations to `derived/agent.db` (W-AI-7).
///
/// JOURNAL-BEFORE-OBSERVE: the harness calls these BEFORE emitting the matching chunk, so a crash
/// mid-run leaves an interrupted partial trace and a resume = replay (never a model re-call). The
/// `run_id` anchors every write to the run header opened by [`spawn_agent_run`]. Citations are
/// converted to the canonical-url-keyed record (the W-STAR key) here, deriving the canonical url
/// from each citation's raw url so WU-6 can star a cited page.
struct AgentVaultJournal {
    vault: vault_core::Vault,
    run_id: String,
}

impl AgentJournal for AgentVaultJournal {
    fn journal_step(
        &self,
        turn: u32,
        kind: &str,
        tool_name: Option<&str>,
        tool_call_id: Option<&str>,
        payload: &str,
    ) -> Result<()> {
        append_agent_step(
            &self.vault,
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
                profile_id: Some(citation.profile_id.clone()).filter(|id| !id.is_empty()),
            })
            .collect();
        record_agent_citations(&self.vault, &self.run_id, &records)?;
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

    let stream_runtime = match tokio_runtime() {
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
    if let Err(error) = begin_agent_run(&setup.vault, &begin) {
        return emit_error(error.to_string());
    }

    let provider = RigLlmProvider::new(setup.llm_runtime.clone());
    let registry = ToolRegistry::with_default_search_tools();
    let journal = AgentVaultJournal { vault: setup.vault.clone(), run_id: run_id.to_string() };
    let control: Arc<dyn AiRunControl> = Arc::new(WorkerCancelControl { cancel });
    let tool_context = AgentToolContext {
        vault: setup.vault.clone(),
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
        // Resolve the host's current UTC offset ONCE per run so the tools render visit timestamps as
        // LOCAL date/time (the model has no clock; the sandbox reads 0).
        time_zone: vault_core::utils::computer_time_zone(),
    };

    // Inject the host-computed system context (current date/time/timezone + archive span) as the
    // FIRST message of every agent run. The model has no clock inside the sandbox and only knows its
    // training cutoff, so without this it guesses the wrong year for "last Friday", searches empty
    // date ranges, and loops. The facts are resolved on the host (OS clock/timezone + a bounded,
    // cached archive read) and prepended ahead of any existing leading system message so it is part
    // of the first turn but never displaces the caller's own system guidance.
    let mut request = setup.request;
    let system_context = vault_core::build_agent_system_context(
        &vault_core::resolve_agent_system_context(&setup.vault),
    );
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
        &setup.vault,
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

    #[test]
    fn chat_send_refuses_when_master_ai_or_assistant_consent_is_off() {
        let vault = vault_core::Vault::open_temp().unwrap();
        // A non-empty request with a configured provider+key must STILL refuse while the master AI
        // switch (or the assistant sub-flag) is off — a previously-configured provider must not let
        // the agent/streaming run fire with consent withdrawn. The gate bails before any
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
        let error = ai_chat_send(&vault, None, &request, |_| panic!("sink must never be reached"))
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
        let error = ai_chat_send(&vault, None, &request, |_| panic!("sink must never be reached"))
            .expect_err("assistant sub-flag off must refuse");
        assert!(error.to_string().contains("assistant"), "names the assistant capability");

        unsafe {
            std::env::remove_var(crate::tests::PROJECT_ROOT_OVERRIDE_ENV);
            std::env::remove_var(crate::tests::TEST_KEYRING_OVERRIDE_ENV);
        }
    }
}
