//! Responsibilities: version and scope opaque keyset continuations.
//! Not responsible for: authorization, signing or storing query results.

use crate::{Error, Result};
use serde::{Deserialize, Serialize, de::DeserializeOwned};
use sha2::{Digest, Sha256};

const CURSOR_VERSION: u8 = 4;
const MAX_CURSOR_LENGTH: usize = 16 * 1024;

#[derive(Serialize, Deserialize)]
pub(super) struct SessionCursor {
    pub version: u8,
    pub scope: String,
    pub generation: i64,
    pub timestamp: Option<i64>,
    pub id: String,
}

#[derive(Serialize, Deserialize)]
pub(super) struct EventCursor {
    pub version: u8,
    pub session_id: String,
    pub line: String,
    pub ordinal: u64,
    pub segment: usize,
}

#[derive(Serialize, Deserialize)]
pub(super) struct RevisionCursor {
    pub version: u8,
    pub source_id: String,
    pub before_id: i64,
}

#[derive(Serialize, Deserialize)]
pub(super) struct SourceCursor {
    pub version: u8,
    pub scope: String,
    pub snapshot_rowid: i64,
    pub after_id: String,
}

pub(super) fn version() -> u8 {
    CURSOR_VERSION
}

pub(super) fn encode(value: &impl Serialize) -> Result<String> {
    Ok(hex::encode(serde_json::to_vec(value)?))
}

pub(super) fn decode<T: DeserializeOwned>(value: &str) -> Result<T> {
    if value.len() > MAX_CURSOR_LENGTH {
        return Err(invalid());
    }
    let bytes = hex::decode(value).map_err(|_| invalid())?;
    serde_json::from_slice(&bytes).map_err(|_| invalid())
}

pub(super) fn scope(value: &impl Serialize) -> Result<String> {
    Ok(hex::encode(Sha256::digest(serde_json::to_vec(value)?)))
}

pub(super) fn valid_version(found: u8) -> Result<()> {
    if found != CURSOR_VERSION {
        return Err(invalid());
    }
    Ok(())
}

pub(super) fn invalid() -> Error {
    Error::InvalidInput("invalid or mismatched continuation cursor".into())
}

pub(super) fn stale() -> Error {
    Error::StaleCursor
}
