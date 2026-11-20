//! Coordinates and bounded reads over one immutable source revision.
//! Published membership and page reads use R. Exact expansion and offline builders use retained evidence.
pub(crate) mod publication;
pub use publication::{
    DisplayRow, OutlineEntry, ParentStatus, ReaderLeaves, ReaderPublication, ResolvedBase,
    SourceMember, SourceObservation,
};
mod framing;
mod interpreter;
pub(crate) mod parts;
mod transport;
pub(crate) mod usage;
mod window;

use crate::{Diagnostic, Error, NativeEvent, Result};
pub use framing::{RecordVisitor, scan_records};
pub use interpreter::RecordContext;
pub use parts::{PartKind, RecordPart};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
pub use vault_store::CancelToken;

/// Coordinates in the original transport, including compressed frame headers.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct NativeOffset(pub u64);
/// Coordinates in the decoded document before BOM or whitespace handling.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct DecodedOffset(pub u64);
/// Includes blanks, damaged rows and entries which have no display representation.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct PhysicalOrdinal(pub u64);

/// A decoder may restart here even when the preceding record crosses this point.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct FrameStart {
    pub native: NativeOffset,
    pub decoded: DecodedOffset,
}

/// Verified native frame bounds in both byte spaces, including frames ending inside a record.
#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct TransportFrame {
    pub start: FrameStart,
    pub native_end: NativeOffset,
    pub decoded_end: DecodedOffset,
}

/// Why exact bytes are available without a corresponding public native event.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub enum RecordCoverage {
    Object,
    Blank,
    Oversized,
    InvalidUtf8,
    Malformed,
    Incomplete,
    NonObject,
}

/// Half-open decoded span; JSONL includes its terminator, document elements exclude separators.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct RecordLocator {
    // Serialized outline pages inherit their identity from the authenticated version key.
    #[serde(skip)]
    pub(crate) owner: Option<Arc<LocatorOwner>>,
    pub ordinal: PhysicalOrdinal,
    pub start: DecodedOffset,
    pub length: u64,
    pub frame: Option<FrameStart>,
    pub coverage: RecordCoverage,
}

#[derive(Debug, Eq, PartialEq)]
pub(crate) struct LocatorOwner {
    compressed: bool,
    revision: i64,
    source: String,
    digest: String,
}
impl LocatorOwner {
    fn new(info: &crate::archive::RevisionInfo, compressed: bool) -> Self {
        Self {
            compressed,
            revision: info.id,
            source: info.source_id.clone(),
            digest: info.digest.clone(),
        }
    }
}

/// A navigation anchor can outlive the archive-local revision handle.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct RecordAddress {
    pub conversation_id: ConversationId,
    pub source_id: SourceId,
    pub revision_digest: String,
    pub ordinal: PhysicalOrdinal,
    pub native_id: Option<String>,
}
/// Stable logical identity across source revisions.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct ConversationId(pub String);
/// Distinguishes independently captured sources in one logical conversation.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct SourceId(pub String);
/// Identifies an immutable interpretation for readers holding a published page.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct PublicationId(pub String);

/// Ordered outbox positions for reporting synchronization progress.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
pub struct IndexLag {
    pub indexed_through: Option<i64>,
    pub target: i64,
}

/// Selects evidence addressing before interpreting native records.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Transport {
    Plain,
    Zstd,
}
impl Transport {
    pub fn from_path(path: &str) -> Self {
        if path.ends_with(".zst") || path.ends_with(".zstd") { Self::Zstd } else { Self::Plain }
    }
}

/// Framing and reading count decoded transport work, including native-frame replay.
/// Fetching and verification report work within the current stored slice. A UI
/// can keep its last decoded position while showing either slice phase.
#[derive(Clone, Copy, Debug, Serialize)]
pub struct ReadProgress {
    pub phase: ReadPhase,
    pub unit: ProgressUnit,
    pub done: u64,
    pub total: Option<u64>,
}

/// Identifies storage work before authentication and native work after verified bytes are released.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
pub enum ReadPhase {
    FetchingSlice,
    VerifyingSlice,
    Framing,
    Reading,
}
/// The byte axis is explicit because native and decoded lengths can differ substantially.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
pub enum ProgressUnit {
    SliceBytes,
    DecodedBytes,
}

/// The reader checks the shared cancel token between bounded decoder buffers.
#[derive(Clone)]
pub struct JobControl {
    pub cancel: CancelToken,
    pub progress: Option<Arc<dyn Fn(ReadProgress) + Send + Sync>>,
    pub max_decoded_bytes: u64,
}
impl Default for JobControl {
    fn default() -> Self {
        Self {
            cancel: CancelToken::new(),
            progress: None,
            max_decoded_bytes: 8 * 1024 * 1024 * 1024,
        }
    }
}
impl JobControl {
    fn checkpoint(&self, phase: ReadPhase, decoded_bytes: u64) -> Result<()> {
        if decoded_bytes > self.max_decoded_bytes {
            return Err(Error::LimitExceeded("decoded byte budget".into()));
        }
        if let Some(report) = &self.progress {
            report(ReadProgress {
                phase,
                unit: ProgressUnit::DecodedBytes,
                done: decoded_bytes,
                total: None,
            })
        }
        self.cancel.check().map_err(|_| Error::Cancelled)
    }
}

/// Continuations are byte offsets within the same physical record.
#[derive(Debug, Serialize)]
pub struct BytePage {
    pub bytes: Vec<u8>,
    pub next: Option<u64>,
}
/// Supplies exact bytes even for oversized records and unsupported native input.
#[derive(Debug, Serialize)]
pub struct ReaderRow {
    pub source_id: SourceId,
    pub revision_digest: String,
    pub record: RecordLocator,
    pub raw: BytePage,
    pub interpretation: Option<InterpretedRecord>,
    /// Typed display fields derived from the complete object when its raw page continues.
    pub preview: Option<DisplayRow>,
}

impl ReaderRow {
    /// Captures the logical anchor while the caller still holds the publication's member mapping.
    pub fn address(&self, conversation_id: ConversationId) -> RecordAddress {
        RecordAddress {
            conversation_id,
            source_id: self.source_id.clone(),
            revision_digest: self.revision_digest.clone(),
            ordinal: self.record.ordinal,
            native_id: self
                .interpretation
                .as_ref()
                .and_then(|row| row.event.native_id.clone())
                .or_else(|| self.preview.as_ref()?.native_id.clone()),
        }
    }
}

/// Keeps normalized export fields and the reader's typed content tied to the same object.
#[derive(Debug, Serialize)]
pub struct InterpretedRecord {
    pub boundary: Option<StructuralBoundary>,
    pub event: NativeEvent,
    pub parts: Vec<RecordPart>,
}

/// Coverage of one declared read or publication, independent of evidence retention.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub enum Coverage {
    Complete,
    Partial { reasons: Vec<Diagnostic> },
    Building,
    Unavailable { reason: Diagnostic },
}

/// Framing coverage includes every physical row, even when it has no public event.
#[derive(Debug, Serialize)]
pub struct ScanReport {
    pub decoded_bytes: u64,
    pub max_frame_bytes: u64,
    pub physical_records: u64,
    pub objects: u64,
    pub coverage: Coverage,
}

/// Native structural information displayed between ordinary messages.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct StructuralBoundary {
    pub kind: BoundaryKind,
    pub tokens_before: Option<u64>,
}
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum BoundaryKind {
    Compaction,
    ContextEdit,
    UnresolvedParent,
}
