//! Core Intelligence regression suite owners.
//!
//! ## Responsibilities
//! - Keep regression coverage outside the production `intelligence::mod` parent.
//! - Group tests by the behavior surface they protect.
//! - Share compact fixture helpers across the focused test modules.
//!
//! ## Not responsible for
//! - Owning production Core Intelligence rebuild or read-model behavior.
//! - Re-exporting test helpers outside `cfg(test)`.
//!
//! ## Dependencies
//! - Sibling modules exercise the private Core Intelligence owners directly.
//! - `fixtures` creates small canonical archive states for deterministic tests.
//!
//! ## Performance notes
//! Fixtures stay small while still covering batch and fallback boundaries; the
//! large-data contract is protected by batch-size assertions and equivalence
//! checks rather than by allocating huge in-memory test datasets.

mod batch_equivalence;
mod fixtures;
mod schema_overview;
mod stage_rebuild;
mod structural_incremental;
