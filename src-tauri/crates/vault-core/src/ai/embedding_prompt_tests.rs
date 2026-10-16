//! Responsibilities: pin model aliases, exact prompt strings and the query dispatch boundary.
//! Not responsible for: page extraction, provider networking or indexing.
//! Failure modes: aliases miss their family; provider names influence selection; incidental
//! letters match E5; query spacing/punctuation changes; missing titles break Gemma; query-only
//! changes invalidate documents; plain documents gain blank lines or labels.
use super::*;

#[test]
fn model_aliases_choose_their_prompt_profile() {
    for (ids, expected) in [
        (
            vec![
                "Qwen/Qwen3-Embedding-0.6B",
                "text-embedding-qwen3-embedding-8b",
                "qwen3-embedding:4b",
            ],
            PromptProfile::Qwen3,
        ),
        (
            vec![
                "google/embeddinggemma-300m",
                "embeddinggemma-2",
                "text-embedding-embeddinggemma-300m-qat",
            ],
            PromptProfile::EmbeddingGemma,
        ),
        (vec!["nomic-embed-text", "nomic-ai/nomic-embed-text-v1.5"], PromptProfile::Nomic),
        (vec!["intfloat/multilingual-e5-large-instruct", "E5_INSTRUCT"], PromptProfile::E5Instruct),
        (
            vec![
                "intfloat/multilingual-e5-large",
                "intfloat/e5-base-v2",
                "multilingual-e5-small",
                "e5.base",
            ],
            PromptProfile::E5,
        ),
        (vec!["mxbai-embed-large"], PromptProfile::Mxbai),
        (
            vec![
                "text-embedding-3-large",
                "bge-m3",
                "gte-base",
                "some5-model",
                "potion-multilingual-128M",
            ],
            PromptProfile::Plain,
        ),
    ] {
        for id in ids {
            assert_eq!(PromptProfile::for_model(id), expected, "{id}");
        }
    }
}

#[test]
fn profiles_render_exact_model_inputs_and_stable_document_formats() {
    let document = EmbeddingDocument { title: Some("Title".into()), body: "Body".into() };
    for (profile, query, text, format) in [
        (PromptProfile::Plain, "find", "Title\nBody", "plain"),
        (
            PromptProfile::Qwen3,
            &format!("Instruct: {SEARCH_TASK}\nQuery:find"),
            "Title\nBody",
            "plain",
        ),
        (
            PromptProfile::EmbeddingGemma,
            "task: search result | query: find",
            "title: Title | text: Body",
            "embeddinggemma",
        ),
        (PromptProfile::Nomic, "search_query: find", "search_document: Title\nBody", "nomic"),
        (
            PromptProfile::E5Instruct,
            &format!("Instruct: {SEARCH_TASK}\nQuery: find"),
            "Title\nBody",
            "plain",
        ),
        (PromptProfile::E5, "query: find", "passage: Title\nBody", "e5"),
        (
            PromptProfile::Mxbai,
            "Represent this sentence for searching relevant passages: find",
            "Title\nBody",
            "plain",
        ),
    ] {
        assert_eq!(profile.render_query("find"), query);
        assert_eq!(profile.render_document(&document), text);
        assert_eq!(profile.document_format(), format);
    }
    let untitled = EmbeddingDocument { title: None, body: "Body".into() };
    assert_eq!(
        PromptProfile::EmbeddingGemma.render_document(&untitled),
        "title: none | text: Body"
    );
    assert_eq!(PromptProfile::Plain.render_document(&untitled), "Body");
    assert_eq!(
        PromptProfile::Plain.render_document(&EmbeddingDocument {
            title: Some("Title".into()),
            body: String::new()
        }),
        "Title"
    );
}

/// Failure modes: dispatch skips a prefix or a transport renders it a second time.
#[tokio::test]
async fn query_dispatch_renders_once_before_selecting_transport() {
    use crate::AiProviderConfig;
    use crate::ai::{AiProviderRuntime, EmbeddingRole, provider};
    let vault = crate::Vault::open_temp().expect("vault");
    for model in ["Qwen/Qwen3-Embedding-0.6B", "google/embeddinggemma-300m", "intfloat/e5-base-v2"]
    {
        let runtime = AiProviderRuntime {
            config: AiProviderConfig {
                id: "embed".into(),
                default_model: model.into(),
                dimensions: Some(8),
                ..Default::default()
            },
            api_key: None,
        };
        let profile = PromptProfile::for_model(model);
        let rendered = profile.render_query("find");
        let vector = provider::embed_query(vault.paths(), &runtime, "find", EmbeddingRole::Query)
            .await
            .expect("query");
        assert_eq!(
            vector,
            provider::stub_embedding_vector(&runtime, &rendered, EmbeddingRole::Query, 8)
        );
        assert_ne!(
            vector,
            provider::stub_embedding_vector(
                &runtime,
                &profile.render_query(&rendered),
                EmbeddingRole::Query,
                8
            )
        );
    }
}
