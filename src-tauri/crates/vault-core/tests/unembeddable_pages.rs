//! A build using the E2E WordLevel model must get past an unknown-only batch and
//! publish the later page. Incremental builds should remember the model's answer.
#![cfg(debug_assertions)]

use browser_history_fixtures::{TakeoutBrowserHistoryFixture, TakeoutBrowserRecord};
use std::{fs, path::Path, process::Command};
use vault_core::{AiIndexRequest, AiProviderConfig, AiProviderPurpose, AiProviderRuntime, Vault};
use vault_store::Db;

#[test]
fn unknown_only_page_does_not_block_the_next_batch() {
    const TEST: &str = "PATHKEEP_TEST_UNKNOWN_PAGE";
    if std::env::var_os(TEST).is_none() {
        let status = Command::new(std::env::current_exe().unwrap())
            .args(["--exact", "unknown_only_page_does_not_block_the_next_batch", "--nocapture"])
            .env(TEST, "1")
            .env("PATHKEEP_TEST_MODEL_SOURCE", "http://127.0.0.1:1/v1")
            .status()
            .unwrap();
        assert!(status.success());
        return;
    }
    let vault = Vault::open_temp().unwrap();
    let repo = vault_core::DEFAULT_STATIC_MODEL_REPO;
    let model = vault_core::model_dir_for_repo(vault.paths(), repo);
    fs::create_dir_all(&model).unwrap();
    let fixtures = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/builtin-model");
    for (dir, name) in
        [("common", "tokenizer.json"), ("common", "config.json"), ("v1", "model.safetensors")]
    {
        fs::copy(fixtures.join(dir).join(name), model.join(name)).unwrap();
    }
    let source = tempfile::tempdir().unwrap();
    let mut history = TakeoutBrowserHistoryFixture::new();
    for (id, title) in [(1, "quux"), (2, "alpha beta")] {
        history = history.add_record(TakeoutBrowserRecord {
            url: format!("https://quux.invalid/{id}"),
            title: Some(title.into()),
            visit_time_unix_ms: 1_800_000_000_000 + id,
            page_transition: Some("LINK".into()),
            client_id: None,
            favicon_url: None,
            ptoken: None,
        });
    }
    history.write(&source.path().join("Chrome/BrowserHistory.json")).unwrap();
    let import = vault_core::TakeoutRequest {
        source_path: source.path().display().to_string(),
        dry_run: false,
    };
    assert_eq!(vault_core::import_takeout(&vault, &import).unwrap().imported_items, 2);
    vault.search().cycle().unwrap();
    let provider = AiProviderRuntime {
        config: AiProviderConfig {
            id: "static".into(),
            name: "Local fixture".into(),
            enabled: true,
            purpose: AiProviderPurpose::Embedding,
            default_model: repo.into(),
            base_url: Some(vault_core::ai::STATIC_INAPP_BASE_URL.into()),
            ..Default::default()
        },
        api_key: None,
    };
    let rt = tokio::runtime::Builder::new_current_thread().enable_all().build().unwrap();
    let request = AiIndexRequest { limit: Some(1), ..Default::default() };
    for (indexed, skipped) in [(1, 1), (0, 2)] {
        let report = rt.block_on(vault_core::build_ai_index(&vault, &provider, &request)).unwrap();
        assert_eq!((report.indexed_items, report.skipped_items), (indexed, skipped));
        let urls = vault
            .store()
            .read(Db::Intelligence, |s| {
                s.query("SELECT url FROM ai_embeddings", (), |r| Ok(r.get::<String>(0)?))
            })
            .unwrap();
        assert_eq!(urls, ["https://quux.invalid/2"]);
        let run = report.run_id.unwrap();
        let unknown = vault
            .store()
            .read(Db::Archive, move |s| {
                s.query_one(
                    "SELECT json_extract(stats_json,'$.unembeddableItems') FROM runs WHERE id=?1",
                    [run],
                    |r| Ok(r.get::<i64>(0)?),
                )
            })
            .unwrap();
        assert_eq!(unknown, 1);
    }
    let mut config = vault.config().clone();
    config.ai.enabled = true;
    config.ai.semantic_index_enabled = true;
    config.ai.embedding_provider_id = Some(provider.config.id.clone());
    config.ai.embedding_providers = vec![provider.config.clone()];
    let search = rt
        .block_on(vault_core::semantic_search_history(
            &vault,
            vault.paths(),
            &config,
            None,
            Some(&provider),
            &vault_core::AiSearchRequest { query: "alpha".into(), ..Default::default() },
        ))
        .unwrap();
    assert_eq!(
        search.items.iter().map(|item| item.url.as_str()).collect::<Vec<_>>(),
        ["https://quux.invalid/2"]
    );
}
