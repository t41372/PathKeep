//! Worker-bridge helpers for optional AI and deterministic intelligence.
//!
//! TODO: M13 - Revisit the remaining `vault-worker` pass-through noise only if
//! the next transport parity audit proves further decomposition would reduce
//! real ownership drift rather than just move thin wrappers between files.

mod ai;
mod core;
mod runtime;

pub(crate) use self::{ai::*, core::*, runtime::*};
