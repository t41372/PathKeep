//! Volume facts used by Storage and by operations that rename a generation into raw-snapshots.
//! Inspection follows the destination a rename will use. It never creates a probe file.
use crate::ProjectPaths;
use std::{fs, io, path::Path};

/// True when the selected folder (or the parent in which it will be created) is on FAT.
pub fn fat_data_folder(folder: &Path) -> io::Result<bool> {
    let folder = existing_directory(folder)?;
    is_fat(&folder)
}

fn existing_directory(path: &Path) -> io::Result<std::path::PathBuf> {
    for ancestor in path.ancestors() {
        match fs::canonicalize(ancestor) {
            Err(e) if e.kind() == io::ErrorKind::NotFound => continue,
            result => return result,
        }
    }
    Err(io::Error::new(io::ErrorKind::NotFound, "no existing parent directory"))
}

/// The installer uses rename for both publication and retaining the previous generation.
pub fn require_snapshot_volume(paths: &ProjectPaths) -> anyhow::Result<()> {
    if !paths.app_root.exists() {
        return Ok(());
    }
    let snapshots = existing_directory(&paths.raw_snapshots_dir)?;
    anyhow::ensure!(
        volume_id(&snapshots)? == volume_id(&paths.app_root)?,
        "snapshot-folder-cross-device: raw-snapshots must be on the archive volume for this operation"
    );
    Ok(())
}

/// The detached package must be a rename away from the source stores.
pub(crate) fn require_handoff_volume(root: &Path, package: &Path) -> anyhow::Result<()> {
    anyhow::ensure!(
        volume_id(root)? == volume_id(&existing_directory(package)?)?,
        "archive-handoff-cross-device: the archive and its sibling package must share a volume"
    );
    Ok(())
}

#[cfg(unix)]
fn volume_id(path: &Path) -> io::Result<u64> {
    use std::os::unix::fs::MetadataExt;
    Ok(fs::metadata(path)?.dev() as u64)
}

#[cfg(any(target_os = "macos", target_os = "linux"))]
fn is_fat(path: &Path) -> io::Result<bool> {
    use std::os::unix::ffi::OsStrExt;
    let path = std::ffi::CString::new(path.as_os_str().as_bytes())?;
    let mut info = std::mem::MaybeUninit::<libc::statfs>::uninit();
    // SAFETY: statfs initializes info on success and the pathname lives through the call.
    if unsafe { libc::statfs(path.as_ptr(), info.as_mut_ptr()) } != 0 {
        return Err(io::Error::last_os_error());
    }
    let info = unsafe { info.assume_init() };
    #[cfg(target_os = "macos")]
    {
        let name = unsafe { std::ffi::CStr::from_ptr(info.f_fstypename.as_ptr()) };
        Ok(matches!(name.to_bytes(), b"msdos" | b"exfat"))
    }
    #[cfg(target_os = "linux")]
    Ok(matches!(info.f_type as u32, 0x4d44 | 0x2011_bab0))
}

#[cfg(windows)]
fn windows_volume(path: &Path) -> io::Result<(u32, String)> {
    use std::os::windows::{fs::OpenOptionsExt, io::AsRawHandle};
    use windows_sys::Win32::Storage::FileSystem::{
        FILE_FLAG_BACKUP_SEMANTICS, GetVolumeInformationByHandleW,
    };
    let file =
        fs::OpenOptions::new().read(true).custom_flags(FILE_FLAG_BACKUP_SEMANTICS).open(path)?;
    let mut serial = 0;
    let mut name = [0u16; 64];
    // SAFETY: file owns the handle, and Windows receives the size of the output buffer.
    let ok = unsafe {
        GetVolumeInformationByHandleW(
            file.as_raw_handle(),
            std::ptr::null_mut(),
            0,
            &mut serial,
            std::ptr::null_mut(),
            std::ptr::null_mut(),
            name.as_mut_ptr(),
            name.len() as u32,
        )
    };
    if ok == 0 {
        return Err(io::Error::last_os_error());
    }
    Ok((
        serial,
        String::from_utf16_lossy(&name[..name.iter().position(|u| *u == 0).unwrap_or(name.len())]),
    ))
}

#[cfg(windows)]
fn volume_id(path: &Path) -> io::Result<u64> {
    Ok(windows_volume(path)?.0.into())
}
#[cfg(windows)]
fn is_fat(path: &Path) -> io::Result<bool> {
    Ok(matches!(windows_volume(path)?.1.to_ascii_uppercase().as_str(), "FAT" | "FAT32" | "EXFAT"))
}
