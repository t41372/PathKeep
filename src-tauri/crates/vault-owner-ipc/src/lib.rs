//! Responsibilities: local owner transport shared by the desktop and its clients.
//! Not responsible for: databases, keys, configuration or business operations.
//! Failure modes: clients gain storage capabilities; platform and wire contracts drift.
//! Tests: transport and desktop owner-process integration tests.
mod client;
mod launch;
mod logs;
pub mod protocol;
mod server;
#[cfg(unix)]
#[path = "unix.rs"]
mod transport;
#[cfg(windows)]
#[path = "windows.rs"]
mod transport;
pub use client::Client;
pub use launch::launch_background;
pub use logs::cap_log;
pub use protocol::{ErrorCode, Request, RpcError};
pub use server::{Progress, Server};

/// Keeps single-instance handoff isolated between users and test/data roots.
pub fn instance_identifier(root: &std::path::Path) -> String {
    use sha2::{Digest, Sha256};
    let root = std::path::absolute(root).unwrap_or_else(|_| root.to_owned());
    let digest = Sha256::digest(root.as_os_str().as_encoded_bytes());
    format!("com.yi-ting.pathkeep.p{:x}", digest)[..54].to_owned()
}
