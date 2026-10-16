//! Responsibilities: keep persisted evidence and identity JSON independent of insertion order.
//! Not responsible for: choosing evidence fields, serializing values or hashing bytes.
use serde_json::Value;

/// rig 0.44 enables serde_json `preserve_order` across the workspace. Evidence and fingerprint
/// bytes predate that feature, so recursively sort objects before storing or hashing them.
pub fn canonicalize_json(value: &mut Value) {
    value.sort_all_objects();
}
