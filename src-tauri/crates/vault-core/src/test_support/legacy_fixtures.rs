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
    crate::enrichment::ensure_visit_content_enrichment_schema(&source).unwrap();
    for table in ["ai_embeddings", "ai_index_ledger"] {
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
    vault.store().write(Db::Intelligence, Priority::Batch, |s| {
        s.execute("DELETE FROM ai_embedding_totals", ())?;
        s.execute("INSERT INTO ai_embedding_totals(provider_id,model,item_count,content_bytes,metadata_bytes,last_indexed_at) SELECT provider_id,model,COUNT(*),SUM(content_bytes),SUM(metadata_bytes),MAX(indexed_at) FROM ai_embeddings GROUP BY provider_id,model", ())?;
        s.execute("DELETE FROM page_content_enrichments", ())?;
        Ok(())
    }).unwrap();
    let fields = columns(&source, "visit_content_enrichments")
        .into_iter()
        .filter(|name| name != "id" && name != "history_id")
        .collect::<Vec<_>>();
    let sql = format!(
        "SELECT history_id,{} FROM visit_content_enrichments ORDER BY fetched_at,id",
        fields.join(",")
    );
    let records = source
        .prepare(&sql)
        .unwrap()
        .query_map([], |r| {
            let id = r.get::<_, i64>(0)?;
            let values = (1..=fields.len())
                .map(|i| r.get::<_, SqliteValue>(i).map(store_value))
                .collect::<rusqlite::Result<Vec<_>>>()?;
            Ok((id, values))
        })
        .unwrap()
        .collect::<rusqlite::Result<Vec<_>>>()
        .unwrap();
    for (id, mut values) in records {
        let key=vault.store().read(Db::Archive,move|s|s.query_one("SELECT p.canonical_url FROM visits v JOIN urls u ON u.id=v.url_id JOIN pages p ON p.id=u.page_id WHERE v.id=?1",[id],|r|Ok(r.get::<String>(0)?))).unwrap();
        values.insert(0, Value::Text(key));
        let mut names = vec!["canonical_url".to_owned()];
        names.extend(fields.clone());
        vault.store().write(Db::Intelligence,Priority::Batch,move|s| {
            let assignments=names.iter().filter(|name|name.as_str()!="canonical_url" && name.as_str()!="content_source").map(|name|format!("{name}=excluded.{name}")).collect::<Vec<_>>().join(",");
            s.execute(&format!("INSERT INTO page_content_enrichments({}) VALUES({}) ON CONFLICT(canonical_url,content_source) DO UPDATE SET {assignments}",names.join(","),vec!["?";names.len()].join(",")),values)?;
            Ok(())
        }).unwrap();
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

/// The SQLite agent plane PathKeep wrote before Turso (`derived/agent.sqlite`,
/// plaintext): its v1 chat and v2 run-trace schema, verbatim, so conversion tests
/// read the same tables an existing install has.
pub fn open_legacy_agent_database(paths: &ProjectPaths) -> Connection {
    std::fs::create_dir_all(&paths.derived_dir).unwrap();
    let connection = Connection::open(paths.derived_dir.join("agent.sqlite")).unwrap();
    connection.pragma_update(None, "foreign_keys", true).unwrap();
    connection
        .execute_batch(
            "CREATE TABLE agent_schema_migrations (version INTEGER PRIMARY KEY, name TEXT NOT NULL, checksum TEXT NOT NULL, applied_at TEXT NOT NULL);
             CREATE TABLE conversations (id TEXT PRIMARY KEY, title TEXT NOT NULL, provider_id TEXT, created_at TEXT NOT NULL, updated_at TEXT NOT NULL, message_count INTEGER NOT NULL DEFAULT 0);
             CREATE INDEX idx_conversations_updated ON conversations(updated_at DESC);
             CREATE TABLE messages (id TEXT PRIMARY KEY, conversation_id TEXT NOT NULL REFERENCES conversations(id) ON DELETE CASCADE, seq INTEGER NOT NULL, role TEXT NOT NULL, content TEXT NOT NULL, reasoning TEXT, tool_calls_json TEXT, status TEXT, created_at TEXT NOT NULL);
             CREATE INDEX idx_messages_conversation_seq ON messages(conversation_id, seq);
             CREATE TABLE agent_runs (id TEXT PRIMARY KEY, conversation_id TEXT REFERENCES conversations(id) ON DELETE CASCADE, message_id TEXT, provider_id TEXT, embedding_provider_id TEXT, status TEXT NOT NULL, iterations INTEGER NOT NULL DEFAULT 0, prompt_tokens INTEGER NOT NULL DEFAULT 0, completion_tokens INTEGER NOT NULL DEFAULT 0, error TEXT, created_at TEXT NOT NULL, updated_at TEXT NOT NULL);
             CREATE INDEX idx_agent_runs_conversation ON agent_runs(conversation_id);
             CREATE TABLE agent_steps (id TEXT PRIMARY KEY, run_id TEXT NOT NULL REFERENCES agent_runs(id) ON DELETE CASCADE, seq INTEGER NOT NULL, turn INTEGER NOT NULL, kind TEXT NOT NULL, tool_name TEXT, tool_call_id TEXT, payload TEXT NOT NULL, created_at TEXT NOT NULL);
             CREATE INDEX idx_agent_steps_run_seq ON agent_steps(run_id, seq);
             CREATE TABLE agent_citations (run_id TEXT NOT NULL REFERENCES agent_runs(id) ON DELETE CASCADE, history_id INTEGER NOT NULL, canonical_url TEXT NOT NULL, url TEXT NOT NULL, title TEXT, visited_at TEXT, score REAL, PRIMARY KEY(run_id, history_id));
             INSERT INTO agent_schema_migrations VALUES (1, 'chat-history-baseline', 'fixture', 'saved'), (2, 'agent-run-trace', 'fixture', 'saved');",
        )
        .unwrap();
    connection
}

/// The SQLite intelligence plane PathKeep wrote before Turso
/// (`derived/history-intelligence.sqlite`), keyed like the archive when `password`
/// is set, with the tables conversion reads: the job queue, fetched page content,
/// refind pages and the AI tables. Their schema is kept here verbatim so these
/// tests don't depend on the old openers, which are going away.
pub fn open_legacy_intelligence_database(
    paths: &ProjectPaths,
    password: Option<&str>,
) -> Connection {
    std::fs::create_dir_all(&paths.derived_dir).unwrap();
    let connection =
        Connection::open(paths.derived_dir.join("history-intelligence.sqlite")).unwrap();
    if let Some(password) = password {
        connection.pragma_update(None, "key", password).unwrap();
    }
    connection.pragma_update(None, "foreign_keys", true).unwrap();
    connection.execute_batch(LEGACY_INTELLIGENCE_SQL).unwrap();
    connection.execute_batch(crate::enrichment::VISIT_CONTENT_ENRICHMENTS_SCHEMA_SQL).unwrap();
    connection
}

const LEGACY_INTELLIGENCE_SQL: &str = r#"
CREATE TABLE IF NOT EXISTS intelligence_jobs (
  id INTEGER PRIMARY KEY AUTOINCREMENT,
  job_type TEXT NOT NULL,
  plugin_id TEXT,
  run_id INTEGER,
  state TEXT NOT NULL,
  priority INTEGER NOT NULL DEFAULT 100,
  attempt INTEGER NOT NULL DEFAULT 0,
  dedupe_key TEXT NOT NULL,
  payload_json TEXT NOT NULL,
  artifact_json TEXT NOT NULL DEFAULT '{}',
  created_at TEXT NOT NULL,
  scheduled_at TEXT NOT NULL,
  started_at TEXT,
  finished_at TEXT,
  heartbeat_at TEXT,
  lease_owner TEXT,
  lease_expires_at TEXT,
  updated_at TEXT NOT NULL,
  last_error TEXT,
  cancellation_reason TEXT,
  stop_requested INTEGER NOT NULL DEFAULT 0,
  UNIQUE(dedupe_key)
);
CREATE INDEX IF NOT EXISTS idx_intelligence_jobs_state
  ON intelligence_jobs(job_type, state, updated_at DESC);
CREATE INDEX IF NOT EXISTS idx_intelligence_jobs_plugin_state
  ON intelligence_jobs(plugin_id, state, updated_at DESC);
CREATE TABLE IF NOT EXISTS intelligence_job_triggers (
  id INTEGER PRIMARY KEY AUTOINCREMENT,
  job_id INTEGER NOT NULL REFERENCES intelligence_jobs(id) ON DELETE CASCADE,
  run_id INTEGER,
  reason TEXT,
  requested_at TEXT NOT NULL
);
CREATE INDEX IF NOT EXISTS idx_intelligence_job_triggers_job_id
  ON intelligence_job_triggers(job_id, requested_at DESC);
CREATE TABLE IF NOT EXISTS deterministic_module_runtime (
  module_id TEXT PRIMARY KEY,
  version TEXT NOT NULL,
  status TEXT NOT NULL,
  depends_on_json TEXT NOT NULL,
  derived_tables_json TEXT NOT NULL,
  last_run_id INTEGER,
  last_built_at TEXT,
  last_invalidated_at TEXT,
  stale_reason TEXT,
  stale_reason_code TEXT,
  notes_json TEXT NOT NULL DEFAULT '[]',
  updated_at TEXT NOT NULL
);
CREATE TABLE IF NOT EXISTS refind_pages (
  profile_id            TEXT NOT NULL,
  canonical_url         TEXT NOT NULL,
  url                   TEXT NOT NULL,
  title                 TEXT,
  registrable_domain    TEXT NOT NULL,
  cross_day_count       INTEGER NOT NULL,
  trail_count           INTEGER NOT NULL,
  search_arrival_count  INTEGER NOT NULL,
  typed_revisit_count   INTEGER NOT NULL,
  refind_score          REAL NOT NULL,
  evidence_json         TEXT NOT NULL,
  first_seen_ms         INTEGER NOT NULL,
  last_seen_ms          INTEGER NOT NULL,
  computed_at           TEXT NOT NULL,
  PRIMARY KEY(profile_id, canonical_url)
);

    CREATE TABLE IF NOT EXISTS ai_embeddings (
      id INTEGER PRIMARY KEY AUTOINCREMENT,
      history_id INTEGER NOT NULL,
      profile_id TEXT NOT NULL,
      url TEXT NOT NULL,
      title TEXT,
      domain TEXT NOT NULL,
      visited_at TEXT NOT NULL,
      content_hash TEXT NOT NULL,
      content_bytes INTEGER NOT NULL,
      provider_id TEXT NOT NULL,
      model TEXT NOT NULL,
      indexed_at TEXT NOT NULL,
      UNIQUE(history_id, provider_id, model, content_hash)
    );
    CREATE TABLE IF NOT EXISTS ai_assistant_runs (
      id INTEGER PRIMARY KEY AUTOINCREMENT,
      run_id INTEGER,
      question TEXT NOT NULL,
      answer TEXT NOT NULL,
      provider_id TEXT NOT NULL,
      embedding_provider_id TEXT NOT NULL,
      citations_json TEXT NOT NULL,
      notes_json TEXT NOT NULL,
      created_at TEXT NOT NULL
    );
    CREATE TABLE IF NOT EXISTS ai_index_ledger (
      provider_id TEXT NOT NULL,
      model TEXT NOT NULL,
      sidecar_table TEXT NOT NULL,
      index_version TEXT NOT NULL,
      state TEXT NOT NULL,
      source_watermark INTEGER,
      last_run_id INTEGER,
      build_started_at TEXT,
      build_finished_at TEXT,
      last_indexed_at TEXT,
      last_cleared_at TEXT,
      last_failure_at TEXT,
      failure_reason TEXT,
      PRIMARY KEY(provider_id, model)
    );
    CREATE INDEX IF NOT EXISTS idx_ai_embeddings_provider_model
      ON ai_embeddings(provider_id, model);
    CREATE INDEX IF NOT EXISTS idx_ai_embeddings_history_id
      ON ai_embeddings(history_id);
"#;
