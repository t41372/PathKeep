//! A Tantivy `Directory` that keeps the search index encrypted on disk.
//!
//! Responsibilities:
//! - Store every file Tantivy writes (segment files while indexing and merging, `meta.json`,
//!   `.managed.json`) as authenticated AES-256-GCM blocks, from the first byte: nothing is
//!   ever written in plaintext, not even temporarily.
//! - Serve reads through a bounded cache of decrypted blocks, which the app clears on lock.
//! - Provide Tantivy's locks and change notifications.
//!
//! Not responsible for: the schema, indexing, queries, or deciding when to rebuild. The search
//! crate does those, and uses [`IndexFileError::find_in`] to recognize an index it must
//! rebuild.
//!
//! How it can fail, and where each case is handled and tested:
//! - Plaintext on disk: segment files go through `writer.rs`, atomic files are sealed in
//!   memory before the temporary file is written. `tests/search_index.rs` watches the
//!   directory during indexing and merging, keeps every file (also those deleted by merges),
//!   and scans all bytes for words from the documents, with a plaintext `MmapDirectory` as
//!   the positive control.
//! - Tampering, truncation, moved blocks or files: `format.rs` lists the checks;
//!   `tests/search_tamper.rs` damages a real index in each of these ways and requires an
//!   integrity error, never different results.
//! - A crash during indexing or merging: Tantivy writes segment files first and names them
//!   in `meta.json` last; `meta.json` is replaced atomically (`atomic_file.rs`); unfinished
//!   files have no final block and are never referred to. Tantivy's garbage collection
//!   removes them later. `tests/search_crash.rs` SIGKILLs indexing and merging processes at
//!   random moments and at each step of the `meta.json` replacement.
//! - A stale lock after a crash blocks the next writer forever. Tantivy's default
//!   `acquire_lock` creates a lock file and deletes it when dropped, which a crash skips.
//!   Locks here are OS file locks (`File::lock`), which the OS releases when the process
//!   dies. `tests/search_dir.rs` kills a child that holds the writer lock.
//! - Readers miss commits. `watch` callbacks fire when `meta.json` is replaced through this
//!   directory. That covers PathKeep, where one process owns the index; changes made by
//!   another process are not noticed, unlike `MmapDirectory`'s polling.
//! - The cache grows without bound, or plaintext stays in memory after lock: see `cache.rs`.
//! - Names and sizes are visible. File names (segment ids and extensions), sizes, and times
//!   are not hidden; sizes reveal roughly how large the index is.

mod cache;
mod file;
mod format;
mod writer;

use std::fmt;
use std::fs::{self, File, OpenOptions, TryLockError};
use std::io::{self, BufWriter};
use std::path::{Path, PathBuf};
use std::sync::Arc;

use tantivy::directory::error::{DeleteError, LockError, OpenReadError, OpenWriteError};
use tantivy::directory::{
    Directory, DirectoryLock, FileHandle, Lock, WatchCallback, WatchCallbackList, WatchHandle,
    WritePtr,
};

use self::cache::BlockCache;
pub use self::cache::CacheStats;
use self::file::EncryptedFile;
pub use self::format::IndexFileError;
use self::format::{FileCipher, FileName, IntegrityLog, Layout};
use self::writer::EncryptedWriter;
use zeroize::Zeroizing;

use crate::atomic_file;
use crate::keys::{DataKey, KeyPurpose, SubKey};

/// The default plaintext block cache size.
pub const DEFAULT_CACHE_BYTES: usize = 64 * 1024 * 1024;

/// The search index's directory. Cloning is cheap; clones share the key, the cache and the
/// watchers, which is how Tantivy uses it (it clones the directory it is given).
#[derive(Clone)]
pub struct EncryptedDirectory {
    inner: Arc<Inner>,
}

struct Inner {
    root: PathBuf,
    key: SubKey,
    key_id: [u8; 16],
    cache: Arc<BlockCache>,
    integrity: Arc<IntegrityLog>,
    watchers: WatchCallbackList,
}

impl EncryptedDirectory {
    /// Opens the index directory `root`, which must exist, with the search-index key derived
    /// from `data_key`. At most `cache_bytes` of decrypted blocks are kept in memory.
    pub fn open(
        root: &Path,
        data_key: &DataKey,
        cache_bytes: usize,
    ) -> io::Result<EncryptedDirectory> {
        if !root.is_dir() {
            return Err(io::Error::new(
                io::ErrorKind::NotFound,
                format!("{} is not a directory", root.display()),
            ));
        }
        let key = data_key.derive(KeyPurpose::SearchIndex);
        let key_id = format::key_id(&key);
        Ok(EncryptedDirectory {
            inner: Arc::new(Inner {
                root: root.to_path_buf(),
                key,
                key_id,
                cache: Arc::new(BlockCache::new(cache_bytes)),
                integrity: Arc::default(),
                watchers: WatchCallbackList::default(),
            }),
        })
    }

    /// Drops and wipes every cached plaintext block. The app calls this when the vault locks,
    /// after dropping its index readers.
    pub fn clear_cache(&self) {
        self.inner.cache.clear();
    }

    pub fn cache_stats(&self) -> CacheStats {
        self.inner.cache.stats()
    }

    /// The first integrity failure that any read through this directory (or a clone) ran
    /// into since it was opened. Check it when Tantivy reports data corruption: Tantivy
    /// replaces some read errors with its own messages (see [`IndexFileError::find_in`]).
    pub fn integrity_error(&self) -> Option<IndexFileError> {
        self.inner.integrity.first()
    }

    fn path(&self, relative: &Path) -> PathBuf {
        self.inner.root.join(relative)
    }

    fn cipher_for(&self, name: &Path, header: &[u8]) -> Result<FileCipher, IndexFileError> {
        FileCipher::parse(&self.inner.key, &self.inner.key_id, &FileName(name), header)
    }
}

impl fmt::Debug for EncryptedDirectory {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "EncryptedDirectory({})", self.inner.root.display())
    }
}

fn open_read_error(error: io::Error, path: &Path) -> OpenReadError {
    if error.kind() == io::ErrorKind::NotFound {
        OpenReadError::FileDoesNotExist(path.to_path_buf())
    } else {
        OpenReadError::wrap_io_error(error, path.to_path_buf())
    }
}

impl Directory for EncryptedDirectory {
    fn get_file_handle(&self, path: &Path) -> Result<Arc<dyn FileHandle>, OpenReadError> {
        let file = File::open(self.path(path)).map_err(|e| open_read_error(e, path))?;
        let opened = EncryptedFile::open(
            path.to_path_buf(),
            file,
            |header| self.cipher_for(path, header),
            self.inner.cache.clone(),
            self.inner.integrity.clone(),
        )
        .map_err(|e| OpenReadError::wrap_io_error(e, path.to_path_buf()))?;
        Ok(Arc::new(opened))
    }

    fn delete(&self, path: &Path) -> Result<(), DeleteError> {
        fs::remove_file(self.path(path)).map_err(|error| {
            if error.kind() == io::ErrorKind::NotFound {
                DeleteError::FileDoesNotExist(path.to_path_buf())
            } else {
                DeleteError::IoError { io_error: Arc::new(error), filepath: path.to_path_buf() }
            }
        })
    }

    fn exists(&self, path: &Path) -> Result<bool, OpenReadError> {
        self.path(path)
            .try_exists()
            .map_err(|e| OpenReadError::wrap_io_error(e, path.to_path_buf()))
    }

    fn open_write(&self, path: &Path) -> Result<WritePtr, OpenWriteError> {
        let file = OpenOptions::new().write(true).create_new(true).open(self.path(path)).map_err(
            |error| {
                if error.kind() == io::ErrorKind::AlreadyExists {
                    OpenWriteError::FileAlreadyExists(path.to_path_buf())
                } else {
                    OpenWriteError::wrap_io_error(error, path.to_path_buf())
                }
            },
        )?;
        let cipher = FileCipher::new_file(&self.inner.key, &self.inner.key_id);
        let writer = EncryptedWriter::start(path.to_path_buf(), file, cipher)
            .map_err(|e| OpenWriteError::wrap_io_error(e, path.to_path_buf()))?;
        Ok(BufWriter::new(Box::new(writer)))
    }

    fn atomic_read(&self, path: &Path) -> Result<Vec<u8>, OpenReadError> {
        let mut sealed =
            Zeroizing::new(fs::read(self.path(path)).map_err(|e| open_read_error(e, path))?);
        open_whole_file(self, path, &mut sealed)
            .map_err(|e| OpenReadError::wrap_io_error(e, path.to_path_buf()))
    }

    fn atomic_write(&self, path: &Path, data: &[u8]) -> io::Result<()> {
        let cipher = FileCipher::new_file(&self.inner.key, &self.inner.key_id);
        let sealed = format::seal_whole_file(&cipher, &FileName(path), data);
        atomic_file::replace(&self.path(path), &sealed)?;
        // Tantivy's `META_FILEPATH`, which it does not export. Readers reload on this file.
        if path == Path::new("meta.json") {
            drop(self.inner.watchers.broadcast());
        }
        Ok(())
    }

    fn sync_directory(&self) -> io::Result<()> {
        atomic_file::sync_directory(&self.inner.root)
    }

    // `File::lock` is stable since Rust 1.89. The workspace builds with the toolchain pinned in
    // rust-toolchain.toml (1.96), above the `msrv` that clippy.toml still names.
    #[clippy::msrv = "1.89"]
    fn acquire_lock(&self, lock: &Lock) -> Result<DirectoryLock, LockError> {
        let file = OpenOptions::new()
            .write(true)
            .create(true)
            .truncate(false)
            .open(self.path(&lock.filepath))
            .map_err(LockError::wrap_io_error)?;
        if lock.is_blocking {
            file.lock().map_err(LockError::wrap_io_error)?;
        } else {
            match file.try_lock() {
                Ok(()) => {}
                Err(TryLockError::WouldBlock) => return Err(LockError::LockBusy),
                Err(TryLockError::Error(error)) => return Err(LockError::wrap_io_error(error)),
            }
        }
        // Closing the file releases the lock; so does the OS when the process dies.
        Ok(DirectoryLock::from(Box::new(file)))
    }

    fn watch(&self, callback: WatchCallback) -> tantivy::Result<WatchHandle> {
        Ok(self.inner.watchers.subscribe(callback))
    }
}

/// Decrypts a whole file read into memory, block by block in place.
fn open_whole_file(
    directory: &EncryptedDirectory,
    path: &Path,
    sealed: &mut [u8],
) -> io::Result<Vec<u8>> {
    let name = FileName(path);
    let header_len = sealed.len().min(format::HEADER_LEN);
    let log = &directory.inner.integrity;
    let cipher = directory.cipher_for(path, &sealed[..header_len]).map_err(|e| log.record(e))?;
    let layout = Layout::from_file_len(sealed.len() as u64, &name).map_err(|e| log.record(e))?;
    let mut plaintext = Vec::with_capacity(layout.plaintext_len as usize);
    for index in 0..layout.blocks {
        let (offset, len) = layout.sealed_range(index);
        let block = &mut sealed[offset as usize..offset as usize + len];
        let plain_len =
            cipher.open(&name, index, layout.is_final(index), block).map_err(|e| log.record(e))?;
        plaintext.extend_from_slice(&block[..plain_len]);
    }
    Ok(plaintext)
}
