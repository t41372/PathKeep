//! Responsibilities: pin eight label byte strings and independent HKDF known answers.
//! Not responsible for: role enumeration or proving a released v0.5 archive opens.
//! Vectors use input bytes 00..1f, no salt; computed independently with Python hmac/SHA256.

use vault_crypto::{DataKey, KeyPurpose};

#[test]
fn historical_labels_derive_the_same_eight_keys() {
    let input = [
        0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16, 17, 18, 19, 20, 21, 22, 23, 24,
        25, 26, 27, 28, 29, 30, 31,
    ];
    let key = DataKey::from_bytes(input.into());
    for (purpose, label, hex) in [
        (
            KeyPurpose::ArchiveDatabase,
            b"pathkeep/v1/archive-database".as_slice(),
            "2bf4ac258d152f13ea3f07caf322046e18ab4f3b1be86c48ebe7553b32c17c10",
        ),
        (
            KeyPurpose::SourceEvidenceDatabase,
            b"pathkeep/v1/source-evidence-database",
            "038b5d2648cbb9c4a6121524ffcda10e1692cbd30d1f9efe460952fc013c5d8b",
        ),
        (
            KeyPurpose::IntelligenceDatabase,
            b"pathkeep/v1/intelligence-database",
            "8133149c05cf0fbbb511a5711780db02b29307f856bd4d817a15c881bc07f2ea",
        ),
        (
            KeyPurpose::AgentDatabase,
            b"pathkeep/v1/agent-database",
            "5b00b3236a65b16b9a6956fee3cf56a2a6c78f6b4724d0101e91e4dfdb669073",
        ),
        (
            KeyPurpose::ScratchDatabase,
            b"pathkeep/v1/scratch-database",
            "dc2576a70858388f41c6d51de676d35a4097b2a637784b78f6e77935795e246e",
        ),
        (
            KeyPurpose::SearchIndex,
            b"pathkeep/v1/search-index",
            "45142f53507f0cd6d8742e4004949fe8bf08ccb89eb7f6334a2945d5e88298db",
        ),
        (
            KeyPurpose::TempFiles,
            b"pathkeep/v1/temp-files",
            "c2b8791ab7213233fe1a1b7d14deeffb50df4be999f91d5f90b5558bba4a2d67",
        ),
        (
            KeyPurpose::BundleArtifacts,
            b"pathkeep/v1/bundle-artifacts",
            "6723c9e136480680202b7c00c6844160cf6cd362e73f40c63bb6c987ed130d7a",
        ),
    ] {
        let mut expected_from_literal_label = [0; 32];
        hkdf::Hkdf::<sha2::Sha256>::new(None, &input)
            .expand(label, &mut expected_from_literal_label)
            .unwrap();
        let derived = key.derive(purpose);
        assert_eq!(derived.to_hex().as_str(), hex, "{purpose:?}");
        assert_eq!(derived.expose_bytes(), &expected_from_literal_label, "{purpose:?} label bytes");
    }
}
