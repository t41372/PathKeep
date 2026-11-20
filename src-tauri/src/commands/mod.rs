//! Tauri command registration surface.
//!
//! Each submodule groups commands by product domain so the desktop shell can
//! expose a stable IPC surface without mixing transport code into the worker or
//! core crates. Command functions should stay thin: validate transport-level
//! types if needed, read session state, and delegate.

mod agent_insights;
mod ai_chat;
mod annotations;
mod app;
mod archive;
mod blocking;
mod chats;
mod data_kinds;
mod desktop;
mod import;
mod intelligence;
mod migration;
mod projects;
mod rollback;
mod schedule;
mod search;
mod security;
mod stars;
mod support;
mod update;

#[cfg(not(test))]
/// Re-exports the full production command surface for `tauri::generate_handler!`.
pub(crate) use self::{
    agent_insights::*, ai_chat::*, annotations::*, app::*, archive::*, chats::*, data_kinds::*,
    desktop::*, import::*, intelligence::*, migration::*, projects::*, rollback::*, schedule::*,
    search::*, security::*, stars::*, support::*, update::*,
};
