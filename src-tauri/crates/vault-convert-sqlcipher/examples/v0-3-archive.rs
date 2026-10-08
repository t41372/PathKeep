//! Writes the data folder a PathKeep 0.3 install leaves behind: an encrypted SQLCipher
//! archive with pages, a star and a note, saved page text in the old Insights database and a
//! chat in the old agent database. The upgrade E2E (`tests/e2e/upgrade.spec.ts`) opens the app
//! on it; what it wrote is printed as JSON so the spec checks against it.
//!
//! cargo run -p vault-convert-sqlcipher --features test-fixtures --example v0-3-archive -- <data folder> <password>
use vault_convert_sqlcipher::test_support;
use vault_core::{AppConfig, ArchiveMode, project_paths_with_root};

fn main() {
    let args = std::env::args().skip(1).collect::<Vec<_>>();
    let [root, password] = args.as_slice() else {
        eprintln!("usage: v0-3-archive <data folder> <password>");
        std::process::exit(2);
    };
    let password = password.as_str();
    let paths = project_paths_with_root(std::path::Path::new(root));
    let config = AppConfig {
        initialized: true,
        git_enabled: false,
        archive_mode: ArchiveMode::Encrypted,
        ..Default::default()
    };
    vault_core::config::ensure_paths(&paths).unwrap();
    vault_core::save_config(&paths, &config).unwrap();
    // Two spellings of one address (the old canonical rule kept `%7E`); History keeps both,
    // and the star and note saved under the old key must find the page under the new one.
    // Visits 1–4 in this order.
    let pages = [
        ("https://example.org/%7Equokka?utm_source=old", "Quokka field notes"),
        ("https://example.org/~quokka", "Quokka field notes"),
        ("https://example.org/reading-list", "Reading list for the coast trip"),
        ("https://example.org/tide-tables", "Tide tables"),
    ];
    test_support::seed_reader_pages(&paths, &config, Some(password), &pages);
    let archive = test_support::open_archive(&paths, &config, Some(password));
    archive.execute("INSERT INTO star(entity_kind,entity_key,starred_at,source_profile) VALUES('url','https://example.org/%7Equokka','2026-05-02T09:00:00Z','chrome:Default')",[]).unwrap();
    archive.execute("INSERT INTO url_annotations(url,notes,created_at,updated_at) VALUES('https://example.org/%7Equokka','Bring the red notebook','2026-05-02T09:00:00Z','2026-05-02T09:00:00Z')",[]).unwrap();
    drop(archive);
    let intelligence = test_support::open_intelligence(&paths, Some(password));
    intelligence.execute("INSERT INTO visit_content_enrichments(history_id,content_source,fetch_status,fetched_at,snippet_json,extraction_json,pipeline_version,enrichment_summary) VALUES(4,'content-fetch','success','2026-05-02T09:00:00Z','{}','{}','v1','Wombat burrow survey at low tide')",[]).unwrap();
    drop(intelligence);
    let agent = test_support::open_agent(&paths);
    agent.execute("INSERT INTO conversations(id,title,created_at,updated_at,message_count) VALUES('trip','Coast trip questions','2026-05-02T09:00:00Z','2026-05-02T09:05:00Z',2)",[]).unwrap();
    agent.execute("INSERT INTO messages(id,conversation_id,seq,role,content,created_at) VALUES('q','trip',1,'user','Where did I read about quokkas?','2026-05-02T09:00:00Z')",[]).unwrap();
    agent.execute("INSERT INTO messages(id,conversation_id,seq,role,content,created_at) VALUES('a','trip',2,'assistant','In your Quokka field notes from this morning.','2026-05-02T09:05:00Z')",[]).unwrap();
    drop(agent);
    println!(
        "{}",
        serde_json::json!({
            "pages": ["Quokka field notes", "Reading list for the coast trip", "Tide tables"],
            "starred": "Quokka field notes",
            "note": "Bring the red notebook",
            "savedText": { "query": "wombat", "page": "Tide tables" },
            "chat": { "title": "Coast trip questions", "answer": "In your Quokka field notes from this morning." },
        })
    );
}
