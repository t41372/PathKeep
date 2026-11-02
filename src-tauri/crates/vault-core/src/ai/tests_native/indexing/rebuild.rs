//! Responsibilities: exercise generation publication against real files, queues and databases.
//! Not responsible for: ranking or the embedding transport.
//! Failure modes (specified before implementation):
//! - a query crosses publication and pairs a plane with another generation's maps;
//! - GC removes a leased generation or a writer's unpublished files;
//! - stop before publication flips the pointer, or stop after publication cancels success;
//! - a crash before the transaction publishes prepared files on reopen;
//! - cancellation removes incremental rows from the serving lineage;
//! - replacement trusts rows of updates it never read and skips visits forever;
//! - resume uses another model's lineage or truncates its own durable source;
//! - one failed deletion prevents later independent GC items from being collected;
//! - failed row cleanup loses the identity needed to retry;
//! - a partial legacy migration removes files the old pointer needs;
//! - migration loses rows, totals or rollback safety;
//! - clear/forget removes leased files or leaves a serving pointer;
//! - serving status/signatures read mutable build sources;
//! - position fallback reads the lineage map instead of immutable forward projection;
//! - unsynced source bytes or directory entries precede the success transaction.
use super::*;
use crate::ReembedScope;
use crate::ai_queue::{
    cancel_ai_job, claim_next_ai_job, enqueue_index_job, mark_running_ai_job_cancelled,
};

/// Records a job's progress and stops the build once a chunk past `stop_after` is durable, the way
/// a pause lands between chunks.
struct JobLedger {
    job_id: i64,
    stop_after: Option<i64>,
    progress: std::sync::Mutex<Option<IndexBackfillProgress>>,
}
impl JobLedger {
    fn new(job_id: i64, stop_after: Option<i64>) -> Arc<Self> {
        Arc::new(Self { job_id, stop_after, progress: std::sync::Mutex::new(None) })
    }
    fn progress(&self) -> IndexBackfillProgress {
        self.progress.lock().unwrap().expect("the build recorded its progress")
    }
}
impl IndexBackfillLedger for JobLedger {
    fn record(&self, progress: IndexBackfillProgress) -> Result<()> {
        *self.progress.lock().unwrap() = Some(progress);
        match self.stop_after {
            Some(after) if progress.next_history_id > after => {
                Err(AiRunCancelled::new("paused for the test").into())
            }
            _ => Ok(()),
        }
    }
    fn job_id(&self) -> Option<i64> {
        Some(self.job_id)
    }
}

/// The in-app model chosen, so searches find pages by their words.
fn static_config(model: &str) -> AppConfig {
    let mut config = base_config();
    let mut provider = static_embedding_provider().config;
    provider.default_model = model.to_owned();
    config.ai.embedding_provider_id = Some(provider.id.clone());
    config.ai.embedding_providers = vec![provider];
    config
}

fn provider_for(config: &AppConfig) -> AiProviderRuntime {
    AiProviderRuntime { config: config.ai.embedding_providers[0].clone(), api_key: None }
}

/// `limit` only sets the chunk size, so a stop lands between single visits.
fn rebuild_request() -> AiIndexRequest {
    AiIndexRequest {
        full_rebuild: true,
        scope: ReembedScope::Full,
        limit: Some(1),
        ..AiIndexRequest::default()
    }
}

/// A rebuild job, claimed the way the worker claims it.
fn running_rebuild_job(vault: &crate::Vault, provider: &AiProviderRuntime) -> i64 {
    let request =
        AiIndexRequest { provider_id: Some(provider.config.id.clone()), ..rebuild_request() };
    let job = enqueue_index_job(vault, &request, false).expect("enqueue the rebuild");
    let claimed = claim_next_ai_job(vault, 300).expect("claim").expect("a queued job");
    assert_eq!(claimed.id, job.id);
    job.id
}

fn finds(
    runtime: &Runtime,
    vault: &crate::Vault,
    provider: &AiProviderRuntime,
    words: &str,
) -> Vec<String> {
    let response = runtime
        .block_on(semantic_search_history(
            vault,
            vault.paths(),
            vault.config(),
            None,
            Some(provider),
            &AiSearchRequest {
                query: words.to_string(),
                profile_id: None,
                domain: None,
                limit: Some(5),
                cursor: None,
                sort: None,
                starred_only: None,
                start_date: None,
                end_date: None,
                browser_kind: None,
            },
        ))
        .expect("semantic search");
    response.items.into_iter().map(|item| item.url).collect()
}

/// Three pages, each found by its own words, indexed completely by `model`.
fn complete_index(model: &str) -> (Runtime, crate::Vault, AiProviderRuntime, TestDatabase) {
    let runtime = Runtime::new().expect("runtime");
    let (_paths, _config, connection) = prepared_archive();
    let config = static_config(model);
    let vault = connection.vault.with_config(config.clone());
    let archive = TestDatabase::new(&vault, Db::Archive);
    for (id, path, title) in
        [(1, "a", "kestrel falcon"), (2, "b", "banana zebra"), (3, "c", "quantum otter")]
    {
        seed_visit(
            &connection,
            id,
            "chrome:Default",
            &format!("https://example.com/{path}"),
            Some(title),
            id,
        );
    }
    drop(connection);
    let provider = provider_for(&config);
    runtime
        .block_on(build_ai_index(&vault, &provider, &AiIndexRequest::default()))
        .expect("the complete index");
    (runtime, vault, provider, archive)
}

fn lease(
    vault: &crate::Vault,
    provider: &AiProviderRuntime,
) -> Arc<super::super::super::generations::GenerationLease> {
    super::super::super::generations::resolve(
        vault,
        &provider.config.id,
        &provider.config.default_model,
    )
    .unwrap()
    .expect("a serving generation")
}

#[test]
fn a_reader_keeps_one_generation_until_its_lease_is_released() {
    let (runtime, vault, provider, _archive) = complete_index(DEFAULT_STATIC_MODEL_REPO);
    let old = lease(&vault, &provider);
    let old_path = old.paths().vectors_dir.clone();
    let old_index =
        FlatVectorIndex::open(old.paths(), &provider.config.id, &provider.config.default_model)
            .unwrap();
    let old_positions = super::super::super::visit_position_map::load(
        old.paths(),
        &provider.config.id,
        &provider.config.default_model,
        &old_index,
        &vault_store::CancelToken::new(),
    )
    .unwrap();
    runtime.block_on(build_ai_index(&vault, &provider, &rebuild_request())).unwrap();
    let new = lease(&vault, &provider);
    assert_ne!(old_path, new.paths().vectors_dir);
    super::super::super::generations::collect(&vault, true).unwrap();
    assert!(old_path.exists());
    assert_eq!(
        super::super::super::visit_position_map::load(
            old.paths(),
            &provider.config.id,
            &provider.config.default_model,
            &old_index,
            &vault_store::CancelToken::new()
        )
        .unwrap(),
        old_positions
    );
    drop(old_index);
    drop(old);
    // The same collector the last-lease reaper runs is repeatable.
    super::super::super::generations::collect(&vault, true).unwrap();
    assert!(!old_path.exists());
}

#[test]
fn a_stopped_rebuild_does_not_remove_updates_of_the_serving_lineage() {
    let (runtime, vault, provider, archive) = complete_index(DEFAULT_STATIC_MODEL_REPO);
    let job = running_rebuild_job(&vault, &provider);
    let paused = JobLedger::new(job, Some(1));
    assert!(
        runtime
            .block_on(build_ai_index_with_control(
                &vault,
                &provider,
                &rebuild_request(),
                None,
                0,
                0,
                Some(paused)
            ))
            .is_err()
    );
    assert_eq!(
        ai_index_status(&vault).unwrap().indexed_items,
        3,
        "status must exclude the paused rebuild lineage"
    );
    seed_visit(&archive, 4, "chrome:Default", "https://example.com/d", Some("nebula comet"), 4);
    runtime.block_on(build_ai_index(&vault, &provider, &AiIndexRequest::default())).unwrap();
    cancel_ai_job(&vault, job).unwrap();
    mark_running_ai_job_cancelled(&vault, job, None).unwrap();
    super::super::super::generations::collect(&vault, true).unwrap();
    let live = lease(&vault, &provider);
    let lineage = live.lineage().to_owned();
    assert_eq!(
        vault
            .read(Db::Intelligence, move |s| Ok(s.query_one(
                "SELECT COUNT(*) FROM ai_embeddings WHERE history_id=4 AND lineage=?1",
                [lineage],
                |r| Ok(r.get::<i64>(0)?)
            )?))
            .unwrap(),
        1
    );
    assert!(
        finds(&runtime, &vault, &provider, "nebula").contains(&"https://example.com/d".to_string())
    );
}

#[test]
fn replacement_does_not_trust_rows_of_updates_it_never_read() {
    let (runtime, vault, provider, archive) = complete_index(DEFAULT_STATIC_MODEL_REPO);
    let job = running_rebuild_job(&vault, &provider);
    let paused = JobLedger::new(job, Some(1));
    assert!(
        runtime
            .block_on(build_ai_index_with_control(
                &vault,
                &provider,
                &rebuild_request(),
                None,
                0,
                0,
                Some(paused.clone())
            ))
            .is_err()
    );
    let cursor = paused.progress();
    seed_visit(&archive, 4, "chrome:Default", "https://example.com/d", Some("nebula comet"), 4);
    runtime.block_on(build_ai_index(&vault, &provider, &AiIndexRequest::default())).unwrap();
    // Its original captured archive boundary excludes visit 4 even on resume.
    runtime
        .block_on(build_ai_index_with_control(
            &vault,
            &provider,
            &rebuild_request(),
            None,
            cursor.next_history_id,
            cursor.embedded_so_far,
            Some(JobLedger::new(job, None)),
        ))
        .unwrap();
    let current = lease(&vault, &provider);
    let lineage = current.lineage().to_owned();
    assert_eq!(
        vault
            .read(Db::Intelligence, move |s| Ok(s.query_one(
                "SELECT COUNT(*) FROM ai_embeddings WHERE history_id=4 AND lineage=?1",
                [lineage],
                |r| Ok(r.get::<i64>(0)?)
            )?))
            .unwrap(),
        0
    );
    let report =
        runtime.block_on(build_ai_index(&vault, &provider, &AiIndexRequest::default())).unwrap();
    assert!(report.indexed_items > 0);
}

#[test]
fn late_stop_cannot_change_a_committed_success() {
    let (runtime, vault, provider, _archive) = complete_index(DEFAULT_STATIC_MODEL_REPO);
    let old = lease(&vault, &provider);
    let job = running_rebuild_job(&vault, &provider);
    runtime
        .block_on(build_ai_index_with_control(
            &vault,
            &provider,
            &rebuild_request(),
            None,
            0,
            0,
            Some(JobLedger::new(job, None)),
        ))
        .unwrap();
    assert!(cancel_ai_job(&vault, job).is_err());
    assert_eq!(crate::ai_queue::load_ai_job(&vault, job).unwrap().state, "succeeded");
    assert_ne!(old.paths().vectors_dir, lease(&vault, &provider).paths().vectors_dir);
}

#[test]
fn prepared_files_without_a_success_transaction_never_serve() {
    let (_runtime, vault, provider, _archive) = complete_index(DEFAULT_STATIC_MODEL_REPO);
    let old = lease(&vault, &provider);
    let build = super::super::super::generations::BuildGeneration::open(
        &vault, &provider, None, 987654, true, false,
    )
    .unwrap();
    let path = build.output().vectors_dir.clone();
    std::fs::write(path.join("partial"), "crash before success").unwrap();
    drop(build);
    super::super::super::generations::collect(&vault, true).unwrap();
    assert_eq!(old.paths().vectors_dir, lease(&vault, &provider).paths().vectors_dir);
    assert!(!path.exists());
}

struct StopBeforeCommit {
    vault: crate::Vault,
    job: i64,
}
impl AiRunControl for StopBeforeCommit {
    fn checkpoint(&self, detail: &str) -> Result<()> {
        if detail.contains("before projection") {
            cancel_ai_job(&self.vault, self.job)?;
        }
        Ok(())
    }
}
#[test]
fn a_stop_before_the_transaction_keeps_the_old_generation_and_deletes_only_its_own_rows() {
    let (runtime, vault, provider, _archive) = complete_index(DEFAULT_STATIC_MODEL_REPO);
    let old = lease(&vault, &provider);
    let job = running_rebuild_job(&vault, &provider);
    let stopped = runtime
        .block_on(build_ai_index_with_control(
            &vault,
            &provider,
            &rebuild_request(),
            Some(Arc::new(StopBeforeCommit { vault: vault.clone(), job })),
            0,
            0,
            Some(JobLedger::new(job, None)),
        ))
        .unwrap_err();
    assert!(stopped.downcast_ref::<AiRunCancelled>().is_some());
    assert_eq!(lease(&vault, &provider).relative(), old.relative());
    mark_running_ai_job_cancelled(&vault, job, None).unwrap();
    super::super::super::generations::collect(&vault, true).unwrap();
    let lineage = format!("j{job}");
    assert!(!old.paths().vectors_dir.parent().unwrap().parent().unwrap().join(&lineage).exists());
    assert_eq!(
        vault
            .read(Db::Intelligence, move |s| Ok(s.query_one(
                "SELECT COUNT(*) FROM ai_embeddings WHERE lineage=?1",
                [lineage],
                |r| Ok(r.get::<i64>(0)?)
            )?))
            .unwrap(),
        0
    );
    assert_eq!(lease(&vault, &provider).metadata().unwrap().vector_count, 3);
}

#[test]
fn a_gc_error_on_one_lineage_does_not_stop_other_items() {
    let (_runtime, vault, provider, _archive) = complete_index(DEFAULT_STATIC_MODEL_REPO);
    let broken = super::super::super::generations::BuildGeneration::open(
        &vault, &provider, None, 987650, true, false,
    )
    .unwrap();
    let broken_dir = broken.output().vectors_dir.parent().unwrap().to_owned();
    let valid = super::super::super::generations::BuildGeneration::open(
        &vault, &provider, None, 987651, true, false,
    )
    .unwrap();
    let valid_dir = valid.output().vectors_dir.parent().unwrap().to_owned();
    drop(broken);
    drop(valid);
    std::fs::write(broken_dir.join("identity.json"), "unreadable metadata").unwrap();
    assert!(super::super::super::generations::collect(&vault, true).is_err());
    assert!(broken_dir.exists());
    assert!(!valid_dir.exists());
    std::fs::remove_dir_all(broken_dir).unwrap();
}

#[test]
fn legacy_migration_retries_partial_moves_without_copying_sources() {
    let (paths, _config, connection) = prepared_archive();
    let vault = &connection.vault;
    let provider = embedding_provider();
    seed_visit(&connection, 1, "chrome:Default", "https://example.com/docs", Some("Docs"), 1);
    seed_embedding(&connection, 1, &provider, "hash");
    let fp = EmbeddingFingerprint::new(
        provider.config.id.clone(),
        provider.config.default_model.clone(),
        3,
        EmbeddingDtype::Float32,
        true,
        EmbeddingPooling::Unknown,
        None,
    );
    let flat = VectorStore::create_stamped(&paths, &fp).unwrap();
    flat.append_vectors(&[(11, vec![1., 0., 0.])]).unwrap();
    let map =
        VisitContentMap::for_provider(&paths, &provider.config.id, &provider.config.default_model);
    map.ensure_created(&paths).unwrap();
    map.append(&[(1, 11)]).unwrap();
    super::super::super::vector_planes::build_planes_from_store(
        &paths,
        &provider.config.id,
        &provider.config.default_model,
    )
    .unwrap();
    // Builds of the old branch shared these rows with abandoned rebuilds. Provenance for a
    // visit absent from the flat source must not make the first legacy update skip it.
    seed_visit(&connection, 2, "chrome:Default", "https://example.com/new", Some("New page"), 2);
    let hash = super::super::super::dedup::build_dedup_content_hash(
        "https://example.com/new",
        Some("New page"),
        None,
    );
    seed_embedding(&connection, 2, &provider, &hash);
    seed_successful_index_ledger(&connection, &provider, 1);
    let dir = paths
        .vectors_dir
        .join(crate::ai_sidecar::provider_table_name(
            &provider.config.id,
            &provider.config.default_model,
        ))
        .join("legacy");
    std::fs::create_dir_all(dir.join("g0")).unwrap();
    std::fs::write(dir.join("g0/partial"), "crash during staging").unwrap();
    ReverseVisitMap::for_provider(&paths, &provider.config.id, &provider.config.default_model)
        .build_from_visit_map(&paths, &map, &flat.read_header().unwrap().unwrap().fingerprint_hash)
        .unwrap();
    let before = std::fs::read(flat.path()).unwrap();
    assert!(super::super::super::generations::interrupt_after_move(vault).is_err());
    assert!(
        super::super::super::generations::resolve(
            vault,
            &provider.config.id,
            &provider.config.default_model
        )
        .unwrap()
        .is_none()
    );
    super::super::super::generations::migrate_flat_indexes(vault).unwrap();
    let live = lease(vault, &provider);
    assert_eq!(live.lineage(), "legacy");
    assert_eq!(
        std::fs::read(
            VectorStore::for_provider(
                &source_paths(vault, &provider),
                &provider.config.id,
                &provider.config.default_model
            )
            .path()
        )
        .unwrap(),
        before
    );
    assert!(!flat.exists());
    assert_eq!(live.metadata().unwrap().vector_count, 1);
    let index =
        FlatVectorIndex::open(live.paths(), &provider.config.id, &provider.config.default_model)
            .unwrap();
    let positions = super::super::super::visit_position_map::load(
        live.paths(),
        &provider.config.id,
        &provider.config.default_model,
        &index,
        &vault_store::CancelToken::new(),
    )
    .unwrap();
    assert_eq!(positions[1], 0);
    super::super::super::generations::migrate_flat_indexes(vault).unwrap();
    assert_eq!(lease(vault, &provider).relative(), live.relative());
    Runtime::new()
        .unwrap()
        .block_on(build_ai_index(vault, &provider, &AiIndexRequest::default()))
        .unwrap();
    let source = source_paths(vault, &provider);
    assert!(
        VisitContentMap::for_provider(&source, &provider.config.id, &provider.config.default_model)
            .mapped_history_ids()
            .unwrap()
            .contains(&2)
    );
}

#[test]
fn incremental_publication_also_keeps_existing_readers_on_their_generation() {
    let (runtime, vault, provider, archive) = complete_index(DEFAULT_STATIC_MODEL_REPO);
    let old = lease(&vault, &provider);
    seed_visit(&archive, 4, "chrome:Default", "https://example.com/new", Some("nebula comet"), 4);
    runtime.block_on(build_ai_index(&vault, &provider, &AiIndexRequest::default())).unwrap();
    let new = lease(&vault, &provider);
    assert_eq!(old.lineage(), new.lineage());
    assert_ne!(old.relative(), new.relative());
    assert_eq!(old.metadata().unwrap().vector_count, 3);
    assert_eq!(new.metadata().unwrap().vector_count, 4);
    let index =
        FlatVectorIndex::open(old.paths(), &provider.config.id, &provider.config.default_model)
            .unwrap();
    let positions = super::super::super::visit_position_map::load(
        old.paths(),
        &provider.config.id,
        &provider.config.default_model,
        &index,
        &vault_store::CancelToken::new(),
    )
    .unwrap();
    assert!(positions.get(4).is_none());
}

#[test]
fn clear_removes_the_pointer_and_keeps_its_leased_generation_until_release() {
    let (runtime, vault, provider, _archive) = complete_index(DEFAULT_STATIC_MODEL_REPO);
    let old = lease(&vault, &provider);
    let path = old.paths().vectors_dir.clone();
    runtime
        .block_on(build_ai_index(
            &vault,
            &provider,
            &AiIndexRequest { clear_only: true, ..Default::default() },
        ))
        .unwrap();
    assert!(
        super::super::super::generations::resolve(
            &vault,
            &provider.config.id,
            &provider.config.default_model
        )
        .unwrap()
        .is_none()
    );
    assert!(path.exists());
    drop(old);
    super::super::super::generations::collect(&vault, true).unwrap();
    assert!(!path.exists());
}

#[test]
fn damaged_build_identity_cannot_remove_a_serving_generation() {
    let (runtime, vault, provider, _archive) = complete_index(DEFAULT_STATIC_MODEL_REPO);
    let old = lease(&vault, &provider);
    let path = old.paths().vectors_dir.clone();
    std::fs::write(path.parent().unwrap().join("identity.json"), "corrupt identity").unwrap();
    assert!(
        runtime.block_on(build_ai_index(&vault, &provider, &AiIndexRequest::default())).is_err()
    );
    assert!(path.exists());
    assert_eq!(lease(&vault, &provider).relative(), old.relative());
    assert_eq!(old.metadata().unwrap().vector_count, 3);
}

#[test]
fn releasing_the_last_retired_lease_schedules_collection_without_another_job() {
    let (runtime, vault, provider, _archive) = complete_index(DEFAULT_STATIC_MODEL_REPO);
    let old = lease(&vault, &provider);
    let path = old.paths().vectors_dir.clone();
    runtime.block_on(build_ai_index(&vault, &provider, &rebuild_request())).unwrap();
    assert!(path.exists());
    drop(old);
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(10);
    while path.exists() && std::time::Instant::now() < deadline {
        std::thread::sleep(std::time::Duration::from_millis(5));
    }
    assert!(!path.exists(), "last-lease collection must not require another index job");
}

#[test]
fn a_crash_before_writing_a_lineage_identity_does_not_leak_its_directory() {
    let (_runtime, vault, provider, _archive) = complete_index(DEFAULT_STATIC_MODEL_REPO);
    let table =
        crate::ai_sidecar::provider_table_name(&provider.config.id, &provider.config.default_model);
    let abandoned = vault.paths().vectors_dir.join(table).join("r987699");
    std::fs::create_dir_all(abandoned.join("g987699")).unwrap();
    super::super::super::generations::collect(&vault, false).unwrap();
    assert!(!abandoned.exists());
}

mod partial;
mod recovery;
