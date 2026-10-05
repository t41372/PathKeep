//! The password envelope and the sub-key hierarchy, against the failure modes listed in
//! `src/envelope.rs` and `src/keys.rs`.

use vault_crypto::{
    DataKey, ENVELOPE_LEN, Envelope, EnvelopeError, KdfParams, KeyPurpose, read_envelope_file,
    write_envelope_file,
};
use zeroize::Zeroizing;

/// Small parameters so the tests run quickly. The format stores them, so this is a real
/// configuration, just a weak one.
const FAST: KdfParams = KdfParams { memory_kib: 1024, iterations: 1, lanes: 1 };

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

fn unhex(text: &str) -> Vec<u8> {
    (0..text.len()).step_by(2).map(|i| u8::from_str_radix(&text[i..i + 2], 16).unwrap()).collect()
}

/// Built outside Rust with Python's `cryptography` package (Argon2id m=1024 KiB, t=2, p=2,
/// salt 10..1f, nonce a0..ab, AES-256-GCM over the data key 40..5f with bytes 0..46 as
/// associated data). It pins the byte layout and the algorithms to an independent
/// implementation, so a change to either fails here rather than locking users out.
const INDEPENDENT_ENVELOPE: &str = "504b454e5601000400000200000002000000101112131415161718191a1b1c1d1e1fa0a1a2a3a4a5a6a7a8a9aaabd834d25fd76ecf44e6beb02992aaad6099b2c2dfa8c2b951f8b89d90b6cca4ce70bc1c956685e5347e0349a0331c1901";

#[test]
fn opens_an_envelope_built_by_an_independent_implementation() {
    let bytes = unhex(INDEPENDENT_ENVELOPE);
    let envelope = Envelope::parse(&bytes).unwrap();
    assert_eq!(envelope.params(), KdfParams { memory_kib: 1024, iterations: 2, lanes: 2 });
    let key = envelope.unwrap(b"correct horse battery staple").unwrap();
    assert_eq!(key.expose_bytes().to_vec(), (0x40u8..0x60).collect::<Vec<_>>());
    assert_eq!(envelope.to_bytes().to_vec(), bytes);
}

#[test]
fn the_right_password_returns_the_same_data_key() {
    let (envelope, key) = Envelope::create(b"hunter2 but longer", FAST).unwrap();
    let bytes = envelope.to_bytes();
    assert_eq!(bytes.len(), ENVELOPE_LEN);
    let reopened = Envelope::parse(&bytes).unwrap().unwrap(b"hunter2 but longer").unwrap();
    assert_eq!(reopened.expose_bytes(), key.expose_bytes());
}

#[test]
fn a_wrong_password_returns_no_key() {
    let (envelope, _) = Envelope::create(b"the password", FAST).unwrap();
    assert_eq!(envelope.unwrap(b"the passworD").unwrap_err(), EnvelopeError::WrongPassword);
    assert_eq!(envelope.unwrap(b"").unwrap_err(), EnvelopeError::WrongPassword);
}

/// Every bit of the envelope is either checked by parsing or authenticated by AES-GCM: no
/// single-bit change yields a key, and none makes the KDF run with parameters beyond the
/// limits (which would hang or exhaust memory).
#[test]
fn every_flipped_bit_is_rejected() {
    let (envelope, _) = Envelope::create(b"pw", FAST).unwrap();
    let original = envelope.to_bytes();
    for byte in 0..ENVELOPE_LEN {
        for bit in 0..8 {
            let mut bytes = original;
            bytes[byte] ^= 1 << bit;
            let result = Envelope::parse(&bytes).and_then(|e| {
                // A flipped parameter bit that stays within the limits still runs the KDF;
                // keep that bounded so the test stays fast.
                assert!(e.params().memory_kib <= 2 * 1024 * 1024 && e.params().iterations <= 64);
                if e.params().memory_kib > 64 * 1024 || e.params().iterations > 8 {
                    return Err(EnvelopeError::WrongPassword);
                }
                e.unwrap(b"pw")
            });
            assert!(result.is_err(), "flipping bit {bit} of byte {byte} still unwrapped");
        }
    }
}

#[test]
fn oversized_parameters_are_rejected_before_the_kdf_runs() {
    let (envelope, _) = Envelope::create(b"pw", FAST).unwrap();
    let base = envelope.to_bytes();
    for (offset, value) in
        [(6, u32::MAX), (6, 4 * 1024 * 1024), (10, 65), (10, 0), (14, 0), (14, 17)]
    {
        let mut bytes = base;
        bytes[offset..offset + 4].copy_from_slice(&u32::to_le_bytes(value));
        assert_eq!(
            Envelope::parse(&bytes).unwrap_err(),
            EnvelopeError::InvalidParameters,
            "offset {offset} value {value}"
        );
    }
}

#[test]
fn truncated_padded_and_foreign_bytes_are_not_envelopes() {
    let (envelope, _) = Envelope::create(b"pw", FAST).unwrap();
    let bytes = envelope.to_bytes();
    assert_eq!(
        Envelope::parse(&bytes[..ENVELOPE_LEN - 1]).unwrap_err(),
        EnvelopeError::NotAnEnvelope
    );
    let mut padded = bytes.to_vec();
    padded.push(0);
    assert_eq!(Envelope::parse(&padded).unwrap_err(), EnvelopeError::NotAnEnvelope);
    assert_eq!(Envelope::parse(b"").unwrap_err(), EnvelopeError::NotAnEnvelope);
    assert_eq!(Envelope::parse(b"SQLite format 3\0").unwrap_err(), EnvelopeError::NotAnEnvelope);
    let mut future = bytes;
    future[5] = 2;
    assert_eq!(Envelope::parse(&future).unwrap_err(), EnvelopeError::UnsupportedVersion(2));
}

#[test]
fn rewrap_keeps_the_data_key_and_retires_the_old_password() {
    let (envelope, key) = Envelope::create(b"old password", FAST).unwrap();
    let stronger = KdfParams { memory_kib: 2048, iterations: 2, lanes: 2 };
    let rewrapped = envelope.rewrap(b"old password", b"new password", stronger).unwrap();
    assert_eq!(rewrapped.params(), stronger);
    assert_eq!(rewrapped.unwrap(b"new password").unwrap().expose_bytes(), key.expose_bytes());
    assert_eq!(rewrapped.unwrap(b"old password").unwrap_err(), EnvelopeError::WrongPassword);

    // Fresh salt and nonce: no byte range of the old envelope survives except the header
    // fields that describe the format.
    let (old, new) = (envelope.to_bytes(), rewrapped.to_bytes());
    assert_ne!(old[18..34], new[18..34], "salt reused");
    assert_ne!(old[34..46], new[34..46], "nonce reused");
    assert_ne!(old[46..], new[46..]);

    // Rewrapping needs the current password.
    assert_eq!(
        envelope.rewrap(b"guess", b"attacker", FAST).unwrap_err(),
        EnvelopeError::WrongPassword
    );
}

#[test]
fn the_same_password_twice_gives_different_envelopes() {
    let (a, _) = Envelope::create(b"same", FAST).unwrap();
    let (b, _) = Envelope::create(b"same", FAST).unwrap();
    assert_ne!(a.to_bytes()[18..], b.to_bytes()[18..]);
}

#[test]
fn envelope_files_round_trip_and_oversized_files_are_refused() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("keys.envelope");
    let (envelope, key) = Envelope::create(b"pw", FAST).unwrap();
    write_envelope_file(&path, &envelope).unwrap();
    let reread = Envelope::parse(&read_envelope_file(&path).unwrap()).unwrap();
    assert_eq!(reread.unwrap(b"pw").unwrap().expose_bytes(), key.expose_bytes());
    // Only the envelope is left: the temporary file was renamed into place.
    let names: Vec<_> = std::fs::read_dir(dir.path())
        .unwrap()
        .map(|e| e.unwrap().file_name().into_string().unwrap())
        .collect();
    assert_eq!(names, ["keys.envelope"]);

    std::fs::write(&path, vec![0u8; 10 * 1024 * 1024]).unwrap();
    let bytes = read_envelope_file(&path).unwrap();
    assert_eq!(bytes.len(), ENVELOPE_LEN + 1);
    assert_eq!(Envelope::parse(&bytes).unwrap_err(), EnvelopeError::NotAnEnvelope);
}

/// Values computed with Python's `hmac` and `hashlib` (RFC 5869 HKDF-SHA256, no salt, the
/// purpose label as info) for the data key 00 01 .. 1f. If a label or the derivation changes,
/// existing databases stop opening; this test is what notices.
#[test]
fn sub_keys_match_independently_computed_values() {
    let key = DataKey::from_bytes(Zeroizing::new(std::array::from_fn(|i| i as u8)));
    let expected = [
        (
            KeyPurpose::ArchiveDatabase,
            "2bf4ac258d152f13ea3f07caf322046e18ab4f3b1be86c48ebe7553b32c17c10",
        ),
        (
            KeyPurpose::SourceEvidenceDatabase,
            "038b5d2648cbb9c4a6121524ffcda10e1692cbd30d1f9efe460952fc013c5d8b",
        ),
        (
            KeyPurpose::IntelligenceDatabase,
            "8133149c05cf0fbbb511a5711780db02b29307f856bd4d817a15c881bc07f2ea",
        ),
        (
            KeyPurpose::AgentDatabase,
            "5b00b3236a65b16b9a6956fee3cf56a2a6c78f6b4724d0101e91e4dfdb669073",
        ),
        (
            KeyPurpose::ScratchDatabase,
            "dc2576a70858388f41c6d51de676d35a4097b2a637784b78f6e77935795e246e",
        ),
        (
            KeyPurpose::SearchIndex,
            "45142f53507f0cd6d8742e4004949fe8bf08ccb89eb7f6334a2945d5e88298db",
        ),
        (KeyPurpose::TempFiles, "c2b8791ab7213233fe1a1b7d14deeffb50df4be999f91d5f90b5558bba4a2d67"),
    ];
    assert_eq!(expected.len(), KeyPurpose::ALL.len());
    for (purpose, hex_key) in expected {
        let sub = key.derive(purpose);
        assert_eq!(sub.purpose(), purpose);
        assert_eq!(hex(sub.expose_bytes()), hex_key, "{purpose:?}");
        assert_eq!(sub.to_hex().as_str(), hex_key);
    }
}

#[test]
fn every_purpose_gets_different_key_material() {
    let (_, key) = Envelope::create(b"pw", FAST).unwrap();
    let mut seen: Vec<[u8; 32]> = vec![*key.expose_bytes()];
    for purpose in KeyPurpose::ALL {
        let sub = *key.derive(purpose).expose_bytes();
        assert!(!seen.contains(&sub), "{purpose:?} repeats key material");
        seen.push(sub);
    }
}

#[test]
fn debug_output_never_contains_key_bytes() {
    let (envelope, key) = Envelope::create(b"pw", FAST).unwrap();
    let key_hex = hex(key.expose_bytes());
    let printed = [
        format!("{key:?}"),
        format!("{key:#?}"),
        format!("{envelope:?}"),
        format!("{:?}", key.derive(KeyPurpose::SearchIndex)),
    ];
    for text in &printed {
        assert!(!text.contains(&key_hex[..8]), "{text}");
    }
    assert_eq!(printed[0], "DataKey(..)");
    assert_eq!(printed[3], "SubKey(SearchIndex, ..)");
    for purpose in KeyPurpose::ALL {
        let sub = key.derive(purpose);
        let sub_hex = sub.to_hex();
        assert!(!format!("{sub:?}").contains(&sub_hex[..8]));
    }
}

/// The production parameters, timed on this machine. Run with
/// `cargo test --release -p vault-crypto --test envelope -- --ignored --nocapture kdf_timing`.
#[test]
#[ignore = "timing measurement; run explicitly in release"]
fn kdf_timing() {
    let candidates = [
        KdfParams { memory_kib: 64 * 1024, iterations: 3, lanes: 4 },
        KdfParams::RECOMMENDED,
        KdfParams { memory_kib: 256 * 1024, iterations: 4, lanes: 4 },
        KdfParams { memory_kib: 512 * 1024, iterations: 2, lanes: 4 },
        KdfParams { memory_kib: 512 * 1024, iterations: 3, lanes: 4 },
    ];
    let load =
        String::from_utf8(std::process::Command::new("uptime").output().unwrap().stdout).unwrap();
    println!("load: {}", load.trim());
    for params in candidates {
        let mut times = Vec::new();
        for _ in 0..5 {
            let (envelope, _) = Envelope::create(b"measure", params).unwrap();
            let started = std::time::Instant::now();
            envelope.unwrap(b"measure").unwrap();
            times.push(started.elapsed().as_secs_f64() * 1000.0);
        }
        times.sort_by(f64::total_cmp);
        println!(
            "m={} MiB t={} p={}: unlock median {:.0} ms (min {:.0}, max {:.0})",
            params.memory_kib / 1024,
            params.iterations,
            params.lanes,
            times[2],
            times[0],
            times[4]
        );
    }
}
