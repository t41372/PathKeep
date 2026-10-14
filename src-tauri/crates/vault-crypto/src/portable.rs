//! Responsibilities: stream portable artifacts through the authenticated file codec.
//! Not responsible for: manifests, path selection, ZIP or database encryption.
//! Failure modes: plaintext temporary files, cross-purpose keys, renamed/truncated
//! ciphertext, unbounded reads or missed cancellation. Tests: tests/portable.rs.
use crate::{DataKey, EncryptedDirectory};
use std::{
    io::{self, Read, Write},
    path::Path,
};
use tantivy::HasLen;
use tantivy::directory::{Directory, TerminatingWrite};

/// Runs a streaming encoder before sealing the final authenticated block. Names
/// are portable, slash-separated relative names, identical on both machines.
pub fn write_artifact<T>(
    root: &Path,
    name: &str,
    key: &DataKey,
    write: impl FnOnce(&mut dyn Write) -> io::Result<T>,
) -> io::Result<T> {
    let directory = EncryptedDirectory::open_artifacts(root, key)?;
    let mut output = directory.open_write(Path::new(name)).map_err(io::Error::other)?;
    let result = write(&mut output)?;
    output.terminate()?;
    Ok(result)
}

/// Runs a streaming decoder, then authenticates any unread trailing bytes.
pub fn read_artifact<T>(
    root: &Path,
    name: &str,
    key: &DataKey,
    read: impl FnOnce(&mut dyn Read) -> io::Result<T>,
) -> io::Result<T> {
    let directory = EncryptedDirectory::open_artifacts(root, key)?;
    let input = directory.open_read(Path::new(name)).map_err(io::Error::other)?;
    let mut reader = ArtifactReader { input, offset: 0 };
    let result = read(&mut reader)?;
    io::copy(&mut reader, &mut io::sink())?;
    Ok(result)
}

/// Plaintext is bounded to a single buffer and encrypted before reaching disk.
pub fn encrypt_artifact(
    root: &Path,
    name: &str,
    key: &DataKey,
    input: &mut impl Read,
    cancel: impl Fn() -> bool,
) -> io::Result<u64> {
    write_artifact(root, name, key, |output| copy(input, output, cancel))
}

/// Decryption has no retained plaintext cache or whole-file allocation.
pub fn decrypt_artifact(
    root: &Path,
    name: &str,
    key: &DataKey,
    output: &mut impl Write,
    cancel: impl Fn() -> bool,
) -> io::Result<u64> {
    read_artifact(root, name, key, |input| copy(input, output, cancel))
}

fn copy(
    input: &mut dyn Read,
    output: &mut dyn Write,
    cancel: impl Fn() -> bool,
) -> io::Result<u64> {
    let mut buffer = zeroize::Zeroizing::new(vec![0u8; 64 * 1024]);
    let mut bytes = 0;
    loop {
        if cancel() {
            return Err(io::Error::other("artifact copy cancelled"));
        }
        let read = input.read(&mut buffer)?;
        if read == 0 {
            break;
        }
        output.write_all(&buffer[..read])?;
        bytes += read as u64;
    }
    Ok(bytes)
}

struct ArtifactReader {
    input: tantivy::directory::FileSlice,
    offset: usize,
}
impl Read for ArtifactReader {
    fn read(&mut self, output: &mut [u8]) -> io::Result<usize> {
        let size = output.len().min(self.input.len() - self.offset);
        let end = self.offset + size;
        output[..size].copy_from_slice(&self.input.read_bytes_slice(self.offset..end)?);
        self.offset = end;
        Ok(size)
    }
}
