//! Real LM Studio streaming chat e2e for the `RigLlmProvider` adapter (W-AI-1).
//!
//! This is an INTEGRATION test (separate crate), so it links `vault-core` as a normal
//! dependency — the REAL rig network path, not the in-crate `cfg(test)` stub. It is gated on
//! `PATHKEEP_LMSTUDIO_E2E=1` so it never runs in the coverage gate / CI (a missing or unset env
//! var skips it). Run it manually against a local LM Studio:
//!
//! ```sh
//! PATHKEEP_LMSTUDIO_E2E=1 cargo test --manifest-path src-tauri/Cargo.toml \
//!   -p vault-core --test lmstudio_chat_stream_e2e -- --nocapture
//! ```
//!
//! Asserts that `chat_stream` against `google/gemma-4-26b-a4b-qat` yields at least one Token
//! chunk (and reports whether Reasoning chunks arrived, since gemma emits reasoning at the
//! highest level).

use std::future::poll_fn;

use futures_core::Stream;
use vault_core::{
    AiProviderConfig, AiProviderPurpose, AiProviderRuntime, AiRequestFormat, LlmChatRequest,
    LlmMessage, LlmProvider, LlmRole, LlmStreamChunk, RigLlmProvider, SecretString,
};

/// Builds the LM Studio provider runtime described in the AI-redesign LM Studio fixture.
fn lmstudio_runtime() -> AiProviderRuntime {
    AiProviderRuntime {
        config: AiProviderConfig {
            id: "lmstudio-e2e".to_string(),
            name: "LM Studio (e2e)".to_string(),
            purpose: AiProviderPurpose::Llm,
            request_format: AiRequestFormat::LmStudio,
            enabled: true,
            base_url: Some("http://localhost:1234/v1".to_string()),
            default_model: "google/gemma-4-26b-a4b-qat".to_string(),
            temperature: Some(0.6),
            // Generous budget so the model finishes its reasoning AND emits the final answer
            // (gemma can spend hundreds of reasoning tokens before answering).
            max_tokens: Some(2048),
            ..AiProviderConfig::default()
        },
        // LM Studio accepts any non-empty key.
        api_key: SecretString::from("lm-studio".to_string()),
    }
}

#[tokio::test]
async fn lmstudio_chat_stream_yields_tokens_and_maybe_reasoning() {
    if std::env::var("PATHKEEP_LMSTUDIO_E2E").as_deref() != Ok("1") {
        eprintln!(
            "skipping LM Studio e2e: set PATHKEEP_LMSTUDIO_E2E=1 with LM Studio running on :1234"
        );
        return;
    }

    let provider = RigLlmProvider::new(lmstudio_runtime());
    // A step-by-step reasoning prompt: gemma emits `reasoning_content` deltas (which rig maps to
    // ReasoningDelta → our `Reasoning` chunk) before the final answer.
    let request = LlmChatRequest::new(
        vec![
            LlmMessage::new(
                LlmRole::System,
                "You are a careful assistant. Reason step by step before answering.",
            ),
            LlmMessage::new(
                LlmRole::User,
                "Think step by step, then give the final answer: what is 17 times 23?",
            ),
        ],
        Some(0.6),
        Some(2048),
    );

    let mut stream = provider.chat_stream(request).await.expect("open LM Studio stream");

    let mut tokens = 0usize;
    let mut reasoning = 0usize;
    let mut tool_calls = 0usize;
    let mut errors = 0usize;
    let mut token_text = String::new();
    let mut reasoning_text = String::new();

    while let Some(item) = poll_fn(|cx| std::pin::Pin::new(&mut stream).poll_next(cx)).await {
        match item {
            Ok(LlmStreamChunk::Token(text)) => {
                tokens += 1;
                token_text.push_str(&text);
            }
            Ok(LlmStreamChunk::Reasoning(text)) => {
                reasoning += 1;
                reasoning_text.push_str(&text);
            }
            Ok(LlmStreamChunk::ToolCall { .. }) => tool_calls += 1,
            Err(error) => {
                errors += 1;
                eprintln!("stream error: {error}");
            }
        }
    }

    eprintln!(
        "LM Studio e2e: tokens={tokens} reasoning={reasoning} toolCalls={tool_calls} errors={errors}"
    );
    eprintln!("--- answer ---\n{}", token_text.trim());
    if reasoning > 0 {
        eprintln!(
            "--- reasoning (truncated) ---\n{}",
            reasoning_text.chars().take(400).collect::<String>()
        );
    }

    assert!(errors == 0, "stream surfaced {errors} error(s)");
    assert!(tokens > 0, "expected at least one Token chunk from LM Studio");
}
