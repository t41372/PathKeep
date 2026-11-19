//! Measures independently compressed rendered pages in a disposable encrypted
//! read-model database. The archive schema and the evidence reader have no dependency on it.
use agent_history::read_model::{PartKind, PhysicalOrdinal, RecordContext};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::{
    io::Read,
    path::Path,
    time::{Duration, Instant},
};
use vault_store::{Db, Priority, Store, params};

use super::{Entry, corpus::Corpus};
const DB: Db = Db::AgentHistoryReadModel;
const PAGE_BYTES: usize = 256 * 1024;
const PAGE_ROWS: usize = 256;

// Each fixture has its own database and hence exactly one publication/renderer.
#[derive(Serialize, Deserialize)]
struct Row {
    ordinal: u64,
    id: Option<String>,
    kind: String,
    role: Option<String>,
    time: Option<i64>,
    text: String,
    parts: Vec<Span>,
}
#[derive(Serialize, Deserialize)]
struct Span {
    kind: String,
    start: usize,
    end: usize,
}

fn render(ordinal: u64, ctx: &RecordContext, payload: Value) -> Row {
    let record = ctx.interpret(PhysicalOrdinal(ordinal), payload).unwrap();
    let mut text = String::new();
    let mut parts = Vec::new();
    for part in record.parts {
        let start = text.len();
        text.push_str(&part.text);
        let kind = match part.kind {
            PartKind::Text => "text",
            PartKind::Thinking => "thinking",
            PartKind::CallInput => "call_input",
            PartKind::CallResult => "call_result",
        };
        parts.push(Span { kind: kind.into(), start, end: text.len() });
    }
    Row {
        ordinal,
        id: record.event.native_id,
        kind: record.event.kind,
        role: record.event.role,
        time: record.event.timestamp,
        text,
        parts,
    }
}

/// Limits the uncompressed working set independently of rank and corpus length.
pub struct Builder<'a> {
    store: &'a Store,
    first: usize,
    count: usize,
    page: Vec<u8>,
    pub compressed_bytes: usize,
}
impl<'a> Builder<'a> {
    pub fn new(store: &'a Store) -> Self {
        store
            .write(DB, Priority::Batch, |tx| {
                tx.execute(
                    "CREATE TABLE rendered(first_rank INTEGER PRIMARY KEY, data BLOB NOT NULL)",
                    (),
                )?;
                Ok(())
            })
            .unwrap();
        Self { store, first: 0, count: 0, page: Vec::new(), compressed_bytes: 0 }
    }
    pub fn add(&mut self, rank: u64, ctx: &RecordContext, value: &Value) {
        let bytes = serde_json::to_vec(&render(rank, ctx, value.clone())).unwrap();
        assert!(bytes.len() < PAGE_BYTES, "fixture row exceeds rendered page");
        if self.count == PAGE_ROWS || self.page.len() + bytes.len() + 1 > PAGE_BYTES {
            self.flush()
        }
        assert_eq!(rank as usize, self.first + self.count);
        self.page.extend(bytes);
        self.page.push(b'\n');
        self.count += 1;
    }
    pub fn flush(&mut self) {
        if self.count == 0 {
            return;
        }
        let data = zstd::stream::encode_all(&self.page[..], 3).unwrap();
        self.compressed_bytes += data.len();
        put(self.store, self.first, data);
        self.first += self.count;
        self.count = 0;
        self.page.clear();
    }
}
fn put(store: &Store, first: usize, data: Vec<u8>) {
    store
        .write(DB, Priority::Batch, move |tx| {
            tx.execute(
                "INSERT OR REPLACE INTO rendered VALUES(?1,?2)",
                params![first as i64, data],
            )?;
            Ok(())
        })
        .unwrap()
}
fn covering(store: &Store, first: usize, count: usize) -> Vec<(usize, Vec<u8>)> {
    store
        .read(DB, move |tx| {
            let start = tx.query_one(
                "SELECT first_rank FROM rendered WHERE first_rank<=?1 ORDER BY first_rank DESC LIMIT 1",
                [first as i64],
                |row| Ok(row.get::<i64>(0)?),
            )?;
            tx.query(
                "SELECT first_rank,data FROM rendered
                 WHERE first_rank>=?1 AND first_rank<?2 ORDER BY first_rank",
                params![start, (first + count) as i64],
                |row| Ok((row.get::<i64>(0)? as usize, row.get(1)?)),
            )
        })
        .unwrap()
}
fn unpack(data: &[u8]) -> Vec<u8> {
    let mut page = Vec::new();
    zstd::stream::Decoder::new(data)
        .unwrap()
        .take(PAGE_BYTES as u64 + 1)
        .read_to_end(&mut page)
        .unwrap();
    assert!(page.len() <= PAGE_BYTES);
    page
}
fn rows(store: &Store, first: usize, count: usize) -> Vec<Row> {
    let mut result = Vec::with_capacity(count);
    for (rank, blob) in covering(store, first, count) {
        let page = unpack(&blob);
        for (i, line) in
            page.split(|byte| *byte == b'\n').filter(|line| !line.is_empty()).enumerate()
        {
            if (first..first + count).contains(&(rank + i)) {
                result.push(serde_json::from_slice(line).unwrap());
            }
        }
    }
    result
}

pub fn checked_window(store: &Store, first: usize, corpus: &Corpus) -> Duration {
    let start = Instant::now();
    let outline: Vec<Entry> = super::load_window(store, first);
    let window = rows(store, first, 200);
    let elapsed = start.elapsed();
    assert_eq!((outline.len(), window.len()), (200, 200));
    for (i, ((loc, _), row)) in outline.iter().zip(window).enumerate() {
        let rank = first + i;
        let native: Value = serde_json::from_slice(&corpus.row(rank)).unwrap();
        let text = native["message"]["content"].as_str().unwrap();
        assert_eq!(
            (row.ordinal, loc.ordinal.0, loc.start.0),
            (rank as u64, rank as u64, corpus.start(rank))
        );
        assert_eq!(row.id.as_deref(), native["uuid"].as_str());
        assert_eq!(row.role.as_deref(), Some("user"));
        assert_eq!(row.kind, "user");
        assert_eq!(row.text, text);
        assert_eq!(row.parts.len(), 1);
        let part = &row.parts[0];
        assert_eq!((&part.kind[..], part.start, part.end), ("text", 0, text.len()));
    }
    elapsed
}

pub fn append_sample(
    store: &Store,
    root: &Path,
    corpus: &Corpus,
    payload_bytes: usize,
    outline_bytes: u64,
    form: &str,
) {
    let rank = corpus.records();
    let db_file = root.join(vault_layout::database_path(DB));
    let wal =
        db_file.with_file_name(format!("{}-wal", db_file.file_name().unwrap().to_string_lossy()));
    store.pause().unwrap().checkpoint().unwrap();
    let file_bytes = db_file.metadata().unwrap().len();
    let mut value: Value = serde_json::from_slice(&corpus.row(20)).unwrap();
    value["uuid"] = format!("appended-{rank}").into();
    let native = serde_json::to_vec(&value).unwrap();
    let start = Instant::now();
    let appended =
        render(rank as u64, &RecordContext::Claude, serde_json::from_slice(&native).unwrap());
    let next = serde_json::to_vec(&appended).unwrap();
    let (mut first, previous) = covering(store, rank - 1, 1).pop().unwrap();
    let mut page = unpack(&previous);
    if rank - first == PAGE_ROWS || page.len() + next.len() + 1 > PAGE_BYTES {
        first = rank;
        page.clear();
    }
    page.extend(next);
    page.push(b'\n');
    let compressed = zstd::stream::encode_all(&page[..], 3).unwrap();
    let submitted = compressed.len();
    put(store, first, compressed);
    let elapsed = start.elapsed();
    let wal_bytes = wal.metadata().map_or(0, |m| m.len());
    let read = rows(store, rank - 1, 2);
    assert_eq!(read[0].ordinal, rank as u64 - 1);
    assert_eq!(read[1].id, appended.id);
    assert_eq!(read[1].text, appended.text);
    println!(
        "{}",
        serde_json::json!({"measurement":"rendered_storage","form":form,"outline_bytes":outline_bytes,"source_bytes":corpus.start(rank),
        "compressed_bytes":payload_bytes,"checkpointed_db_bytes":file_bytes,
        "database_bytes_per_source_gb":(file_bytes-outline_bytes) as f64*1e9/corpus.start(rank) as f64,
        "append_native_bytes":native.len(),"append_start_rank":first,"append_pages_written":1,
        "append_compressed_bytes_written":submitted,"append_wal_file_bytes":wal_bytes,"append_ms":elapsed.as_secs_f64()*1000.})
    );
}
