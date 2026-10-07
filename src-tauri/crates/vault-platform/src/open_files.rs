//! Responsibilities: raise this process's soft open-file limit at startup.
//! Not responsible for: deciding how many files the archive keeps open; see
//! `vault-core/tests/open_file_budget.rs` for that budget.
//! Failure modes: a macOS app started from Finder or at login gets a soft
//! limit of 256. The archive's databases, the search index's segments, logs,
//! sockets and the webview's pipes share it, so a large archive could fail
//! with "Too many open files" in the middle of a backup or search.

/// The soft limit the app asks for. macOS's `OPEN_MAX`; far above what the
/// app needs (the budget test holds one open archive to a fraction of 256),
/// and low enough that every supported system's hard limit allows it.
#[cfg(unix)]
const WANTED: libc::rlim_t = 10_240;

/// Raises the soft limit towards `WANTED`, never lowering it and never past
/// the hard limit. Returns the limit in force afterwards.
#[cfg(unix)]
pub fn raise_open_file_limit() -> std::io::Result<u64> {
    let mut limit = libc::rlimit { rlim_cur: 0, rlim_max: 0 };
    // SAFETY: `limit` is a valid, writable rlimit.
    if unsafe { libc::getrlimit(libc::RLIMIT_NOFILE, &mut limit) } != 0 {
        return Err(std::io::Error::last_os_error());
    }
    let target = WANTED.min(limit.rlim_max);
    if limit.rlim_cur >= target {
        return Ok(limit.rlim_cur);
    }
    let raised = libc::rlimit { rlim_cur: target, rlim_max: limit.rlim_max };
    // SAFETY: `raised` is a valid rlimit whose soft value is within the hard one.
    if unsafe { libc::setrlimit(libc::RLIMIT_NOFILE, &raised) } != 0 {
        return Err(std::io::Error::last_os_error());
    }
    Ok(target)
}

/// Windows has no comparable per-process limit on handles.
#[cfg(not(unix))]
pub fn raise_open_file_limit() -> std::io::Result<u64> {
    Ok(u64::MAX)
}

#[cfg(all(test, unix))]
mod tests {
    use super::*;

    #[test]
    fn raises_a_low_soft_limit_but_never_lowers_or_passes_the_hard_one() {
        let mut original = libc::rlimit { rlim_cur: 0, rlim_max: 0 };
        unsafe { libc::getrlimit(libc::RLIMIT_NOFILE, &mut original) };
        let low =
            libc::rlimit { rlim_cur: 256.min(original.rlim_max), rlim_max: original.rlim_max };
        assert_eq!(unsafe { libc::setrlimit(libc::RLIMIT_NOFILE, &low) }, 0);

        let raised = raise_open_file_limit().unwrap();
        assert_eq!(raised, WANTED.min(original.rlim_max));
        // Running it again changes nothing.
        assert_eq!(raise_open_file_limit().unwrap(), raised);

        let high = libc::rlimit { rlim_cur: original.rlim_max, rlim_max: original.rlim_max };
        if original.rlim_max > WANTED && unsafe { libc::setrlimit(libc::RLIMIT_NOFILE, &high) } == 0
        {
            assert_eq!(raise_open_file_limit().unwrap(), original.rlim_max);
        }
        unsafe { libc::setrlimit(libc::RLIMIT_NOFILE, &original) };
    }
}
