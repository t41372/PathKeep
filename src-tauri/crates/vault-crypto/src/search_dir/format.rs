//! The on-disk format of one encrypted search index file, and sealing and opening its blocks.
//!
//! Responsibilities: the header layout, where each block lives, the per-file key, the nonce
//! and the associated data of each block, and the errors that say why a file cannot be read.
//!
//! Not responsible for: file IO, caching, or Tantivy's API (`writer.rs`, `file.rs`, `cache.rs`,
//! `mod.rs`).
//!
//! Format, version 1:
//!
//! ```text
//! header   magic "PKSI" (4) | version 1 (1) | key id (16) | file salt (32)      = 53 bytes
//! block 0  AES-256-GCM ciphertext (BLOCK_LEN bytes) | tag (16)
//! block 1  ...
//! last     ciphertext (0..=BLOCK_LEN bytes) | tag (16)
//! ```
//!
//! - Every block but the last holds exactly `BLOCK_LEN` (64 KiB) of plaintext, so block `i`
//!   starts at `HEADER_LEN + i * (BLOCK_LEN + TAG_LEN)` and the plaintext length follows from
//!   the file size without decrypting anything.
//! - File key: HKDF-SHA256(search-index sub-key, salt = file salt, info =
//!   `pathkeep/v1/search-index-file`). The salt is random for every file written, including
//!   each rewrite of `meta.json`, so no two files share a key.
//! - Nonce: the block index as u64 little-endian, then four zero bytes. Each file key seals
//!   each block index once, so a nonce never repeats under a key.
//! - Associated data of block `i`: the header, `i` (u64 LE), the block's plaintext length
//!   (u32 LE), a final-block flag (1 byte), then the file's name inside the index directory.
//!
//! How it can fail, and what each check catches:
//! - A block's bytes are changed: the tag fails.
//! - Blocks are swapped or moved within a file: the block index is in the associated data
//!   and the nonce.
//! - A block or a whole file is moved to another file name: each file has its own key, and
//!   the name is in the associated data.
//! - The file is truncated inside a block: that block's length and tag no longer match.
//! - Whole blocks are cut off the end: the new last block was sealed as not final.
//! - Blocks are appended: the old last block was sealed as final but is no longer last.
//! - The header is edited: it is in every block's associated data, and the salt picks the key.
//! - The file was written under another data key (an index left over from another vault):
//!   the key id differs, which is reported as `WrongKey` rather than as tampering, so the
//!   caller can rebuild the index instead of alarming the user.
//! - The writer never finished (crash, error): there is no final block, reported as
//!   `Incomplete`.
//! - Not detected here: an older complete version of the same file put back in place (for
//!   example a previous `meta.json`). Every version is authentic. The search crate has to
//!   notice an index that is behind, for example by comparing the outbox position in the
//!   commit payload with the archive.

use std::fmt;
use std::path::Path;
use std::sync::{Arc, Mutex};

use aes_gcm::aead::{AeadInPlace, KeyInit};
use aes_gcm::{Aes256Gcm, Nonce, Tag};
use tantivy::TantivyError;
use tantivy::directory::error::OpenReadError;

use crate::keys::SubKey;

/// Plaintext bytes per block, except the last.
pub(crate) const BLOCK_LEN: usize = 64 * 1024;
pub(crate) const TAG_LEN: usize = 16;
pub(crate) const HEADER_LEN: usize = 4 + 1 + KEY_ID_LEN + SALT_LEN;
const MAGIC: &[u8; 4] = b"PKSI";
const VERSION: u8 = 1;
const KEY_ID_LEN: usize = 16;
const SALT_LEN: usize = 32;
/// Bytes one full block occupies on disk.
const SEALED_BLOCK_LEN: u64 = (BLOCK_LEN + TAG_LEN) as u64;

/// Why a file of the encrypted search index cannot be read.
///
/// `Debug` prints the same sentence as `Display`: Tantivy formats the `io::Error` around this
/// one with `{:?}`, and the message should read plainly in its errors and in logs.
#[derive(Clone, PartialEq, Eq, thiserror::Error)]
pub enum IndexFileError {
    #[error("search index file {file} is not in PathKeep's encrypted format")]
    NotEncrypted { file: String },
    #[error(
        "search index file {file} uses format version {version}, which this PathKeep cannot read"
    )]
    UnsupportedVersion { file: String, version: u8 },
    #[error(
        "search index file {file} was encrypted under a different key; the index must be rebuilt"
    )]
    WrongKey { file: String },
    #[error("search index file {file} is incomplete: it was never finished, or it was truncated")]
    Incomplete { file: String },
    #[error(
        "search index file {file}, block {block}: integrity check failed \
         (the file was changed, truncated, or its blocks were moved)"
    )]
    Tampered { file: String, block: u64 },
}

impl fmt::Debug for IndexFileError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Display::fmt(self, f)
    }
}

impl IndexFileError {
    /// Finds this crate's error inside an error from Tantivy or `std::io`. Tantivy keeps the
    /// `io::Error` from a read in a field that is not an error source (`TantivyError::IoError`,
    /// `OpenReadError::IoError`), so the chain is walked by type. The search crate uses this,
    /// together with `EncryptedDirectory::integrity_error` for the errors Tantivy replaces with
    /// its own, to tell a damaged or foreign index, which it rebuilds, from other failures.
    pub fn find_in<'a>(error: &'a (dyn std::error::Error + 'static)) -> Option<&'a IndexFileError> {
        let mut current = Some(error);
        while let Some(error) = current {
            let io: Option<&std::io::Error> =
                if let Some(found) = error.downcast_ref::<IndexFileError>() {
                    return Some(found);
                } else if let Some(io) = error.downcast_ref::<std::io::Error>() {
                    Some(io)
                } else if let Some(io) = error.downcast_ref::<Arc<std::io::Error>>() {
                    Some(io)
                } else if let Some(OpenReadError::IoError { io_error, .. }) =
                    error.downcast_ref::<OpenReadError>()
                {
                    Some(io_error)
                } else if let Some(TantivyError::IoError(io_error)) =
                    error.downcast_ref::<TantivyError>()
                {
                    Some(io_error)
                } else {
                    None
                };
            if let Some(found) = io
                .and_then(|io| io.get_ref())
                .and_then(|inner| inner.downcast_ref::<IndexFileError>())
            {
                return Some(found);
            }
            current = error.source();
        }
        None
    }

    pub(crate) fn into_io(self) -> std::io::Error {
        std::io::Error::new(std::io::ErrorKind::InvalidData, self)
    }
}

/// The first integrity failure a directory has run into. Tantivy does not pass every read
/// error on: it ignores any error opening a segment's positions file and later reports
/// "Data corruption: Failed to open field ...'s positions" instead (`SegmentReader::open`,
/// tantivy 0.26.2 `segment_reader.rs:162`). The directory remembers what it saw, so the
/// search crate can still tell a damaged index from a schema mismatch.
#[derive(Default)]
pub(crate) struct IntegrityLog(Mutex<Option<IndexFileError>>);

impl IntegrityLog {
    /// Remembers `error` if it is the first, and returns it as the `io::Error` to report.
    pub(crate) fn record(&self, error: IndexFileError) -> std::io::Error {
        self.0.lock().unwrap().get_or_insert_with(|| error.clone());
        error.into_io()
    }

    pub(crate) fn first(&self) -> Option<IndexFileError> {
        self.0.lock().unwrap().clone()
    }
}

/// The file name as it goes into the associated data and the error messages.
pub(crate) struct FileName<'a>(pub &'a Path);

impl FileName<'_> {
    fn bytes(&self) -> &[u8] {
        self.0.as_os_str().as_encoded_bytes()
    }
}

impl fmt::Display for FileName<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0.display())
    }
}

/// The 16-byte key id written into every header: a fingerprint of the search-index sub-key,
/// so a file from another vault is recognized without trying to decrypt it.
pub(crate) fn key_id(index_key: &SubKey) -> [u8; KEY_ID_LEN] {
    let derived = index_key.expand(&[], b"pathkeep/v1/search-index-key-id");
    let mut id = [0u8; KEY_ID_LEN];
    id.copy_from_slice(&derived[..KEY_ID_LEN]);
    id
}

/// A parsed header plus the cipher for that file.
pub(crate) struct FileCipher {
    header: [u8; HEADER_LEN],
    cipher: Aes256Gcm,
}

impl FileCipher {
    /// Starts a new file: fresh salt, fresh key.
    pub(crate) fn new_file(index_key: &SubKey, key_id: &[u8; KEY_ID_LEN]) -> FileCipher {
        let mut salt = [0u8; SALT_LEN];
        getrandom::fill(&mut salt).expect("operating system random number generator failed");
        let mut header = [0u8; HEADER_LEN];
        header[..4].copy_from_slice(MAGIC);
        header[4] = VERSION;
        header[5..5 + KEY_ID_LEN].copy_from_slice(key_id);
        header[5 + KEY_ID_LEN..].copy_from_slice(&salt);
        FileCipher::with_header(index_key, header)
    }

    /// Reads a header from the start of an existing file.
    pub(crate) fn parse(
        index_key: &SubKey,
        key_id: &[u8; KEY_ID_LEN],
        name: &FileName,
        bytes: &[u8],
    ) -> Result<FileCipher, IndexFileError> {
        let file = name.to_string();
        if bytes.len() < 5 || &bytes[..4] != MAGIC {
            // A file that the writer created but crashed before its header reached disk is
            // shorter than the magic; treat it as unfinished rather than foreign.
            return Err(if bytes.len() <= MAGIC.len() && MAGIC.starts_with(bytes) {
                IndexFileError::Incomplete { file }
            } else {
                IndexFileError::NotEncrypted { file }
            });
        }
        if bytes[4] != VERSION {
            return Err(IndexFileError::UnsupportedVersion { file, version: bytes[4] });
        }
        if bytes.len() < HEADER_LEN {
            return Err(IndexFileError::Incomplete { file });
        }
        if &bytes[5..5 + KEY_ID_LEN] != key_id {
            return Err(IndexFileError::WrongKey { file });
        }
        let mut header = [0u8; HEADER_LEN];
        header.copy_from_slice(&bytes[..HEADER_LEN]);
        Ok(FileCipher::with_header(index_key, header))
    }

    fn with_header(index_key: &SubKey, header: [u8; HEADER_LEN]) -> FileCipher {
        let key = index_key.expand(&header[5 + KEY_ID_LEN..], b"pathkeep/v1/search-index-file");
        let cipher = Aes256Gcm::new_from_slice(&key[..]).expect("file key is 32 bytes");
        FileCipher { header, cipher }
    }

    pub(crate) fn header(&self) -> &[u8; HEADER_LEN] {
        &self.header
    }

    /// The random salt, different for every file written; part of the block cache's key.
    pub(crate) fn salt(&self) -> [u8; SALT_LEN] {
        self.header[5 + KEY_ID_LEN..].try_into().unwrap()
    }

    /// Encrypts `block` in place and returns the tag to write after it.
    pub(crate) fn seal(
        &self,
        name: &FileName,
        index: u64,
        is_final: bool,
        block: &mut [u8],
    ) -> [u8; TAG_LEN] {
        let aad = self.associated_data(name, index, block.len(), is_final);
        let tag = self
            .cipher
            .encrypt_in_place_detached(&nonce(index), &aad, block)
            .expect("a 64 KiB block is within AES-GCM's length limit");
        tag.into()
    }

    /// Decrypts `sealed` (ciphertext followed by its tag) in place and returns the plaintext
    /// length. On failure the buffer is left as ciphertext.
    pub(crate) fn open(
        &self,
        name: &FileName,
        index: u64,
        is_final: bool,
        sealed: &mut [u8],
    ) -> Result<usize, IndexFileError> {
        let len = sealed.len() - TAG_LEN;
        let (block, tag) = sealed.split_at_mut(len);
        let aad = self.associated_data(name, index, len, is_final);
        self.cipher
            .decrypt_in_place_detached(&nonce(index), &aad, block, Tag::from_slice(tag))
            .map_err(|_| IndexFileError::Tampered { file: name.to_string(), block: index })?;
        Ok(len)
    }

    fn associated_data(&self, name: &FileName, index: u64, len: usize, is_final: bool) -> Vec<u8> {
        let name = name.bytes();
        let mut aad = Vec::with_capacity(HEADER_LEN + 13 + name.len());
        aad.extend_from_slice(&self.header);
        aad.extend_from_slice(&index.to_le_bytes());
        aad.extend_from_slice(&(len as u32).to_le_bytes());
        aad.push(u8::from(is_final));
        aad.extend_from_slice(name);
        aad
    }
}

fn nonce(index: u64) -> Nonce<aes_gcm::aead::consts::U12> {
    let mut nonce = [0u8; 12];
    nonce[..8].copy_from_slice(&index.to_le_bytes());
    nonce.into()
}

/// Where the blocks of a finished file are, computed from its size on disk.
#[derive(Debug, Clone, Copy)]
pub(crate) struct Layout {
    pub(crate) blocks: u64,
    pub(crate) plaintext_len: u64,
}

impl Layout {
    /// `Incomplete` when the size cannot be a finished file: a header with no final block, or
    /// a last block shorter than its tag.
    pub(crate) fn from_file_len(file_len: u64, name: &FileName) -> Result<Layout, IndexFileError> {
        let incomplete = || IndexFileError::Incomplete { file: name.to_string() };
        let body = file_len.checked_sub(HEADER_LEN as u64).ok_or_else(incomplete)?;
        if body == 0 {
            return Err(incomplete());
        }
        let blocks = body.div_ceil(SEALED_BLOCK_LEN);
        let last_sealed = body - (blocks - 1) * SEALED_BLOCK_LEN;
        if last_sealed < TAG_LEN as u64 {
            return Err(incomplete());
        }
        let plaintext_len = (blocks - 1) * BLOCK_LEN as u64 + (last_sealed - TAG_LEN as u64);
        Ok(Layout { blocks, plaintext_len })
    }

    /// Byte offset of block `index` in the file, and how many bytes it occupies (with tag).
    pub(crate) fn sealed_range(&self, index: u64) -> (u64, usize) {
        let offset = HEADER_LEN as u64 + index * SEALED_BLOCK_LEN;
        let len = if index + 1 == self.blocks {
            (self.plaintext_len - index * BLOCK_LEN as u64) as usize + TAG_LEN
        } else {
            BLOCK_LEN + TAG_LEN
        };
        (offset, len)
    }

    pub(crate) fn is_final(&self, index: u64) -> bool {
        index + 1 == self.blocks
    }
}

/// Encrypts a whole file at once, for `atomic_write`.
pub(crate) fn seal_whole_file(cipher: &FileCipher, name: &FileName, plaintext: &[u8]) -> Vec<u8> {
    let blocks = plaintext.len().div_ceil(BLOCK_LEN).max(1);
    let mut out = Vec::with_capacity(HEADER_LEN + plaintext.len() + blocks * TAG_LEN);
    out.extend_from_slice(cipher.header());
    for index in 0..blocks {
        let start = index * BLOCK_LEN;
        let end = (start + BLOCK_LEN).min(plaintext.len());
        let at = out.len();
        out.extend_from_slice(&plaintext[start..end]);
        let tag = cipher.seal(name, index as u64, index + 1 == blocks, &mut out[at..]);
        out.extend_from_slice(&tag);
    }
    out
}
