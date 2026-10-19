//! Responsibilities: Open/create the index, serialize writes, publish outbox commits and
//! rebuilds.
//! Not responsible for: Outbox SQL, normalization, or encryption implementation.
//! Failure modes: Missing/corrupt/versioned indexes open silently; apply duplicates page
//! ids or loses the watermark; cancellation leaks partial rebuilds; old readers stop
//! working; failed streams publish; crashes publish mixed data. Tested in recovery and
//! crash and canonical. First builds must expose partial coverage durably.

use crate::{
    Ast, Batch, Config, Error, Filters, PageDocument, ReloadPolicy, Result, SearchSession,
};
use crate::{
    error::checked,
    filter,
    progress::Snapshot,
    schema::{self, Fields},
};
use serde::{Deserialize, Serialize};
use std::path::Path;
use std::sync::{Arc, Mutex, RwLock};
use tantivy::directory::MmapDirectory;
use tantivy::{Directory, Index, IndexReader, IndexSettings, IndexWriter, Term};
use vault_crypto::{CacheStats, DataKey, EncryptedDirectory};

/// One owning process, serialized writes, independently pinned reader sessions.
/// No canonical data or data key is retained: EncryptedDirectory owns its derived key.
pub struct SearchIndex {
    index: Index,
    lease: Arc<()>,
    reader: RwLock<PublishedReader>,
    writer: Mutex<Option<IndexWriter>>,
    fields: Fields,
    config: Config,
    directory: Option<EncryptedDirectory>,
}

struct PublishedReader {
    reader: IndexReader,
    last_seq: u64,
    initial_build: bool,
}

#[derive(Serialize, Deserialize)]
struct Payload {
    schema_version: u32,
    analyzer_version: u32,
    #[serde(default)]
    projection_version: u32,
    last_seq: u64,
    #[serde(default)]
    initial_build: bool,
}
impl Payload {
    fn new(last_seq: u64, projection_version: u32) -> Self {
        Self {
            schema_version: schema::SCHEMA_VERSION,
            analyzer_version: schema::ANALYZER_VERSION,
            projection_version,
            last_seq,
            initial_build: false,
        }
    }
    fn encode(&self) -> String {
        serde_json::to_string(self).expect("integer-only payload is serializable")
    }
}
/// Reports completed source documents, including a final update after publication.
#[derive(Clone, Copy, Debug)]
pub struct RebuildProgress {
    pub documents: u64,
    pub complete: bool,
}

impl SearchIndex {
    /// Creates a new empty index explicitly. Existing indexes are never silently reset.
    pub fn create(path: &Path, key: Option<&DataKey>, config: Config) -> Result<Self> {
        config.validate()?;
        std::fs::create_dir_all(path)?;
        let (directory, encrypted) = directory(path, key, &config)?;
        if checked(Index::exists(&*directory).map_err(Into::into), encrypted.as_ref())? {
            return Err(Error::Input(
                "index already exists; open it and rebuild explicitly".into(),
            ));
        }
        let (schema, fields) = schema::schema();
        let index = checked(
            Index::create(directory, schema, IndexSettings::default()),
            encrypted.as_ref(),
        )?;
        schema::register(&index);
        let mut writer = checked(
            index.writer_with_num_threads(config.writer_threads, config.writer_memory_bytes),
            encrypted.as_ref(),
        )?;
        let mut prepared = checked(writer.prepare_commit(), encrypted.as_ref())?;
        let mut payload = Payload::new(0, config.projection_version);
        payload.initial_build = true;
        prepared.set_payload(&payload.encode());
        checked(prepared.commit(), encrypted.as_ref())?;
        let reader = checked(
            index.reader_builder().reload_policy(tantivy::ReloadPolicy::Manual).try_into(),
            encrypted.as_ref(),
        )?;
        Ok(Self {
            index,
            lease: Arc::new(()),
            reader: RwLock::new(PublishedReader { reader, last_seq: 0, initial_build: true }),
            writer: Mutex::new(Some(writer)),
            fields,
            config,
            directory: encrypted,
        })
    }
    /// Never creates or repairs on a read path. Missing, incompatible or corrupt derived
    /// data is returned as RebuildNeeded; permission and other operational failures stay errors.
    pub fn open(path: &Path, key: Option<&DataKey>, config: Config) -> Result<Self> {
        config.validate()?;
        if !path.try_exists()? {
            return Err(Error::rebuild("index directory is missing"));
        }
        let (directory, encrypted) = directory(path, key, &config)?;
        if !checked(Index::exists(&*directory).map_err(Into::into), encrypted.as_ref())? {
            return Err(Error::rebuild("index metadata is missing"));
        }
        let index = checked(Index::open(directory), encrypted.as_ref())?;
        let (schema, fields) = schema::schema();
        if index.schema() != schema {
            return Err(Error::rebuild("schema differs from this search engine"));
        }
        schema::register(&index);
        let reader = checked(
            index.reader_builder().reload_policy(tantivy::ReloadPolicy::Manual).try_into(),
            encrypted.as_ref(),
        )?;
        let meta = checked(index.load_metas(), encrypted.as_ref())?;
        let payload: Payload = serde_json::from_str(&meta.payload.unwrap_or_default())
            .map_err(|e| Error::rebuild(format!("invalid commit payload: {e}")))?;
        let opened = Self {
            index,
            lease: Arc::new(()),
            reader: RwLock::new(PublishedReader {
                reader,
                last_seq: payload.last_seq,
                initial_build: payload.initial_build,
            }),
            writer: Mutex::new(None),
            fields,
            config,
            directory: encrypted,
        };
        opened.last_applied_seq()?;
        Ok(opened)
    }
    /// The durable watermark comes from the commit, including after an interrupted reply.
    pub fn last_applied_seq(&self) -> Result<u64> {
        let meta = checked(self.index.load_metas(), self.directory.as_ref())?;
        let payload = meta.payload.ok_or_else(|| Error::rebuild("commit has no outbox payload"))?;
        let payload: Payload = serde_json::from_str(&payload)
            .map_err(|e| Error::rebuild(format!("invalid commit payload: {e}")))?;
        if payload.schema_version != schema::SCHEMA_VERSION
            || payload.analyzer_version != schema::ANALYZER_VERSION
            || payload.projection_version != self.config.projection_version
        {
            return Err(Error::rebuild("schema/analyzer version mismatch"));
        }
        Ok(payload.last_seq)
    }
    /// A first build can publish partial coverage without acknowledging its source outbox.
    pub fn initial_build_incomplete(&self) -> Result<bool> {
        let meta = checked(self.index.load_metas(), self.directory.as_ref())?;
        let payload: Payload = serde_json::from_str(&meta.payload.unwrap_or_default())
            .map_err(|e| Error::rebuild(format!("invalid commit payload: {e}")))?;
        Ok(payload.initial_build)
    }
    /// Idempotent replay: already committed batches are ignored. Caller supplies a
    /// complete ordered outbox prefix; deleting outbox rows is the caller's responsibility.
    pub fn apply(&self, batch: Batch) -> Result<()> {
        let mut guard = self.writer.lock().expect("search writer mutex poisoned");
        if batch.last_seq <= self.last_applied_seq()? {
            return Ok(());
        }
        let writer = self.writer(&mut guard)?;
        let result = (|| -> Result<()> {
            for id in batch.deletes {
                writer.delete_term(Term::from_field_u64(self.fields.id, id));
            }
            for page in batch.upserts {
                writer.delete_term(Term::from_field_u64(self.fields.id, page.page_id));
                checked(writer.add_document(self.fields.document(page)), self.directory.as_ref())?;
            }
            checked(
                commit(writer, batch.last_seq, self.config.projection_version),
                self.directory.as_ref(),
            )
        })();
        self.finish_write(writer, result)
    }
    /// Streams a fresh generation in strictly ascending page_id order (canonical PK
    /// order), enforcing uniqueness with O(1) memory. No intermediate commit occurs.
    /// Existing sessions and newly started searches see the old commit until publication.
    pub fn rebuild<I, P, C>(&self, stream: I, last_seq: u64, mut report: P, cancel: C) -> Result<()>
    where
        I: IntoIterator<Item = Result<PageDocument>>,
        P: FnMut(RebuildProgress),
        C: Fn() -> bool,
    {
        self.rebuild_inner(stream, last_seq, &mut report, cancel, false)
    }
    /// Publishes first-open coverage every 1,024 documents. Incomplete commits carry
    /// watermark zero and an explicit marker; reopen must restart this source stream.
    pub fn build_initial<I, P, C>(
        &self,
        stream: I,
        last_seq: u64,
        mut report: P,
        cancel: C,
    ) -> Result<()>
    where
        I: IntoIterator<Item = Result<PageDocument>>,
        P: FnMut(RebuildProgress),
        C: Fn() -> bool,
    {
        if self.last_applied_seq()? != 0 {
            return Err(Error::Input("initial build cannot replace a completed index".into()));
        }
        self.rebuild_inner(stream, last_seq, &mut report, cancel, true)
    }
    fn rebuild_inner<I, P, C>(
        &self,
        stream: I,
        last_seq: u64,
        report: &mut P,
        cancel: C,
        initial: bool,
    ) -> Result<()>
    where
        I: IntoIterator<Item = Result<PageDocument>>,
        P: FnMut(RebuildProgress),
        C: Fn() -> bool,
    {
        let mut guard = self.writer.lock().expect("search writer mutex poisoned");
        if last_seq < self.last_applied_seq()? {
            return Err(Error::Input(
                "rebuild source is older than the committed outbox sequence".into(),
            ));
        }
        let writer = self.writer(&mut guard)?;
        let result = (|| {
            checked(writer.delete_all_documents(), self.directory.as_ref())?;
            let mut previous = None;
            let mut documents = 0;
            let mut publish_at = 1024;
            let mut publish_stride = 1024;
            report(RebuildProgress { documents, complete: false });
            for page in stream {
                if cancel() {
                    return Err(Error::Cancelled);
                }
                let page = page?;
                if previous.is_some_and(|id| page.page_id <= id) {
                    return Err(Error::Input(
                        "rebuild documents must have strictly increasing page_id".into(),
                    ));
                }
                previous = Some(page.page_id);
                checked(writer.add_document(self.fields.document(page)), self.directory.as_ref())?;
                documents += 1;
                if documents % 1024 == 0 {
                    if initial && documents >= publish_at {
                        let mut prepared =
                            checked(writer.prepare_commit(), self.directory.as_ref())?;
                        let mut payload = Payload::new(0, self.config.projection_version);
                        payload.initial_build = true;
                        prepared.set_payload(&payload.encode());
                        checked(prepared.commit(), self.directory.as_ref())?;
                        self.reload()?;
                        publish_stride = (publish_stride * 2).min(16384);
                        publish_at = documents + publish_stride;
                    }
                    report(RebuildProgress { documents, complete: false });
                }
            }
            if cancel() {
                return Err(Error::Cancelled);
            }
            let mut prepared = checked(writer.prepare_commit(), self.directory.as_ref())?;
            if cancel() {
                checked(prepared.abort(), self.directory.as_ref())?;
                return Err(Error::Cancelled);
            }
            prepared.set_payload(&Payload::new(last_seq, self.config.projection_version).encode());
            checked(prepared.commit(), self.directory.as_ref())?;
            Ok(documents)
        })();
        let documents = result.as_ref().copied().unwrap_or(0);
        self.finish_write(writer, result.map(|_| ()))?;
        report(RebuildProgress { documents, complete: true });
        Ok(())
    }
    /// The owner can reclaim a retired directory only after indexes and snapshots release it.
    pub fn generation_lease(&self) -> std::sync::Weak<()> {
        Arc::downgrade(&self.lease)
    }
    /// A retired writer need not retain indexing arenas while old readers finish.
    pub fn release_writer(&self) {
        self.writer.lock().expect("search writer lock").take();
    }
    pub fn num_documents(&self) -> u64 {
        self.reader.read().expect("search publication lock").reader.searcher().num_docs()
    }
    /// Pins the current reader generation and query; changes never alter this session.
    pub fn search(&self, ast: Ast, filters: Filters) -> Result<SearchSession> {
        let published = self.reader.read().expect("search publication lock");
        let searcher = published.reader.searcher();
        let query = ast.compile(&self.fields)?;
        let (query, plan) = checked(
            filter::plan(&searcher, &self.fields, &filters, query),
            self.directory.as_ref(),
        )?;
        Ok(SearchSession {
            snapshot: Arc::new(Snapshot {
                searcher,
                _lease: self.lease.clone(),
                last_seq: published.last_seq,
                initial_build: published.initial_build,
                query,
                filters,
                plan,
                directory: self.directory.clone(),
            }),
        })
    }
    /// Publishes the latest committed generation to new sessions in Manual mode.
    pub fn reload(&self) -> Result<()> {
        let mut published = self.reader.write().expect("search publication lock");
        let meta = checked(self.index.load_metas(), self.directory.as_ref())?;
        let payload: Payload = serde_json::from_str(&meta.payload.unwrap_or_default())
            .map_err(|e| Error::rebuild(format!("invalid commit payload: {e}")))?;
        checked(published.reader.reload(), self.directory.as_ref())?;
        published.last_seq = payload.last_seq;
        published.initial_build = payload.initial_build;
        Ok(())
    }
    /// Cache occupancy is observable; the configured bound covers retained cache entries.
    pub fn cache_stats(&self) -> Option<CacheStats> {
        self.directory.as_ref().map(EncryptedDirectory::cache_stats)
    }
    fn writer<'a>(&self, slot: &'a mut Option<IndexWriter>) -> Result<&'a mut IndexWriter> {
        if slot.is_none() {
            let writer: IndexWriter = checked(
                self.index.writer_with_num_threads(
                    self.config.writer_threads,
                    self.config.writer_memory_bytes,
                ),
                self.directory.as_ref(),
            )?;
            // A killed delete commit can leave <segment>.<opstamp>.del. Replay uses
            // the same opstamp, so collect uncommitted files BEFORE writing again.
            checked(writer.garbage_collect_files().wait(), self.directory.as_ref())?;
            *slot = Some(writer);
        }
        Ok(slot.as_mut().unwrap())
    }
    fn finish_write(&self, writer: &mut IndexWriter, result: Result<()>) -> Result<()> {
        if result.is_err() {
            checked(writer.rollback(), self.directory.as_ref())?;
            checked(writer.garbage_collect_files().wait(), self.directory.as_ref())?;
        }
        if self.config.reload_policy == ReloadPolicy::AfterCommit {
            self.reload()?;
        }
        result
    }
}
fn commit(writer: &mut IndexWriter, seq: u64, projection_version: u32) -> tantivy::Result<()> {
    let mut prepared = writer.prepare_commit()?;
    prepared.set_payload(&Payload::new(seq, projection_version).encode());
    #[cfg(feature = "crash-points")]
    if std::env::var("PATHKEEP_SEARCH_COMMIT_PAUSE").as_deref() == Ok("prepared") {
        use std::io::Write;
        println!("search-commit-prepared");
        let _ = std::io::stdout().flush();
        loop {
            std::thread::park();
        }
    }
    prepared.commit()?;
    Ok(())
}
fn directory(
    path: &Path,
    key: Option<&DataKey>,
    config: &Config,
) -> Result<(Box<dyn Directory>, Option<EncryptedDirectory>)> {
    match key {
        Some(key) => {
            let dir = EncryptedDirectory::open(path, key, config.cache_bytes)?;
            Ok((Box::new(dir.clone()), Some(dir)))
        }
        None => {
            Ok((Box::new(MmapDirectory::open(path).map_err(tantivy::TantivyError::from)?), None))
        }
    }
}
