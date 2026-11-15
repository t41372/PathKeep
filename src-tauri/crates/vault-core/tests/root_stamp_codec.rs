//! Responsibilities: specify scalar compatibility and byte-preserving stamped control round trips.
//! Not responsible for: admitting a stamped root at level zero or publishing a new stamp.
//! `round_trip` goes through the codec that reads and writes `config.json`. It decodes a stamp
//! at any level; whether a root with that stamp is admitted is `parse_control`'s decision.
use vault_core::{AppConfig, ArchiveMode};
#[path = "../../../../tests/root_admission/support/decision_contract.rs"]
mod decision_contract;
#[allow(dead_code)]
#[path = "../../../../tests/root_admission/support/schema_contract.rs"]
mod schema_contract;
const STAMPED: &str =
    include_str!("../../../../tests/root_admission/fixtures/compatible-higher.json");
const RAW_COMPATIBILITY: &str = "{ \"minWriter\" : 1,\n    \"futureNote\": {\"spelling\":\"\\u0061\", \"number\":1e2}, \"revision\": 2,  \"minReader\": 1\n  }";

fn round_trip(bytes: &str, change: impl FnOnce(&mut AppConfig)) -> anyhow::Result<String> {
    let mut document = vault_core::config::decode_document(bytes.as_bytes())?;
    change(&mut document.config);
    vault_core::config::encode_config(&document.config, document.compatibility.as_ref())
}

#[test]
fn the_version_rule_matches_every_ordering_of_level_and_floors() {
    decision_contract::assert_decisions(|level, _revision, min_reader, min_writer| {
        use vault_core::admission::VersionDecision::*;
        match vault_core::admission::decide(level, min_reader, min_writer) {
            Admitted => "admitted",
            ReaderTooOld => "archive-reader-too-old",
            WriterTooOld => "archive-writer-too-old",
        }
    });
}

#[test]
fn a_legacy_control_is_written_back_byte_for_byte_as_the_released_reader_expects() {
    // What v0.5.0 reads is `serde_json::to_string_pretty` of the settings. A save by this build
    // of a root without a stamp must produce exactly that.
    let legacy = include_str!("../../../../tests/root_admission/fixtures/legacy.json");
    let document = vault_core::config::decode_document(legacy.as_bytes()).unwrap();
    assert!(document.compatibility.is_none());
    assert_eq!(
        vault_core::config::encode_config(&document.config, None).unwrap(),
        serde_json::to_string_pretty(&document.config).unwrap()
    );
}

#[test]
fn stamped_codec_preserves_compatibility_bytes_when_a_known_setting_changes() {
    assert!(STAMPED.contains(RAW_COMPATIBILITY), "fixture must contain the exact unusual bytes");
    let encoded = round_trip(STAMPED, |config| config.git_enabled = true)
        .expect("the codec reads stamped control independently of root admission");
    assert!(encoded.contains(RAW_COMPATIBILITY), "{encoded}");
    let value: serde_json::Value = serde_json::from_str(&encoded).unwrap();
    assert_eq!(value["gitEnabled"], true);
    assert_eq!(value["archiveMode"]["compatibility"]["futureNote"]["spelling"], "a");
}

#[test]
fn stamped_codec_preserves_compatibility_bytes_when_mode_changes() {
    let encoded = round_trip(STAMPED, |config| config.archive_mode = ArchiveMode::Encrypted)
        .expect("the disk codec accepts the stamped object");
    assert!(encoded.contains(RAW_COMPATIBILITY), "{encoded}");
    let value: serde_json::Value = serde_json::from_str(&encoded).unwrap();
    assert_eq!(value["archiveMode"]["formatVersion"], 2);
    assert_eq!(value["archiveMode"]["mode"], "Encrypted");
}

#[test]
fn legacy_scalar_codec_never_publishes_a_stamp_during_an_ordinary_save() {
    for literal in [r#"{"archiveMode":"Plaintext"}"#, r#"{"archiveMode":"Encrypted"}"#] {
        let encoded = round_trip(literal, |config| config.git_enabled = false).unwrap();
        let before: serde_json::Value = serde_json::from_str(literal).unwrap();
        let after: serde_json::Value = serde_json::from_str(&encoded).unwrap();
        assert_eq!(before["archiveMode"], after["archiveMode"]);
        assert!(after["archiveMode"].is_string());
    }
}
