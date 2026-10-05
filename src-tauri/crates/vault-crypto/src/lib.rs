//! PathKeep's encryption at rest, below the storage engines.
//!
//! Responsibilities:
//! - The password envelope that wraps the vault's data key, and the sub-keys derived from it
//!   (`envelope`, `keys`).
//! - The Turso IO layer that encrypts sort and hash spill files (`temp_io`).
//! - The Tantivy `Directory` that keeps the search index encrypted on disk (`search_dir`).
//!
//! Not responsible for: opening databases, the search schema, indexing or queries, the OS
//! keychain, or deciding when the vault locks. `vault-store` and the search crate build on
//! this one.
//!
//! The README next to this crate describes the formats, the key hierarchy and the threat
//! model.

mod atomic_file;
mod crash_point;
mod envelope;
mod keys;
mod temp_io;

pub use envelope::{
    ENVELOPE_LEN, Envelope, EnvelopeError, KdfParams, read_envelope_file, write_envelope_file,
};
pub use keys::{DataKey, KEY_LEN, KeyPurpose, SubKey};
pub use temp_io::{TURSO_TEMP_FILE_NAME, TempCryptIo};
