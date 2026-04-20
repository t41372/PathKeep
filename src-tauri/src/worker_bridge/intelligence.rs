//! Worker-bridge helpers for optional AI and deterministic intelligence.
//!
//! TODO: M11 - Revisit the remaining `vault-worker` pass-through noise after the
//! app-wide transport audit decides whether further decomposition would reduce
//! real ownership drift rather than just move thin wrappers between files.

mod ai;
mod core;
mod runtime;

pub(crate) use self::{ai::*, core::*, runtime::*};
