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
