//! Responsibilities: keep the log files PathKeep's background processes append to bounded.
//! Not responsible for: what they log, or the app's own rotating log.
//! Failure modes: a log grows without end (the background app's stderr and a scheduled run's
//! output were appended to for good); the older copy is lost or the process that writes the
//! file stops reaching it.

use std::{fs, io, path::Path};

/// Past this size a log keeps one older copy and starts again.
pub const LOG_CAP_BYTES: u64 = 4 * 1024 * 1024;

/// Keeps `path` under [`LOG_CAP_BYTES`]: past it, the file is copied to `<name>.1` (replacing the
/// copy before) and emptied. Copied and emptied, not renamed, because a process that holds it
/// open (launchd, for a scheduled run) goes on appending to the same file.
pub fn cap_log(path: &Path) -> io::Result<()> {
    cap_log_at(path, LOG_CAP_BYTES)
}

fn cap_log_at(path: &Path, cap: u64) -> io::Result<()> {
    match fs::metadata(path) {
        Ok(metadata) if metadata.len() > cap => {}
        Ok(_) => return Ok(()),
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(()),
        Err(error) => return Err(error),
    }
    let mut older = path.as_os_str().to_owned();
    older.push(".1");
    fs::copy(path, &older)?;
    fs::OpenOptions::new().write(true).open(path)?.set_len(0)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    #[test]
    fn a_log_past_its_cap_keeps_one_older_copy_and_starts_again() {
        let dir = tempfile::tempdir().unwrap();
        let log = dir.path().join("worker.stderr.log");
        // Held open for appending, as launchd holds a scheduled run's output.
        let mut writer = fs::OpenOptions::new().create(true).append(true).open(&log).unwrap();
        writer.write_all(b"first run\n").unwrap();
        cap_log_at(&log, 100).unwrap();
        assert_eq!(fs::read(&log).unwrap(), b"first run\n", "under the cap: as it was");

        writer.write_all(&[b'x'; 200]).unwrap();
        cap_log_at(&log, 100).unwrap();
        assert_eq!(fs::read(&log).unwrap().len(), 0);
        assert_eq!(fs::read(dir.path().join("worker.stderr.log.1")).unwrap().len(), 210);
        writer.write_all(b"next run\n").unwrap();
        assert_eq!(fs::read(&log).unwrap(), b"next run\n", "the writer goes on in the same file");

        assert!(cap_log_at(&dir.path().join("missing.log"), 100).is_ok());
    }
}
