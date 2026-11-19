//! Responsibilities: pin the current owner's archive lifecycle using literal disk observations.
//! Not responsible for: v0.5 release compatibility, future roles, UI, or power-loss guarantees.
//! Each scenario owns synthetic browser, keyring, scheduler and archive roots inside this clone.

#[path = "vault_lifecycle/contracts.rs"]
mod contracts;
#[path = "vault_lifecycle/conversations.rs"]
mod conversations;
#[path = "vault_lifecycle/index.rs"]
mod index;
#[path = "vault_lifecycle/opening.rs"]
mod opening;
#[path = "vault_lifecycle/portable.rs"]
mod portable;
#[path = "vault_lifecycle/security.rs"]
mod security;
#[path = "vault_lifecycle/snapshots.rs"]
mod snapshots;
#[path = "vault_lifecycle/support/mod.rs"]
mod support;
#[path = "vault_lifecycle/wipe.rs"]
mod wipe;
