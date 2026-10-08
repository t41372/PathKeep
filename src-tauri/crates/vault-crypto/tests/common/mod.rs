//! Responsibilities: support real process-kill tests, byte scanning and memory observations.
//! Not responsible for: production cryptography or deciding whether an operation may publish.
//! Failure modes: missing atomically replaced plaintext files, ignored child failures or unbounded
//! scanner buffers.
//!
//! Shared test helpers: child processes that can be SIGKILLed, a reproducible random source,
//! a raw-byte scanner for plaintext, and load averages for timing records.
//!
//! A crash test re-runs its own test binary as a child, selecting one `#[ignore]`d test that
//! does the work only when `VAULT_CRYPTO_CHILD` is set. The parent reads the child's stdout
//! lines, kills it with SIGKILL at a chosen moment, and inspects what is left on disk.

#![allow(dead_code)]

pub mod pages;

use std::collections::BTreeMap;
use std::io::{BufRead, BufReader, Read};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{Receiver, RecvTimeoutError, channel};
use std::thread::JoinHandle;
use std::time::{Duration, Instant};

pub const CHILD_ENV: &str = "VAULT_CRYPTO_CHILD";

/// True inside a child started by [`ChildProcess::spawn`].
pub fn is_child() -> bool {
    std::env::var_os(CHILD_ENV).is_some()
}

pub struct ChildProcess {
    child: std::process::Child,
    lines: Receiver<String>,
    /// Every line received so far, including those `wait_for_line` read past.
    seen: Vec<String>,
}

impl ChildProcess {
    /// Runs the test called `test_name` from this test binary in a new process.
    pub fn spawn(test_name: &str, env: &[(&str, &str)]) -> ChildProcess {
        let mut command = Command::new(std::env::current_exe().unwrap());
        command
            .args([test_name, "--exact", "--include-ignored", "--nocapture", "--test-threads=1"])
            .env(CHILD_ENV, "1")
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::inherit());
        for (key, value) in env {
            command.env(key, value);
        }
        let mut child = command.spawn().unwrap();
        let stdout = child.stdout.take().unwrap();
        let (sender, lines) = channel();
        std::thread::spawn(move || {
            for line in BufReader::new(stdout).lines() {
                let Ok(line) = line else { break };
                if sender.send(line).is_err() {
                    break;
                }
            }
        });
        ChildProcess { child, lines, seen: Vec::new() }
    }

    pub fn pid(&self) -> u32 {
        self.child.id()
    }

    /// Waits for a stdout line that contains `marker` and returns it. Panics on timeout or if
    /// the child exits first. (libtest prints `test name ... ` without a newline, so the
    /// child's first line does not start with what the child printed.)
    pub fn wait_for_line(&mut self, marker: &str, timeout: Duration) -> String {
        let deadline = Instant::now() + timeout;
        loop {
            let left = deadline.saturating_duration_since(Instant::now());
            match self.lines.recv_timeout(left) {
                Ok(line) => {
                    self.seen.push(line.clone());
                    if line.contains(marker) {
                        return line;
                    }
                }
                Err(RecvTimeoutError::Timeout) => {
                    self.kill();
                    panic!("child did not print {marker:?} within {timeout:?}");
                }
                Err(RecvTimeoutError::Disconnected) => {
                    let status = self.child.wait().unwrap();
                    panic!("child exited ({status}) before printing {marker:?}");
                }
            }
        }
    }

    /// Every line the child has printed so far, without waiting.
    pub fn lines_so_far(&mut self) -> &[String] {
        self.seen.extend(self.lines.try_iter());
        &self.seen
    }

    /// SIGKILL on Unix: no destructors, no buffered writes, no atexit handlers.
    pub fn kill(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }

    /// Waits for a normal exit, asserts it succeeded, and returns everything the child
    /// printed to stdout.
    pub fn wait_success(mut self, timeout: Duration) -> Vec<String> {
        let deadline = Instant::now() + timeout;
        let mut lines = Vec::new();
        loop {
            lines.extend(self.lines.try_iter());
            if let Some(status) = self.child.try_wait().unwrap() {
                // The pipe closes at exit; collect what the reader thread has not passed on yet.
                while let Ok(line) = self.lines.recv_timeout(Duration::from_secs(5)) {
                    lines.push(line);
                }
                assert!(status.success(), "child failed: {status}\n{}", lines.join("\n"));
                return lines;
            }
            if Instant::now() > deadline {
                self.kill();
                panic!("child did not finish within {timeout:?}\n{}", lines.join("\n"));
            }
            std::thread::sleep(Duration::from_millis(20));
        }
    }
}

impl Drop for ChildProcess {
    fn drop(&mut self) {
        if let Ok(None) = self.child.try_wait() {
            self.kill();
        }
    }
}

/// xorshift64*: enough to spread kill times; seeded explicitly so every run can be replayed.
pub struct Rng(u64);

impl Rng {
    pub fn new(seed: u64) -> Rng {
        Rng(seed.max(1))
    }

    /// Seed from `VAULT_CRYPTO_SEED` if set, otherwise from the clock. Printed by callers.
    pub fn from_env_or_clock() -> (Rng, u64) {
        let seed = std::env::var("VAULT_CRYPTO_SEED")
            .ok()
            .and_then(|s| s.parse().ok())
            .unwrap_or_else(|| {
                std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap()
                    .as_nanos() as u64
            });
        (Rng::new(seed), seed)
    }

    pub fn next_u64(&mut self) -> u64 {
        self.0 ^= self.0 >> 12;
        self.0 ^= self.0 << 25;
        self.0 ^= self.0 >> 27;
        self.0.wrapping_mul(0x2545_F491_4F6C_DD1D)
    }

    /// Uniform in `0..bound`.
    pub fn below(&mut self, bound: u64) -> u64 {
        self.next_u64() % bound
    }
}

/// Every needle that occurs anywhere in any regular file under `dir`, with the file it was
/// found in. Streams files in 1 MiB chunks with overlap, so large files are fine.
pub fn find_plaintext(dir: &Path, needles: &[&[u8]]) -> Vec<(String, String)> {
    let mut found = Vec::new();
    let mut stack = vec![dir.to_path_buf()];
    while let Some(dir) = stack.pop() {
        let Ok(entries) = std::fs::read_dir(&dir) else { continue };
        for entry in entries.flatten() {
            let path = entry.path();
            let Ok(meta) = std::fs::symlink_metadata(&path) else { continue };
            if meta.is_dir() {
                stack.push(path);
            } else if meta.is_file() {
                for needle in scan_file(&path, needles) {
                    found.push((
                        path.display().to_string(),
                        String::from_utf8_lossy(needle).into_owned(),
                    ));
                }
            }
        }
    }
    found
}

/// The needles that occur in one file.
pub fn scan_file<'a>(path: &Path, needles: &[&'a [u8]]) -> Vec<&'a [u8]> {
    let Ok(mut file) = std::fs::File::open(path) else { return Vec::new() };
    let overlap = needles.iter().map(|n| n.len()).max().unwrap_or(1) - 1;
    let mut hits = vec![false; needles.len()];
    let mut buffer = vec![0u8; (1 << 20) + overlap];
    let mut carried = 0;
    loop {
        let read = match file.read(&mut buffer[carried..]) {
            Ok(0) | Err(_) => break,
            Ok(n) => n,
        };
        let end = carried + read;
        for (i, needle) in needles.iter().enumerate() {
            if !hits[i] && contains(&buffer[..end], needle) {
                hits[i] = true;
            }
        }
        carried = overlap.min(end);
        buffer.copy_within(end - carried..end, 0);
    }
    needles.iter().zip(hits).filter(|(_, hit)| *hit).map(|(n, _)| *n).collect()
}

/// Whether `needle` occurs in `haystack`. Compares only where the first byte matches, which
/// keeps scans of ciphertext (where it almost never does) fast.
pub fn contains(haystack: &[u8], needle: &[u8]) -> bool {
    let Some((&first, rest)) = needle.split_first() else { return true };
    if haystack.len() < needle.len() {
        return false;
    }
    let last_start = haystack.len() - needle.len();
    haystack[..=last_start]
        .iter()
        .enumerate()
        .any(|(i, &byte)| byte == first && &haystack[i + 1..i + needle.len()] == rest)
}

/// `uptime`'s load averages, for timing records.
pub fn load_averages() -> String {
    let out =
        Command::new("uptime").output().map(|o| String::from_utf8_lossy(&o.stdout).into_owned());
    out.ok()
        .and_then(|text| {
            text.split("load average")
                .nth(1)
                .map(|s| s.trim_start_matches(['s', ':', ' ']).trim().to_owned())
        })
        .unwrap_or_default()
}

/// Peak resident set size of this process so far, in MiB.
pub fn peak_rss_mib() -> f64 {
    let mut usage: libc::rusage = unsafe { std::mem::zeroed() };
    // SAFETY: getrusage writes into the struct we pass and has no other effect.
    unsafe { libc::getrusage(libc::RUSAGE_SELF, &mut usage) };
    let max = usage.ru_maxrss as f64;
    if cfg!(target_os = "macos") { max / (1024.0 * 1024.0) } else { max / 1024.0 }
}

/// Current resident set size of a process, in MiB, from `ps`.
pub fn current_rss_mib(pid: u32) -> f64 {
    let out = Command::new("ps").args(["-o", "rss=", "-p", &pid.to_string()]).output().unwrap();
    String::from_utf8_lossy(&out.stdout).trim().parse::<f64>().unwrap_or(0.0) / 1024.0
}

/// Records every file that appears under the watched directories while it runs, and keeps
/// each one's bytes after the program deletes it: new files are hard-linked into a
/// quarantine directory (on the same file system), so the final contents of short-lived
/// spill files and merged-away segments can still be scanned.
///
/// Limit: only the last bytes of each file are seen. Plaintext that was written and then
/// overwritten is caught by recording writes at the IO layer instead, where that is possible.
pub struct Watcher {
    stop: Arc<AtomicBool>,
    handle: Option<JoinHandle<Vec<WatchedFile>>>,
    quarantine: PathBuf,
}

#[derive(Debug, Clone)]
pub struct WatchedFile {
    pub path: PathBuf,
    pub peak_bytes: u64,
    /// Where the file's bytes can still be read, even if the original was deleted.
    pub copy: Option<PathBuf>,
}

impl Watcher {
    pub fn start(dirs: Vec<PathBuf>, quarantine: PathBuf) -> Watcher {
        std::fs::create_dir_all(&quarantine).unwrap();
        let stop = Arc::new(AtomicBool::new(false));
        let flag = stop.clone();
        let q = quarantine.clone();
        let handle = std::thread::spawn(move || {
            let mut seen: BTreeMap<PathBuf, WatchedFile> = BTreeMap::new();
            loop {
                let last_pass = flag.load(Ordering::Acquire);
                let mut files = Vec::new();
                for dir in &dirs {
                    list_files(dir, &q, &mut files);
                }
                for (path, len) in files {
                    let n = seen.len();
                    let entry = seen.entry(path.clone()).or_insert_with(|| {
                        let name = path.file_name().unwrap().to_string_lossy().into_owned();
                        let link = q.join(format!("{n:06}-{name}"));
                        let copy = std::fs::hard_link(&path, &link).is_ok().then_some(link);
                        WatchedFile { path, peak_bytes: 0, copy }
                    });
                    entry.peak_bytes = entry.peak_bytes.max(len);
                }
                for file in seen.values_mut() {
                    if let Some(len) =
                        file.copy.as_ref().and_then(|c| std::fs::metadata(c).ok()).map(|m| m.len())
                    {
                        file.peak_bytes = file.peak_bytes.max(len);
                    }
                }
                if last_pass {
                    break;
                }
                std::thread::sleep(Duration::from_millis(2));
            }
            seen.into_values().collect()
        });
        Watcher { stop, handle: Some(handle), quarantine }
    }

    /// Scans retained links and current paths: atomic replacement can leave the
    /// original hard link pointing at different bytes from the final live file.
    pub fn finish(mut self, needles: &[&[u8]]) -> Vec<(WatchedFile, Vec<String>)> {
        self.stop.store(true, Ordering::Release);
        let files = self.handle.take().unwrap().join().unwrap();
        files
            .into_iter()
            .map(|file| {
                let hits = file
                    .copy
                    .iter()
                    .chain(std::iter::once(&file.path))
                    .flat_map(|source| scan_file(source, needles))
                    .map(|n| String::from_utf8_lossy(n).into_owned())
                    .collect::<std::collections::BTreeSet<_>>()
                    .into_iter()
                    .collect();
                (file, hits)
            })
            .collect()
    }
}

impl Drop for Watcher {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Release);
        if let Some(handle) = self.handle.take() {
            let _ = handle.join();
        }
        let _ = std::fs::remove_dir_all(&self.quarantine);
    }
}

fn list_files(dir: &Path, skip: &Path, out: &mut Vec<(PathBuf, u64)>) {
    let Ok(entries) = std::fs::read_dir(dir) else { return };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.starts_with(skip) {
            continue;
        }
        let Ok(meta) = std::fs::symlink_metadata(&path) else { continue };
        if meta.is_dir() {
            list_files(&path, skip, out);
        } else if meta.is_file() {
            out.push((path, meta.len()));
        }
    }
}
