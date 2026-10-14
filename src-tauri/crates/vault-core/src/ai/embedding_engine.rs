//! Responsibilities: pick the embedding engine for a resolved provider, and dispatch to it without
//! boxing: the in-app static model (`embedding_static`) for the in-app provider, the external
//! OpenAI-compatible adapter (`embedding_external`) for any other.
//! Not responsible for: what either engine does, provider config and secrets (the worker resolves
//! the runtime), or the embed loop and its queue (`indexing`).

use super::embedding_external::ExternalEmbeddingProvider;
use super::embedding_static::{
    StaticEmbeddingProvider, runtime_uses_static, select_static_embedding_provider,
};
use super::traits::{EmbeddingDescriptor, EmbeddingProvider, EmbeddingRole};
use crate::AiProviderRuntime;
use crate::config::ProjectPaths;
use anyhow::Result;

/// The engine for one resolved provider: the in-app static model when the provider is the in-app
/// one (its base URL is [`super::embedding_static::STATIC_INAPP_BASE_URL`]), the external adapter
/// for any other. A static model that is not downloaded yet is an error the caller turns into a
/// download prompt rather than a failed job.
pub fn select_embedding_provider(
    paths: &ProjectPaths,
    runtime: &AiProviderRuntime,
) -> Result<AnyEmbeddingProvider> {
    if runtime_uses_static(runtime) {
        return select_static_embedding_provider(paths, runtime);
    }
    Ok(AnyEmbeddingProvider::External(ExternalEmbeddingProvider::new(runtime.clone())?))
}

/// The embedding engines, dispatched by `match`.
///
/// Not `Box<dyn EmbeddingProvider>`: the trait's `embed` returns `impl Future` (monomorphized for
/// the 14.4M-row embed loop), so it is not `dyn`-compatible, and boxing would add an allocation
/// per call.
pub enum AnyEmbeddingProvider {
    /// OpenAI-compatible `/v1/embeddings` external provider.
    External(ExternalEmbeddingProvider),
    /// The in-app static (model2vec) engine: symmetric, mean-pooled, L2-normalized vectors from a
    /// static embedding matrix, with no network. Boxed because the loaded matrix is large and the
    /// enum is moved on every iteration of the embed loop.
    Static(Box<StaticEmbeddingProvider>),
}

impl EmbeddingProvider for AnyEmbeddingProvider {
    // `async move` (not `async fn`): each variant's `embed` returns a distinct future type, so a
    // `match` cannot return them directly without boxing; one `async move` keeps the dispatch
    // allocation-free.
    #[allow(clippy::manual_async_fn)]
    fn embed(
        &self,
        texts: &[String],
        role: EmbeddingRole,
    ) -> impl std::future::Future<Output = Result<Vec<Vec<f32>>>> + Send {
        async move {
            match self {
                Self::External(provider) => provider.embed(texts, role).await,
                Self::Static(provider) => provider.embed(texts, role).await,
            }
        }
    }

    fn model_id(&self) -> &str {
        match self {
            Self::External(provider) => provider.model_id(),
            Self::Static(provider) => provider.model_id(),
        }
    }

    fn descriptor(&self) -> EmbeddingDescriptor {
        match self {
            Self::External(provider) => provider.descriptor(),
            Self::Static(provider) => provider.descriptor(),
        }
    }
}
