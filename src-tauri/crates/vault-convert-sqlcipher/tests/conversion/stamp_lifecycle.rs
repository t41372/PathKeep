//! Converts the former SQLite ledger and flat-format vector files together.
//! Published coverage must survive copying, while a conversion that hides a duplicate
//! or a later backup in the converted archive must invalidate the old index.
use super::*;
use vault_core::{AiIndexWarning, AiProviderConfig, AiProviderPurpose, AiSemanticStaleness, Vault};

const PROVIDER: &str = "embed";
const MODEL: &str = "model";

fn converted_index(duplicate: bool) -> (tempfile::TempDir, Vault) {
    let root = tempfile::tempdir().unwrap();
    let paths = project_paths_with_root(root.path());
    let mut config = AppConfig {
        initialized: true,
        git_enabled: false,
        archive_mode: ArchiveMode::Plaintext,
        ..Default::default()
    };
    config.ai.enabled = true;
    config.ai.semantic_index_enabled = true;
    config.ai.embedding_provider_id = Some(PROVIDER.into());
    config.ai.embedding_providers = vec![AiProviderConfig {
        id: PROVIDER.into(),
        default_model: MODEL.into(),
        enabled: true,
        purpose: AiProviderPurpose::Embedding,
        ..Default::default()
    }];
    vault_core::config::ensure_paths(&paths).unwrap();
    vault_core::save_config(&vault_core::ControlGate::acquire(&paths), &config).unwrap();
    vault_convert_sqlcipher::test_support::seed_reader_pages(
        &paths,
        &config,
        None,
        &[("https://conversion.example/a", "First page")],
    );
    let count = if duplicate {
        let db = vault_convert_sqlcipher::test_support::open_archive(&paths, &config, None);
        db.execute_batch("INSERT INTO source_profiles(id,browser_kind,profile_key,profile_name,profile_path,discovered_at)
            VALUES(2,'takeout','takeout::browser-history','Import','fixture','then');
            INSERT INTO visits(id,url_id,source_profile_id,visit_time_ms,visit_time_iso,created_by_run_id,source_visit_id)
            SELECT 2,url_id,2,visit_time_ms,visit_time_iso,created_by_run_id,'2' FROM visits WHERE id=1;").unwrap();
        2_i64
    } else {
        1
    };
    let captured = (count << 32) ^ count;
    {
        let db = vault_convert_sqlcipher::test_support::open_intelligence(&paths, None);
        db.execute("INSERT INTO ai_index_ledger(provider_id,model,sidecar_table,index_version,state,source_watermark,last_indexed_at)
            VALUES(?1,?2,'flat','legacy','ready',?3,'then')",rusqlite::params![PROVIDER,MODEL,captured]).unwrap();
        for id in 1..=count {
            db.execute("INSERT INTO ai_embeddings(history_id,profile_id,url,domain,visited_at,content_hash,content_bytes,provider_id,model,indexed_at)
                VALUES(?1,'chrome:Default','https://conversion.example/a','conversion.example','then','fixture',1,?2,?3,'then')",rusqlite::params![id,PROVIDER,MODEL]).unwrap();
        }
    }
    let fp = vault_core::EmbeddingFingerprint::new(
        PROVIDER,
        MODEL,
        2,
        vault_core::EmbeddingDtype::Float32,
        true,
        vault_core::EmbeddingPooling::Unknown,
        None,
    );
    vault_core::VectorStore::create_stamped(&paths, &fp)
        .unwrap()
        .append_vectors(&[(1, vec![1.0, 0.0])])
        .unwrap();
    let map = vault_core::VisitContentMap::for_provider(&paths, PROVIDER, MODEL);
    map.ensure_created(&paths).unwrap();
    map.append(&(1..=count).map(|id| (id, 1)).collect::<Vec<_>>()).unwrap();
    vault_core::build_planes_from_store(&paths, PROVIDER, MODEL).unwrap();
    vault_core::ReverseVisitMap::for_provider(&paths, PROVIDER, MODEL)
        .build_from_visit_map(&paths, &map, &fp.hash())
        .unwrap();
    let converted = convert(&paths, &config, None, Default::default(), |_| {}).unwrap().vault;
    (root, converted)
}

fn token(vault: &Vault) -> Option<i64> {
    vault
        .store()
        .read(Db::Archive, |s| {
            s.query_one(
                "SELECT (SELECT value FROM archive_totals WHERE name='visitMembershipToken')",
                (),
                |r| Ok(r.get(0)?),
            )
        })
        .unwrap()
}

fn stale(vault: &Vault, expected: bool) {
    let warning = vault_core::ai_index_status(vault).unwrap().warning_code;
    let reason = vault_core::ai::search_staleness_for_fixture(vault, PROVIDER, MODEL).unwrap();
    assert_eq!(reason, expected.then_some(AiSemanticStaleness::Watermark));
    if expected {
        assert!(
            matches!(
                warning,
                Some(AiIndexWarning::IndexStale { reason: AiSemanticStaleness::Watermark })
            ),
            "{warning:?}"
        );
    } else {
        assert!(warning.is_none(), "{warning:?}");
    }
}

#[test]
fn stamp_lifecycle_conversion_certifies_only_unchanged_legacy_membership() {
    for duplicate in [false, true] {
        let (_root, vault) = converted_index(duplicate);
        stale(&vault, duplicate);
        let initial = token(&vault);
        assert_eq!(
            initial.is_some(),
            duplicate,
            "a conversion that hid a duplicate changed membership in its transaction"
        );
        vault_core::test_support::history::History::chrome(&vault)
            .page(7, "https://converted.example/new", Some("New page"), 1_775_000_000_000)
            .commit();
        assert_ne!(token(&vault), initial);
        stale(&vault, true);
    }
}

#[test]
fn stamp_lifecycle_old_snapshot_and_move_replace_the_destination_index() {
    for moving in [false, true] {
        let (_root, live) = converted_index(false);
        stale(&live, false);
        let restored = if moving {
            let fixtures = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
                .join("../../../tests/fixtures/v0.3-archive");
            let expected: serde_json::Value =
                serde_json::from_slice(&std::fs::read(fixtures.join("expected.json")).unwrap())
                    .unwrap();
            let options = vault_core::ApplyImportOptions {
                confirm_overwrite: true,
                source_archive_key: Some(expected["password"].as_str().unwrap().into()),
            };
            vault_convert_sqlcipher::bundle::import(
                live.paths(),
                Some(&live),
                None,
                &fixtures.join("bundle.zip"),
                &options,
                Default::default(),
                |_| {},
            )
            .unwrap()
            .1
            .vault
        } else {
            let old_root = tempfile::tempdir().unwrap();
            let paths = project_paths_with_root(old_root.path());
            vault_convert_sqlcipher::test_support::seed_reader_pages(
                &paths,
                live.config(),
                None,
                &[("https://restored.example/page", "Restored")],
            );
            vault_convert_sqlcipher::restore_snapshot(
                live.paths(),
                Some(&live),
                None,
                &paths.legacy.archive,
                None,
                None,
                Default::default(),
                |_| {},
            )
            .unwrap()
            .vault
        };
        let previous_pair = restored
            .store()
            .read(Db::Intelligence, |s| {
                s.query_one(
                    "SELECT COUNT(*) FROM ai_index_ledger WHERE provider_id=?1 AND model=?2",
                    vault_store::params![PROVIDER, MODEL],
                    |r| Ok(r.get::<i64>(0)?),
                )
            })
            .unwrap();
        assert_eq!(previous_pair, 0);
        assert!(
            vault_core::ai::search_staleness_for_fixture(&restored, PROVIDER, MODEL)
                .unwrap()
                .is_none()
        );
        vault_core::test_support::history::History::new(&restored, "chrome:AfterMove")
            .page(1, "https://after.example/page", None, 1_775_000_000_000)
            .commit();
        assert!(token(&restored).is_some());
    }
}
