//! Named points where a crash test can stop a child process.
//!
//! Responsibilities: when the test-only `crash-points` feature is on and the environment
//! variable `VAULT_CRYPTO_CRASH_AT` is `<name>:<n>`, the n-th time the process reaches the
//! point called `<name>` that thread prints `crash-point <name>` to stdout and stops forever, so the
//! parent test can SIGKILL it in exactly that state. Small atomic steps (writing a 94-byte
//! file, a rename) are over too quickly for a randomly timed kill to land inside them.
//!
//! Not responsible for: anything in a normal build. Without the feature, `crash_point` is an
//! empty inline function and no environment variable is read.

#[cfg(feature = "crash-points")]
pub(crate) fn crash_point(name: &str) {
    use std::io::Write;
    use std::sync::Mutex;

    static HITS: Mutex<Vec<(String, u64)>> = Mutex::new(Vec::new());

    let Ok(spec) = std::env::var("VAULT_CRYPTO_CRASH_AT") else { return };
    let Some((target, nth)) = spec.rsplit_once(':') else { return };
    if target != name {
        return;
    }
    let nth: u64 = nth.parse().expect("VAULT_CRYPTO_CRASH_AT is <name>:<n>");
    let mut hits = HITS.lock().unwrap();
    let count = match hits.iter_mut().find(|(seen, _)| seen == name) {
        Some((_, count)) => {
            *count += 1;
            *count
        }
        None => {
            hits.push((name.to_owned(), 1));
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
pub(crate) fn crash_point(_name: &str) {}
