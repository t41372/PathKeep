//! Responsibilities: verify portable artifact streams reuse authenticated encryption.
//! Not responsible for: ZIP layout, database copying or filesystem installation.
//! Failure modes: plaintext reaches disk, another purpose/key opens the file,
//! renamed or truncated ciphertext is accepted, cancellation publishes complete data.
use std::io::Cursor;
use vault_crypto::{DataKey, decrypt_artifact, encrypt_artifact};
use zeroize::Zeroizing;

#[test]
fn artifacts_round_trip_without_plaintext_and_authenticate_their_names() {
    let root = tempfile::tempdir().unwrap();
    let key = DataKey::from_bytes(Zeroizing::new([17; 32]));
    let data = b"PRIVATE-PORTABLE-HISTORY".repeat(9000);
    encrypt_artifact(root.path(), "history.pkf", &key, &mut Cursor::new(&data), || false).unwrap();
    let cipher = std::fs::read(root.path().join("history.pkf")).unwrap();
    assert!(!cipher.windows(24).any(|b| b == b"PRIVATE-PORTABLE-HISTORY"));
    let mut plain = Vec::new();
    decrypt_artifact(root.path(), "history.pkf", &key, &mut plain, || false).unwrap();
    assert_eq!(plain, data);
    std::fs::copy(root.path().join("history.pkf"), root.path().join("renamed.pkf")).unwrap();
    assert!(decrypt_artifact(root.path(), "renamed.pkf", &key, &mut Vec::new(), || false).is_err());
    let other = DataKey::from_bytes(Zeroizing::new([18; 32]));
    assert!(
        decrypt_artifact(root.path(), "history.pkf", &other, &mut Vec::new(), || false).is_err()
    );
    std::fs::write(root.path().join("history.pkf"), &cipher[..cipher.len() - 1]).unwrap();
    assert!(decrypt_artifact(root.path(), "history.pkf", &key, &mut Vec::new(), || false).is_err());
}

#[test]
fn cancelled_artifact_has_no_readable_complete_payload() {
    let root = tempfile::tempdir().unwrap();
    let key = DataKey::from_bytes(Zeroizing::new([17; 32]));
    assert!(
        encrypt_artifact(root.path(), "partial.pkf", &key, &mut Cursor::new(b"private"), || true)
            .is_err()
    );
    assert!(decrypt_artifact(root.path(), "partial.pkf", &key, &mut Vec::new(), || false).is_err());
}
