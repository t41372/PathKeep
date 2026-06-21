//! Real LM Studio embedding e2e for the `ExternalEmbeddingProvider` adapter (W-AI-4a).
//!
//! This is an INTEGRATION test (separate crate), so it links `vault-core` as a normal dependency
//! — the REAL reqwest `/v1/embeddings` network path, not the in-crate `cfg(test)` stub. It is
//! gated on `PATHKEEP_LMSTUDIO_E2E=1` so it never runs in the coverage gate / CI (a missing or
//! unset env var skips it). Run it manually against a local LM Studio:
//!
//! ```sh
//! PATHKEEP_LMSTUDIO_E2E=1 cargo test --manifest-path src-tauri/Cargo.toml \
//!   -p vault-core --test lmstudio_embedding_e2e -- --nocapture
//! ```
//!
//! Asserts that `embed` against `text-embedding-qwen3-embedding-0.6b` returns a non-empty vector
//! of consistent length, near-unit L2 norm (the adapter normalizes defensively), and that two
//! different texts produce different vectors.

use vault_core::{
    AiProviderConfig, AiProviderPurpose, AiProviderRuntime, AiRequestFormat, EmbeddingProvider,
    EmbeddingRole, ExternalEmbeddingProvider, SecretString,
};

/// Builds the LM Studio embedding provider runtime from the AI-redesign LM Studio fixture.
fn lmstudio_embedding_runtime() -> AiProviderRuntime {
    AiProviderRuntime {
        config: AiProviderConfig {
            id: "lmstudio-embed-e2e".to_string(),
            name: "LM Studio Embedding (e2e)".to_string(),
            purpose: AiProviderPurpose::Embedding,
            request_format: AiRequestFormat::LmStudio,
            enabled: true,
            base_url: Some("http://localhost:1234/v1".to_string()),
            default_model: "text-embedding-qwen3-embedding-0.6b".to_string(),
            // No explicit dimension → request the model's native size (D4); we read the truth back.
            dimensions: None,
            ..AiProviderConfig::default()
        },
        // LM Studio accepts any non-empty key.
        api_key: SecretString::from("lm-studio".to_string()),
    }
}

fn l2_norm(vector: &[f32]) -> f32 {
    vector.iter().map(|value| value * value).sum::<f32>().sqrt()
}

#[tokio::test]
async fn lmstudio_embedding_returns_consistent_normalized_distinct_vectors() {
    if std::env::var("PATHKEEP_LMSTUDIO_E2E").as_deref() != Ok("1") {
        eprintln!(
            "skipping LM Studio embedding e2e: set PATHKEEP_LMSTUDIO_E2E=1 with LM Studio on :1234"
        );
        return;
    }

    let provider =
        ExternalEmbeddingProvider::new(lmstudio_embedding_runtime()).expect("build provider");

    let texts = vec![
        "the quick brown fox jumps over the lazy dog".to_string(),
        "a treatise on the macroeconomics of central banking".to_string(),
        "the quick brown fox jumps over the lazy dog".to_string(),
    ];
    let vectors =
        provider.embed(&texts, EmbeddingRole::Document).await.expect("embed against LM Studio");

    assert_eq!(vectors.len(), texts.len(), "one vector per input");
    let dim = vectors[0].len();
    assert!(dim > 0, "expected a non-empty embedding vector");

    eprintln!("LM Studio embedding e2e: observed dim = {dim}");
    let sample: Vec<f32> = vectors[0].iter().take(8).copied().collect();
    eprintln!("--- sample (first 8 components of vector 0) ---\n{sample:?}");

    for (index, vector) in vectors.iter().enumerate() {
        assert_eq!(vector.len(), dim, "vector {index} has inconsistent length");
        let norm = l2_norm(vector);
        assert!(
            (norm - 1.0).abs() < 1e-3,
            "vector {index} should be near unit L2 norm, got {norm}"
        );
    }

    // Two semantically different texts differ; the identical text (index 0 vs 2) matches.
    assert_ne!(vectors[0], vectors[1], "different texts must produce different vectors");
    assert_eq!(vectors[0], vectors[2], "identical text must produce identical vectors");

    // A query-role embedding of the same text is still a valid unit vector (role is a no-op for
    // the OpenAI-compatible path, but the call must succeed and stay normalized).
    let query = provider
        .embed(&["the quick brown fox".to_string()], EmbeddingRole::Query)
        .await
        .expect("embed query");
    assert_eq!(query[0].len(), dim, "query embedding shares the model dimension");
    assert!((l2_norm(&query[0]) - 1.0).abs() < 1e-3, "query embedding near unit norm");
}
