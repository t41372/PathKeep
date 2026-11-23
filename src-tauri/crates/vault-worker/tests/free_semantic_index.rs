//! Exercise removal across two published model generations, the archive and the lexical index.
#[allow(dead_code)]
#[path = "agent_ai/provider.rs"]
mod provider;
#[allow(dead_code)]
#[path = "root_admission/support.rs"]
mod support;

use serde_json::{Value, from_value, json};
use support::Fixture;
use vault_core::{
    AiProviderConfig, AiProviderPurpose, AiProviderRuntime, ControlGate, HistoryQuery, ai_queue,
    load_config, save_config,
};
use vault_store::Db;
use vault_worker::data_kinds::browsing_semantic;

fn preview(f: &Fixture) -> anyhow::Result<Value> {
    browsing_semantic::preview(&f.session, from_value(json!({}))?)
}
fn free(f: &Fixture, receipt: &Value) -> anyhow::Result<Value> {
    browsing_semantic::free(
        &f.session,
        from_value(json!({"previewId":receipt["previewId"],"confirm":true}))?,
    )
}
fn set_search(f: &Fixture, enabled: bool) {
    let mut cfg = load_config(&f.paths()).unwrap();
    cfg.ai.enabled = true;
    cfg.ai.semantic_index_enabled = enabled;
    save_config(&ControlGate::acquire(&f.paths()), &cfg).unwrap();
}
fn model_bytes(path: &std::path::Path) -> u64 {
    if path.is_file() {
        return path.metadata().unwrap().len();
    }
    std::fs::read_dir(path).unwrap().map(|entry| model_bytes(&entry.unwrap().path())).sum()
}

#[test]
fn semantic_search_or_a_queued_build_refuses_removal() {
    let f = Fixture::new(true);
    set_search(&f, true);
    assert_eq!(preview(&f).unwrap_err().to_string(), "task-action-unavailable");
    set_search(&f, false);
    let gate = vault_core::archive::ArchiveOpGate::acquire(&f.paths());
    assert_eq!(preview(&f).unwrap_err().to_string(), "archive-busy");
    drop(gate);
    let replacement = f.session.begin_maintenance().unwrap();
    assert_eq!(preview(&f).unwrap_err().to_string(), "archive-busy");
    drop(replacement);
    let receipt = preview(&f).unwrap();
    ai_queue::enqueue_index_job(f.vault(), &Default::default(), false).unwrap();
    assert_eq!(preview(&f).unwrap_err().to_string(), "archive-busy");
    assert_eq!(free(&f, &receipt).unwrap_err().to_string(), "archive-busy");
}

#[test]
fn a_backup_or_replacement_preview_expires_the_receipt() {
    let f = Fixture::new(true);
    set_search(&f, false);
    let first = preview(&f).unwrap();
    let current = preview(&f).unwrap();
    assert_eq!(free(&f, &first).unwrap_err().to_string(), "preview-expired");
    f.browser_checkpoint();
    assert_eq!(free(&f, &current).unwrap_err().to_string(), "preview-expired");
}

#[test]
fn a_same_size_sidecar_replacement_requires_a_new_preview() {
    let f = Fixture::new(true);
    set_search(&f, false);
    let legacy = f.paths().semantic_index_dir;
    std::fs::create_dir_all(&legacy).unwrap();
    std::fs::write(legacy.join("first"), b"abc").unwrap();
    let receipt = preview(&f).unwrap();
    assert_eq!(receipt["keptBytes"], "3");
    std::fs::rename(legacy.join("first"), legacy.join("second")).unwrap();
    assert_eq!(free(&f, &receipt).unwrap_err().to_string(), "preview-expired");
    assert_eq!(std::fs::read(legacy.join("second")).unwrap(), b"abc");
}

#[test]
fn free_counts_and_reclaims_both_models_but_preserves_visits_and_word_search() {
    let f = Fixture::new(true);
    f.browser_checkpoint();
    let server = provider::Provider::new();
    let runtime = tokio::runtime::Runtime::new().unwrap();
    let mut cfg = load_config(&f.paths()).unwrap();
    cfg.ai.enabled = true;
    cfg.ai.semantic_index_enabled = true;
    let mut tables = Vec::new();
    for model in ["first", "second"] {
        let provider = AiProviderConfig {
            id: "embed".into(),
            purpose: AiProviderPurpose::Embedding,
            enabled: true,
            default_model: model.into(),
            base_url: Some(server.url.clone()),
            dimensions: Some(3),
            ..Default::default()
        };
        cfg.ai.embedding_provider_id = Some(provider.id.clone());
        cfg.ai.embedding_providers = vec![provider.clone()];
        save_config(&ControlGate::acquire(&f.paths()), &cfg).unwrap();
        let vault = f.vault().with_config(cfg.clone());
        runtime
            .block_on(vault_core::build_ai_index(
                &vault,
                &AiProviderRuntime { config: provider, api_key: None },
                &Default::default(),
            ))
            .unwrap();
        tables.push(
            f.paths().vectors_dir.join(vault_core::ai_sidecar::provider_table_name("embed", model)),
        );
    }
    set_search(&f, false);
    let off = load_config(&f.paths()).unwrap();
    let vault = f.vault().with_config(off.clone());
    let status = || vault_core::ai_index_status(&vault).unwrap();
    let storage = || {
        vault_core::archive::browsing_storage_items(&vault)
            .unwrap()
            .into_iter()
            .filter(|(id, _)| {
                matches!(
                    id,
                    vault_layout::ArtifactId::Vectors | vault_layout::ArtifactId::LegacySemantic
                )
            })
            .map(|(_, bytes)| bytes)
            .sum::<u64>()
    };
    let rows = |table: &'static str| {
        vault
            .store()
            .read(Db::Intelligence, move |s| {
                s.query_one(&format!("SELECT COUNT(*) FROM {table}"), (), |r| Ok(r.get::<i64>(0)?))
            })
            .unwrap()
    };
    vault.search().cycle().unwrap();
    let visits = |q: Option<String>| {
        vault_core::list_history(&vault, HistoryQuery { q, ..Default::default() })
            .unwrap()
            .items
            .into_iter()
            .map(|v| v.url)
            .collect::<Vec<_>>()
    };
    let words = || visits(Some("admission".into()));
    let before = visits(None);
    assert_eq!(before, ["https://admission.example.test/"]);
    assert_eq!(words(), before);
    assert_eq!(rows("ai_index_ledger"), 2);
    assert!(tables.iter().all(|p| p.is_dir()));
    let legacy = f.paths().semantic_index_dir.join("old-sidecar");
    std::fs::write(&legacy, b"legacy").unwrap();
    let dashboard =
        || vault_core::load_dashboard_snapshot(&vault).unwrap().storage.semantic_sidecar_bytes;
    let expected = tables.iter().map(|p| model_bytes(p)).sum::<u64>() + 6;
    assert_eq!(storage(), expected);
    assert_eq!(dashboard(), expected);
    assert!(expected > 0);
    let receipt = preview(&f).unwrap();
    assert_eq!(receipt["keptBytes"], expected.to_string());
    assert_eq!(status().semantic_sidecar_bytes, expected);
    assert_eq!(
        free(&f, &receipt).unwrap(),
        json!({"freedBytes":expected.to_string(),"keptBytes":"0"})
    );
    assert_eq!(storage(), 0);
    assert_eq!(dashboard(), 0);
    assert!(tables.iter().all(|p| !p.exists()));
    for table in ["ai_index_ledger", "ai_embeddings", "ai_embedding_totals"] {
        assert_eq!(rows(table), 0);
    }
    assert_eq!(status().state, "empty");
    assert_eq!(status().semantic_sidecar_bytes, 0);
    assert_eq!(visits(None), before);
    assert_eq!(words(), before);
    assert_eq!(
        load_config(&f.paths()).unwrap().ai.embedding_provider_id,
        off.ai.embedding_provider_id
    );
    assert_eq!(
        serde_json::to_value(load_config(&f.paths()).unwrap().ai.embedding_providers).unwrap(),
        serde_json::to_value(off.ai.embedding_providers).unwrap()
    );
}
