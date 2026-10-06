//! Responsibilities: seed canonical test facts from independent legacy fixture inputs.
//! Not responsible for: application migration or proving production writer correctness.
//! Failure modes: lost IDs, cross-root copies, missing search handoffs and stale native read counters.
//! Tests: archive browse/count parity and the ported AI/search suites.

use crate::{AppConfig, ProjectPaths, Vault};
use rusqlite::{Connection, types::Value as SqliteValue};
use vault_store::{Db, Priority, Value};

fn columns(connection: &Connection, table: &str) -> Vec<String> {
    connection
        .prepare(&format!("PRAGMA table_info({table})"))
        .unwrap()
        .query_map([], |r| r.get(1))
        .unwrap()
        .collect::<rusqlite::Result<_>>()
        .unwrap()
}

/// Seeds native readers from fixture facts, preserving each visit identity.
pub fn copy_to_vault(vault: &Vault, paths: &ProjectPaths, config: &AppConfig) {
    let source = crate::archive::open_archive_connection(paths, config, None).unwrap();
    for table in [
        "runs",
        "source_profiles",
        "urls",
        "visits",
        "search_terms",
        "star",
        "url_annotations",
        "url_tags",
        "import_batches",
    ] {
        let fields = columns(&source, table)
            .into_iter()
            .filter(|c| c != "registrable_domain")
            .collect::<Vec<_>>();
        let mut statement =
            source.prepare(&format!("SELECT {} FROM {table}", fields.join(","))).unwrap();
        let rows = statement
            .query_map([], |r| {
                (0..fields.len())
                    .map(|i| r.get::<_, SqliteValue>(i).map(store_value))
                    .collect::<rusqlite::Result<Vec<_>>>()
            })
            .unwrap()
            .collect::<rusqlite::Result<Vec<_>>>()
            .unwrap();
        vault.write(Db::Archive,Priority::Batch,move |s| {
            for mut row in rows {
                let mut names=fields.clone();
                if table=="url_annotations" || table=="url_tags" {
                    let index=fields.iter().position(|c| c=="url").unwrap();
                    let Value::Text(url)=&row[index] else {unreachable!()};
                    names.push("canonical_url".into()); row.push(Value::Text(crate::pages::canonical_key(url)));
                }
                if table=="urls" {
                    let index=fields.iter().position(|c|c=="url").unwrap();
                    let Value::Text(url)=&row[index] else {unreachable!("fixture URL is text")};
                    let page=s.query_one("INSERT INTO pages(url,registrable_domain,canonical_url) VALUES(?1,?2,?3) ON CONFLICT(url) DO UPDATE SET url=excluded.url RETURNING id",vault_store::params![url.as_str(),crate::visit_taxonomy::registrable_domain_for_url(url),crate::pages::canonical_key(url)],|r|Ok(r.get::<i64>(0)?))?;
                    names.push("page_id".into());row.push(Value::Integer(page));
                }
                let primary=match table {"url_tags"=>"url,tag","url_annotations"=>"url","star"=>"entity_kind,entity_key",_=>"id"};
                let assignments=names.iter().filter(|name|!primary.split(',').any(|key|key==name.as_str())).map(|name|format!("{name}=excluded.{name}")).collect::<Vec<_>>().join(",");
                s.execute(&format!("INSERT INTO {table}({}) VALUES({}) ON CONFLICT({primary}) DO UPDATE SET {assignments}",names.join(","),vec!["?";names.len()].join(",")),row)?;
            }
            Ok(())
        }).unwrap();
    }
    vault
        .store()
        .write(Db::Archive, Priority::Batch, |s| {
            let pages =
                s.query("SELECT id FROM pages ORDER BY id", (), |r| Ok(r.get::<i64>(0)?))?;
            for page in pages {
                crate::archive::search_documents::refresh(s, page)?;
            }
            // Direct fixture inserts must also supply the counters native readers consume.
            // Production writes maintain these in their own fact transactions.
            for (name, table, predicate) in [
                ("sourceProfiles", "source_profiles", "enabled=1"),
                ("urls", "urls", "1"),
                ("visits", "visits", "reverted_at IS NULL"),
                ("downloads", "downloads", "reverted_at IS NULL"),
                ("manifests", "manifests", "1"),
                ("snapshots", "snapshots", "1"),
            ] {
                s.execute(&format!("UPDATE archive_totals SET value=(SELECT COUNT(*) FROM {table} WHERE {predicate}) WHERE name=?1"), [name])?;
            }
            s.execute("INSERT INTO archive_totals(name,value) SELECT 'sourceVisits:'||p.id,(SELECT COUNT(*) FROM visits v WHERE v.source_profile_id=p.id AND v.reverted_at IS NULL) FROM source_profiles p WHERE 1 ON CONFLICT(name) DO UPDATE SET value=excluded.value", ())?;
            Ok(())
        })
        .unwrap();
    vault.search().cycle().unwrap();
}

fn store_value(value: SqliteValue) -> Value {
    match value {
        SqliteValue::Null => Value::Null,
        SqliteValue::Integer(v) => Value::Integer(v),
        SqliteValue::Real(v) => Value::Real(v),
        SqliteValue::Text(v) => Value::Text(v),
        SqliteValue::Blob(v) => Value::Blob(v),
    }
}

/// Independent inputs for reader tests that have not migrated from SQLite.
/// Never accepts a Vault or reads the result of a Turso write.
pub fn seed_reader_archive(
    paths: &ProjectPaths,
    config: &AppConfig,
    key: Option<&str>,
    url: &str,
    title: &str,
) {
    seed_reader_pages(paths, config, key, &[(url, title)]);
}

/// Supplies independent SQLite facts for unported readers, never translated writer results.
pub fn seed_reader_pages(
    paths: &ProjectPaths,
    config: &AppConfig,
    key: Option<&str>,
    pages: &[(&str, &str)],
) {
    let db = crate::archive::open_archive_connection(paths, config, key).unwrap();
    let at = crate::utils::now_rfc3339();
    let ms = chrono::Utc::now().timestamp_millis();
    db.execute("INSERT OR IGNORE INTO runs(id,run_type,trigger,started_at,finished_at,status,stats_json,warnings_json) VALUES(1,'backup','manual',?1,?1,'success','{}','[]')",[&at]).unwrap();
    db.execute("INSERT OR IGNORE INTO source_profiles(id,browser_kind,browser_family,browser_product,profile_name,profile_path,discovered_at,enabled,profile_key,updated_at) VALUES(1,'chrome','chromium','Chrome','Default','/fixture',?1,1,'chrome:Default',?1)",[&at]).unwrap();
    for (index, (url, title)) in pages.iter().enumerate() {
        let id = index as i64 + 1;
        db.execute("INSERT OR IGNORE INTO urls(id,url,title,visit_count,typed_count,first_visit_ms,first_visit_iso,last_visit_ms,last_visit_iso,source_profile_id,created_by_run_id,source_url_id,registrable_domain) VALUES(?5,?1,?2,1,1,?3,?4,?3,?4,1,1,?5,'example.com')",rusqlite::params![url,title,ms,at,id]).unwrap();
        db.execute("INSERT OR IGNORE INTO visits(id,url_id,source_profile_id,visit_time_ms,visit_time_iso,created_by_run_id,source_visit_id) VALUES(?3,?3,1,?1,?2,1,?4)",rusqlite::params![ms,at,id,id.to_string()]).unwrap();
    }
    db.execute("INSERT OR IGNORE INTO downloads(id,source_profile_id,source_download_id,created_by_run_id,target_path,start_time_ms) VALUES(1,1,1,1,'/fixture/download',?1)",[ms]).unwrap();
}

/// Supplies existing semantic fixture inputs to the native read contract. Vector files
/// remain the real fixture files; this copies only metadata, never search results.
pub fn copy_search_metadata(vault: &Vault, paths: &ProjectPaths, config: &AppConfig) {
    let source = crate::archive::open_intelligence_connection(paths, config, None).unwrap();
    for table in ["ai_embeddings", "ai_index_ledger", "visit_content_enrichments"] {
        let fields = columns(&source, table);
        let mut statement =
            source.prepare(&format!("SELECT {} FROM {table}", fields.join(","))).unwrap();
        let rows = statement
            .query_map([], |r| {
                (0..fields.len())
                    .map(|i| r.get::<_, SqliteValue>(i).map(store_value))
                    .collect::<rusqlite::Result<Vec<_>>>()
            })
            .unwrap()
            .collect::<rusqlite::Result<Vec<_>>>()
            .unwrap();
        vault
            .store()
            .write(Db::Intelligence, Priority::Batch, move |s| {
                s.execute(&format!("DELETE FROM {table}"), ())?;
                for row in rows {
                    s.execute(
                        &format!(
                            "INSERT INTO {table}({}) VALUES({})",
                            fields.join(","),
                            vec!["?"; fields.len()].join(",")
                        ),
                        row,
                    )?;
                }
                Ok(())
            })
            .unwrap();
    }
    let enrichments=source.prepare("SELECT history_id,enrichment_summary,extraction_json FROM visit_content_enrichments WHERE fetch_status='success'").unwrap().query_map([],|r|Ok((r.get::<_,i64>(0)?,r.get::<_,Option<String>>(1)?,r.get::<_,Option<String>>(2)?))).unwrap().collect::<rusqlite::Result<Vec<_>>>().unwrap();
    for (visit, summary, extraction) in enrichments {
        let page = vault
            .store()
            .read(Db::Archive, move |s| {
                s.query(
                    "SELECT u.page_id FROM visits v JOIN urls u ON u.id=v.url_id WHERE v.id=?1",
                    [visit],
                    |r| Ok(r.get::<i64>(0)?),
                )
            })
            .unwrap()
            .into_iter()
            .next();
        if let Some(page) = page {
            let text = crate::archive::enrichment_text_for_index(
                summary.as_deref(),
                extraction.as_deref(),
            );
            crate::archive::set_enrichment_text(vault, page, &text).unwrap();
        }
    }
    vault.search().cycle().unwrap();
}
