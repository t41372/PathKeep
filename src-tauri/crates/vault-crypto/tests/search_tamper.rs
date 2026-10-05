//! A damaged encrypted index fails with an integrity error, never with different results.
//!
//! A real index is built, then copies of it are damaged in every way listed in
//! `src/search_dir/format.rs`: a flipped byte in a block, an edited header, a file cut inside
//! a block or at a block boundary, two blocks swapped, a block taken from another file, a file
//! replaced by another. After each, two things are checked:
//! - Searching it with Tantivy (open, count and rank several words, read every stored
//!   document and every fast-field value) either returns exactly what the undamaged index
//!   returned, because the damaged block was never read, or fails with `IndexFileError`.
//!   It never returns other results and never panics.
//! - Reading every file in full through the directory fails with `IndexFileError`, so the
//!   damage is caught whatever Tantivy happens to read.

mod common;

use std::panic::AssertUnwindSafe;
use std::path::{Path, PathBuf};

use common::Rng;
use common::pages;
use tantivy::schema::Value;
use tantivy::{Directory, Index, IndexSettings, IndexWriter, TantivyDocument};
use vault_crypto::{DataKey, EncryptedDirectory, IndexFileError};
use zeroize::Zeroizing;

const HEADER: usize = 53;
const SEALED_BLOCK: usize = 64 * 1024 + 16;
const PAGES: u64 = 10_000;

fn data_key() -> DataKey {
    DataKey::from_bytes(Zeroizing::new([9u8; 32]))
}

fn open_directory(dir: &Path) -> EncryptedDirectory {
    EncryptedDirectory::open(dir, &data_key(), 8 * 1024 * 1024).unwrap()
}

fn build(dir: &Path) {
    let (schema, fields) = pages::schema();
    let index = Index::create(open_directory(dir), schema, IndexSettings::default()).unwrap();
    let mut writer: IndexWriter = index.writer_with_num_threads(1, 20_000_000).unwrap();
    writer.set_merge_policy(Box::new(tantivy::indexer::NoMergePolicy));
    pages::add_pages(&writer, &fields, 0..PAGES / 2, 0);
    writer.commit().unwrap();
    pages::add_pages(&writer, &fields, PAGES / 2..PAGES, 1);
    writer.commit().unwrap();
    writer.wait_merging_threads().unwrap();
}

/// Everything a search can read, as one string; or the error that stopped it.
fn search_everything(directory: &EncryptedDirectory) -> tantivy::Result<String> {
    let (_, fields) = pages::schema();
    let index = Index::open(directory.clone())?;
    let searcher = index.reader()?.searcher();
    let mut out = String::new();
    for (word, _) in pages::probe_words(0..2_000) {
        let query = pages::title_term(&fields, &word);
        out += &format!(
            "{word}:{}:{:?};",
            pages::count(&searcher, &*query)?,
            pages::top(&searcher, &fields, &*query, 20)?
        );
    }
    for segment in searcher.segment_readers() {
        let store = segment.get_store_reader(0)?;
        for doc in 0..segment.max_doc() {
            let doc: TantivyDocument = store.get(doc)?;
            out += doc.get_first(fields.url).and_then(|v| v.as_str()).unwrap_or("");
        }
        let ids = segment.fast_fields().u64("pk_id")?;
        let visits = segment.fast_fields().i64("pk_last_visit")?;
        for doc in 0..segment.max_doc() {
            out += &format!("{:?}{:?}", ids.first(doc), visits.first(doc));
        }
    }
    Ok(out)
}

/// Reads every index file completely through the directory.
fn read_every_file(dir: &Path) -> Result<(), String> {
    let directory = open_directory(dir);
    let mut first_error = None;
    for name in index_files(dir) {
        let result = if name.ends_with(".json") {
            directory.atomic_read(Path::new(&name)).map(drop).map_err(|e| e.to_string())
        } else {
            directory
                .open_read(Path::new(&name))
                .map_err(|e| e.to_string())
                .and_then(|slice| slice.read_bytes().map(drop).map_err(|e| e.to_string()))
        };
        if let Err(error) = result {
            first_error.get_or_insert(error);
        }
    }
    first_error.map_or(Ok(()), Err)
}

fn index_files(dir: &Path) -> Vec<String> {
    let mut names: Vec<String> = std::fs::read_dir(dir)
        .unwrap()
        .map(|e| e.unwrap().file_name().into_string().unwrap())
        .filter(|n| !n.ends_with(".lock"))
        .collect();
    names.sort();
    names
}

fn copy_index(from: &Path, to: &Path) {
    std::fs::create_dir_all(to).unwrap();
    for name in index_files(from) {
        std::fs::copy(from.join(&name), to.join(&name)).unwrap();
    }
}

fn blocks_of(len: usize) -> usize {
    (len - HEADER).div_ceil(SEALED_BLOCK)
}

fn block_range(len: usize, index: usize) -> std::ops::Range<usize> {
    let start = HEADER + index * SEALED_BLOCK;
    start..(start + SEALED_BLOCK).min(len)
}

struct Damage {
    what: String,
    apply: Box<dyn Fn(&Path)>,
}

fn damages(original: &Path, rng: &mut Rng) -> Vec<Damage> {
    let files = index_files(original);
    let mut out: Vec<Damage> = Vec::new();
    let read = |name: &str| std::fs::read(original.join(name)).unwrap();
    for name in &files {
        let bytes = read(name);
        let blocks = blocks_of(bytes.len());
        // A flipped byte inside a random block.
        let block = rng.below(blocks as u64) as usize;
        let range = block_range(bytes.len(), block);
        let at = range.start + rng.below(range.len() as u64) as usize;
        let (n, b) = (name.clone(), bytes.clone());
        out.push(Damage {
            what: format!("{name}: byte {at} (block {block}) flipped"),
            apply: Box::new(move |dir| {
                let mut bytes = b.clone();
                bytes[at] ^= 0x20;
                std::fs::write(dir.join(&n), bytes).unwrap();
            }),
        });
        // Header fields: version byte, key id, salt.
        for (field, at) in
            [("key id", 5 + rng.below(16) as usize), ("salt", 21 + rng.below(32) as usize)]
        {
            let (n, b) = (name.clone(), bytes.clone());
            out.push(Damage {
                what: format!("{name}: header {field} byte {at} flipped"),
                apply: Box::new(move |dir| {
                    let mut bytes = b.clone();
                    bytes[at] ^= 0x01;
                    std::fs::write(dir.join(&n), bytes).unwrap();
                }),
            });
        }
        // Cut inside the last block.
        let cut =
            bytes.len() - 1 - rng.below(16.min(bytes.len() as u64 - HEADER as u64 - 1)) as usize;
        let (n, b) = (name.clone(), bytes.clone());
        out.push(Damage {
            what: format!("{name}: cut to {cut} of {} bytes", bytes.len()),
            apply: Box::new(move |dir| std::fs::write(dir.join(&n), &b[..cut]).unwrap()),
        });
        if blocks >= 2 {
            // Cut at a block boundary: what remains is whole, authentic blocks.
            let keep = HEADER + (blocks - 1) * SEALED_BLOCK;
            let (n, b) = (name.clone(), bytes.clone());
            out.push(Damage {
                what: format!("{name}: last block cut off"),
                apply: Box::new(move |dir| std::fs::write(dir.join(&n), &b[..keep]).unwrap()),
            });
            // Two full blocks swapped.
            if blocks >= 3 {
                let (n, b) = (name.clone(), bytes.clone());
                out.push(Damage {
                    what: format!("{name}: blocks 0 and 1 swapped"),
                    apply: Box::new(move |dir| {
                        let mut bytes = b.clone();
                        let (first, second) = (block_range(b.len(), 0), block_range(b.len(), 1));
                        bytes[first.clone()].copy_from_slice(&b[second.clone()]);
                        bytes[second].copy_from_slice(&b[first]);
                        std::fs::write(dir.join(&n), bytes).unwrap();
                    }),
                });
            }
        }
    }
    // Blocks and whole files moved between files of the same kind.
    let by_extension =
        |ext: &str| files.iter().filter(|n| n.ends_with(ext)).cloned().collect::<Vec<_>>();
    for ext in [".idx", ".store", ".fast", ".term", ".pos", ".fieldnorm"] {
        let same = by_extension(ext);
        if same.len() < 2 {
            continue;
        }
        let (a, b) = (same[0].clone(), same[1].clone());
        let (bytes_a, bytes_b) = (read(&a), read(&b));
        let (target, source) = (a.clone(), bytes_b.clone());
        out.push(Damage {
            what: format!("{a} replaced by {b}"),
            apply: Box::new(move |dir| std::fs::write(dir.join(&target), &source).unwrap()),
        });
        if blocks_of(bytes_a.len()) == blocks_of(bytes_b.len())
            || (bytes_a.len() > SEALED_BLOCK + HEADER && bytes_b.len() > SEALED_BLOCK + HEADER)
        {
            let (target, a_bytes, b_bytes) = (a.clone(), bytes_a.clone(), bytes_b.clone());
            out.push(Damage {
                what: format!("{a}: block 0 taken from {b}"),
                apply: Box::new(move |dir| {
                    let mut bytes = a_bytes.clone();
                    let range = block_range(a_bytes.len(), 0);
                    let from = block_range(b_bytes.len(), 0);
                    let len = range.len().min(from.len());
                    bytes[range.start..range.start + len]
                        .copy_from_slice(&b_bytes[from.start..from.start + len]);
                    std::fs::write(dir.join(&target), bytes).unwrap();
                }),
            });
        }
    }
    let managed = read(".managed.json");
    out.push(Damage {
        what: "meta.json replaced by .managed.json".into(),
        apply: Box::new(move |dir| std::fs::write(dir.join("meta.json"), &managed).unwrap()),
    });
    out
}

#[test]
fn every_kind_of_damage_is_an_integrity_error() {
    let work = tempfile::tempdir().unwrap();
    let original = work.path().join("original");
    std::fs::create_dir(&original).unwrap();
    build(&original);
    let baseline = search_everything(&open_directory(&original)).unwrap();
    read_every_file(&original).unwrap();

    let (mut rng, seed) = Rng::from_env_or_clock();
    println!("seed {seed}");
    let damages = damages(&original, &mut rng);
    let (mut in_error, mut in_directory, mut not_read_by_search) = (0, 0, 0);
    let mut failures = Vec::new();
    for (i, damage) in damages.iter().enumerate() {
        let dir: PathBuf = work.path().join(format!("case-{i}"));
        copy_index(&original, &dir);
        (damage.apply)(&dir);

        // Tantivy drops the error from a damaged positions file and reports its own "data
        // corruption" instead; the directory still knows (`EncryptedDirectory::integrity_error`).
        let directory = open_directory(&dir);
        match std::panic::catch_unwind(AssertUnwindSafe(|| search_everything(&directory))) {
            Err(_) => failures.push(format!("{}: search panicked", damage.what)),
            Ok(Ok(result)) if result == baseline => not_read_by_search += 1,
            Ok(Ok(_)) => {
                failures.push(format!("{}: search returned different results", damage.what))
            }
            Ok(Err(error)) => {
                match (IndexFileError::find_in(&error), directory.integrity_error()) {
                    (Some(_), _) => in_error += 1,
                    (None, Some(_)) => {
                        in_directory += 1;
                        println!(
                            "Tantivy's own error, integrity failure recorded: {}",
                            damage.what
                        );
                    }
                    (None, None) => failures.push(format!(
                        "{}: search failed without an integrity error: {error}",
                        damage.what
                    )),
                }
            }
        }
        match read_every_file(&dir) {
            Ok(()) => failures.push(format!("{}: reading every file did not notice", damage.what)),
            Err(message) if message.contains("search index file") => {}
            Err(message) => failures.push(format!("{}: unexpected error {message}", damage.what)),
        }
        std::fs::remove_dir_all(&dir).unwrap();
    }
    println!(
        "{} kinds of damage over {} files: {in_error} stopped the search with an integrity error, \
         {in_directory} with Tantivy's own error while the directory recorded the integrity failure, \
         {not_read_by_search} were in blocks the search never read (results unchanged); \
         reading every file caught all of them",
        damages.len(),
        index_files(&original).len()
    );
    assert!(failures.is_empty(), "{failures:#?}");
}
