//! Responsibilities: define the unreleased evidence and rebuildable projection schemas.
//! Not responsible for: parsing sources, data publication or choosing archive paths.
use vault_store::{Db, Migration, MigrationRegistry};

pub(super) fn migrations() -> MigrationRegistry {
    let mut registry = MigrationRegistry::default();
    for db in [Db::AgentHistoryEvidence, Db::AgentHistoryReadModel] {
        registry.register(db, schema_steps(db));
    }
    registry
}

/// The migrations of one of this crate's two stores, for an owner that composes them with its
/// own: PathKeep's archive opens these stores in the same `Store` as the browsing ones.
///
/// # Panics
/// For a role this crate does not own; asking for one is a bug in the caller's composition.
pub fn schema_steps(db: Db) -> Vec<Migration> {
    match db {
        Db::AgentHistoryEvidence => steps(
            &[
                EVIDENCE,
                crate::projects::schema::EVIDENCE,
                crate::insights::EVIDENCE,
                super::move_import::SCHEMA,
            ]
            .concat(),
        ),
        Db::AgentHistoryReadModel => steps(
            &[
                PROJECTION,
                crate::read_model::publication::schema::TABLES,
                &[vault_store::PAIR_SCHEMA],
                crate::chats::schema::TABLES,
                crate::projects::schema::MODEL,
                crate::insights::MODEL,
            ]
            .concat(),
        ),
        other => panic!("agent history does not own the {other:?} store"),
    }
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
        id TEXT PRIMARY KEY,
        harness TEXT NOT NULL,
        root TEXT NOT NULL,
        relative_path TEXT NOT NULL,
        kind TEXT NOT NULL,
        retention TEXT NOT NULL,
        last_seen_at INTEGER,
        missing_since INTEGER,
        latest_revision_id INTEGER,
        observed_at INTEGER NOT NULL
    )",
    "CREATE TABLE ah_revisions (
        id INTEGER PRIMARY KEY,
        source_id TEXT NOT NULL REFERENCES ah_sources(id),
        digest TEXT NOT NULL DEFAULT '',
        byte_len INTEGER NOT NULL DEFAULT 0,
        captured_at INTEGER NOT NULL,
        state TEXT NOT NULL,
        capture_format TEXT NOT NULL DEFAULT 'native',
        codec TEXT NOT NULL,
        bytes_retained INTEGER NOT NULL DEFAULT 1,
        manifest_revision_id INTEGER,
        next_ordinal INTEGER NOT NULL DEFAULT 0,
        staged_byte_len INTEGER NOT NULL DEFAULT 0
    )",
    "CREATE INDEX ah_revisions_source_state ON ah_revisions(source_id, state, id)",
    "CREATE INDEX ah_revisions_digest ON ah_revisions(source_id, digest, state)",
    "CREATE INDEX ah_revisions_state ON ah_revisions(state, id)",
    "CREATE INDEX ah_sources_location ON ah_sources(harness, root, relative_path)",
    "CREATE INDEX ah_sources_head ON ah_sources(latest_revision_id)",
    "CREATE TABLE ah_chunks (
        hash TEXT PRIMARY KEY,
        data BLOB NOT NULL,
        codec TEXT NOT NULL
    )",
    "CREATE TABLE ah_chunk_garbage (
        hash TEXT PRIMARY KEY
    )",
    "CREATE TABLE ah_revision_chunks (
        revision_id INTEGER NOT NULL REFERENCES ah_revisions(id),
        ordinal INTEGER NOT NULL,
        chunk_hash TEXT NOT NULL REFERENCES ah_chunks(hash),
        byte_len INTEGER NOT NULL,
        byte_offset INTEGER NOT NULL,
        PRIMARY KEY(revision_id, ordinal)
    )",
    "CREATE UNIQUE INDEX ah_revision_chunks_offset ON ah_revision_chunks(revision_id,
        byte_offset)",
    "CREATE INDEX ah_revision_chunks_hash ON ah_revision_chunks(chunk_hash)",
    "CREATE INDEX ah_revisions_manifest ON ah_revisions(manifest_revision_id)",
    USER_STATE,
    "CREATE TABLE ah_user_state_pending (
        conversation_id TEXT PRIMARY KEY,
        generation INTEGER NOT NULL
    )",
    "CREATE TABLE ah_state_generation (
        id INTEGER PRIMARY KEY,
        generation INTEGER NOT NULL
    )",
    "INSERT INTO ah_state_generation VALUES(1, 0)",
    "CREATE TABLE ah_source_observations (
        run_id INTEGER NOT NULL,
        source_id TEXT NOT NULL,
        seen INTEGER NOT NULL,
        observed_at INTEGER NOT NULL,
        PRIMARY KEY(run_id, source_id)
    )",
    "CREATE INDEX ah_source_observations_source ON ah_source_observations(source_id, run_id)",
    "CREATE TABLE ah_folder_observations (
        id INTEGER PRIMARY KEY,
        path TEXT NOT NULL,
        exists_on_disk INTEGER NOT NULL,
        observed_at INTEGER NOT NULL,
        git_json TEXT
    )",
    "CREATE INDEX ah_folder_observations_path ON ah_folder_observations(path, id DESC)",
    "CREATE TABLE ah_parse_reports (
        revision_id INTEGER NOT NULL,
        parser_version TEXT NOT NULL,
        report_json TEXT NOT NULL,
        PRIMARY KEY(revision_id, parser_version)
    )",
    "CREATE TABLE ah_reference_observations (
        id INTEGER PRIMARY KEY,parent_revision INTEGER NOT NULL,reference_key TEXT NOT NULL,
        reason TEXT,observed_at INTEGER NOT NULL
    )",
    "CREATE INDEX ah_reference_observations_lookup
        ON ah_reference_observations(parent_revision,reference_key,id DESC)",
    "CREATE TABLE ah_backup_runs (
        id INTEGER PRIMARY KEY,
        status TEXT NOT NULL,
        started_at INTEGER NOT NULL,
        finished_at INTEGER,
        report_json TEXT
    )",
    "CREATE INDEX ah_backup_runs_status ON ah_backup_runs(status, id)",
    "CREATE TABLE ah_reference_bindings (
        id INTEGER PRIMARY KEY,
        parent_revision INTEGER NOT NULL REFERENCES ah_revisions(id),
        reference_key TEXT NOT NULL,
        target_revision INTEGER NOT NULL REFERENCES ah_revisions(id),
        observed_at INTEGER NOT NULL
    )",
    "CREATE INDEX ah_reference_bindings_lookup ON ah_reference_bindings(parent_revision,
        reference_key, id DESC)",
    "CREATE INDEX ah_sources_harness_id ON ah_sources(harness, id)",
    "CREATE INDEX ah_sources_kind_id ON ah_sources(kind, id)",
    "CREATE INDEX ah_sources_harness_kind_id ON ah_sources(harness, kind, id)",
    "ALTER TABLE ah_backup_runs ADD COLUMN public_id TEXT",
    "UPDATE ah_backup_runs SET public_id=lower(hex(randomblob(16)))",
    "CREATE UNIQUE INDEX ah_backup_runs_public ON ah_backup_runs(public_id)",
    "CREATE TABLE ah_run_totals(run_id INTEGER NOT NULL,harness TEXT NOT NULL,root TEXT NOT NULL,
        metric TEXT NOT NULL,quantity INTEGER NOT NULL,PRIMARY KEY(run_id,harness,root,metric))",
    "CREATE TABLE ah_run_file_problems(run_id INTEGER NOT NULL,source_id TEXT NOT NULL,metric TEXT NOT NULL,
        PRIMARY KEY(run_id,source_id,metric))",
    "CREATE TABLE ah_run_files(run_id INTEGER NOT NULL, source_id TEXT NOT NULL,
        revision_id INTEGER, bytes INTEGER NOT NULL,outcome TEXT NOT NULL,classified INTEGER NOT NULL DEFAULT 0,
        PRIMARY KEY(run_id,source_id))",
    "CREATE TABLE ah_conversation_digests(conversation_id TEXT NOT NULL,digest TEXT NOT NULL,PRIMARY KEY(conversation_id,digest))",
    "CREATE TABLE ah_source_conversations(source_id TEXT PRIMARY KEY,conversation_id TEXT NOT NULL,
        is_child INTEGER NOT NULL,revision_id INTEGER NOT NULL)",
    "CREATE INDEX ah_source_conversations_id ON ah_source_conversations(conversation_id)",
    "CREATE TABLE ah_run_conversations(run_id INTEGER NOT NULL,conversation_id TEXT NOT NULL,
        source_id TEXT NOT NULL,outcome TEXT NOT NULL,PRIMARY KEY(run_id,conversation_id))",
    "CREATE TABLE ah_root_state(harness TEXT NOT NULL,root TEXT NOT NULL,state TEXT NOT NULL,run_id INTEGER NOT NULL,
        PRIMARY KEY(harness,root))",
    "CREATE TABLE ah_run_roots(run_id INTEGER NOT NULL,root_id TEXT NOT NULL,tool TEXT NOT NULL,
        state TEXT NOT NULL,PRIMARY KEY(run_id,root_id))",
    "CREATE INDEX ah_run_roots_recent ON ah_run_roots(root_id,run_id DESC)",
    "CREATE TABLE ah_capture_selections (id TEXT PRIMARY KEY, descriptor_json TEXT NOT NULL, enable_hash TEXT)",
    "CREATE TABLE ah_desktop_backups (
        run_id INTEGER PRIMARY KEY REFERENCES ah_backup_runs(id),
        task_id TEXT NOT NULL UNIQUE, request_id TEXT NOT NULL UNIQUE,
        request_hash TEXT NOT NULL, selection_id TEXT NOT NULL REFERENCES ah_capture_selections(id),
        scope_json TEXT NOT NULL, trigger TEXT NOT NULL, retry_of TEXT,
        stop_reason TEXT, acknowledged INTEGER NOT NULL DEFAULT 0
    )",
    "CREATE TABLE ah_backup_deliveries(request_id TEXT PRIMARY KEY,request_hash TEXT NOT NULL,
        task_id TEXT NOT NULL REFERENCES ah_desktop_backups(task_id),decision TEXT NOT NULL)",
    "CREATE INDEX ah_desktop_backups_retry ON ah_desktop_backups(retry_of)",
    "CREATE INDEX ah_sources_root_id ON ah_sources(harness,root,id)",
    "CREATE INDEX ah_backup_runs_time ON ah_backup_runs(started_at DESC,id DESC)",
    "CREATE TABLE ah_capture_notes(id INTEGER PRIMARY KEY,root_id TEXT NOT NULL,tool TEXT NOT NULL,subject TEXT NOT NULL,code TEXT NOT NULL,
        qualifier TEXT NOT NULL,first_run INTEGER NOT NULL,UNIQUE(root_id,subject,code,qualifier))",
    "CREATE TABLE ah_run_notes(run_id INTEGER NOT NULL,note_id INTEGER NOT NULL REFERENCES ah_capture_notes(id),
        path TEXT,ordinal TEXT,detail TEXT,PRIMARY KEY(run_id,note_id))",
    "CREATE TABLE ah_run_note_counts(run_id INTEGER NOT NULL,root_id TEXT NOT NULL,tool TEXT NOT NULL,code TEXT NOT NULL,
        version TEXT NOT NULL,seen INTEGER NOT NULL,fresh INTEGER NOT NULL,PRIMARY KEY(run_id,root_id,code,version))",
    "CREATE TABLE ah_format_notices(tool TEXT NOT NULL,version TEXT NOT NULL,run_id INTEGER NOT NULL,
        dismissed INTEGER NOT NULL DEFAULT 0,PRIMARY KEY(tool,version))",
    "CREATE INDEX ah_format_notices_run ON ah_format_notices(run_id,dismissed)",
    "CREATE VIEW ah_pending_backup_attention AS
        SELECT d.run_id FROM ah_desktop_backups d JOIN ah_backup_runs r ON r.id=d.run_id
        WHERE r.status IN ('failed','partial') AND d.acknowledged=0
        UNION SELECT run_id FROM ah_format_notices WHERE dismissed=0",
    "ALTER TABLE ah_capture_selections ADD COLUMN first_task_id TEXT",
    "WITH timeline AS MATERIALIZED (
        SELECT s.id,MAX(CASE WHEN d.trigger='enable' THEN s.rowid END) OVER (ORDER BY s.rowid) origin
        FROM ah_capture_selections s LEFT JOIN ah_desktop_backups d ON d.task_id=s.id)
     UPDATE ah_capture_selections SET first_task_id=CASE WHEN enable_hash IS NOT NULL THEN id ELSE (
        SELECT origin.id FROM timeline t JOIN ah_capture_selections origin ON origin.rowid=t.origin
        WHERE t.id=ah_capture_selections.id) END",
    "CREATE TABLE ah_retiring_memberships(source_id TEXT PRIMARY KEY)",
    "INSERT INTO ah_retiring_memberships SELECT c.source_id
     FROM ah_sources s JOIN ah_source_conversations c ON c.source_id=s.id
     WHERE s.harness='claude-code' AND (s.relative_path='audit.jsonl' OR s.relative_path LIKE '%/audit.jsonl')",
    vault_store::PAIR_SCHEMA,
];

const USER_STATE: &str = "CREATE TABLE ah_user_state (
        conversation_id TEXT PRIMARY KEY,
        starred INTEGER NOT NULL DEFAULT 0,
        put_away INTEGER NOT NULL DEFAULT 0,
        trashed_at INTEGER,
        title TEXT
    )";

const PROJECTION: &[&str] = &[
    USER_STATE,
    "CREATE TABLE ah_state_generation (
        id INTEGER PRIMARY KEY,
        generation INTEGER NOT NULL,
        visibility INTEGER NOT NULL DEFAULT 0
    )",
    "INSERT INTO ah_state_generation(id,generation) VALUES(1,-1)",
    "CREATE TABLE ah_sessions (
        id TEXT PRIMARY KEY,
        source_id TEXT NOT NULL,
        native_id TEXT NOT NULL,
        harness TEXT NOT NULL,
        project_path TEXT,
        title TEXT,
        created_at INTEGER,
        updated_at INTEGER,
        origin TEXT NOT NULL,
        harness_version TEXT,
        parent_session_id TEXT,
        model TEXT,
        revision_id INTEGER NOT NULL,
        complete INTEGER NOT NULL,
        raw_head_current INTEGER NOT NULL DEFAULT 1,
        attachment_generation INTEGER NOT NULL DEFAULT 0,
        attachments_not_kept INTEGER NOT NULL DEFAULT 0,
        projection_id INTEGER NOT NULL,
        parser_version TEXT NOT NULL,
        conversation_id TEXT NOT NULL,
        origin_mark TEXT NOT NULL,
        first_record_digest TEXT NOT NULL,
        source_root TEXT NOT NULL,
        relative_path TEXT NOT NULL,
        relation_json TEXT NOT NULL,
        title_source TEXT NOT NULL,
        parent_path TEXT,
        generation INTEGER NOT NULL DEFAULT 0,
        current_only INTEGER NOT NULL DEFAULT 0,
        source_time TEXT,
        generation_group TEXT,
        forked_from_native_id TEXT,
        fork_path TEXT,
        rollout_native_id TEXT,
        lineage_base_native_id TEXT,
        copy_of_source_id TEXT,
        conversation_current INTEGER NOT NULL DEFAULT 0
    )",
    "CREATE INDEX ah_sessions_copy ON ah_sessions(copy_of_source_id)",
    "CREATE INDEX ah_sessions_order ON ah_sessions( conversation_id, generation DESC,
        updated_at DESC, source_time DESC, id DESC )",
    "CREATE INDEX ah_sessions_group ON ah_sessions(harness, source_root, generation_group)",
    "CREATE INDEX ah_sessions_conversation ON ah_sessions(conversation_id)",
    "CREATE INDEX ah_sessions_rollout ON ah_sessions(harness, rollout_native_id, source_root)",
    "CREATE INDEX ah_sessions_lineage ON ah_sessions(harness, lineage_base_native_id)",
    "CREATE INDEX ah_sessions_parent ON ah_sessions(harness, parent_session_id)",
    "CREATE TABLE ah_lineage_validation (
        revision_id INTEGER NOT NULL,
        end_ordinal INTEGER NOT NULL,
        byte_len INTEGER NOT NULL,
        valid INTEGER NOT NULL,
        prefix_digest TEXT,
        own_ordinal INTEGER,
        PRIMARY KEY(revision_id,end_ordinal,byte_len)
    )",
    "CREATE INDEX ah_sessions_native ON ah_sessions(harness, native_id, source_root,
        relative_path)",
    "CREATE TABLE ah_event_references (
        projection_id INTEGER NOT NULL,ordinal INTEGER NOT NULL,reference_key TEXT NOT NULL,
        occurrences INTEGER NOT NULL,PRIMARY KEY(projection_id,ordinal,reference_key)
    )",
    "CREATE TABLE ah_reference_outcomes (
        parent_revision INTEGER NOT NULL,reference_key TEXT NOT NULL,reason TEXT,
        PRIMARY KEY(parent_revision,reference_key)
    )",
    "CREATE TABLE ah_attachment_generations (
        id INTEGER PRIMARY KEY,state TEXT NOT NULL
    )",
    "CREATE TABLE ah_attachment_counts (
        generation INTEGER NOT NULL,ordinal INTEGER NOT NULL,count INTEGER NOT NULL,
        PRIMARY KEY(generation,ordinal)
    )",
    "CREATE TABLE ah_projection_gaps (
        projection_id INTEGER NOT NULL,ordinal INTEGER,code TEXT NOT NULL
    )",
    "CREATE INDEX ah_projection_gaps_lookup ON ah_projection_gaps(projection_id,ordinal)",
    "CREATE TABLE ah_events (
        id INTEGER PRIMARY KEY,
        session_id TEXT NOT NULL,
        revision_id INTEGER NOT NULL,
        projection_id INTEGER NOT NULL,
        ordinal INTEGER NOT NULL,
        native_id TEXT,
        parent_id TEXT,
        kind TEXT NOT NULL,
        role TEXT,
        timestamp INTEGER,
        text TEXT NOT NULL,
        payload TEXT NOT NULL
    )",
    "CREATE UNIQUE INDEX ah_events_session_projection ON ah_events(session_id, projection_id,
        ordinal)",
    "CREATE TABLE ah_meta (
        key TEXT PRIMARY KEY,
        value TEXT NOT NULL
    )",
    "INSERT INTO ah_meta(key, value) VALUES('generation', '0' )",
    "INSERT INTO ah_meta(key, value) VALUES('next_projection_id', '0' )",
    "CREATE TABLE ah_coverage (
        source_id TEXT PRIMARY KEY,
        complete INTEGER NOT NULL,affects_conversations INTEGER NOT NULL DEFAULT 1
    )",
    "CREATE INDEX ah_coverage_complete ON ah_coverage(complete)",
    "CREATE TABLE ah_outbox (
        seq INTEGER PRIMARY KEY,
        session_id TEXT NOT NULL,
        revision_id INTEGER NOT NULL,
        projection_id INTEGER NOT NULL
    )",
    "CREATE INDEX ah_outbox_session ON ah_outbox(session_id, seq)",
    "CREATE TABLE ah_projection_work (
        session_id TEXT PRIMARY KEY,
        revision_id INTEGER NOT NULL,
        projection_id INTEGER NOT NULL,
        parser_version TEXT NOT NULL
    )",
    "CREATE TABLE ah_projection_retired (
        session_id TEXT NOT NULL,
        projection_id INTEGER NOT NULL,
        PRIMARY KEY(session_id, projection_id)
    )",
    "CREATE INDEX ah_sessions_updated ON ah_sessions(conversation_current, updated_at DESC,
        conversation_id DESC)",
    "CREATE INDEX ah_sessions_created ON ah_sessions(conversation_current, created_at DESC,
        conversation_id DESC)",
    "CREATE INDEX ah_sessions_harness_updated ON ah_sessions(conversation_current, harness,
        updated_at DESC, conversation_id DESC)",
    "CREATE INDEX ah_sessions_harness_created ON ah_sessions(conversation_current, harness,
        created_at DESC, conversation_id DESC)",
    "CREATE INDEX ah_sessions_project_updated ON ah_sessions(conversation_current,
        project_path, updated_at DESC, conversation_id DESC)",
    "CREATE INDEX ah_sessions_project_created ON ah_sessions(conversation_current,
        project_path, created_at DESC, conversation_id DESC)",
    "CREATE INDEX ah_sessions_root_project ON ah_sessions(harness, source_root, project_path)",
];

#[cfg(test)]
mod compatibility {
    use super::*;
    use crate::archive::{AgentHistoryResources, Archive};
    use std::sync::Arc;
    use vault_store::{Config, Priority, Store};
    #[test]
    fn upgrading_an_edited_grant_recovers_its_enable_and_retires_a_legacy_audit_membership() {
        let temp = tempfile::tempdir().unwrap();
        let config = || {
            let mut c = Config::new(temp.path());
            c.databases = vec![Db::AgentHistoryEvidence, Db::AgentHistoryReadModel, Db::Scratch];
            c.migrations = migrations();
            c
        };
        let mut old = config();
        let end = EVIDENCE
            .iter()
            .position(|s| {
                s.starts_with("ALTER TABLE ah_capture_selections ADD COLUMN first_task_id")
            })
            .unwrap();
        old.migrations = MigrationRegistry::default();
        old.migrations.register(Db::AgentHistoryEvidence, steps(&EVIDENCE[..end]));
        old.migrations.register(Db::AgentHistoryReadModel, steps(PROJECTION));
        let store = Store::open(old).unwrap();
        store.write(Db::AgentHistoryEvidence,Priority::Interactive,|tx|{
            tx.execute("INSERT INTO ah_capture_selections VALUES('enable','[]','original'),('edit','[]',NULL),('other','[]','other')",())?;
            tx.execute("INSERT INTO ah_backup_runs(id,status,started_at,public_id) VALUES(1,'complete',1,'enable'),(2,'partial',2,'other')",())?;
            tx.execute("INSERT INTO ah_desktop_backups(run_id,task_id,request_id,request_hash,selection_id,scope_json,trigger)
                VALUES(1,'enable','request','original','enable','{}','enable'),(2,'other','request2','other','other','{}','enable')",())?;
            tx.execute("INSERT INTO ah_sources(id,harness,root,relative_path,kind,retention,observed_at) VALUES(
                'audit','claude-code','/fixture','local-agent-mode-sessions/a/b/local_test/audit.jsonl','transcript','appended',1)",())?;
            tx.execute("INSERT INTO ah_source_conversations VALUES('audit','retired',0,1)",())?;
            Ok(())
        }).unwrap();
        drop(store);
        let store = Arc::new(Store::open(config()).unwrap());
        let _archive = Archive::from_store(
            store.clone(),
            AgentHistoryResources { search_path: temp.path().join("search"), search_key: None },
        )
        .unwrap();
        let (first,members)=store.read(Db::AgentHistoryEvidence,|tx|tx.query_one(
            "SELECT first_task_id,(SELECT COUNT(*) FROM ah_source_conversations) FROM ah_capture_selections WHERE id='edit'",(),
            |r|Ok((r.get::<String>(0)?,r.get::<i64>(1)?)))).unwrap();
        assert_eq!(first, "enable");
        assert_eq!(members, 0);
    }
}
