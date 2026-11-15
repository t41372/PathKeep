//! Responsibilities: fail when the shape of a persisted record changes without the snapshot in
//! the repository changing with it, and when a released revision's snapshot changes at all.
//! Not responsible for: database schemas, binary formats, or the meaning of a field (a changed
//! default or unit keeps its shape; that still needs a decision by a person).
//!
//! When this fails after an intended change:
//! 1. Decide what a released PathKeep does with an archive that has the new shape, and write it
//!    in `persisted_schema/formats.json` under the revision the change belongs to. A change to a
//!    revision that is listed under `released` needs a new revision.
//! 2. Rewrite the snapshot and commit it with the change:
//!    `PATHKEEP_WRITE_SCHEMA_SNAPSHOT=1 cargo test -p vault-core --test persisted_schema`
use std::{fs, path::PathBuf};
#[path = "../../../../tests/root_admission/support/schema_contract.rs"]
mod schema_contract;

fn folder() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/persisted_schema")
}

#[test]
fn persisted_shapes_match_the_snapshot_of_their_revision() {
    let revision = vault_core::persisted_schema::REVISION;
    let path = folder().join(format!("revision-{revision}.json"));
    let actual = vault_core::persisted_schema::document();
    if std::env::var_os("PATHKEEP_WRITE_SCHEMA_SNAPSHOT").is_some() {
        let snapshot = serde_json::json!({ "rootRevision": revision, "schema": actual });
        fs::write(&path, serde_json::to_string_pretty(&snapshot).unwrap() + "\n").unwrap();
    }
    let snapshot = fs::read(&path)
        .unwrap_or_else(|error| panic!("{}: {error}; see the top of this file", path.display()));
    let formats: serde_json::Value =
        serde_json::from_slice(&fs::read(folder().join("formats.json")).unwrap()).unwrap();
    schema_contract::assert_schema_contract(
        &actual,
        &snapshot,
        revision,
        &formats,
        vault_core::utils::sha256_hex,
    );
}

#[test]
fn the_snapshot_revision_is_the_build_level_or_the_one_it_is_heading_to() {
    let level = vault_core::admission::PRODUCT_ROOT_LEVEL;
    let revision = vault_core::persisted_schema::REVISION;
    assert!(
        revision == level || revision == level + 1,
        "the build is level {level}, the snapshot is for revision {revision}"
    );
}

#[test]
fn every_released_revision_still_has_its_snapshot() {
    let formats: serde_json::Value =
        serde_json::from_slice(&fs::read(folder().join("formats.json")).unwrap()).unwrap();
    for released in formats["released"].as_array().unwrap() {
        let revision = released["rootRevision"].as_u64().unwrap();
        let bytes =
            fs::read(folder().join(format!("revision-{revision}.json"))).unwrap_or_else(|error| {
                panic!("the snapshot of released revision {revision}: {error}")
            });
        assert_eq!(
            released["schemaSha256"],
            vault_core::utils::sha256_hex(&bytes),
            "a released revision's snapshot was edited: {released}"
        );
    }
}
