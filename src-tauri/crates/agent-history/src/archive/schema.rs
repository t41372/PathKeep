//! Responsibilities: define immutable, versioned schemas for cold evidence and hot projections.
//! Not responsible for: parsing sources, data publication or choosing archive paths.
use vault_store::{Db, Migration, MigrationRegistry};

pub(super) fn migrations() -> MigrationRegistry {
    let mut registry = MigrationRegistry::default();
    registry.register(Db::SourceEvidence, steps(EVIDENCE));
    registry.register(Db::Archive, steps(PROJECTION));
    registry
}

fn steps(statements: &[&str]) -> Vec<Migration> {
    statements
        .iter()
        .enumerate()
        .map(|(index, sql)| Migration::sql(index as i64 + 1, *sql))
        .collect()
}

const EVIDENCE: &[&str] = &[
    "CREATE TABLE ah_sources (
        id TEXT PRIMARY KEY, harness TEXT NOT NULL, root TEXT NOT NULL,
        relative_path TEXT NOT NULL, kind TEXT NOT NULL,
        latest_revision_id INTEGER, observed_at INTEGER NOT NULL
    )",
    "CREATE TABLE ah_revisions (
        id INTEGER PRIMARY KEY, source_id TEXT NOT NULL REFERENCES ah_sources(id),
        digest TEXT NOT NULL DEFAULT '', byte_len INTEGER NOT NULL DEFAULT 0,
        captured_at INTEGER NOT NULL, state TEXT NOT NULL,
        capture_format TEXT NOT NULL DEFAULT 'native',
        next_ordinal INTEGER NOT NULL DEFAULT 0,
        staged_byte_len INTEGER NOT NULL DEFAULT 0
    )",
    "CREATE INDEX ah_revisions_source_state ON ah_revisions(source_id,state,id)",
    "CREATE INDEX ah_revisions_digest ON ah_revisions(source_id,digest,state)",
    "CREATE INDEX ah_revisions_state ON ah_revisions(state,id)",
    "CREATE INDEX ah_sources_location ON ah_sources(harness,root,relative_path)",
    "CREATE INDEX ah_sources_head ON ah_sources(latest_revision_id)",
    "CREATE TABLE ah_chunks (hash TEXT PRIMARY KEY, data BLOB NOT NULL)",
    "CREATE TABLE ah_revision_chunks (
        revision_id INTEGER NOT NULL REFERENCES ah_revisions(id), ordinal INTEGER NOT NULL,
        chunk_hash TEXT NOT NULL REFERENCES ah_chunks(hash), byte_len INTEGER NOT NULL,
        PRIMARY KEY(revision_id,ordinal)
    )",
    "CREATE INDEX ah_revision_chunks_hash ON ah_revision_chunks(chunk_hash)",
    "CREATE TABLE ah_parse_reports (
        revision_id INTEGER NOT NULL,parser_version TEXT NOT NULL,report_json TEXT NOT NULL,
        PRIMARY KEY(revision_id,parser_version)
    )",
    "CREATE TABLE ah_backup_runs (
        id INTEGER PRIMARY KEY,status TEXT NOT NULL,started_at INTEGER NOT NULL,
        finished_at INTEGER,report_json TEXT
    )",
    "CREATE INDEX ah_backup_runs_status ON ah_backup_runs(status,id)",
    "CREATE TABLE ah_reference_bindings (
        id INTEGER PRIMARY KEY,parent_revision INTEGER NOT NULL REFERENCES ah_revisions(id),
        reference_key TEXT NOT NULL,target_revision INTEGER NOT NULL REFERENCES ah_revisions(id),
        observed_at INTEGER NOT NULL
    )",
    "CREATE INDEX ah_reference_bindings_lookup ON ah_reference_bindings(parent_revision,reference_key,id DESC)",
    "CREATE INDEX ah_sources_harness_id ON ah_sources(harness,id)",
    "CREATE INDEX ah_sources_kind_id ON ah_sources(kind,id)",
    "CREATE INDEX ah_sources_harness_kind_id ON ah_sources(harness,kind,id)",
];

const PROJECTION: &[&str] = &[
    "CREATE TABLE ah_sessions (
        id TEXT PRIMARY KEY, source_id TEXT NOT NULL, native_id TEXT NOT NULL,
        harness TEXT NOT NULL, project_path TEXT, title TEXT, created_at INTEGER,
        updated_at INTEGER, origin TEXT NOT NULL, harness_version TEXT,
        parent_session_id TEXT, model TEXT, revision_id INTEGER NOT NULL,
        complete INTEGER NOT NULL, projection_id INTEGER NOT NULL,
        parser_version TEXT NOT NULL
    )",
    "CREATE TABLE ah_events (
        id INTEGER PRIMARY KEY, session_id TEXT NOT NULL, revision_id INTEGER NOT NULL,
        projection_id INTEGER NOT NULL, ordinal INTEGER NOT NULL,
        native_id TEXT, parent_id TEXT, kind TEXT NOT NULL,
        role TEXT, timestamp INTEGER, text TEXT NOT NULL, payload TEXT NOT NULL
    )",
    "CREATE UNIQUE INDEX ah_events_session_projection ON ah_events(session_id,projection_id,ordinal)",
    "CREATE TABLE ah_meta (key TEXT PRIMARY KEY,value TEXT NOT NULL)",
    "INSERT INTO ah_meta(key,value) VALUES('generation','0')",
    "INSERT INTO ah_meta(key,value) VALUES('next_projection_id','0')",
    "INSERT INTO ah_meta(key,value) VALUES('normalization_incomplete','0')",
    "CREATE TABLE ah_coverage (source_id TEXT PRIMARY KEY,complete INTEGER NOT NULL)",
    "CREATE INDEX ah_coverage_complete ON ah_coverage(complete)",
    "CREATE TABLE ah_outbox (
        seq INTEGER PRIMARY KEY,session_id TEXT NOT NULL,revision_id INTEGER NOT NULL,
        projection_id INTEGER NOT NULL
    )",
    "CREATE INDEX ah_outbox_session ON ah_outbox(session_id,seq)",
    "CREATE TABLE ah_projection_work (
        session_id TEXT PRIMARY KEY,revision_id INTEGER NOT NULL,
        projection_id INTEGER NOT NULL,parser_version TEXT NOT NULL
    )",
    "CREATE TABLE ah_projection_retired (
        session_id TEXT NOT NULL, projection_id INTEGER NOT NULL,
        PRIMARY KEY(session_id,projection_id)
    )",
    "CREATE INDEX ah_sessions_updated ON ah_sessions(updated_at DESC,id DESC)",
    "CREATE INDEX ah_sessions_created ON ah_sessions(created_at DESC,id DESC)",
    "CREATE INDEX ah_sessions_harness_updated ON ah_sessions(harness,updated_at DESC,id DESC)",
    "CREATE INDEX ah_sessions_harness_created ON ah_sessions(harness,created_at DESC,id DESC)",
    "CREATE INDEX ah_sessions_project_updated ON ah_sessions(project_path,updated_at DESC,id DESC)",
    "CREATE INDEX ah_sessions_project_created ON ah_sessions(project_path,created_at DESC,id DESC)",
];
