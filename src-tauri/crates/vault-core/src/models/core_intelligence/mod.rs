//! Core Intelligence transport model façade.
//!
//! ## Responsibilities
//! - Re-export the stable DTO surface consumed by Tauri commands, the worker,
//!   and frontend IPC bindings.
//! - Keep request, read-model, analytics, overview, and trusted-output payloads
//!   in focused owner modules.
//! - Preserve serde names and enum tags while backend implementation modules
//!   continue to evolve.
//!
//! ## Not responsible for
//! - Running Core Intelligence rebuilds or read-model SQL.
//! - Defining frontend layout or copy.
//! - Owning optional AI/embedding provider payloads.
//!
//! ## Dependencies
//! - `serde` derives inside each DTO-family module.
//! - `schedule::GeneratedFile` for local-host artifact previews.
//!
//! ## Performance notes
//! - These are transport structs only. Large result sets must stay bounded by
//!   request limit/page fields before being placed into these DTOs.

mod analytics;
mod exports;
mod overview;
mod reads;
mod requests;
mod shared;

#[cfg(test)]
mod tests;

pub use analytics::*;
pub use exports::*;
pub use overview::*;
pub use reads::*;
pub use requests::*;
pub use shared::*;
