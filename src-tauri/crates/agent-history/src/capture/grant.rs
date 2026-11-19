//! Bind a reviewed source to the same native directory across owner restarts.
//! These probes open the directory itself and never enumerate its children.
use std::path::Path;

/// Retain a native directory handle throughout review to detect a replacement.
pub fn directory_handle(path: &Path) -> std::io::Result<same_file::Handle> {
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        let file = std::fs::OpenOptions::new()
            .read(true)
            .custom_flags(libc::O_DIRECTORY | libc::O_NOFOLLOW)
            .open(path)?;
        same_file::Handle::from_file(file)
    }
    #[cfg(windows)]
    {
        use std::os::windows::fs::{MetadataExt, OpenOptionsExt};
        const REPARSE_POINT: u32 = 0x400;
        let file = std::fs::OpenOptions::new()
            .read(true)
            .custom_flags(0x0200_0000 | 0x0020_0000)
            .open(path)?;
        let metadata = file.metadata()?;
        if !metadata.is_dir() || metadata.file_attributes() & REPARSE_POINT != 0 {
            return Err(std::io::ErrorKind::NotADirectory.into());
        }
        same_file::Handle::from_file(file)
    }
}
/// Compare the volume and native file identity of a still-open directory.
pub fn directory_identity(handle: &same_file::Handle) -> std::io::Result<String> {
    file_identity(handle.as_file())
}

pub(super) fn file_identity(file: &std::fs::File) -> std::io::Result<String> {
    #[cfg(unix)]
    {
        use std::os::unix::fs::MetadataExt;
        let meta = file.metadata()?;
        let created = meta
            .created()
            .ok()
            .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
            .map(|d| d.as_nanos());
        Ok(format!("unix:{}:{}:{created:?}", meta.dev(), meta.ino()))
    }
    #[cfg(windows)]
    {
        let info = winapi_util::file::information(file)?;
        Ok(format!(
            "windows:{}:{}:{:?}",
            info.volume_serial_number(),
            info.file_index(),
            info.creation_time()
        ))
    }
}

/// Validate the saved path and directory identity without opening any children.
/// Missing media and a replacement directory have different retry implications.
pub fn directory_issue(
    source: &crate::SourceRoot,
    root_id: &str,
    expected: &str,
    archive: &Path,
) -> Option<&'static str> {
    let Ok(real) = super::normalized_root(&source.root) else { return Some("root-unreadable") };
    if real != source.root
        || real.starts_with(archive)
        || archive.starts_with(&real)
        || super::root_identity(source).as_deref().ok() != Some(root_id)
    {
        return Some("root-changed");
    }
    match directory_handle(&real).and_then(|d| directory_identity(&d)) {
        Ok(value) if value == expected => None,
        Ok(_) => Some("root-changed"),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Some("root-missing"),
        Err(_) => Some("root-unreadable"),
    }
}
