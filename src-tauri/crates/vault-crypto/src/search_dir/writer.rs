//! Writing one encrypted index file as a stream, for Tantivy's `open_write`.
//!
//! Responsibilities: collect plaintext into 64 KiB blocks, seal each block as it fills, and
//! seal the last block, marked final, when Tantivy terminates the writer; then fsync.
//!
//! Not responsible for: `atomic_write` files (`meta.json`, `.managed.json`), which are small
//! and sealed in one piece by `format::seal_whole_file`.
//!
//! How it can fail:
//! - Plaintext reaches disk. Only sealed blocks are ever written; the open block stays in
//!   memory, and is wiped when the writer is dropped.
//! - A crash or an error leaves a partly written file that later reads as complete. The
//!   final block is sealed only in `terminate`, so such a file has no final block and fails
//!   to open with `Incomplete`. Tantivy never refers to a file it has not terminated: a
//!   segment's files are listed in `meta.json` only after they are finished.
//! - `flush` makes data visible before `terminate`. It does not, and cannot without sealing a
//!   short block in the middle of the file: Tantivy's `MmapDirectory` flush also writes
//!   nothing durable, and Tantivy reads a file only after terminating it.
//! - A full block at the end is sealed as a middle block. A full block is sealed only once
//!   more bytes arrive, so the last block, full or not, is always the one `terminate` seals.

use std::fs::File;
use std::io::{self, Write};
use std::path::PathBuf;

use tantivy::directory::{AntiCallToken, TerminatingWrite};
use zeroize::Zeroizing;

use super::format::{BLOCK_LEN, FileCipher, FileName};

pub(crate) struct EncryptedWriter {
    name: PathBuf,
    file: File,
    cipher: FileCipher,
    /// Plaintext of the block being filled.
    block: Zeroizing<Vec<u8>>,
    next_index: u64,
}

impl EncryptedWriter {
    /// Takes a newly created, empty file and writes its header.
    pub(crate) fn start(
        name: PathBuf,
        mut file: File,
        cipher: FileCipher,
    ) -> io::Result<EncryptedWriter> {
        file.write_all(cipher.header())?;
        Ok(EncryptedWriter {
            name,
            file,
            cipher,
            block: Zeroizing::new(Vec::with_capacity(BLOCK_LEN)),
            next_index: 0,
        })
    }

    fn seal_block(&mut self, is_final: bool) -> io::Result<()> {
        let tag =
            self.cipher.seal(&FileName(&self.name), self.next_index, is_final, &mut self.block);
        self.file.write_all(&self.block)?;
        self.file.write_all(&tag)?;
        self.block.clear();
        self.next_index += 1;
        Ok(())
    }
}

impl Write for EncryptedWriter {
    fn write(&mut self, data: &[u8]) -> io::Result<usize> {
        let mut rest = data;
        while !rest.is_empty() {
            if self.block.len() == BLOCK_LEN {
                self.seal_block(false)?;
            }
            let take = rest.len().min(BLOCK_LEN - self.block.len());
            self.block.extend_from_slice(&rest[..take]);
            rest = &rest[take..];
        }
        Ok(data.len())
    }

    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

impl TerminatingWrite for EncryptedWriter {
    fn terminate_ref(&mut self, _: AntiCallToken) -> io::Result<()> {
        self.seal_block(true)?;
        self.file.sync_data()
    }
}
