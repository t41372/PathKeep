//! Distinguish a denied Windows pathname probe from deletion of the open file.
//! Worker retirement and retry policy belong to the store.

use std::io;

/// An inconclusive handle query leaves the original access error in force.
pub(super) fn confirmed_deletion(
    error: &io::Error,
    query: impl FnOnce() -> io::Result<bool>,
) -> bool {
    matches!(error.raw_os_error(), Some(5 | 32 | 33 | 303)) && query().unwrap_or(false)
}

#[cfg(windows)]
pub(super) fn delete_pending(file: &std::fs::File) -> io::Result<bool> {
    use std::os::windows::io::AsRawHandle;
    use windows_sys::Win32::Storage::FileSystem::{
        FILE_STANDARD_INFO, FileStandardInfo, GetFileInformationByHandleEx,
    };
    let mut info = FILE_STANDARD_INFO::default();
    // The borrow keeps the handle open. The output buffer matches the requested information class.
    let ok = unsafe {
        GetFileInformationByHandleEx(
            file.as_raw_handle(),
            FileStandardInfo,
            (&raw mut info).cast(),
            size_of_val(&info) as u32,
        )
    };
    if ok == 0 { Err(io::Error::last_os_error()) } else { Ok(info.DeletePending) }
}
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn windows_name_refusal_requires_confirmation_from_the_handle() {
        for code in [5, 32, 33, 303] {
            let error = std::io::Error::from_raw_os_error(code);
            assert!(!confirmed_deletion(&error, || Ok(false)), "ordinary refusal {code}");
            assert!(!confirmed_deletion(&error, || Err(
                std::io::ErrorKind::PermissionDenied.into()
            )));
            assert!(confirmed_deletion(&error, || Ok(true)));
        }
        assert!(!confirmed_deletion(&std::io::Error::from_raw_os_error(123), || panic!(
            "unrelated error"
        )));
    }

    #[cfg(windows)]
    #[test]
    fn same_name_error_before_and_after_marking_the_held_file_for_deletion() {
        use std::os::windows::{fs::OpenOptionsExt, io::AsRawHandle};
        use windows_sys::Win32::Storage::FileSystem::{
            DELETE, FILE_DISPOSITION_INFO, FILE_READ_ATTRIBUTES, FileDispositionInfo,
            SetFileInformationByHandle,
        };
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("evidence.db");
        std::fs::write(&path, b"held evidence").unwrap();
        let held = std::fs::File::open(&path).unwrap();
        let denied = std::io::Error::from_raw_os_error(5);
        assert!(!confirmed_deletion(&denied, || delete_pending(&held)));
        let delete = std::fs::OpenOptions::new()
            .access_mode(DELETE | FILE_READ_ATTRIBUTES)
            .open(&path)
            .unwrap();
        let disposition = FILE_DISPOSITION_INFO { DeleteFile: true };
        // The setter reads this fixed-size structure during the call.
        let ok = unsafe {
            SetFileInformationByHandle(
                delete.as_raw_handle(),
                FileDispositionInfo,
                (&raw const disposition).cast(),
                size_of_val(&disposition) as u32,
            )
        };
        assert_ne!(ok, 0, "{}", std::io::Error::last_os_error());
        drop(delete);
        assert!(confirmed_deletion(&denied, || delete_pending(&held)));
        drop(held);
        assert!(!path.exists());
    }
}
