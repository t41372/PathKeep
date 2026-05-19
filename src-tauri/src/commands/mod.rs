//! Tauri command registration surface.
//!
//! Each submodule groups commands by product domain so the desktop shell can
//! expose a stable IPC surface without mixing transport code into the worker or
//! core crates. Command functions should stay thin: validate transport-level
//! types if needed, read session state, and delegate.

mod annotations;
mod app;
mod archive;
mod blocking;
mod import;
mod intelligence;
mod remote;
mod schedule;
mod security;
mod support;
mod update;

#[cfg(not(test))]
/// Re-exports the full production command surface for `tauri::generate_handler!`.
pub(crate) use self::{
    annotations::*, app::*, archive::*, import::*, intelligence::*, remote::*, schedule::*,
    security::*, support::*, update::*,
};
