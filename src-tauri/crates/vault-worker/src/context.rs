//! Shared worker context helpers.
//!
//! This module keeps the repetitive desktop-worker setup in one place:
//! project-path resolution, config hydration, App Lock enforcement, provider
//! lookup, and AI queue/runtime helpers.
//!
//! The worker crate sits between the desktop/Tauri facade and `vault-core`.
//! It is allowed to compose platform adapters and canonical domain functions,
//! but it should not quietly redefine the product contract. The helpers here
//! therefore preserve a few important invariants from the accepted docs:
//!
//! - App Lock is a session boundary, not archive encryption.
//! - Provider/keyring truth comes from Settings + native stores, not ad-hoc
//!   environment state.
//! - AI/semantic side effects still run against the canonical archive ledger,
//!   so schema guards must be in place before queue work starts.
//! - Worker commands should fail with actionable messages instead of raw
//!   transport or provider errors whenever we can shape them honestly.

use anyhow::{Context, Result};
use tokio::runtime::Runtime;
use vault_core::{
    AiIndexStatus, AiProviderConfig, AiProviderPurpose, AiProviderRuntime, AiSearchResponse,
    AppConfig, AppLockStatus, IntelligenceStatus, ai_index_status, ai_queue,
    app_lock_status_with_biometric, archive, ensure_app_lock_unlocked, hydrate_app_lock_config,
    intelligence_status, load_config,
};
use vault_platform::{
    app_lock_biometric_state, keyring_get_provider_api_key, provider_api_key_saved,
};

/// Builds the short-lived Tokio runtime used by blocking worker entrypoints.
///
/// The worker intentionally stays synchronous at its public boundary so the
/// desktop process can call it from CLI/Tauri code without adopting a global
/// async runtime. Each long-running AI operation creates a scoped runtime only
/// for the duration of that task.
pub(crate) fn tokio_runtime() -> Result<Runtime> {
    Runtime::new().context("creating tokio runtime for PathKeep worker")
}

/// Marks providers with whether their secrets currently exist in native storage.
///
/// This derived flag is part of the Settings/read-model honesty layer. The
/// secret itself stays in the system keyring; config only records whether the
/// user can expect that key to be available.
pub(crate) fn hydrate_provider_collection(providers: &mut [AiProviderConfig]) {
    for provider in providers {
        provider.api_key_saved = provider_api_key_saved(&provider.id);
    }
}

/// Fills the config fields that are derived from native stores instead of JSON.
pub(crate) fn hydrate_derived_config_state(config: &mut AppConfig) {
    hydrate_provider_collection(&mut config.ai.llm_providers);
    hydrate_provider_collection(&mut config.ai.embedding_providers);
}

/// Loads config and enriches it with native/keyring-backed state.
pub(crate) fn load_hydrated_config(paths: &vault_core::ProjectPaths) -> Result<AppConfig> {
    let mut config = load_config(paths)?;
    hydrate_derived_config_state(&mut config);
    hydrate_app_lock_config(paths, &mut config)?;
    Ok(config)
}

/// Loads config and enforces the current App Lock session boundary.
pub(crate) fn load_unlocked_config(paths: &vault_core::ProjectPaths) -> Result<AppConfig> {
    let config = load_hydrated_config(paths)?;
    ensure_app_lock_unlocked(paths, &config)?;
    Ok(config)
}

/// Reads the current host biometric capability snapshot.
pub(crate) fn current_app_lock_biometric_state() -> vault_core::AppLockBiometricState {
    app_lock_biometric_state()
}

/// Resolves the App Lock status with the current biometric capability folded in.
pub(crate) fn resolved_app_lock_status(
    paths: &vault_core::ProjectPaths,
    config: &AppConfig,
) -> Result<AppLockStatus> {
    app_lock_status_with_biometric(paths, config, current_app_lock_biometric_state())
}

/// Loads the semantic-index read model, degrading to an honest warning on error.
pub(crate) fn derive_ai_status(
    paths: &vault_core::ProjectPaths,
    config: &AppConfig,
    session_database_key: Option<&str>,
) -> AiIndexStatus {
    match ai_index_status(paths, config, session_database_key) {
        Ok(status) => status,
        Err(error) => AiIndexStatus {
            enabled: config.ai.enabled,
            assistant_enabled: config.ai.assistant_enabled,
            mcp_enabled: config.ai.mcp_enabled,
            skill_enabled: config.ai.skill_enabled,
            llm_provider_id: config.ai.llm_provider_id.clone(),
            embedding_provider_id: config.ai.embedding_provider_id.clone(),
            warning: Some(error.to_string()),
            ..AiIndexStatus::default()
        },
    }
}

/// Loads the derived-intelligence read model, degrading to a warning on error.
pub(crate) fn derive_intelligence_status(
    paths: &vault_core::ProjectPaths,
    config: &AppConfig,
    session_database_key: Option<&str>,
) -> IntelligenceStatus {
    match intelligence_status(paths, config, session_database_key) {
        Ok(status) => status,
        Err(error) => {
            IntelligenceStatus { warning: Some(error.to_string()), ..IntelligenceStatus::default() }
        }
    }
}

/// Resolves a provider config plus its native secret into a runtime payload.
///
/// This keeps the worker honest about capability mismatches: if a provider is
/// disabled, assigned to the wrong purpose, or missing its API key, the worker
/// refuses before any network call starts.
pub(crate) fn resolve_provider_runtime(
    providers: &[AiProviderConfig],
    provider_id: &str,
    expected_purpose: AiProviderPurpose,
) -> Result<AiProviderRuntime> {
    let config = providers
        .iter()
        .find(|provider| provider.id == provider_id)
        .cloned()
        .with_context(|| format!("provider {provider_id} was not found in Settings"))?;
    // Honor the per-provider on/off: a provider the user disabled in Settings must never be
    // selected for a run, even when it is still the configured default and holds a stored key.
    // The "enable provider" wording is load-bearing — `queue_failure_from_error` maps it to the
    // `provider-disabled` queue code so a disabled provider surfaces as manual-review, not a retry.
    if !config.enabled {
        anyhow::bail!(
            "Provider {} is turned off in Settings — enable provider {} before using it.",
            config.name,
            config.name
        );
    }
    if config.purpose != expected_purpose {
        anyhow::bail!(
            "Provider {} is configured for {:?}, not {:?}.",
            config.name,
            config.purpose,
            expected_purpose
        );
    }
    let api_key = keyring_get_provider_api_key(provider_id)?
        .with_context(|| format!("store an API key for provider {}", config.name))?;
    Ok(AiProviderRuntime { config, api_key: api_key.into() })
}

/// Resolves the embedding provider selected for a build/search request.
pub(crate) fn selected_embedding_provider_runtime(
    config: &AppConfig,
    preferred_id: Option<&str>,
) -> Result<AiProviderRuntime> {
    let provider_id = preferred_id
        .or(config.ai.embedding_provider_id.as_deref())
        .context("select an embedding provider in Settings before building the semantic index")?;
    resolve_provider_runtime(
        &config.ai.embedding_providers,
        provider_id,
        AiProviderPurpose::Embedding,
    )
}

/// Resolves the LLM provider selected for an assistant request.
pub(crate) fn selected_llm_provider_runtime(
    config: &AppConfig,
    preferred_id: Option<&str>,
) -> Result<AiProviderRuntime> {
    let provider_id = preferred_id
        .or(config.ai.llm_provider_id.as_deref())
        .context("select an LLM provider in Settings before using the assistant")?;
    resolve_provider_runtime(&config.ai.llm_providers, provider_id, AiProviderPurpose::Llm)
}

/// Resolves the currently configured embedding provider when semantic search is optional.
pub(crate) fn selected_optional_embedding_runtime(
    config: &AppConfig,
) -> Result<Option<AiProviderRuntime>> {
    match config.ai.embedding_provider_id.as_deref() {
        Some(provider_id) => resolve_provider_runtime(
            &config.ai.embedding_providers,
            provider_id,
            AiProviderPurpose::Embedding,
        )
        .map(Some),
        None => Ok(None),
    }
}

/// Adds an explicit lexical-fallback note when semantic provider resolution fails.
pub(crate) fn search_response_with_resolution_note(
    mut response: AiSearchResponse,
    resolution_error: Option<anyhow::Error>,
) -> AiSearchResponse {
    if let Some(error) = resolution_error {
        response.notes.push(format!(
            "Semantic retrieval is unavailable right now: {}. Showing lexical results only.",
            error
        ));
    }
    response
}

/// Opens the rebuildable intelligence storage plane for worker-owned AI state.
pub(crate) fn ai_archive_connection(
    paths: &vault_core::ProjectPaths,
    config: &AppConfig,
    session_database_key: Option<&str>,
) -> Result<rusqlite::Connection> {
    archive::open_intelligence_connection(paths, config, session_database_key)
}

/// Looks up the provider config referenced by a request before resolving secrets.
pub(crate) fn provider_config_for_request(
    config: &AppConfig,
    provider_id: Option<&str>,
    purpose: AiProviderPurpose,
) -> Result<AiProviderConfig> {
    let (provider_id, providers, empty_message) = match purpose {
        AiProviderPurpose::Embedding => (
            provider_id.or(config.ai.embedding_provider_id.as_deref()),
            &config.ai.embedding_providers,
            "select an embedding provider in Settings before building the semantic index",
        ),
        AiProviderPurpose::Llm => (
            provider_id.or(config.ai.llm_provider_id.as_deref()),
            &config.ai.llm_providers,
            "select an LLM provider in Settings before using the assistant",
        ),
    };
    let provider_id = provider_id.context(empty_message)?;
    providers
        .iter()
        .find(|provider| provider.id == provider_id)
        .cloned()
        .with_context(|| format!("provider {provider_id} was not found in Settings"))
}

/// Normalizes raw AI/provider failures into queue retry semantics.
///
/// Queue controls are part of the product contract: transient network or quota
/// failures can requeue with backoff, while configuration/purpose mismatches
/// must stop and surface as manual-review work.
pub(crate) fn queue_failure_from_error(error: &anyhow::Error) -> ai_queue::AiJobFailure {
    let message = error.to_string();
    let lower = message.to_lowercase();
    if lower.contains("rate limit") || lower.contains("quota") || lower.contains("429") {
        return ai_queue::AiJobFailure {
            error_code: Some("rate-limited".to_string()),
            error_message: message,
            retryable: true,
            retry_after_seconds: 300,
            summary: Some("Provider quota window has not reset yet.".to_string()),
        };
    }
    if lower.contains("timed out")
        || lower.contains("dns")
        || lower.contains("network")
        || lower.contains("refused")
    {
        return ai_queue::AiJobFailure {
            error_code: Some("network-error".to_string()),
            error_message: message,
            retryable: true,
            retry_after_seconds: 30,
            summary: Some("Retrying the AI job after a transient network failure.".to_string()),
        };
    }
    let error_code = if lower.contains("api key") || lower.contains("store an api key") {
        Some("secret-missing".to_string())
    } else if lower.contains("model") && lower.contains("not found") {
        Some("bad-model".to_string())
    } else if lower.contains("enable provider") {
        Some("provider-disabled".to_string())
    } else if lower.contains("not configured for") || lower.contains("does not support") {
        Some("unsupported-capability".to_string())
    } else {
        Some("provider-error".to_string())
    };
    ai_queue::AiJobFailure {
        error_code,
        error_message: message,
        retryable: false,
        retry_after_seconds: 0,
        summary: Some("This AI job needs manual review before it can be replayed.".to_string()),
    }
}
