//! A Turso IO layer that encrypts the engine's spill files.
//!
//! Turso 0.8.1 writes sort, GROUP BY, hash-join, ephemeral-table and index-build spills to
//! `$TMPDIR/<random dir>/tursodb_temp_file` in plaintext, through the connection's IO object
//! (`turso_core` `io/mod.rs` `TempFile::new`). `vault-store` passes this layer to
//! `turso::Builder::with_io_impl`, so those files hold ciphertext instead.
//!
//! Responsibilities:
//! - Encrypt every file whose name is exactly `tursodb_temp_file` with AES-256-CTR under a key
//!   that exists only in memory, and decrypt it on read before the engine sees the bytes.
//! - Pass every other file (databases, WALs, the `.tshm` coordination file) to the platform IO
//!   unchanged; Turso's native encryption already covers database pages.
//!
//! Not responsible for:
//! - Turso's per-connection TEMP database (`CREATE TEMP TABLE`, `temp.`). Turso opens it
//!   through a fresh IO object of its own, so this layer never sees it, and it is plaintext.
//!   PathKeep does not use TEMP tables; scratch work goes to an ATTACHed encrypted
//!   `scratch.db`.
//! - Integrity. Spill files are scratch space the engine wrote moments ago in the same
//!   process; this layer keeps them confidential, not authenticated.
//!
//! Keys: each spill file's key is HKDF-SHA256(temp-files sub-key, salt = 32 random bytes,
//! info = `pathkeep/v1/temp-file`). The salt is never written anywhere, so the key cannot be
//! recomputed after the file is closed or the process dies, even with the password. With a
//! key per file, the CTR counter can start at zero and the keystream position is the file
//! offset.
//!
//! How it can fail:
//! - Turso renames its spill file or adds a spill path with another name, and plaintext
//!   reaches disk silently. Nothing in this layer can notice. `tests/temp_io.rs` makes Turso
//!   spill for sort, GROUP BY, hash join, CREATE INDEX and work on an ATTACHed scratch
//!   database, and scans every byte written, both through this layer and in `$TMPDIR` and the
//!   database directory. That test must be rerun on every Turso upgrade.
//! - A data path skips encryption. Writes go through `pwrite` or `pwritev`; both encrypt a
//!   copy of the buffer. `truncate` and `sync` move no data. The shared-WAL methods are never
//!   called on spill files and keep the trait's defaults.
//! - Reads return ciphertext, or decrypt the wrong range, and queries return wrong rows. The
//!   read completion is wrapped so the buffer is decrypted at its file offset, for exactly the
//!   bytes read, before the engine's callback runs. The test compares every query's rows with
//!   a run without this layer.
//! - Keystream reuse. Turso rewrites spill pages in place, and CTR at the same offset under
//!   the same key reuses keystream: two captured versions of one region XOR to the XOR of
//!   their plaintexts. An attacker would need both versions; a stolen disk normally holds the
//!   last one (plus whatever older blocks the SSD kept). Accepted for scratch data whose key
//!   dies with the process.
//! - Two files share a key. Per-file salts are 32 random bytes.
//! - A crash leaves spill directories behind. Their contents are ciphertext under a key that
//!   no longer exists anywhere.
//! - Speed. AES-CTR is cheap only with hardware AES; on ARM that needs the `aes_armv8` cfg in
//!   `.cargo/config.toml` (part-b measured 50 s instead of 7 s for a hash join without it).

use std::path::Path;
use std::ptr::NonNull;
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};

use aes::Aes256;
use aes::cipher::KeyInit;
use ctr::cipher::{InnerIvInit, StreamCipher, StreamCipherCoreWrapper, StreamCipherSeek};
use turso_core::MemoryIO;
use turso_core::io::clock::{MonotonicInstant, WallClockInstant};
use turso_core::io::{
    Buffer, Clock, Completion, File, FileId, FileSyncType, IO, OpenFlags, PlatformIO,
};

use crate::keys::{DataKey, KeyPurpose, SubKey};

/// The file name Turso 0.8.1 gives every sort, hash and ephemeral-table spill file.
pub const TURSO_TEMP_FILE_NAME: &str = "tursodb_temp_file";

type Aes256Ctr = StreamCipherCoreWrapper<ctr::CtrCore<Aes256, ctr::flavors::Ctr128BE>>;

/// Turso IO that encrypts spill files and passes everything else through.
pub struct TempCryptIo {
    inner: Arc<dyn IO>,
    key: SubKey,
    files_encrypted: AtomicU64,
}

impl TempCryptIo {
    /// Wraps the platform's default IO, as `turso::Builder` would use without a custom IO.
    pub fn new(data_key: &DataKey) -> turso_core::Result<Arc<TempCryptIo>> {
        Ok(TempCryptIo::wrapping(Arc::new(PlatformIO::new()?), data_key))
    }

    /// Wraps another IO, for callers (and tests) that need a different base layer.
    pub fn wrapping(inner: Arc<dyn IO>, data_key: &DataKey) -> Arc<TempCryptIo> {
        Arc::new(TempCryptIo {
            inner,
            key: data_key.derive(KeyPurpose::TempFiles),
            files_encrypted: AtomicU64::new(0),
        })
    }

    /// How many spill files this layer has encrypted, so callers can tell it is in use.
    pub fn files_encrypted(&self) -> u64 {
        self.files_encrypted.load(Ordering::Relaxed)
    }
}

impl std::fmt::Debug for TempCryptIo {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("TempCryptIo")
            .field("files_encrypted", &self.files_encrypted())
            .finish_non_exhaustive()
    }
}

fn is_spill_file(path: &str) -> bool {
    Path::new(path).file_name().is_some_and(|name| name == TURSO_TEMP_FILE_NAME)
}

/// A spill file: every byte at offset `pos` is XORed with the keystream at `pos`.
struct EncryptedSpillFile {
    inner: Arc<dyn File>,
    /// The expanded AES key, kept so each read and write skips the key schedule; spill files
    /// see many small operations. Wiped on drop (the `aes` crate's `zeroize` feature).
    cipher: Aes256,
}

impl EncryptedSpillFile {
    fn apply_keystream(cipher: &Aes256, pos: u64, data: &mut [u8]) {
        let core = ctr::CtrCore::<Aes256, ctr::flavors::Ctr128BE>::inner_iv_init(
            cipher.clone(),
            &[0u8; 16].into(),
        );
        let mut ctr = Aes256Ctr::from_core(core);
        ctr.seek(pos);
        ctr.apply_keystream(data);
    }

    fn encrypted_copy(&self, pos: u64, buffer: &Buffer) -> Arc<Buffer> {
        let mut data = buffer.as_slice().to_vec();
        Self::apply_keystream(&self.cipher, pos, &mut data);
        Arc::new(Buffer::new(data))
    }
}

impl File for EncryptedSpillFile {
    fn lock_file(&self, exclusive: bool) -> turso_core::Result<()> {
        self.inner.lock_file(exclusive)
    }

    fn unlock_file(&self) -> turso_core::Result<()> {
        self.inner.unlock_file()
    }

    fn pread(&self, pos: u64, c: Completion) -> turso_core::Result<Completion> {
        let buffer = c.as_read().buf_arc();
        let cipher = self.cipher.clone();
        let outer = c.clone();
        let decrypting = Completion::new_read(buffer, move |result| {
            match result {
                Ok((buffer, read)) => {
                    if read > 0 {
                        let plaintext = &mut buffer.as_mut_slice()[..read as usize];
                        Self::apply_keystream(&cipher, pos, plaintext);
                    }
                    outer.complete(read);
                }
                Err(error) => outer.error(error),
            }
            None
        });
        // The engine waits on `c`, which the wrapped completion finishes after decrypting.
        let _decrypting = self.inner.pread(pos, decrypting)?;
        Ok(c)
    }

    fn pwrite(
        &self,
        pos: u64,
        buffer: Arc<Buffer>,
        c: Completion,
    ) -> turso_core::Result<Completion> {
        let encrypted = self.encrypted_copy(pos, &buffer);
        self.inner.pwrite(pos, encrypted, c)
    }

    fn pwritev(
        &self,
        pos: u64,
        buffers: Vec<Arc<Buffer>>,
        c: Completion,
    ) -> turso_core::Result<Completion> {
        let mut at = pos;
        let encrypted = buffers
            .iter()
            .map(|buffer| {
                let copy = self.encrypted_copy(at, buffer);
                at += buffer.len() as u64;
                copy
            })
            .collect();
        self.inner.pwritev(pos, encrypted, c)
    }

    fn sync(&self, c: Completion, sync_type: FileSyncType) -> turso_core::Result<Completion> {
        self.inner.sync(c, sync_type)
    }

    fn size(&self) -> turso_core::Result<u64> {
        self.inner.size()
    }

    fn truncate(&self, len: u64, c: Completion) -> turso_core::Result<Completion> {
        self.inner.truncate(len, c)
    }
}

impl Clock for TempCryptIo {
    fn current_time_monotonic(&self) -> MonotonicInstant {
        self.inner.current_time_monotonic()
    }

    fn current_time_wall_clock(&self) -> WallClockInstant {
        self.inner.current_time_wall_clock()
    }
}

/// Every method forwards to the wrapped IO, so the platform IO's own choices (shared-WAL
/// coordination, fixed buffers, completion polling) stay in effect. Only `open_file` differs.
impl IO for TempCryptIo {
    fn open_file(
        &self,
        path: &str,
        flags: OpenFlags,
        direct: bool,
    ) -> turso_core::Result<Arc<dyn File>> {
        let file = self.inner.open_file(path, flags, direct)?;
        if !is_spill_file(path) {
            return Ok(file);
        }
        let mut salt = [0u8; 32];
        getrandom::fill(&mut salt).expect("operating system random number generator failed");
        let key = self.key.expand(&salt, b"pathkeep/v1/temp-file");
        let cipher = Aes256::new_from_slice(&key[..]).expect("spill file key is 32 bytes");
        self.files_encrypted.fetch_add(1, Ordering::Relaxed);
        Ok(Arc::new(EncryptedSpillFile { inner: file, cipher }))
    }

    fn open_shared_wal_file(&self, path: &str) -> turso_core::Result<Arc<dyn File>> {
        self.inner.open_shared_wal_file(path)
    }

    fn remove_file(&self, path: &str) -> turso_core::Result<()> {
        self.inner.remove_file(path)
    }

    fn supports_shared_wal_coordination(&self) -> bool {
        self.inner.supports_shared_wal_coordination()
    }

    fn step(&self) -> turso_core::Result<()> {
        self.inner.step()
    }

    fn cancel(&self, completions: &[Completion]) -> turso_core::Result<()> {
        self.inner.cancel(completions)
    }

    fn drain_completions(&self, completions: &[Completion]) -> turso_core::Result<()> {
        self.inner.drain_completions(completions)
    }

    fn wait_for_completion(&self, completion: Completion) -> turso_core::Result<()> {
        self.inner.wait_for_completion(completion)
    }

    fn generate_random_number(&self) -> i64 {
        self.inner.generate_random_number()
    }

    fn fill_bytes(&self, dest: &mut [u8]) {
        self.inner.fill_bytes(dest)
    }

    fn get_memory_io(&self) -> Arc<MemoryIO> {
        self.inner.get_memory_io()
    }

    fn register_fixed_buffer(&self, ptr: NonNull<u8>, len: usize) -> turso_core::Result<u32> {
        self.inner.register_fixed_buffer(ptr, len)
    }

    fn yield_now(&self) {
        self.inner.yield_now()
    }

    fn sleep(&self, duration: std::time::Duration) {
        self.inner.sleep(duration)
    }

    fn file_id(&self, path: &str) -> turso_core::Result<FileId> {
        self.inner.file_id(path)
    }
}
