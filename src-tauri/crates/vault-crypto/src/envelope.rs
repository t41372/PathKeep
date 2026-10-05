//! The password envelope: the data key, wrapped under a key derived from the user's password.
//!
//! Responsibilities:
//! - Create a vault's data key and wrap it: Argon2id turns the password into a key-encryption
//!   key (KEK), and AES-256-GCM encrypts the data key under it.
//! - Unwrap it again with the password, and rewrap it under a new password without changing
//!   the data key, so a password change rewrites 94 bytes instead of every database.
//! - Store the KDF parameters in the envelope, authenticated, so they can be raised later.
//!
//! Not responsible for: where the envelope lives or how it reaches disk (`write_envelope_file`
//! below uses `atomic_file.rs`), the OS keychain, or password rules.
//!
//! Format, version 1, 94 bytes:
//!
//! | offset | size | field                                          |
//! | ------ | ---- | ---------------------------------------------- |
//! | 0      | 5    | magic `PKENV`                                  |
//! | 5      | 1    | version, `1`                                   |
//! | 6      | 4    | Argon2id memory in KiB, u32 little-endian      |
//! | 10     | 4    | Argon2id iterations, u32 LE                    |
//! | 14     | 4    | Argon2id lanes, u32 LE                         |
//! | 18     | 16   | Argon2id salt                                  |
//! | 34     | 12   | AES-GCM nonce                                  |
//! | 46     | 32   | AES-256-GCM ciphertext of the data key         |
//! | 78     | 16   | AES-GCM tag                                    |
//!
//! Bytes 0..46 are the AES-GCM associated data. Version 1 means Argon2id v0x13 and
//! AES-256-GCM; another algorithm needs another version.
//!
//! How it can fail:
//! - The wrong password unwraps something. AES-GCM authenticates the result, so a wrong
//!   password fails with [`EnvelopeError::WrongPassword`] and no key comes out.
//! - Someone edits the stored parameters, for example to weaken the KDF for the next rewrap,
//!   or edits the salt, nonce or ciphertext. The header is the associated data, so any changed
//!   byte fails authentication; a test flips every bit of the envelope.
//! - An edited envelope makes unlock hang or exhaust memory. The KDF runs before
//!   authentication can say anything, so the parameters are untrusted input: memory, iteration
//!   and lane counts outside fixed limits are rejected before the KDF runs.
//! - A truncated, padded or foreign file is mistaken for an envelope. The length, magic and
//!   version are checked exactly.
//! - A rewrap reuses the salt or nonce. Every wrap draws both from the OS random generator.
//! - Secrets outlive their use. The KEK, the Argon2 working memory (which is derived from the
//!   password) and the unwrapped key buffer are zeroized; the AES-GCM instance is built with the
//!   `zeroize` feature. The password itself belongs to the caller.
//! - A crash during a rewrap leaves no envelope. `write_envelope_file` replaces the file
//!   atomically; `tests/envelope_crash.rs` kills child processes at every step.

use std::fmt;
use std::io::{self, Read};
use std::path::Path;

use aes_gcm::aead::{AeadInPlace, KeyInit};
use aes_gcm::{Aes256Gcm, Nonce, Tag};
use argon2::{Algorithm, Argon2, Block, Params, Version};
use zeroize::{Zeroize, Zeroizing};

use crate::atomic_file;
use crate::keys::{DataKey, KEY_LEN};

const MAGIC: &[u8; 5] = b"PKENV";
const VERSION: u8 = 1;
const HEADER_LEN: usize = 46;
const TAG_LEN: usize = 16;

/// Length of a version 1 envelope in bytes.
pub const ENVELOPE_LEN: usize = HEADER_LEN + KEY_LEN + TAG_LEN;

/// Largest memory cost an envelope may ask for: 2 GiB, a quarter of the 8 GB target machine.
const MAX_MEMORY_KIB: u32 = 2 * 1024 * 1024;
/// Largest iteration count an envelope may ask for.
const MAX_ITERATIONS: u32 = 64;
/// Largest lane count an envelope may ask for.
const MAX_LANES: u32 = 16;

/// Argon2id cost parameters.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct KdfParams {
    pub memory_kib: u32,
    pub iterations: u32,
    pub lanes: u32,
}

impl KdfParams {
    /// The parameters new envelopes use. 256 MiB and 3 passes; see the crate README for the
    /// measurements behind them. `argon2` 0.5 computes lanes one after another, so 4 lanes
    /// cost the same time as 1 today; they are 4 so that a parallel implementation can use
    /// the target machine's 4 cores later without changing the format.
    pub const RECOMMENDED: KdfParams =
        KdfParams { memory_kib: 256 * 1024, iterations: 3, lanes: 4 };

    fn validate(self) -> Result<Params, EnvelopeError> {
        if self.memory_kib > MAX_MEMORY_KIB
            || self.iterations > MAX_ITERATIONS
            || self.lanes == 0
            || self.lanes > MAX_LANES
        {
            return Err(EnvelopeError::InvalidParameters);
        }
        Params::new(self.memory_kib, self.iterations, self.lanes, Some(KEY_LEN))
            .map_err(|_| EnvelopeError::InvalidParameters)
    }
}

/// Why an envelope could not be used.
#[derive(Debug, thiserror::Error, PartialEq, Eq)]
pub enum EnvelopeError {
    #[error("not a PathKeep key envelope")]
    NotAnEnvelope,
    #[error("key envelope version {0} is not supported by this version of PathKeep")]
    UnsupportedVersion(u8),
    #[error("the key envelope's KDF parameters are outside the allowed range")]
    InvalidParameters,
    /// AES-GCM authentication failed. A wrong password and a changed envelope look the same.
    #[error("wrong password, or the key envelope was changed")]
    WrongPassword,
}

/// A parsed envelope. It holds no secret: the data key inside is encrypted.
#[derive(Clone, PartialEq, Eq)]
pub struct Envelope {
    params: KdfParams,
    salt: [u8; 16],
    nonce: [u8; 12],
    wrapped_key: [u8; KEY_LEN],
    tag: [u8; TAG_LEN],
}

impl Envelope {
    /// Creates a vault's data key and wraps it under `password`.
    pub fn create(
        password: &[u8],
        params: KdfParams,
    ) -> Result<(Envelope, DataKey), EnvelopeError> {
        let data_key = DataKey::generate();
        let envelope = Envelope::wrap(&data_key, password, params)?;
        Ok((envelope, data_key))
    }

    /// Parses envelope bytes. Checks the layout and the parameter limits; only
    /// [`Envelope::unwrap`] can tell whether the contents are authentic.
    pub fn parse(bytes: &[u8]) -> Result<Envelope, EnvelopeError> {
        if bytes.len() < MAGIC.len() + 1 || &bytes[..MAGIC.len()] != MAGIC {
            return Err(EnvelopeError::NotAnEnvelope);
        }
        let version = bytes[MAGIC.len()];
        if version != VERSION {
            return Err(EnvelopeError::UnsupportedVersion(version));
        }
        if bytes.len() != ENVELOPE_LEN {
            return Err(EnvelopeError::NotAnEnvelope);
        }
        let u32_at = |at: usize| u32::from_le_bytes(bytes[at..at + 4].try_into().unwrap());
        let params = KdfParams { memory_kib: u32_at(6), iterations: u32_at(10), lanes: u32_at(14) };
        params.validate()?;
        let mut envelope = Envelope {
            params,
            salt: [0; 16],
            nonce: [0; 12],
            wrapped_key: [0; KEY_LEN],
            tag: [0; TAG_LEN],
        };
        envelope.salt.copy_from_slice(&bytes[18..34]);
        envelope.nonce.copy_from_slice(&bytes[34..46]);
        envelope.wrapped_key.copy_from_slice(&bytes[46..78]);
        envelope.tag.copy_from_slice(&bytes[78..94]);
        Ok(envelope)
    }

    pub fn to_bytes(&self) -> [u8; ENVELOPE_LEN] {
        let mut bytes = [0u8; ENVELOPE_LEN];
        bytes[..HEADER_LEN].copy_from_slice(&self.header());
        bytes[46..78].copy_from_slice(&self.wrapped_key);
        bytes[78..94].copy_from_slice(&self.tag);
        bytes
    }

    pub fn params(&self) -> KdfParams {
        self.params
    }

    /// Recovers the data key. Runs the full KDF, so it takes as long as the parameters say.
    pub fn unwrap(&self, password: &[u8]) -> Result<DataKey, EnvelopeError> {
        let kek = derive_kek(password, &self.salt, self.params)?;
        let cipher = Aes256Gcm::new_from_slice(&kek[..]).expect("KEK is 32 bytes");
        let mut key = Zeroizing::new(self.wrapped_key);
        cipher
            .decrypt_in_place_detached(
                Nonce::from_slice(&self.nonce),
                &self.header(),
                &mut key[..],
                Tag::from_slice(&self.tag),
            )
            .map_err(|_| EnvelopeError::WrongPassword)?;
        Ok(DataKey::from_bytes(key))
    }

    /// Wraps the same data key under `new_password`, with a fresh salt and nonce and the
    /// given parameters. The old password must unwrap this envelope first.
    pub fn rewrap(
        &self,
        old_password: &[u8],
        new_password: &[u8],
        params: KdfParams,
    ) -> Result<Envelope, EnvelopeError> {
        let data_key = self.unwrap(old_password)?;
        Envelope::wrap(&data_key, new_password, params)
    }

    fn wrap(
        data_key: &DataKey,
        password: &[u8],
        params: KdfParams,
    ) -> Result<Envelope, EnvelopeError> {
        let mut envelope = Envelope {
            params,
            salt: [0; 16],
            nonce: [0; 12],
            wrapped_key: *data_key.expose_bytes(),
            tag: [0; TAG_LEN],
        };
        getrandom::fill(&mut envelope.salt)
            .expect("operating system random number generator failed");
        getrandom::fill(&mut envelope.nonce)
            .expect("operating system random number generator failed");
        let kek = derive_kek(password, &envelope.salt, params)?;
        let cipher = Aes256Gcm::new_from_slice(&kek[..]).expect("KEK is 32 bytes");
        let header = envelope.header();
        let tag = cipher
            .encrypt_in_place_detached(
                Nonce::from_slice(&envelope.nonce),
                &header,
                &mut envelope.wrapped_key,
            )
            .expect("a 32-byte message is within AES-GCM's length limit");
        envelope.tag.copy_from_slice(&tag);
        Ok(envelope)
    }

    fn header(&self) -> [u8; HEADER_LEN] {
        let mut header = [0u8; HEADER_LEN];
        header[..5].copy_from_slice(MAGIC);
        header[5] = VERSION;
        header[6..10].copy_from_slice(&self.params.memory_kib.to_le_bytes());
        header[10..14].copy_from_slice(&self.params.iterations.to_le_bytes());
        header[14..18].copy_from_slice(&self.params.lanes.to_le_bytes());
        header[18..34].copy_from_slice(&self.salt);
        header[34..46].copy_from_slice(&self.nonce);
        header
    }
}

impl fmt::Debug for Envelope {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Envelope")
            .field("version", &VERSION)
            .field("params", &self.params)
            .finish_non_exhaustive()
    }
}

/// Argon2id(password, salt) → 32-byte KEK. The working memory is allocated here so it can be
/// zeroized afterwards; `Argon2::hash_password_into` would free it without wiping it.
fn derive_kek(
    password: &[u8],
    salt: &[u8; 16],
    params: KdfParams,
) -> Result<Zeroizing<[u8; KEY_LEN]>, EnvelopeError> {
    let argon2 = Argon2::new(Algorithm::Argon2id, Version::V0x13, params.validate()?);
    let mut memory = vec![Block::default(); argon2.params().block_count()];
    let mut kek = Zeroizing::new([0u8; KEY_LEN]);
    let result = argon2.hash_password_into_with_memory(password, salt, &mut kek[..], &mut memory);
    memory.zeroize();
    // The inputs are validated above, and Argon2 accepts any password and a 16-byte salt.
    result.expect("Argon2id inputs were validated");
    Ok(kek)
}

/// Writes envelope bytes to `path` so that a crash leaves either the previous file or the new
/// one, never neither and never a mix.
pub fn write_envelope_file(path: &Path, envelope: &Envelope) -> io::Result<()> {
    atomic_file::replace(path, &envelope.to_bytes())
}

/// Reads an envelope file. Reads at most one byte more than an envelope, so a huge file put
/// in its place cannot exhaust memory.
pub fn read_envelope_file(path: &Path) -> io::Result<Vec<u8>> {
    let mut bytes = Vec::with_capacity(ENVELOPE_LEN + 1);
    std::fs::File::open(path)?.take(ENVELOPE_LEN as u64 + 1).read_to_end(&mut bytes)?;
    Ok(bytes)
}
