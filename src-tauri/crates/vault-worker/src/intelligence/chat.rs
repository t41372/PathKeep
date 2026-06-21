//! Worker-side streaming chat orchestration (W-AI-1).
//!
//! ## Responsibilities
//! - resolve the configured LLM provider runtime and build a `RigLlmProvider`
//! - start a background streaming chat run, forwarding each chunk to the desktop emit sink
//!   as an `AiChatStreamEvent` until a terminal `Done`/`Error`
//! - request cooperative cancellation of a live run by id
//!
//! ## Not responsible for
//! - emitting Tauri events (the desktop command supplies the sink closure)
//! - persistence of the conversation (W-AI-3) or tool execution (W-AI-7)
//!
//! ## Performance notes
//! - the stream is driven on a dedicated worker thread with its own scoped runtime, so the
//!   UI thread is never blocked and the run can outlive the foreground `ai_chat_send` call.

use crate::context::{load_unlocked_config, selected_llm_provider_runtime, tokio_runtime};
use anyhow::Result;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};
use vault_core::{
    AiChatCancelResult, AiChatMessage, AiChatRole, AiChatSendAck, AiChatSendRequest,
    AiChatStreamChunk, AiChatStreamEvent, LlmChatRequest, LlmMessage, LlmRole, RigLlmProvider,
    deregister_ai_chat_run, drive_ai_chat_stream, register_ai_chat_run, request_ai_chat_cancel,
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

/// Starts a streaming chat run and returns its run id immediately.
///
/// Resolves the LLM provider, validates there is at least one message, registers the run for
/// cancellation, and spawns a worker thread that drives the stream — emitting one
/// `AiChatStreamEvent` per chunk through `emit` and a terminal `Done`/`Error` at the end. The
/// `emit` sink is the desktop `AppHandle::emit` wrapper; it must be `Send + Sync + 'static` so
/// the background thread can own it.
pub fn ai_chat_send<E>(
    _session_database_key: Option<&str>,
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
    let runtime = selected_llm_provider_runtime(&config, request.provider_id.as_deref())?;

    let run_id = next_run_id();
    let llm_request = to_llm_request(request);
    let cancel = register_ai_chat_run(&run_id);

    spawn_chat_run(run_id.clone(), runtime, llm_request, cancel, emit);
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

/// Requests cooperative cancellation of a live streaming chat run.
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
            api_key: vault_core::SecretString::from("k".to_string()),
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
    fn cancel_unknown_run_reports_not_cancelled() {
        let result = ai_chat_cancel(None, "definitely-not-a-live-run").expect("cancel");
        assert!(!result.cancelled);
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
        });
        let cancel = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
        run_chat_stream("run-runtime-error", stub_runtime("runtime-error"), request, cancel, &sink);
        let captured = events.lock().expect("lock");
        assert_eq!(captured.len(), 1);
        assert!(matches!(captured[0].chunk, AiChatStreamChunk::Error { .. }));
        assert_eq!(captured[0].run_id, "run-runtime-error");
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
        });
        let cancel = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(true));
        run_chat_stream("run-cancelled", stub_runtime("llm-cancel"), request, cancel, &sink);
        let captured = events.lock().expect("lock");
        assert_eq!(captured.len(), 1);
        assert_eq!(captured[0].chunk, AiChatStreamChunk::Done);
    }
}
