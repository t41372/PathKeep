//! Responsibilities: guard the agent tools against malformed model input.
//! Not responsible for: product implementation or external network execution.
//! Failure modes: a model-supplied cursor panics the worker on a byte slice.
use super::*;

#[test]
fn output_pages_keep_multibyte_characters_and_reject_partial_character_cursors() {
    let raw = "你好🦀".repeat(2000);
    let first: Value =
        serde_json::from_str(&super::super::report_pages::output(&raw, None).unwrap()).unwrap();
    let fragment = first["jsonFragment"].as_str().unwrap();
    assert!(raw.starts_with(fragment));
    assert!(fragment.len() < raw.len());
    let bad = serde_json::to_string(&(crate::utils::sha256_hex(raw.as_bytes()), 1_usize)).unwrap();
    assert!(super::super::report_pages::output(&raw, Some(&bad)).is_err());
}
