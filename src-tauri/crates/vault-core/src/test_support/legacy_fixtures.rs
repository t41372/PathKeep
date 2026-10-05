//! Responsibilities: seed native SQLite reader fixtures and canonical lookup inputs
//! for legacy AI algorithm tests. Public starred filters are refused during migration.
//! Not responsible for: translating Turso results, proving writer-to-reader integration,
//! or converting application data. Writer tests inspect the real Vault directly.
//! Failure modes: fixtures hide writer defects, lose IDs or cross archive roots.

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

/// AI reads still use their SQLite fixture, while favorites resolve canonical IDs
/// through Vault. Seed both from those same facts, preserving each visit identity.
pub fn copy_to_vault(vault: &Vault, paths: &ProjectPaths, config: &AppConfig) {
    let source = crate::archive::open_archive_connection(paths, config, None).unwrap();
    for table in [
        "runs",
        "source_profiles",
        "urls",
        "visits",
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
                s.execute(&format!("INSERT OR IGNORE INTO {table}({}) VALUES({})",names.join(","),vec!["?";names.len()].join(",")),row)?;
            }
            Ok(())
        }).unwrap();
    }
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
    crate::archive::rebuild_search_projection(paths, config, key).unwrap();
}
