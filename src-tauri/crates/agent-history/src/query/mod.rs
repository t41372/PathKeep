//! Responsibilities: bounded, stable views of published agent history.
//! Not responsible for: native parsing, backup writes, search indexing or UI grouping.
//! Pagination binds a cursor to its filter and publication version; it never
//! silently mixes generations after a source is rewritten.

mod cursor;
mod events;
mod revisions;
mod sessions;
mod sources;

use crate::{
    Error, Result,
    model::{ArtifactKind, Harness},
};
use serde::{Deserialize, Serialize};

/// A bounded result and the continuation token for the same logical view.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Page<T> {
    pub items: Vec<T>,
    pub next_cursor: Option<String>,
}

/// Lists retained physical sources, including raw-only records and attachments
/// that have no parsed session. No source directory needs to remain available.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SourceQuery {
    pub harness: Option<Harness>,
    pub kind: Option<ArtifactKind>,
    pub limit: usize,
    pub after: Option<String>,
}

impl Default for SourceQuery {
    fn default() -> Self {
        Self { harness: None, kind: None, limit: 100, after: None }
    }
}

/// Separate creation and activity ordering without inferring either from paths.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SessionSort {
    #[default]
    Updated,
    Created,
}

/// Exact source filters. Projects shared by different harnesses remain queryable
/// together; no path rewriting or heuristic project merging occurs here.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SessionQuery {
    pub harness: Option<Harness>,
    pub project_path: Option<String>,
    pub sort: SessionSort,
    pub limit: usize,
    pub after: Option<String>,
}

impl Default for SessionQuery {
    fn default() -> Self {
        Self {
            harness: None,
            project_path: None,
            sort: SessionSort::Updated,
            limit: 100,
            after: None,
        }
    }
}

/// Published metadata keeps a physical source identity alongside its native
/// session identity, which may be duplicated across profiles or harnesses.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SessionView {
    pub id: String,
    pub source_id: String,
    pub native_id: String,
    pub harness: Harness,
    pub project_path: Option<String>,
    pub title: Option<String>,
    pub created_at: Option<i64>,
    pub updated_at: Option<i64>,
    pub origin: String,
    pub harness_version: Option<String>,
    pub parent_session_id: Option<String>,
    pub model: Option<String>,
    #[serde(with = "opaque_id")]
    pub revision_id: i64,
    /// A parser replacement has a new identity even when its raw bytes are unchanged.
    #[serde(with = "opaque_id")]
    pub projection_id: i64,
    pub parser_version: String,
    pub complete: bool,
}

/// Event continuations are valid only for the projection that produced them.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct EventQuery {
    pub session_id: String,
    pub limit: usize,
    pub after: Option<String>,
}

impl EventQuery {
    /// Starts a bounded walk without requiring callers to construct a cursor.
    pub fn new(session_id: impl Into<String>) -> Self {
        Self { session_id: session_id.into(), limit: 100, after: None }
    }
}

pub(crate) fn page_limit(limit: usize) -> Result<i64> {
    if !(1..=500).contains(&limit) {
        return Err(Error::InvalidInput("page limit must be between 1 and 500".into()));
    }
    Ok(limit as i64)
}

pub(crate) fn operation(
    error: impl std::error::Error + Send + Sync + 'static,
) -> vault_store::Error {
    vault_store::Error::Operation(Box::new(error))
}

pub(crate) fn generation(session: &vault_store::Session<'_>) -> vault_store::Result<i64> {
    session.query_one(
        "SELECT coalesce((SELECT CAST(value AS INTEGER) FROM ah_meta WHERE key='generation'),0)",
        (),
        |row| Ok(row.get(0)?),
    )
}

pub(crate) mod opaque_id {
    use serde::{Deserialize, Deserializer, Serializer, de::Error};

    pub fn serialize<S: Serializer>(value: &i64, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(&value.to_string())
    }

    pub fn deserialize<'de, D: Deserializer<'de>>(deserializer: D) -> Result<i64, D::Error> {
        #[derive(Deserialize)]
        #[serde(untagged)]
        enum Identity {
            Text(String),
            Number(i64),
        }
        match Identity::deserialize(deserializer)? {
            Identity::Text(value) => value.parse().map_err(D::Error::custom),
            Identity::Number(value) => Ok(value),
        }
    }

    pub(crate) mod option {
        use serde::{Deserialize, Deserializer, Serializer};

        pub fn serialize<S: Serializer>(
            value: &Option<i64>,
            serializer: S,
        ) -> Result<S::Ok, S::Error> {
            match value {
                Some(value) => serializer.serialize_some(&value.to_string()),
                None => serializer.serialize_none(),
            }
        }

        pub fn deserialize<'de, D: Deserializer<'de>>(
            deserializer: D,
        ) -> Result<Option<i64>, D::Error> {
            #[derive(Deserialize)]
            struct Identity(#[serde(with = "super")] i64);
            Ok(Option::<Identity>::deserialize(deserializer)?.map(|value| value.0))
        }
    }
}
