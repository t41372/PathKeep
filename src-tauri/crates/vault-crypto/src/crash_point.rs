//! Named points where a crash test can stop a child process.
//!
//! Responsibilities: when the test-only `crash-points` feature is on and the environment
//! variable `VAULT_CRYPTO_CRASH_AT` is `<step>:<n>` or `<step>@<file name>:<n>`, the n-th time
//! the process reaches that step (for that file) the thread prints `crash-point <step>` to
//! stdout and stops forever, so the parent test can SIGKILL the process in exactly that
//! state. Small atomic steps (writing a 94-byte file, a rename) are over too quickly for a
//! randomly timed kill to land inside them.
//!
//! Not responsible for: anything in a normal build. Without the feature, `crash_point` is an
//! empty inline function and no environment variable is read.

#[cfg(feature = "crash-points")]
pub(crate) fn crash_point(name: &str, file: &std::path::Path) {
    use std::io::Write;
    use std::sync::Mutex;

    static HITS: Mutex<Vec<(String, u64)>> = Mutex::new(Vec::new());

    let Ok(spec) = std::env::var("VAULT_CRYPTO_CRASH_AT") else { return };
    let Some((target, nth)) = spec.rsplit_once(':') else { return };
    let file_name = file.file_name().map(|n| n.to_string_lossy()).unwrap_or_default();
    if target != name && target != format!("{name}@{file_name}") {
        return;
    }
    let nth: u64 = nth.parse().expect("VAULT_CRYPTO_CRASH_AT is <name>:<n>");
    let mut hits = HITS.lock().unwrap();
    let count = match hits.iter_mut().find(|(seen, _)| *seen == target) {
        Some((_, count)) => {
            *count += 1;
            *count
        }
        None => {
            hits.push((target.to_owned(), 1));
            1
        }
    };
    if count == nth {
        let mut out = std::io::stdout().lock();
        writeln!(out, "crash-point {name}").unwrap();
        out.flush().unwrap();
        loop {
            std::thread::park();
        }
    }
}

#[cfg(not(feature = "crash-points"))]
#[inline(always)]
pub(crate) fn crash_point(_name: &str, _file: &std::path::Path) {}
