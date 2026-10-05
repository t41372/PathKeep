//! Reading one finished, encrypted index file, as a Tantivy `FileHandle`.
//!
//! Responsibilities: open the file, check its header and that it was finished, and serve
//! byte ranges by decrypting the blocks they cover, through the shared cache: small reads are
//! copied out of cached blocks, reads of a block or more are cached whole and shared.
//!
//! Not responsible for: the format details (`format.rs`) or deciding what to cache.
//!
//! How it can fail:
//! - A read returns bytes from the wrong place. Ranges are mapped to blocks by fixed
//!   arithmetic and copied out block by block; tests compare random ranges of files of
//!   awkward sizes (0, 1, 64 KiB - 1, 64 KiB, 64 KiB + 1, several blocks) with the plaintext.
//! - A truncated file is read as a shorter valid one. Opening a file decrypts its last block
//!   and requires the final-block flag, so a file cut at a block boundary fails at open,
//!   whatever Tantivy reads first.
//! - Tantivy gets wrong bytes instead of an error. Every block is authenticated before any of
//!   its bytes are copied out; a failure is an `io::Error` of kind `InvalidData` wrapping
//!   [`IndexFileError`].
//! - Plaintext lingers in freed memory. Bytes handed to Tantivy are in a buffer that is
//!   wiped when the last reference to it goes.
//! - A large read fills the block cache twice (once as blocks, once as the range). Blocks
//!   decrypted for a large read are not added to the cache; only the range is.
//! - The file is deleted while open (a merge removes old segments while a searcher still uses
//!   them). The handle keeps the open file descriptor, so reads continue on Unix, as with
//!   Tantivy's own `MmapDirectory`.

use std::fs::File;
use std::io;
use std::ops::Range;
use std::path::PathBuf;
use std::sync::Arc;

use sha2::{Digest, Sha256};
use tantivy::HasLen;
use tantivy::directory::{FileHandle, OwnedBytes};
use zeroize::Zeroizing;

use super::cache::{BlockCache, CacheKey, FileIdentity, WipedBytes};
use super::format::{
    BLOCK_LEN, FileCipher, FileName, HEADER_LEN, IndexFileError, IntegrityLog, Layout, TAG_LEN,
};

pub(crate) struct EncryptedFile {
    name: PathBuf,
    file: File,
    cipher: FileCipher,
    identity: FileIdentity,
    layout: Layout,
    cache: Arc<BlockCache>,
    log: Arc<IntegrityLog>,
}

impl EncryptedFile {
    /// Opens a finished file and verifies its last block. `file` is positioned anywhere.
    pub(crate) fn open(
        name: PathBuf,
        file: File,
        cipher_for: impl FnOnce(&[u8]) -> Result<FileCipher, IndexFileError>,
        cache: Arc<BlockCache>,
        log: Arc<IntegrityLog>,
    ) -> io::Result<EncryptedFile> {
        let file_len = file.metadata()?.len();
        let mut header = vec![0u8; HEADER_LEN.min(file_len as usize)];
        read_at(&file, 0, &mut header)?;
        let cipher = cipher_for(&header).map_err(|e| log.record(e))?;
        let layout =
            Layout::from_file_len(file_len, &FileName(&name)).map_err(|e| log.record(e))?;
        let identity: FileIdentity = Sha256::new()
            .chain_update(cipher.salt())
            .chain_update(file_len.to_le_bytes())
            .chain_update(name.as_os_str().as_encoded_bytes())
            .finalize()
            .into();
        let opened = EncryptedFile { name, file, cipher, identity, layout, cache, log };
        opened.block(layout.blocks - 1, true)?;
        Ok(opened)
    }

    /// The plaintext of block `index`, from the cache or decrypted from disk, and added to the
    /// cache if `keep` is set.
    fn block(&self, index: u64, keep: bool) -> io::Result<OwnedBytes> {
        let key = CacheKey::Block(self.identity, index);
        if let Some(block) = self.cache.get(&key) {
            return Ok(block);
        }
        let (offset, sealed_len) = self.layout.sealed_range(index);
        let mut sealed = Zeroizing::new(vec![0u8; sealed_len]);
        read_at(&self.file, offset, &mut sealed)?;
        let name = FileName(&self.name);
        let is_final = self.layout.is_final(index);
        match self.cipher.open(&name, index, is_final, &mut sealed) {
            Ok(len) => {
                let mut plain = std::mem::take(&mut *sealed);
                plain.truncate(len);
                let block = WipedBytes(plain).into_owned_bytes();
                Ok(if keep { self.cache.insert(key, block) } else { block })
            }
            Err(error) => {
                // A last block that authenticates as a middle block means the file stops at a
                // block boundary before its end: unfinished, or cut short.
                if is_final
                    && sealed_len == BLOCK_LEN + TAG_LEN
                    && self.cipher.open(&name, index, false, &mut sealed).is_ok()
                {
                    return Err(self
                        .log
                        .record(IndexFileError::Incomplete { file: name.to_string() }));
                }
                Err(self.log.record(error))
            }
        }
    }
}

impl FileHandle for EncryptedFile {
    fn read_bytes(&self, range: Range<usize>) -> io::Result<OwnedBytes> {
        if range.start > range.end || range.end as u64 > self.layout.plaintext_len {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                format!(
                    "read of {range:?} outside {} ({} bytes)",
                    self.name.display(),
                    self.layout.plaintext_len
                ),
            ));
        }
        if range.is_empty() {
            return Ok(OwnedBytes::empty());
        }
        let large = range.len() >= BLOCK_LEN;
        let key = CacheKey::Range(self.identity, range.start as u64, range.end as u64);
        if large && let Some(bytes) = self.cache.get(&key) {
            return Ok(bytes);
        }
        let mut out = WipedBytes(Vec::with_capacity(range.len()));
        let first = range.start / BLOCK_LEN;
        let last = (range.end - 1) / BLOCK_LEN;
        for index in first..=last {
            let block = self.block(index as u64, !large)?;
            let block_start = index * BLOCK_LEN;
            let from = range.start.max(block_start) - block_start;
            let to = range.end.min(block_start + block.len()) - block_start;
            out.0.extend_from_slice(&block.as_slice()[from..to]);
        }
        let bytes = out.into_owned_bytes();
        Ok(if large { self.cache.insert(key, bytes) } else { bytes })
    }
}

impl HasLen for EncryptedFile {
    fn len(&self) -> usize {
        self.layout.plaintext_len as usize
    }
}

impl std::fmt::Debug for EncryptedFile {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "EncryptedFile({}, {} bytes)", self.name.display(), self.layout.plaintext_len)
    }
}

#[cfg(unix)]
fn read_at(file: &File, offset: u64, buf: &mut [u8]) -> io::Result<()> {
    std::os::unix::fs::FileExt::read_exact_at(file, buf, offset)
}

#[cfg(windows)]
fn read_at(file: &File, mut offset: u64, mut buf: &mut [u8]) -> io::Result<()> {
    use std::os::windows::fs::FileExt;
    while !buf.is_empty() {
        match file.seek_read(buf, offset)? {
            0 => return Err(io::ErrorKind::UnexpectedEof.into()),
            n => {
                buf = &mut buf[n..];
                offset += n as u64;
            }
        }
    }
    Ok(())
}
