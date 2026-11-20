//! Responsibilities: describe archive identities, paths and each lifecycle's membership.
//! Not responsible for: filesystem I/O, schema SQL, credentials, or admitting an archive.

mod catalog;
mod identity;
mod paths;
mod policies;

pub use catalog::{ArtifactSpec, DatabaseSpec, database_spec, db_artifact, spec};
pub use identity::{ArtifactId, Db, KeyPurpose};
pub use paths::*;
pub use policies::*;

/// Records unreadability at capture. Repair independently verifies the selected evidence.
pub const UNREADABLE_CONVERSATIONS: &str = "conversations-unreadable";

/// Bytes of the displaced open file when its live pathname is missing or occupied by another file.
pub const HELD_CONVERSATIONS: &str = "held-conversations";

/// Families introduced by the first agent-aware release. A downgrade detaches them by rename.
pub const NATIVE_COPIES_V06: &str = "raw-snapshots/maintenance/v06";
pub const PREVIOUS_GENERATIONS_V06: &str = "quarantine/v06/previous-generations";
pub const MOVE_FILES_V3: &str = "exports/pathkeep/format-3";
