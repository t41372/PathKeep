//! The data key and the sub-keys derived from it.
//!
//! Responsibilities:
//! - Hold the random 256-bit data key, and every key derived from it, so that the bytes are
//!   wiped when the value is dropped and never appear in `Debug` output.
//! - Derive one independent sub-key per use with HKDF-SHA256, so no two uses share key
//!   material.
//!
//! Not responsible for:
//! - Storing keys. The password envelope (`envelope.rs`) and the OS keychain do that.
//! - Deciding when keys go away. The app drops them, and closes everything opened with them,
//!   when the vault locks.
//!
//! How it can fail:
//! - Two uses end up with the same key, so a leak or a cipher weakness in one (say, the
//!   search index's AES-GCM) reaches another (a Turso database). Each purpose has its own
//!   HKDF info label; a test derives all of them and checks they differ.
//! - A label changes in a refactor. Every existing database would then fail to open with
//!   "Decryption failed", and the search index would be rebuilt. Known-answer values,
//!   computed with an independent HKDF implementation and pinned in the tests, catch this.
//! - Key bytes reach a log through `Debug`, a panic message or an error. `Debug` is written by
//!   hand and prints only the purpose; no error type in this crate carries key bytes.
//! - Key bytes outlive the lock in freed memory. Both types keep the key in one heap
//!   allocation (moves copy only the pointer) and zeroize it on drop. Copies made elsewhere
//!   are the caller's to manage: `to_hex` returns a `Zeroizing<String>`, and cipher instances
//!   in this crate are built with the RustCrypto `zeroize` feature.

use std::fmt;

use hkdf::Hkdf;
use sha2::Sha256;
use zeroize::{Zeroize, Zeroizing};

/// Size in bytes of the data key and of every sub-key.
pub const KEY_LEN: usize = 32;

/// What a sub-key is for. Each purpose gets key material no other purpose sees.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum KeyPurpose {
    /// `archive/history-vault.db`, opened with Turso's native encryption.
    ArchiveDatabase,
    /// `archive/source-evidence.db`.
    SourceEvidenceDatabase,
    /// `derived/intelligence.db`.
    IntelligenceDatabase,
    /// `derived/agent.db`.
    AgentDatabase,
    /// `derived/scratch.db`, the ATTACHed database used instead of TEMP tables.
    ScratchDatabase,
    /// The Tantivy search index, through [`crate::EncryptedDirectory`].
    SearchIndex,
    /// Turso's sort and hash spill files, through [`crate::TempCryptIo`].
    TempFiles,
}

impl KeyPurpose {
    /// Every purpose, for tests and for code that has to open all databases.
    pub const ALL: [KeyPurpose; 7] = [
        KeyPurpose::ArchiveDatabase,
        KeyPurpose::SourceEvidenceDatabase,
        KeyPurpose::IntelligenceDatabase,
        KeyPurpose::AgentDatabase,
        KeyPurpose::ScratchDatabase,
        KeyPurpose::SearchIndex,
        KeyPurpose::TempFiles,
    ];

    /// The HKDF info label. Changing one makes existing data unreadable.
    fn label(self) -> &'static [u8] {
        match self {
            KeyPurpose::ArchiveDatabase => b"pathkeep/v1/archive-database",
            KeyPurpose::SourceEvidenceDatabase => b"pathkeep/v1/source-evidence-database",
            KeyPurpose::IntelligenceDatabase => b"pathkeep/v1/intelligence-database",
            KeyPurpose::AgentDatabase => b"pathkeep/v1/agent-database",
            KeyPurpose::ScratchDatabase => b"pathkeep/v1/scratch-database",
            KeyPurpose::SearchIndex => b"pathkeep/v1/search-index",
            KeyPurpose::TempFiles => b"pathkeep/v1/temp-files",
        }
    }
}

/// The random key that every database and the search index are encrypted under, directly or
/// through a sub-key. A password change rewraps it; it never changes for the life of a vault.
pub struct DataKey(Box<[u8; KEY_LEN]>);

impl DataKey {
    /// A fresh key from the operating system's random number generator.
    pub(crate) fn generate() -> Self {
        let mut key = Box::new([0u8; KEY_LEN]);
        getrandom::fill(&mut key[..]).expect("operating system random number generator failed");
        DataKey(key)
    }

    /// Rebuilds the key from bytes the OS keychain stored, which is how auto-unlock works
    /// without the password. The argument is wiped when it is dropped.
    pub fn from_bytes(bytes: Zeroizing<[u8; KEY_LEN]>) -> Self {
        DataKey(Box::new(*bytes))
    }

    /// The raw bytes, for storing in the OS keychain. Anything else should use a sub-key.
    pub fn expose_bytes(&self) -> &[u8; KEY_LEN] {
        &self.0
    }

    /// Derives the key for one purpose: HKDF-SHA256 with no salt, the data key as input key
    /// material and the purpose's label as info. The data key is already uniformly random, so
    /// the extract step needs no salt.
    pub fn derive(&self, purpose: KeyPurpose) -> SubKey {
        SubKey { purpose, key: hkdf_expand(&self.0[..], None, purpose.label()) }
    }
}

impl Drop for DataKey {
    fn drop(&mut self) {
        self.0.zeroize();
    }
}

impl fmt::Debug for DataKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("DataKey(..)")
    }
}

/// A key for exactly one purpose, derived from the [`DataKey`].
pub struct SubKey {
    purpose: KeyPurpose,
    key: Box<[u8; KEY_LEN]>,
}

impl SubKey {
    pub fn purpose(&self) -> KeyPurpose {
        self.purpose
    }

    /// The raw key bytes.
    pub fn expose_bytes(&self) -> &[u8; KEY_LEN] {
        &self.key
    }

    /// Lowercase hex, the form Turso's `EncryptionOpts::hexkey` and `?hexkey=` URIs take.
    pub fn to_hex(&self) -> Zeroizing<String> {
        const DIGITS: &[u8; 16] = b"0123456789abcdef";
        let mut hex = Zeroizing::new(String::with_capacity(KEY_LEN * 2));
        for byte in self.key.iter() {
            hex.push(DIGITS[usize::from(byte >> 4)] as char);
            hex.push(DIGITS[usize::from(byte & 0x0f)] as char);
        }
        hex
    }

    /// Derives a further key from this one, for code in this crate that needs a key per file.
    pub(crate) fn expand(&self, salt: &[u8], info: &[u8]) -> Zeroizing<[u8; KEY_LEN]> {
        let key = hkdf_expand(&self.key[..], Some(salt), info);
        Zeroizing::new(*key)
    }
}

impl Drop for SubKey {
    fn drop(&mut self) {
        self.key.zeroize();
    }
}

impl fmt::Debug for SubKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "SubKey({:?}, ..)", self.purpose)
    }
}

fn hkdf_expand(ikm: &[u8], salt: Option<&[u8]>, info: &[u8]) -> Box<[u8; KEY_LEN]> {
    let mut out = Box::new([0u8; KEY_LEN]);
    Hkdf::<Sha256>::new(salt, ikm)
        .expand(info, &mut out[..])
        .expect("32 bytes is a valid HKDF-SHA256 output length");
    out
}
