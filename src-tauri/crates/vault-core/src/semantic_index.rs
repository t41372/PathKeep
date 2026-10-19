//! Responsibilities: choose a serving index and expose its durable build state.
//! Not responsible for: secrets, model requests, queue mutations or archive scans.
use crate::{AppConfig, EmbeddingIndexRef, IndexBuildJob, SemanticIndexState, Vault, ai_queue};
use anyhow::Result;
use vault_store::{Db, OptionalRow, Session, params_owned as params};

/// Resolves selection without opening a provider or reading secrets.
pub fn chosen(config: &AppConfig) -> Option<EmbeddingIndexRef> {
    let id = config.ai.embedding_provider_id.as_deref()?;
    let provider = config.ai.embedding_providers.iter().find(|p| p.id == id)?;
    Some(EmbeddingIndexRef { provider_id: id.to_owned(), model: provider.default_model.clone() })
}

pub(crate) fn complete(s: &Session<'_>, pair: &EmbeddingIndexRef) -> Result<bool> {
    Ok(s.query_one(
        "SELECT state FROM ai_index_ledger WHERE provider_id=?1 AND model=?2",
        params![pair.provider_id, pair.model],
        |r| Ok(r.get::<String>(0)?),
    )
    .optional()?
    .as_deref()
        == Some("ready"))
}

/// Uses ledger primary-key prefixes and the active-state job index, never the archive or vectors.
pub fn load_state(vault: &Vault) -> Result<SemanticIndexState> {
    let config = vault.config().clone();
    vault.read(Db::Intelligence, move |s| load_state_from(s, &config))
}

pub(crate) fn load_state_from(s: &Session<'_>, config: &AppConfig) -> Result<SemanticIndexState> {
    let chosen = chosen(config);
    let chosen_complete =
        chosen.as_ref().map(|pair| complete(s, pair)).transpose()?.unwrap_or(false);
    let serving = if chosen_complete {
        chosen.clone()
    } else {
        let mut completed = Vec::new();
        for provider in &config.ai.embedding_providers {
            completed.extend(s.query(
                "SELECT model,build_finished_at FROM ai_index_ledger WHERE provider_id=?1 AND state='ready' AND build_finished_at IS NOT NULL",
                [provider.id.as_str()], |r| Ok((r.get::<String>(1)?, EmbeddingIndexRef {
                    provider_id: provider.id.clone(), model: r.get(0)?,
                })))?);
        }
        completed.sort_by(|a, b| {
            b.0.cmp(&a.0)
                .then_with(|| a.1.provider_id.cmp(&b.1.provider_id))
                .then_with(|| a.1.model.cmp(&b.1.model))
        });
        completed.into_iter().next().map(|(_, pair)| pair).or_else(|| chosen.clone())
    };
    let build = ai_queue::index_control::active_jobs(s)?.into_iter().find_map(|(job, payload)| {
        let pair = ai_queue::index_control::payload_pair(&payload, config);
        if chosen.is_none() || pair != chosen {
            return None;
        }
        let ai_queue::AiJobPayload::Index { cursor, .. } = payload else {
            return None;
        };
        Some(IndexBuildJob {
            job_id: job.id,
            state: if job.state == "stale" { "queued".into() } else { job.state },
            paused_by: job.paused_by,
            pause_message: job.error_message,
            done: cursor.embedded_so_far,
            total: cursor.embed_target,
        })
    });
    Ok(SemanticIndexState { chosen, serving, chosen_complete, build })
}
