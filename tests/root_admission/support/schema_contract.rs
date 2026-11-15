//! Responsibilities: express the schema/snapshot/release-row assertions without git history.
//! Not responsible for: inventing a schema from samples or pretending current serde types derive it.
//! Consumers must supply the actual generated schema and a checked-in snapshot.
pub fn assert_schema_contract(
    actual: &serde_json::Value,
    snapshot_bytes: &[u8],
    snapshot_revision: u64,
    format_table: &serde_json::Value,
    sha256: impl Fn(&[u8]) -> String,
) {
    let snapshot: serde_json::Value = serde_json::from_slice(snapshot_bytes).unwrap();
    assert_eq!(
        actual, &snapshot["schema"],
        "persisted serde shape changed without reviewing the snapshot"
    );
    assert_eq!(snapshot["rootRevision"], snapshot_revision);
    assert!(
        format_table["formats"]
            .as_array()
            .unwrap()
            .iter()
            .any(|row| row["revision"] == snapshot_revision),
        "the root revision must have a format-table row"
    );
    for released in format_table["released"].as_array().unwrap() {
        if released["rootRevision"] == snapshot_revision {
            assert_eq!(
                released["schemaSha256"],
                sha256(snapshot_bytes),
                "a released revision's snapshot is immutable: {released}"
            );
        }
    }
}
