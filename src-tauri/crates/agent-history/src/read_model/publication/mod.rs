//! Publishes immutable source pages and conversation membership in the existing R role.
//! Evidence retention, adapter semantics and user marks belong to their existing owners.
mod branches;
mod build;
mod catalog;
mod cleanup;
mod evidence;
mod leaves;
mod lookup;
mod manifest;
mod membership;
mod navigation;
pub use leaves::ReaderLeaves;
use navigation::Extents;
mod pages;
mod read;
mod repair;
mod runs;
mod visible;
mod window;
pub use evidence::SourceObservation;
use window::Window;
pub(crate) mod schema;
use super::*;
pub(crate) use catalog::{expire_versions, install, refresh};
use serde::{Deserialize, Serialize};
use vault_store::Db;

const DB: Db = Db::AgentHistoryReadModel;
pub(crate) const CODEC: i64 = 4;
pub(crate) const RENDERER: i64 = 4;
const REPLAY_LIMIT: u64 = 32 * 1024 * 1024;
const OUTLINE: i64 = 1;
const LOOKUP: i64 = 2;
const FRAMES: i64 = 3;
const LEAVES: i64 = 4;
const RANK: i64 = 5;
const DIRECTORY: i64 = 6;
const NONEMPTY: i64 = 9;
const AMBIGUOUS: i64 = 10;
const EXTENTS: i64 = 11;
// Renderer version participates in the physical page key.
const RENDERED: i64 = 1000 + RENDERER;

/// Pins a reader to one elected membership, rank space and coverage assessment.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReaderPublication {
    pub conversation_id: String,
    pub publication: PublicationId,
    pub current_source: String,
    pub parent_id: Option<String>,
    pub parent_status: ParentStatus,
    pub physical_records: u64,
    pub visible_records: u64,
    pub complete: bool,
    pub attachments_not_kept: u64,
    pub input_digest: String,
    pub coverage_reason: Option<String>,
}
/// Resolution of the elected child's native parent reference.
#[derive(Clone, Copy, Debug, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ParentStatus {
    #[default]
    None,
    Resolved,
    Unresolved,
}

/// The actual member and decoded prefix that satisfied a native history-base reference.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ResolvedBase {
    pub source_id: String,
    pub own_end: u64,
    pub prefix_digest: String,
}

/// Identifies the retained revision and explains its role in the selected conversation.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SourceMember {
    #[serde(default)]
    pub awaiting_rebuild: bool,
    #[serde(default)]
    pub parent_status: ParentStatus,
    pub max_frame_bytes: u64,
    pub format_version: Option<u64>,
    pub transport: Transport,
    pub rollout_id: Option<String>,
    pub source_time: Option<String>,
    pub raw_head_current: bool,
    pub source_id: String,
    #[serde(with = "crate::query::opaque_id")]
    pub revision: i64,
    pub digest: String,
    pub bytes: u64,
    pub harness: String,
    pub root: String,
    pub relative_path: String,
    pub native_id: String,
    pub origin: String,
    pub parser: String,
    pub generation: i64,
    pub copy_of: Option<String>,
    pub relation: crate::adapters::identity::SourceRelation,
    pub own_records: u64,
    pub line_records: u64,
    pub line_visible_records: u64,
    pub resolved_base: Option<ResolvedBase>,
    pub rendered: bool,
    pub observation: SourceObservation,
    pub parent_id: Option<String>,
    pub fork_id: Option<String>,
}
/// Bounded reader parts with a physical address for exact expansion.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DisplayRow {
    pub boundary: Option<StructuralBoundary>,
    pub source_id: String,
    pub ordinal: PhysicalOrdinal,
    pub native_id: Option<String>,
    pub parent_id: Option<String>,
    pub kind: String,
    pub role: Option<String>,
    pub timestamp: Option<i64>,
    pub parts: Vec<RecordPart>,
    pub record_bytes: u64,
    pub truncated: bool,
    pub tokens_before: Option<u64>,
}
/// Native navigation metadata, independent of the original JSON payload.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OutlineEntry {
    pub boundary: Option<StructuralBoundary>,
    pub visible: bool,
    pub locator: RecordLocator,
    pub context: RecordContext,
    pub id: Option<String>,
    pub parent: Option<String>,
    run_start: u64,
    declared_root: bool,
    pub kind: String,
    pub role: Option<String>,
    pub timestamp: Option<i64>,
    pub first_kept: Option<String>,
    pub tokens_before: Option<u64>,
}

#[cfg(test)]
mod tests;

type Entry = OutlineEntry;

fn navigation_error(error: Error) -> Error {
    match error {
        Error::Integrity(_) => Error::ReaderRebuildRequired,
        other => other,
    }
}

#[cfg(test)]
mod measure;

// Only row-local derived failures can be parked. Owner failures, cancellation and
// evidence reads (tagged with their role) keep their original error.
fn local_reader_damage(error: &Error) -> bool {
    match error {
        Error::ReaderRebuildRequired | Error::Integrity(_) | Error::Json(_) => true,
        Error::Store(store) => store.is_row_data_error(),
        _ => false,
    }
}
fn derived_read_error(error: vault_store::Error) -> Error {
    let error = Error::from(error);
    if local_reader_damage(&error) { Error::ReaderRebuildRequired } else { error }
}
