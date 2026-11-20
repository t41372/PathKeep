//! Release-scale indexing and retrieval through the installed in-app model.
#[path = "../../agent-history/tests/support/search_corpus.rs"]
#[allow(dead_code)]
mod corpus;
use agent_history::Archive;
use vault_core::{
    AiProviderConfig, AiProviderRuntime,
    ai::{conversation_vectors, select_embedding_provider},
};
#[test]
#[ignore = "requires the verified production model files"]
fn build_conversation_embeddings() {
    let temp = tempfile::tempdir().unwrap();
    let root = temp.path().join("archive");
    let roots = std::env::var("AH_SEARCH_ROOTS").map_or(120000, |n| n.parse().unwrap());
    std::fs::create_dir_all(&root).unwrap();
    let mut paths = vault_core::project_paths_with_root(&root);
    paths.models_dir = std::path::PathBuf::from(std::env::var_os("AH_MODELS_DIR").unwrap());
    let r = AiProviderRuntime {
        config: AiProviderConfig {
            id: "in-app".into(),
            base_url: Some(vault_core::ai::STATIC_INAPP_BASE_URL.into()),
            default_model: vault_core::ai::DEFAULT_STATIC_MODEL_REPO.into(),
            ..Default::default()
        },
        api_key: None,
    };
    use vault_core::{AiCapability, AppConfig, ControlGate, KindState, Vault};
    let mut cfg = AppConfig { initialized: true, ..Default::default() };
    cfg.ai.enabled = true;
    cfg.ai.semantic_index_enabled = true;
    cfg.ai.embedding_provider_id = Some(r.config.id.clone());
    cfg.ai.embedding_providers = vec![r.config.clone()];
    cfg.agent_viewer.build_semantic_index = true;
    cfg.capture_policy.agent_history = KindState::On;
    std::fs::write(&paths.config_path, serde_json::to_vec(&cfg).unwrap()).unwrap();
    vault_core::save_config(&ControlGate::acquire(&paths), &cfg).unwrap();
    let initial = Vault::open(paths.clone(), cfg.clone(), None).unwrap();
    initial.agent_history().unwrap();
    initial.close().unwrap();
    drop(initial);
    let distinct_prompts = std::env::var("AH_DISTINCT_PROMPTS").as_deref() == Ok("1");
    corpus::seed_projection(&root, roots, distinct_prompts);
    let vault = Vault::open(paths.clone(), cfg, None).unwrap();
    let a = vault.agent_history().unwrap();
    let permit =
        || vault_core::ai::conversations::consent(&vault, AiCapability::SemanticIndex).map(drop);
    eprintln!("Corpus ready; measuring model load and publication");
    let started = std::time::Instant::now();
    let engine = select_embedding_provider(&paths, &r).unwrap();
    let rt = tokio::runtime::Builder::new_current_thread().enable_all().build().unwrap();
    let cancel = Default::default();
    let g = rt
        .block_on(conversation_vectors::build(&a, &r, &engine, &cancel, permit, |_, _| {}))
        .unwrap();
    let elapsed = started.elapsed().as_millis();
    assert_eq!(g.passages, roots as u64 * 120);
    assert_eq!(g.vectors, if distinct_prompts { roots as u64 * 30 + 2 } else { 3 });
    let bytes: u64 = walkdir::WalkDir::new(
        vault_layout::path(&root, vault_layout::ArtifactId::AgentHistorySearch).join("semantic"),
    )
    .into_iter()
    .map(|e| e.unwrap())
    .filter(|e| e.file_type().is_file())
    .map(|e| e.metadata().unwrap().len())
    .sum();
    println!(
        "{}",
        serde_json::json!({"distinctPrompts":distinct_prompts,"roots":roots,"passages":g.passages,"uniqueVectors":g.vectors,"elapsedMs":elapsed,"bytes":bytes,"model":r.config.default_model})
    );
}

#[test]
#[ignore = "requires the verified production model files"]
fn model2vec_ranks_a_paraphrase_above_an_unrelated_conversation() {
    let temp = tempfile::tempdir().unwrap();
    let root = temp.path().join("archive");
    let archive = Archive::create_unencrypted(&root).unwrap();
    let pi = temp.path().join("pi");
    std::fs::create_dir_all(pi.join("sessions")).unwrap();
    for (id, text) in [
        ("pet", "The veterinarian treated a sick puppy at the animal hospital."),
        ("query", "A database index accelerates SQL joins and query planning."),
    ] {
        let header = serde_json::json!({"type":"session","version":3,"id":id,"cwd":"/project","timestamp":"2026-11-10T12:00:00Z"});
        let message = serde_json::json!({"type":"message","id":"u","parentId":null,"message":{"role":"user","content":text}});
        std::fs::write(pi.join(format!("sessions/{id}.jsonl")), format!("{header}\n{message}\n"))
            .unwrap();
    }
    archive
        .backup(
            &[agent_history::SourceRoot { harness: agent_history::Harness::Pi, root: pi }],
            &Default::default(),
        )
        .unwrap();
    let mut paths = vault_core::project_paths_with_root(&root);
    paths.models_dir = std::env::var_os("AH_MODELS_DIR").unwrap().into();
    let runtime = AiProviderRuntime {
        config: AiProviderConfig {
            id: "in-app".into(),
            base_url: Some(vault_core::ai::STATIC_INAPP_BASE_URL.into()),
            default_model: vault_core::ai::DEFAULT_STATIC_MODEL_REPO.into(),
            ..Default::default()
        },
        api_key: None,
    };
    let engine = select_embedding_provider(&paths, &runtime).unwrap();
    let rt = tokio::runtime::Builder::new_current_thread().enable_all().build().unwrap();
    let cancel = Default::default();
    rt.block_on(conversation_vectors::build(
        &archive,
        &runtime,
        &engine,
        &cancel,
        || Ok(()),
        |_, _| {},
    ))
    .unwrap();
    let ranked = rt
        .block_on(conversation_vectors::rank(
            &archive,
            &runtime,
            &engine,
            "medical care for an ill dog",
            &cancel,
            || Ok(()),
        ))
        .unwrap();
    assert_eq!((ranked.generation.passages, ranked.generation.vectors), (2, 2));
    use vault_core::ai::{EmbeddingProvider, EmbeddingRole};
    let q = rt
        .block_on(engine.embed(&["medical care for an ill dog".into()], EmbeddingRole::Query))
        .unwrap();
    let other = rt
        .block_on(engine.embed(
            &["A database index accelerates SQL joins and query planning.".into()],
            EmbeddingRole::Document,
        ))
        .unwrap();
    let unrelated_score: f32 = q[0].iter().zip(&other[0]).map(|(x, y)| x * y).sum();
    let hits = ranked.hits;
    assert_eq!(hits.len(), 2);
    assert!(hits[0].score > unrelated_score);
    let first = archive.ai_passage(hits[0].event, hits[0].chunk).unwrap().unwrap();
    assert!(first.text.contains("veterinarian"), "{first:?}");
    assert!(hits.iter().skip(1).all(|h| h.score < hits[0].score));
    println!(
        "{}",
        serde_json::json!({"query":"medical care for an ill dog","petScore":hits[0].score,"unrelatedFloatScore":unrelated_score,"otherReturnedScore":hits.get(1).map(|h|h.score)})
    );
}
