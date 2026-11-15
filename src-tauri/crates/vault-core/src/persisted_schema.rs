//! Responsibilities: describe the shape of every JSON control record this build writes into a
//! data folder, generated from the types that are actually serialized.
//! Not responsible for: database schemas (each database's migration table), binary formats
//! (their own golden bytes), or deciding what a shape change means for older builds.
//!
//! How this could fail, and what guards it:
//! - A field is added, renamed or retyped in a persisted record and nobody asks what a released
//!   build does with it. `tests/persisted_schema.rs` compares this document to the snapshot in
//!   the repository, so the change has to appear as an edit to that file in the same commit.
//! - A record is persisted and never listed here, so its shape can change unseen. The list below
//!   is the only place to add one; `docs/architecture/archive-compatibility.md` names the files
//!   that are not covered yet.

use schemars::{JsonSchema, SchemaGenerator};
use serde_json::{Map, Value};

/// The archive format revision these shapes belong to. During a format change it runs one ahead
/// of `admission::PRODUCT_ROOT_LEVEL`, until the build creates everything that revision requires;
/// otherwise the two are equal, as they are now.
pub const REVISION: u64 = 1;

/// One record: the file it is stored in, under the data folder or inside a bundle or snapshot.
fn record<T: JsonSchema>(records: &mut Map<String, Value>, stored_in: &str) {
    // A generator of its own, so each record carries the full definition of every type it
    // contains and a change inside a shared type shows up under each record that holds it.
    let settings = schemars::generate::SchemaSettings::default().with_transform(strip_prose);
    let schema = SchemaGenerator::new(settings).into_root_schema_for::<T>();
    let mut schema = serde_json::to_value(schema).expect("schema");
    shortest_floats(&mut schema);
    let previous = records.insert(stored_in.to_owned(), schema);
    assert!(previous.is_none(), "{stored_in} is listed twice");
}

/// Doc comments become `description` in a schema. Rewording one is not a format change.
fn strip_prose(schema: &mut schemars::Schema) {
    schema.remove("description");
    schemars::transform::transform_subschemas(&mut strip_prose, schema);
}

/// An `f32` default such as 0.15 is written as 0.15 or as 0.15000000596046448 depending on
/// which serde_json features the build happens to enable. The snapshot must not depend on that,
/// so a number that is exactly an `f32` is written the way the `f32` prints.
fn shortest_floats(value: &mut Value) {
    match value {
        Value::Number(number) if number.is_f64() => {
            let wide = number.as_f64().expect("a float");
            let narrow = wide as f32;
            if f64::from(narrow) == wide
                && let Some(short) =
                    narrow.to_string().parse::<f64>().ok().and_then(serde_json::Number::from_f64)
            {
                *number = short;
            }
        }
        Value::Array(items) => items.iter_mut().for_each(shortest_floats),
        Value::Object(fields) => fields.values_mut().for_each(shortest_floats),
        _ => {}
    }
}

/// The shape of everything listed, keyed by where it is stored.
pub fn document() -> Value {
    let mut records = Map::new();
    // The settings. `archiveMode` is described as the settings hold it; on disk it is either
    // that bare value or the stamped object `admission` parses.
    record::<crate::AppConfig>(&mut records, "config.json");
    record::<crate::maintenance::install::Journal>(&mut records, "archive/.pk-install-v2.json");
    // The same file, while an archive is brought up to this build's layout.
    record::<crate::maintenance::creation::CreationRecord>(
        &mut records,
        "archive/.pk-install-v2.json (stores being created)",
    );
    record::<crate::archive::wipe::WipeIntent>(&mut records, ".pk-wipe-v2.json");
    record::<crate::app_lock::AppLockStateRecord>(&mut records, "app-lock-state.json");
    record::<crate::app_lock::AppLockSecretRecord>(&mut records, "app-lock-passcode.json");
    record::<Vec<crate::maintenance::snapshot::FileProof>>(
        &mut records,
        "<snapshot>/snapshot.json",
    );
    record::<crate::migration::ExportManifest>(
        &mut records,
        "<bundle>/pathkeep-export-manifest.json",
    );
    record::<crate::ScheduledBackupAttempt>(&mut records, "schedule/attempts/attempt-*.json");
    Value::Object(records)
}
