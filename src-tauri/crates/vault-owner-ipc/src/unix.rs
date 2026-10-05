//! Responsibilities: private Unix socket, peer identity and crash recovery.
//! Not responsible for: acquiring ownership or application dispatch.
//! Failure modes: stale socket blocks startup; live socket is replaced; symlinks or
//! permissive modes expose requests to another user. Tests: transport.rs.
use std::{
    io,
    os::unix::fs::{DirBuilderExt, MetadataExt, PermissionsExt},
    path::{Path, PathBuf},
};
use tokio::net::{UnixListener, UnixStream};

pub struct Listener {
    listener: UnixListener,
    path: PathBuf,
}
impl Listener {
    pub async fn bind(root: &Path) -> io::Result<Self> {
        let directory = root.join("ipc");
        match std::fs::DirBuilder::new().mode(0o700).create(&directory) {
            Ok(()) => {}
            Err(error) if error.kind() == io::ErrorKind::AlreadyExists => {}
            Err(error) => return Err(error),
        }
        let metadata = std::fs::symlink_metadata(&directory)?;
        if !metadata.is_dir() || metadata.uid() != uid() {
            return Err(io::Error::new(
                io::ErrorKind::PermissionDenied,
                "owner IPC directory must belong to the current user and not be a symlink",
            ));
        }
        std::fs::set_permissions(&directory, std::fs::Permissions::from_mode(0o700))?;
        let path = directory.join("owner.sock");
        match UnixStream::connect(&path).await {
            Ok(_) => {
                return Err(io::Error::new(
                    io::ErrorKind::AddrInUse,
                    "PathKeep owner endpoint is already running",
                ));
            }
            Err(error) if error.kind() == io::ErrorKind::ConnectionRefused => {
                std::fs::remove_file(&path)?
            }
            Err(error) if error.kind() == io::ErrorKind::NotFound => {}
            Err(error) => return Err(error),
        }
        let listener = UnixListener::bind(&path)?;
        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o600))?;
        Ok(Self { listener, path })
    }
    pub async fn accept(&mut self) -> io::Result<UnixStream> {
        loop {
            let (stream, _) = self.listener.accept().await?;
            if stream.peer_cred().is_ok_and(|credentials| credentials.uid() == uid()) {
                return Ok(stream);
            }
        }
    }
}
impl Drop for Listener {
    fn drop(&mut self) {
        let _ = std::fs::remove_file(&self.path);
    }
}
fn uid() -> u32 {
    unsafe { libc::geteuid() }
}
pub async fn connect(root: &Path) -> io::Result<UnixStream> {
    let stream = UnixStream::connect(root.join("ipc/owner.sock")).await?;
    if stream.peer_cred()?.uid() != uid() {
        return Err(io::Error::new(
            io::ErrorKind::PermissionDenied,
            "PathKeep owner belongs to another user",
        ));
    }
    Ok(stream)
}
pub fn not_running(error: &io::Error) -> bool {
    matches!(error.kind(), io::ErrorKind::NotFound | io::ErrorKind::ConnectionRefused)
}
