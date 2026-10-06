//! Responsibilities: preserve assistant orchestration, prompt composition and run traces.
//! Not responsible for: search indexing, candidate pools or archive search hydration.
//! Failure modes: citations or cancellation vanish during retrieval extraction.
//! Tests: existing assistant/agent contract suites.
use super::native_ledger::{begin_ai_run, finalize_ai_run_failure, finalize_ai_run_success};
use super::search::{
    MODEL_FACING_ROW_CAP, SearchContext, SearchHistoryTool, search_history_internal,
};
use super::*;
use vault_store::{Db, Priority};

/// Answers one user question against archive history with evidence-backed citations.
pub async fn answer_history_question(
    vault: &crate::Vault,
    paths: &ProjectPaths,
    config: &AppConfig,
    key: Option<&str>,
    llm_provider: &AiProviderRuntime,
    embedding_provider: Option<&AiProviderRuntime>,
    request: &AiAssistantRequest,
) -> Result<AiAssistantResponse> {
    answer_history_question_with_control(
        vault,
        paths,
        config,
        key,
        llm_provider,
        embedding_provider,
        request,
        None,
    )
    .await
}

/// Answers one assistant question while honoring optional cooperative stop checkpoints.
pub async fn answer_history_question_with_control(
    vault: &crate::Vault,
    paths: &ProjectPaths,
    config: &AppConfig,
    key: Option<&str>,
    llm_provider: &AiProviderRuntime,
    embedding_provider: Option<&AiProviderRuntime>,
    request: &AiAssistantRequest,
    run_control: Option<Arc<dyn AiRunControl>>,
) -> Result<AiAssistantResponse> {
    validate_provider(llm_provider, AiProviderPurpose::Llm)?;
    if !config.ai.enabled || !config.ai.assistant_enabled {
        anyhow::bail!("Enable AI analysis and the assistant in Settings before asking questions.")
    }
    let run_id = begin_ai_run(
        vault,
        "assistant",
        "manual",
        json!({
            "providerId": llm_provider.config.id,
            "embeddingProviderId": embedding_provider
                .map(|provider| provider.config.id.clone())
                .unwrap_or_else(|| "lexical-fallback".to_string()),
            "questionLength": request.question.len(),
        }),
    )?;

    let result: Result<AiAssistantResponse> = async {
        let retrieval_request = AiSearchRequest {
            query: request.question.clone(),
            profile_id: request.profile_id.clone(),
            domain: request.domain.clone(),
            // CONTEXT CAP: the seeded evidence is rendered into the model's preamble, so cap the page
            // at the model-facing row limit (the retrieval ceiling [`MAX_SEARCH_ROWS`] is for the
            // sandbox, whose rows never reach the model). Preserves the prior `clamp(1, 50)` behavior.
            limit: Some(config.ai.retrieval_top_k.clamp(1, MODEL_FACING_ROW_CAP)),
            cursor: None,
            sort: None,
            starred_only: None,
            start_date: None,
            end_date: None,
        };
        let run_control = run_control.as_ref();
        let search_response = await_with_ai_cancellation(
            run_control,
            "Assistant run was cancelled before retrieval finished.",
            search_history_internal(vault, paths, config, key, embedding_provider, &retrieval_request),
        )
        .await?;
        let seeded_citations = search_response
            .items
            .iter()
            .map(|item| AiCitation {
                history_id: item.history_id,
                profile_id: item.profile_id.clone(),
                url: item.url.clone(),
                title: item.title.clone(),
                visited_at: item.visited_at.clone(),
                score: Some(item.score),
                canonical_url: None,
            })
            .collect::<Vec<_>>();
        let citations = Arc::new(Mutex::new(seeded_citations.clone()));
        let tool_context = SearchContext {
            vault: vault.clone(),
            paths: paths.clone(),
            config: config.clone(),
            database_key: key.map(ToOwned::to_owned),
            embedding_provider: embedding_provider.cloned(),
            default_profile_id: request.profile_id.clone(),
            default_domain: request.domain.clone(),
            default_limit: config.ai.retrieval_top_k.max(1),
            citations: Arc::clone(&citations),
            run_control: run_control.cloned(),
        };
        let tools: Vec<Box<dyn ToolDyn>> =
            vec![Box::new(SearchHistoryTool { context: tool_context })];
        let preamble = build_assistant_preamble(config, &search_response);
        let answer = await_with_ai_cancellation(
            run_control,
            "Assistant run was cancelled while waiting for the model response.",
            run_llm_agent(llm_provider, &preamble, tools, &request.question),
        )
        .await?;

        let mut final_citations = citations.lock().await.clone();
        final_citations.sort_by_key(|item| item.history_id);
        final_citations.dedup_by_key(|item| item.history_id);

        let embedding_provider_id = embedding_provider
            .map(|provider| provider.config.id.clone())
            .unwrap_or_else(|| "lexical-fallback".to_string());
        let final_answer = if final_citations.is_empty() {
            "I couldn't find enough matching history evidence to answer that confidently yet. Try narrowing the profile or domain, or rebuild the semantic index and ask again.".to_string()
        } else {
            answer
        };
        #[rustfmt::skip]
        record_assistant_run(
            vault,
            run_id,
            request,
            &final_answer,
            &llm_provider.config.id,
            &embedding_provider_id,
            &final_citations,
            &search_response.notes,
        )?;

        Ok(AiAssistantResponse {
            state: if final_citations.is_empty() {
                "insufficient-evidence".to_string()
            } else {
                "completed".to_string()
            },
            answer: final_answer,
            job_id: None,
            run_id: Some(run_id),
            provider_id: llm_provider.config.id.clone(),
            embedding_provider_id,
            citations: final_citations,
            notes: search_response.notes,
        })
    }
    .await;

    match result {
        Ok(response) => {
            finalize_ai_run_success(
                vault,
                run_id,
                json!({
                    "providerId": response.provider_id,
                    "embeddingProviderId": response.embedding_provider_id,
                    "citations": response.citations.len(),
                }),
            )?;
            Ok(response)
        }
        Err(error) => {
            finalize_ai_run_failure(
                vault,
                run_id,
                &error.to_string(),
                json!({
                    "providerId": llm_provider.config.id,
                    "embeddingProviderId": embedding_provider
                        .map(|provider| provider.config.id.clone())
                        .unwrap_or_else(|| "lexical-fallback".to_string()),
                }),
            )?;
            Err(error)
        }
    }
}

/// Builds the assistant preamble from the retrieval seed set and system prompt.
pub(super) fn build_assistant_preamble(
    config: &AppConfig,
    search_response: &AiSearchResponse,
) -> String {
    let context = search_response
        .items
        .iter()
        .enumerate()
        .map(|(index, item)| {
            format!(
                "[{index}] {visited_at} | {profile_id} | {url}\nTitle: {title}\nMatch: {reason}\nScore: {score:.3}",
                index = index + 1,
                visited_at = item.visited_at,
                profile_id = item.profile_id,
                url = item.url,
                title = item.title.clone().unwrap_or_else(|| "(untitled)".to_string()),
                reason = item.match_reason,
                score = item.score
            )
        })
        .collect::<Vec<_>>()
        .join("\n\n");

    format!(
        "{system_prompt}\n\nYou are working inside PathKeep. Always ground answers in the history evidence below or by calling the search_history tool. Cite the visit date, profile, and URL you relied on. If the evidence is incomplete, say so.\n\nInitial evidence:\n{context}",
        system_prompt = config.ai.assistant_system_prompt,
        context = if context.is_empty() {
            "No indexed evidence was found. Use the search_history tool or explain that the archive has no matching records.".to_string()
        } else {
            context
        }
    )
}

/// Persists one assistant run trace after the final answer is known.
pub(super) fn record_assistant_run(
    vault: &crate::Vault,
    run_id: i64,
    request: &AiAssistantRequest,
    answer: &str,
    llm_provider_id: &str,
    embedding_provider_id: &str,
    citations: &[AiCitation],
    notes: &[String],
) -> Result<()> {
    let values = vault_store::params_owned![
        run_id,
        request.question,
        answer,
        llm_provider_id,
        embedding_provider_id,
        serde_json::to_string(citations)?,
        serde_json::to_string(notes)?,
        now_rfc3339()
    ];
    vault.write(Db::Intelligence, Priority::Interactive, move |session| {
        session.execute(INSERT_ASSISTANT_RUN_SQL, values)?;
        Ok(())
    })
}
