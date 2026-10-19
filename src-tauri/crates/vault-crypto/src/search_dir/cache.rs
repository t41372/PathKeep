//! A bounded, least-recently-used cache of decrypted index data.
//!
//! Responsibilities: keep recently read plaintext so repeated reads skip disk and AES-GCM,
//! never hold more than the configured number of plaintext bytes, and let the plaintext be
//! wiped once nothing uses it (the app clears the cache on lock).
//!
//! It holds two kinds of entries under one byte budget:
//! - single 64 KiB blocks, from which small reads (postings, stored-document blocks) copy;
//! - whole ranges of 64 KiB or more, returned to Tantivy without copying. Tantivy asks for the
//!   same large ranges on every query (a fast-field column, a field's norms, a term's postings)
//!   and, with `MmapDirectory`, gets them for free. Copying them per query would cost time and
//!   memory in proportion to the index (field norms alone are one byte per document per
//!   scored field).
//!
//! Not responsible for: plaintext Tantivy still holds. A range handed out stays readable by
//! Tantivy after the cache evicts it or is cleared, and is wiped when Tantivy drops its last
//! reference. The app must drop its `IndexReader`s and `Index` on lock as well.
//!
//! How it can fail:
//! - Memory grows past the limit. Every insert evicts least-recently-used entries until the
//!   total is within the limit; an entry larger than the whole limit is not cached at all.
//!   `tests/search_index.rs` reads an index several times its limit and checks both the
//!   cache's own count and the process's resident memory. Memory that Tantivy holds through
//!   handed-out ranges is outside this limit, as it is outside any cache with `MmapDirectory`.
//! - A stale entry is served after a file was rewritten. Entries are keyed by the file's
//!   random salt, which changes with every write, never by its name alone.
//! - An entry verified for one file is served for another, or a block verified as a middle
//!   block is served as the last one. If the key were the salt alone, a file copied over
//!   another name, or cut short, would share the original's cached blocks and skip the name
//!   and final-block checks (the tamper test found exactly this). The key is SHA-256 of the
//!   salt, the file's length and its name, computed when the file is opened; a file changed
//!   in any of these ways is a different file to the cache and is verified again.
//! - Plaintext lingers after eviction. Every entry is a buffer that wipes itself when its last
//!   reference goes.
//! - Lock contention between search threads. One mutex guards a hash map and a recency index;
//!   decryption happens outside it. The benchmark runs four threads at once.

use std::collections::{BTreeMap, HashMap};
use std::ops::Deref;
use std::sync::Mutex;

use tantivy::directory::OwnedBytes;

/// SHA-256 of a file's salt, length and name: one version of one file, as found on disk.
pub(crate) type FileIdentity = [u8; 32];

#[derive(Clone, Copy, PartialEq, Eq, Hash)]
pub(crate) enum CacheKey {
    /// Block `index` of a file.
    Block(FileIdentity, u64),
    /// Plaintext bytes `start..end` of a file.
    Range(FileIdentity, u64, u64),
}

/// Counters for tests, diagnostics and the benchmark.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CacheStats {
    /// Plaintext bytes the cache holds now.
    pub bytes: usize,
    pub entries: usize,
    pub limit: usize,
    pub hits: u64,
    pub misses: u64,
}

pub(crate) struct BlockCache {
    limit: usize,
    state: Mutex<State>,
}

#[derive(Default)]
struct State {
    entries: HashMap<CacheKey, (OwnedBytes, u64)>,
    /// Last-use tick → entry, oldest first.
    recency: BTreeMap<u64, CacheKey>,
    next_tick: u64,
    bytes: usize,
    hits: u64,
    misses: u64,
}

impl BlockCache {
    pub(crate) fn new(limit: usize) -> BlockCache {
        BlockCache { limit, state: Mutex::default() }
    }

    /// A cheap clone of the cached bytes (they are reference-counted, not copied).
    pub(crate) fn get(&self, key: &CacheKey) -> Option<OwnedBytes> {
        let mut state = self.state.lock().unwrap();
        let state = &mut *state;
        let tick = state.next_tick;
        match state.entries.get_mut(key) {
            Some((bytes, last_used)) => {
                state.recency.remove(last_used);
                state.recency.insert(tick, *key);
                *last_used = tick;
                state.next_tick += 1;
                state.hits += 1;
                Some(bytes.clone())
            }
            None => {
                state.misses += 1;
                None
            }
        }
    }

    /// Adds an entry and returns the bytes to use: the ones already cached if another thread
    /// got there first.
    pub(crate) fn insert(&self, key: CacheKey, bytes: OwnedBytes) -> OwnedBytes {
        if bytes.len() > self.limit {
            return bytes;
        }
        let mut state = self.state.lock().unwrap();
        if let Some((existing, _)) = state.entries.get(&key) {
            return existing.clone();
        }
        while state.bytes + bytes.len() > self.limit {
            let (_, oldest) =
                state.recency.pop_first().expect("the cache holds bytes, so it holds entries");
            let (evicted, _) = state.entries.remove(&oldest).unwrap();
            state.bytes -= evicted.len();
        }
        let tick = state.next_tick;
        state.next_tick += 1;
        state.bytes += bytes.len();
        state.recency.insert(tick, key);
        state.entries.insert(key, (bytes.clone(), tick));
        bytes
    }

    /// Drops every entry. Each is wiped as its last reference goes.
    pub(crate) fn clear(&self) {
        let mut state = self.state.lock().unwrap();
        state.entries.clear();
        state.recency.clear();
        state.bytes = 0;
    }

    pub(crate) fn stats(&self) -> CacheStats {
        let state = self.state.lock().unwrap();
        CacheStats {
            bytes: state.bytes,
            entries: state.entries.len(),
            limit: self.limit,
            hits: state.hits,
            misses: state.misses,
        }
    }
}

/// Plaintext in memory, wiped when the last reference to it goes. Everything the cache holds,
/// and everything handed to Tantivy, is one of these inside an `OwnedBytes`.
pub(crate) struct WipedBytes(pub(crate) Vec<u8>);

impl WipedBytes {
    pub(crate) fn into_owned_bytes(self) -> OwnedBytes {
        OwnedBytes::new(self)
    }
}

impl Deref for WipedBytes {
    type Target = [u8];
    fn deref(&self) -> &[u8] {
        &self.0
    }
}

impl Drop for WipedBytes {
    fn drop(&mut self) {
        zeroize::Zeroize::zeroize(&mut self.0);
    }
}

// SAFETY: the bytes live in the Vec's heap allocation, which never moves or changes while
// the WipedBytes exists (nothing mutates it until drop), so references stay valid when the
// WipedBytes value itself moves.
unsafe impl ownedbytes::StableDeref for WipedBytes {}
