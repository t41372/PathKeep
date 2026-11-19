//! Shared facilities for synthetic Turso measurements. Nothing here opens a user's archive.
use std::{path::Path, time::Duration};
use vault_crypto::{DataKey, TempCryptIo};
use vault_store::{Config, Db, Store};

pub fn size(name: &str, default: usize) -> usize {
    let value =
        std::env::var(name).map_or(default, |s| s.parse().expect("positive measurement size"));
    assert!(value > 0, "{name} must be positive");
    value
}
pub fn open(root: &Path, encrypted: bool, role: Db) -> Store {
    let key = DataKey::from_bytes(zeroize::Zeroizing::new([0x39; 32]));
    let mut config = Config::new(root);
    config.databases = vec![Db::Scratch, role];
    for db in config.databases.iter().copied() {
        if encrypted {
            config.keys[db as usize] =
                Some(*key.derive(vault_layout::database_spec(db).key).expose_bytes())
        }
    }
    config.io = Some(TempCryptIo::new(&key).unwrap());
    Store::open(config).unwrap()
}

pub fn timings(mut samples: Vec<Duration>) -> serde_json::Value {
    samples.sort_unstable();
    let ms = |p: usize| samples[(samples.len() * p).div_ceil(100_usize) - 1].as_secs_f64() * 1000.;
    serde_json::json!({"n":samples.len(),"p50_ms":ms(50usize),"p95_ms":ms(95),"p99_ms":ms(99)})
}

/// Evict only closed synthetic files. Linux verifies residency after requesting
/// eviction so a tmpfs, pinned mapping or ignored hint cannot report a cold sample.
pub fn evict(root: &Path) -> bool {
    #[cfg(any(target_os = "linux", target_os = "macos"))]
    {
        for entry in walkdir::WalkDir::new(root) {
            let entry = entry.unwrap();
            if entry.file_type().is_file() {
                evict_file(entry.path())
            }
        }
        true
    }
    #[cfg(not(any(target_os = "linux", target_os = "macos")))]
    {
        let _ = root;
        false
    }
}
#[cfg(target_os = "macos")]
fn evict_file(path: &Path) {
    use std::os::fd::AsRawFd;
    let mut file = std::fs::File::open(path).unwrap();
    // SAFETY: the open descriptor stays alive throughout fcntl.
    assert_eq!(unsafe { libc::fcntl(file.as_raw_fd(), libc::F_NOCACHE, 1) }, 0);
    std::io::copy(&mut file, &mut std::io::sink()).unwrap();
}
#[cfg(target_os = "linux")]
fn evict_file(path: &Path) {
    use std::os::fd::AsRawFd;
    use std::os::unix::fs::MetadataExt;
    let metadata = path.metadata().unwrap();
    retry_eviction(
        path,
        || {
            let handles = std::fs::read_dir("/proc/self/fd")
                .unwrap()
                .filter_map(|e| std::fs::metadata(e.ok()?.path()).ok())
                .filter(|m| m.dev() == metadata.dev() && m.ino() == metadata.ino())
                .count();
            if handles > 0 {
                return (handles, 0);
            }
            let file = std::fs::File::open(path).unwrap();
            file.sync_all().unwrap();
            let len = usize::try_from(file.metadata().unwrap().len()).unwrap();
            if len == 0 {
                return (0, 0);
            }
            let fd = file.as_raw_fd();
            // SAFETY: this mapping is never dereferenced and is unmapped before retry.
            let resident = unsafe {
                assert_eq!(libc::posix_fadvise(fd, 0, 0, libc::POSIX_FADV_DONTNEED), 0);
                let ptr = libc::mmap(
                    std::ptr::null_mut(),
                    len,
                    libc::PROT_NONE,
                    libc::MAP_PRIVATE,
                    fd,
                    0,
                );
                assert_ne!(ptr, libc::MAP_FAILED);
                let page = usize::try_from(libc::sysconf(libc::_SC_PAGESIZE)).unwrap();
                let mut residency = vec![0; len.div_ceil(page)];
                let status = libc::mincore(ptr, len, residency.as_mut_ptr());
                assert_eq!(libc::munmap(ptr, len), 0);
                assert_eq!(status, 0);
                residency.into_iter().filter(|bits| bits & 1 != 0).count()
            };
            drop(file);
            (0, resident)
        },
        std::thread::sleep,
    )
}

#[cfg(any(target_os = "linux", test))]
fn retry_eviction(
    path: &Path,
    mut check: impl FnMut() -> (usize, usize),
    mut wait: impl FnMut(Duration),
) {
    for attempt in 1..=6 {
        let (handles, resident) = check();
        if handles == 0 && resident == 0 {
            return;
        }
        eprintln!(
            "{}",
            serde_json::json!({"measurement":"eviction_retry","path":path,"attempt":attempt,"resident_pages":resident,"open_handles":handles})
        );
        assert!(
            attempt < 6,
            "{} remains resident after six evictions; use a disk-backed TMPDIR",
            path.display()
        );
        wait(Duration::from_millis(25 << (attempt - 1)));
    }
}

#[cfg(test)]
mod eviction_tests {
    use super::*;
    #[test]
    fn transient_residency_retries_the_named_file() {
        let mut attempts = 0;
        let path = Path::new("synthetic/agent-history.db");
        retry_eviction(
            path,
            || {
                attempts += 1;
                (usize::from(attempts == 1), usize::from(attempts == 2))
            },
            |_| {},
        );
        assert_eq!(attempts, 3);
    }
}
