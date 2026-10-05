//! Responsibilities: Expose the search engine contract.
//! Not responsible for: Canonical data, normalization, UI, and cryptography.
//! Failure modes: Integration can accidentally depend on vault-core; the manifest and
//! workspace gates guard the boundary.

mod analyzer;
mod collector;
mod error;
mod filter;
mod index;
mod model;
mod progress;
mod query;
mod schema;
mod statistics;

pub use collector::Hit;
pub use error::{Error, Result};
pub use filter::{FilterPlan, Filters};
pub use index::{RebuildProgress, SearchIndex};
pub use model::{Batch, Config, PageDocument, ReloadPolicy, TextField};
pub use progress::{CountHandle, CountStep, Cursor, PageHandle, PageStep, Progress, SearchSession};
pub use query::{Ast, PhraseTerm};
