//! A crash in the middle of a password change leaves an envelope that opens, with the old
//! password or the new one, to the same data key. Checked by SIGKILLing a child process at
//! each step of the atomic replacement, and at random moments in a loop of rewraps.

mod common;

use std::path::{Path, PathBuf};
use std::time::Duration;

use common::{ChildProcess, Rng, is_child};
use vault_crypto::{Envelope, KdfParams, read_envelope_file, write_envelope_file};

const FAST: KdfParams = KdfParams { memory_kib: 1024, iterations: 1, lanes: 1 };
const PASSWORD_A: &[u8] = b"password A";
const PASSWORD_B: &[u8] = b"password B";

/// Creates an envelope under password A and returns its path and the data key bytes.
fn fresh_envelope(dir: &Path) -> (PathBuf, [u8; 32]) {
    let path = dir.join("keys.envelope");
    let (envelope, key) = Envelope::create(PASSWORD_A, FAST).unwrap();
    write_envelope_file(&path, &envelope).unwrap();
    (path, *key.expose_bytes())
}

/// Which password opens the envelope at `path`, after checking it holds `expected_key`.
fn opening_password(path: &Path, expected_key: &[u8; 32]) -> &'static [u8] {
    let envelope = Envelope::parse(&read_envelope_file(path).unwrap()).expect("envelope damaged");
    let by_a = envelope.unwrap(PASSWORD_A).ok();
    let by_b = envelope.unwrap(PASSWORD_B).ok();
    match (by_a, by_b) {
        (Some(key), None) => {
            assert_eq!(key.expose_bytes(), expected_key);
            PASSWORD_A
        }
        (None, Some(key)) => {
            assert_eq!(key.expose_bytes(), expected_key);
            PASSWORD_B
        }
        _ => panic!("envelope opens with neither or both passwords"),
    }
}

fn temp_file_left(path: &Path) -> bool {
    path.with_file_name(".keys.envelope.pk-tmp").exists()
}

#[test]
fn a_kill_at_each_step_of_the_replacement_leaves_old_or_new() {
    let steps = [
        ("atomic-replace/temp-written", PASSWORD_A, true),
        ("atomic-replace/temp-synced", PASSWORD_A, true),
        ("atomic-replace/renamed", PASSWORD_B, false),
        ("atomic-replace/directory-synced", PASSWORD_B, false),
    ];
    for (step, expected_password, temp_expected) in steps {
        let dir = tempfile::tempdir().unwrap();
        let (path, key) = fresh_envelope(dir.path());
        let mut child = ChildProcess::spawn(
            "child_rewrap_once",
            &[
                ("ENVELOPE", path.to_str().unwrap()),
                ("VAULT_CRYPTO_CRASH_AT", &format!("{step}:1")),
            ],
        );
        child.wait_for_line(&format!("crash-point {step}"), Duration::from_secs(60));
        child.kill();

        assert_eq!(opening_password(&path, &key), expected_password, "killed at {step}");
        assert_eq!(temp_file_left(&path), temp_expected, "killed at {step}");

        // The next password change works and cleans up the stale temporary file.
        let envelope = Envelope::parse(&read_envelope_file(&path).unwrap()).unwrap();
        let next = envelope.rewrap(expected_password, b"password C", FAST).unwrap();
        write_envelope_file(&path, &next).unwrap();
        assert!(!temp_file_left(&path));
        let reopened = Envelope::parse(&read_envelope_file(&path).unwrap()).unwrap();
        assert_eq!(reopened.unwrap(b"password C").unwrap().expose_bytes(), &key);
    }
}

#[test]
fn kills_at_random_moments_during_repeated_rewraps_leave_a_working_envelope() {
    let (mut rng, seed) = Rng::from_env_or_clock();
    println!("seed {seed}");
    let runs = 30;
    let mut finished_on = [0usize; 2];
    let mut temp_left = 0;
    for run in 0..runs {
        let dir = tempfile::tempdir().unwrap();
        let (path, key) = fresh_envelope(dir.path());
        let mut child =
            ChildProcess::spawn("child_rewrap_forever", &[("ENVELOPE", path.to_str().unwrap())]);
        child.wait_for_line("ready", Duration::from_secs(60));
        // Let a random number of rewraps complete first; each takes a few milliseconds.
        std::thread::sleep(Duration::from_micros(rng.below(150_000)));
        child.kill();
        let rewraps = child.drain_lines().iter().filter(|l| l.contains("rewrapped")).count();

        let password = opening_password(&path, &key);
        finished_on[usize::from(password == PASSWORD_B)] += 1;
        temp_left += usize::from(temp_file_left(&path));
        println!(
            "run {run}: {rewraps} rewraps completed, opens with {}",
            String::from_utf8_lossy(password)
        );
    }
    println!(
        "{runs} kills: A {} / B {}, stale temporary file after {temp_left}",
        finished_on[0], finished_on[1]
    );
}

/// Child: one password change A → B, stopped by `VAULT_CRYPTO_CRASH_AT`.
#[test]
#[ignore = "runs only as a child of a crash test"]
fn child_rewrap_once() {
    if !is_child() {
        return;
    }
    let path = PathBuf::from(std::env::var("ENVELOPE").unwrap());
    let envelope = Envelope::parse(&read_envelope_file(&path).unwrap()).unwrap();
    let next = envelope.rewrap(PASSWORD_A, PASSWORD_B, FAST).unwrap();
    write_envelope_file(&path, &next).unwrap();
    panic!("the crash point was not reached");
}

/// Child: changes the password back and forth until it is killed.
#[test]
#[ignore = "runs only as a child of a crash test"]
fn child_rewrap_forever() {
    if !is_child() {
        return;
    }
    let path = PathBuf::from(std::env::var("ENVELOPE").unwrap());
    let mut current = PASSWORD_A;
    println!("ready");
    loop {
        let next = if current == PASSWORD_A { PASSWORD_B } else { PASSWORD_A };
        let envelope = Envelope::parse(&read_envelope_file(&path).unwrap()).unwrap();
        write_envelope_file(&path, &envelope.rewrap(current, next, FAST).unwrap()).unwrap();
        current = next;
        println!("rewrapped");
    }
}
