//! Responsibilities: retain the parent module's behavior regressions on native storage.
//! Not responsible for: product implementation or external network execution.
//! Failure modes: incorrect boundaries, lost records, replay drift and invalid state transitions.
use super::*;
use crate::{
    intelligence::run_core_intelligence,
    models::{AppConfig, ArchiveMode, CoreIntelligenceRebuildRequest},
};
use vault_store::Session;

#[test]
fn preview_intelligence_local_host_builds_fixed_generated_files() {
    let vault = prepared_archive();
    let _paths = vault.paths().clone();
    let _config = vault.config().clone();

    let preview = preview_intelligence_local_host(
        &vault,
        &IntelligenceLocalHostRequest {
            date_range: crate::models::DateRange {
                start: "2024-04-01".to_string(),
                end: "2024-04-30".to_string(),
            },
            profile_id: Some("chrome:Default".to_string()),
            locale: "en".to_string(),
        },
    )
    .expect("preview");

    assert!(preview.artifact_root.ends_with("integrations/core-intelligence/browser-snippet-v1"));
    assert!(
        preview
            .entry_file_path
            .ends_with("integrations/core-intelligence/browser-snippet-v1/index.html")
    );
    assert_eq!(preview.generated_files.len(), 2);
    assert_eq!(
        preview.generated_files[0].relative_path,
        "integrations/core-intelligence/browser-snippet-v1/index.html"
    );
    assert_eq!(
        preview.generated_files[1].relative_path,
        "integrations/core-intelligence/browser-snippet-v1/bundle.json"
    );
    assert!(preview.generated_files[0].contents.contains("PathKeep Core Intelligence Snippet"));
    assert_eq!(preview.bundle.host_id, BROWSER_SNIPPET_HOST_ID);
    assert_eq!(preview.bundle.bundle_version, LOCAL_HOST_BUNDLE_VERSION);
    assert!(preview.bundle.trusted_only_card_count >= 1);
    assert!(preview.bundle.boundary_notes.iter().any(|note| note.contains("Trusted-only")));
}

#[test]
fn build_intelligence_local_host_writes_artifacts_and_reports_installed_host() {
    let vault = prepared_archive();
    let _paths = vault.paths().clone();
    let _config = vault.config().clone();

    let result = build_intelligence_local_host(
        &vault,
        &IntelligenceLocalHostRequest {
            date_range: crate::models::DateRange {
                start: "2024-04-01".to_string(),
                end: "2024-04-30".to_string(),
            },
            profile_id: Some("chrome:Default".to_string()),
            locale: "en".to_string(),
        },
    )
    .expect("build");

    let installed_host = result.installed_host.expect("installed host");
    assert!(Path::new(&installed_host.entry_file_path).exists());
    assert!(Path::new(&installed_host.artifact_root).join("bundle.json").exists());
    let saved_bundle =
        fs::read_to_string(Path::new(&installed_host.artifact_root).join("bundle.json"))
            .expect("read bundle");
    let parsed_bundle: IntelligenceLocalHostBundle =
        serde_json::from_str(&saved_bundle).expect("parse bundle");
    assert_eq!(parsed_bundle.date_range, result.bundle.date_range);
    assert_eq!(parsed_bundle.profile_id, result.bundle.profile_id);
    assert_eq!(parsed_bundle.trusted_only_card_ids, result.bundle.trusted_only_card_ids);
    assert_eq!(
        parsed_bundle.public_snapshot.top_domains,
        result.bundle.public_snapshot.top_domains
    );
    assert!(
        fs::read_to_string(&installed_host.entry_file_path)
            .expect("read html")
            .contains("Trusted-only cards are still present")
    );
}

#[test]
fn installed_host_loader_reports_missing_corrupt_and_valid_artifacts() {
    let vault = prepared_archive();
    let paths = vault.paths().clone();
    let _config = vault.config().clone();
    let artifact_root = local_host_root(&paths);
    let bundle_path = artifact_root.join("bundle.json");
    let entry_path = artifact_root.join("index.html");

    let (empty, warnings) = load_installed_host(&paths, "en").expect("empty load");
    assert!(empty.is_none());
    assert!(warnings.is_empty());

    fs::create_dir_all(&artifact_root).expect("artifact root");
    fs::create_dir_all(&bundle_path).expect("bundle path as dir");
    let (missing, warnings) = load_installed_host(&paths, "zh-TW").expect("read failure");
    assert!(missing.is_none());
    assert_eq!(warnings.len(), 1);
    assert!(warnings[0].contains("無法讀取"));
    fs::remove_dir(&bundle_path).expect("remove bundle dir");

    fs::write(&bundle_path, "{not-json").expect("corrupt bundle");
    let (missing, warnings) = load_installed_host(&paths, "zh-CN").expect("parse failure");
    assert!(missing.is_none());
    assert_eq!(warnings.len(), 1);
    assert!(warnings[0].contains("无法解析"));

    let built = build_intelligence_local_host(
        &vault,
        &IntelligenceLocalHostRequest {
            date_range: crate::models::DateRange {
                start: "2024-04-01".to_string(),
                end: "2024-04-30".to_string(),
            },
            profile_id: None,
            locale: "en".to_string(),
        },
    )
    .expect("build");
    fs::remove_file(&entry_path).expect("remove entry");
    let (missing, warnings) = load_installed_host(&paths, "en").expect("missing entry");
    assert!(missing.is_none());
    assert_eq!(warnings.len(), 1);
    assert!(warnings[0].contains("index.html"));

    fs::write(&entry_path, "ok").expect("restore entry");
    let (installed, warnings) = load_installed_host(&paths, "en").expect("valid load");
    assert!(warnings.is_empty());
    assert_eq!(installed.expect("installed").bundle.bundle_version, built.bundle.bundle_version);
}

#[test]
fn local_host_rendering_covers_locales_empty_states_and_html_escaping() {
    assert!(host_copy("zh-TW").summary_title.contains("受信任"));
    assert!(host_copy("zh-CN").summary_title.contains("受信任"));
    assert!(host_copy("en-US").summary_title.contains("Trusted"));
    assert!(build_boundary_notes("zh-TW")[0].contains("deterministic"));
    assert!(build_boundary_notes("zh-CN")[1].contains("trusted-only"));

    let vault = prepared_archive();
    let _paths = vault.paths().clone();
    let _config = vault.config().clone();
    let mut bundle = preview_intelligence_local_host(
        &vault,
        &IntelligenceLocalHostRequest {
            date_range: crate::models::DateRange {
                start: "2024-04-01".to_string(),
                end: "2024-04-30".to_string(),
            },
            profile_id: None,
            locale: "en".to_string(),
        },
    )
    .expect("preview")
    .bundle;
    bundle.locale = "zh-CN".to_string();
    bundle.profile_id = Some("chrome:<Default>".to_string());
    bundle.embed_cards.clear();
    bundle.widget_snapshot.highlights.clear();
    bundle.widget_snapshot.notes.clear();
    bundle.public_snapshot.top_domains.clear();
    bundle.public_snapshot.search_engines.clear();
    bundle.public_snapshot.discovery_trend.points.clear();
    bundle.public_snapshot.notes.clear();
    let html = render_browser_snippet_html(&bundle, &host_copy("zh-CN"));
    assert!(html.contains("Profile: chrome:&lt;Default&gt;"));
    assert!(html.contains("暂时没有可用的卡片"));
    assert!(html.contains("没有可用的搜索引擎活动"));
    assert!(html.contains("没有可用的发现趋势点"));

    let plain_card = crate::models::IntelligenceEmbedCardPayload {
        card_id: "plain".to_string(),
        card_type: "summary".to_string(),
        title: "A <plain> card".to_string(),
        body: "Body & detail".to_string(),
        internal_only: false,
        ..Default::default()
    };
    let cards = render_embed_cards(&[plain_card], &host_copy("en"));
    assert!(cards.contains("A &lt;plain&gt; card"));
    assert!(!cards.contains("badge"));
    assert!(!cards.contains("metric"));
}

fn prepared_archive() -> crate::Vault {
    let config =
        AppConfig { initialized: true, archive_mode: ArchiveMode::Plaintext, ..Default::default() };
    let vault = crate::Vault::open_temp().unwrap().with_config(config);
    vault
        .write(vault_store::Db::Archive, vault_store::Priority::Batch, |s| {
            seed_core_intelligence_fixture(s);
            Ok(())
        })
        .unwrap();
    run_core_intelligence(&vault, &CoreIntelligenceRebuildRequest::default()).unwrap();
    vault
}

fn seed_core_intelligence_fixture(connection: &Session<'_>) {
    connection
        .execute(
            "INSERT INTO runs (
                id, run_type, trigger, started_at, timezone, status, profile_scope_json, warnings_json, stats_json, due_only
             ) VALUES (
                1, 'backup', 'manual', '2026-04-14T00:00:00Z', 'UTC', 'success', '[]', '[]', '{}', 0
             )",
            (),
        )
        .expect("run");
    connection
        .execute(
            "INSERT INTO source_profiles (
                id, browser_kind, browser_version, profile_name, profile_path, discovered_at, enabled, profile_key, updated_at
             ) VALUES (
                1, 'chrome', '1', 'Default', '/tmp/profile', '2026-04-14T00:00:00Z', 1, 'chrome:Default', '2026-04-14T00:00:00Z'
             )",
            (),
        )
        .expect("profile");
    connection.execute("INSERT INTO pages(id,url,canonical_url,registrable_domain) VALUES(1,'https://www.google.com/search?q=sqlite+wal','https://www.google.com/search?q=sqlite+wal','google.com'),(2,'https://github.com/example/repo/issues/42','https://github.com/example/repo/issues/42','github.com')",()).unwrap();
    connection
        .execute(
            "INSERT INTO urls (
                id, url, title, visit_count, typed_count, first_visit_ms, first_visit_iso, last_visit_ms, last_visit_iso,
                source_profile_id, created_by_run_id, source_url_id, hidden, payload_hash, recorded_at, page_id
             ) VALUES
             (1, 'https://www.google.com/search?q=sqlite+wal', 'sqlite wal - Google Search', 1, 0, 1, '1970-01-01T00:00:00Z', 1, '1970-01-01T00:00:00Z', 1, 1, 11, 0, 'hash-1', '2026-04-14T00:00:00Z',1),
             (2, 'https://github.com/example/repo/issues/42', 'Issue 42', 2, 1, 2, '1970-01-01T00:00:02Z', 86400002, '1970-01-02T00:00:00Z', 1, 1, 12, 0, 'hash-2', '2026-04-14T00:00:00Z',2)",
            (),
        )
        .expect("urls");
    connection
        .execute(
            "INSERT INTO visits (
                id, url_id, source_visit_id, visit_time_ms, visit_time_iso, transition_type, visit_duration_ms,
                source_profile_id, created_by_run_id, from_visit, is_known_to_sync, event_fingerprint, payload_hash, recorded_at
             ) VALUES
             (1, 1, '1', 1711929600000, '2024-04-01T00:00:00Z', 1, 0, 1, 1, NULL, 0, 'fingerprint-1', 'visit-hash-1', '2026-04-14T00:00:00Z'),
             (2, 2, '2', 1711929660000, '2024-04-01T00:01:00Z', 1, 0, 1, 1, 1, 0, 'fingerprint-2', 'visit-hash-2', '2026-04-14T00:00:00Z'),
             (3, 2, '3', 1712016000000, '2024-04-02T00:00:00Z', 1, 0, 1, 1, NULL, 0, 'fingerprint-3', 'visit-hash-3', '2026-04-14T00:00:00Z')",
            (),
        )
        .expect("visits");
    connection
        .execute(
            "INSERT INTO search_terms (
                id, url_id, term, normalized_term, source_profile_id, created_by_run_id, profile_id
             ) VALUES (
                1, 1, 'sqlite wal', 'sqlite wal', 1, 1, 'chrome:Default'
             )",
            (),
        )
        .expect("search term");
    connection
        .execute(
            "UPDATE archive_totals SET value=(SELECT COUNT(*) FROM visits) WHERE name='visits'",
            (),
        )
        .unwrap();
    connection
        .execute(
            "INSERT INTO archive_totals(name,value) SELECT 'sourceVisits:1',COUNT(*) FROM visits",
            (),
        )
        .unwrap();
}
