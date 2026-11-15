//! Responsibilities: record actual relative files, byte markers and unknown paths for review.
//! Not responsible for: selecting lifecycle members or changing expected files after a refactor.

use super::{MARKERS, expected_root};
use serde_json::{Value, json};
use std::{collections::BTreeMap, fs, path::Path};

/// Saves raw evidence before assertions, and returns stable facts for literal comparison.
pub fn observe(root: &Path, vault: Option<&vault_core::Vault>, name: &str) -> Value {
    if let Some(vault) = vault {
        vault.search().set_paused_for_tests(true);
        vault.search().cycle().unwrap();
    }
    let logical = vault.map(|vault| json!({
        "archive/history-vault.db": super::store_marker(vault, vault_store::Db::Archive),
        "archive/source-evidence.db": super::store_marker(vault, vault_store::Db::SourceEvidence),
        "derived/intelligence.db": super::store_marker(vault, vault_store::Db::Intelligence),
        "derived/agent.db": super::store_marker(vault, vault_store::Db::Agent),
        "archive/agent-history.db": super::store_marker(vault, vault_store::Db::AgentHistoryEvidence),
        "derived/agent-history-read-model.db": super::store_marker(vault, vault_store::Db::AgentHistoryReadModel),
        "searchTitles": super::search_titles(vault),
        "askMessages": vault_worker::load_ai_conversation(vault, "lifecycle-chat").unwrap()
            .map(|chat| chat.messages.into_iter().map(|message| message.content).collect::<Vec<_>>()),
        "savedTextLocators": vault.store().read(vault_store::Db::Intelligence, |s| {
            s.query("SELECT readable_text_blob_path FROM page_content_enrichments ORDER BY canonical_url,content_source", (), |r| Ok(r.get::<Option<String>>(0)?))
        }).unwrap(),
    }));
    let pause = vault.map(|vault| vault.store().pause().unwrap());
    if let Some(pause) = &pause {
        pause.checkpoint().unwrap();
    }
    let files = expected_root::files_under(root).unwrap();
    let mut markers: BTreeMap<&str, Vec<String>> =
        MARKERS.into_iter().map(|s| (s, vec![])).collect();
    let mut hashes = BTreeMap::new();
    let mut bundles = BTreeMap::new();
    let mut snapshot_proofs = BTreeMap::new();
    let mut headers = BTreeMap::new();
    let mut gap_families = std::collections::BTreeSet::new();
    for path in &files {
        let bytes = fs::read(root.join(path)).unwrap();
        hashes.insert(path.clone(), vault_core::utils::sha256_hex(&bytes));
        if path.ends_with(".pathkeep") {
            bundles.insert(path.clone(), super::zip_entries(&root.join(path)));
        }
        if path.ends_with("/snapshot.json") {
            snapshot_proofs.insert(path.clone(), serde_json::from_slice::<Value>(&bytes).unwrap());
        }
        if path.ends_with(".db") {
            headers.insert(
                path.clone(),
                if bytes.is_empty() {
                    "empty"
                } else if bytes.starts_with(b"SQLite format 3\0") {
                    "sqlite"
                } else {
                    "non-sqlite"
                },
            );
        }
        for marker in MARKERS {
            if bytes.windows(marker.len()).any(|window| window == marker.as_bytes()) {
                markers.get_mut(marker).unwrap().push(path.clone());
                if let Some(known) = expected_root::classify(path)
                    && known.at_rest == expected_root::AtRest::PlainKnownGap
                {
                    gap_families.insert(known.pattern);
                }
            }
        }
    }
    let mut names = super::names::Names::default();
    let mut normalized_files: Vec<_> = files.iter().map(|p| names.path(p)).collect();
    normalized_files.sort();
    let normalized_markers: BTreeMap<_, _> = markers
        .iter()
        .map(|(marker, paths)| {
            let mut paths: Vec<_> = paths.iter().map(|p| names.path(p)).collect();
            paths.sort();
            (*marker, paths)
        })
        .collect();
    let headers: BTreeMap<_, _> =
        headers.into_iter().map(|(path, header)| (names.path(&path), header)).collect();
    let config = vault_core::project_paths_with_root(root);
    let mode =
        config.config_path.exists().then(|| vault_core::load_config(&config).unwrap().archive_mode);
    let observed = json!({"files":normalized_files,"markers":normalized_markers,
        "logical":logical,"databaseHeaders":headers,"configMode":mode,"plaintextGapFamilies":gap_families,
        "unknown":expected_root::unknown_files(root).unwrap()});
    let artifact = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../../artifacts/vault-lifecycle");
    // Only the explicit synthetic override is read; never fall back to browser discovery.
    let source_hash = std::env::var_os("CHB_CHROME_USER_DATA_DIR")
        .map(std::path::PathBuf::from)
        .map(|path| path.join("Default/History"))
        .filter(|path| path.is_file())
        .map(|path| vault_core::file_sha256_hex(&path).unwrap());
    fs::create_dir_all(&artifact).unwrap();
    fs::write(
        artifact.join(format!("{name}.json")),
        serde_json::to_vec_pretty(&json!({
            "observed":observed, "rawFiles":files, "rawMarkers":markers, "sha256":hashes,
            "sourceHistorySha256":source_hash, "bundleEntries":bundles, "snapshotProofs":snapshot_proofs,
        }))
        .unwrap(),
    )
    .unwrap();
    observed
}

/// Reads the reviewed step-1 literal; there is deliberately no update/generation mode.
pub fn expected(name: &str) -> Value {
    let path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/vault_lifecycle/expected")
        .join(format!("{name}.json"));
    serde_json::from_slice(&fs::read(path).unwrap()).unwrap()
}
