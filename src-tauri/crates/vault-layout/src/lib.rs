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
