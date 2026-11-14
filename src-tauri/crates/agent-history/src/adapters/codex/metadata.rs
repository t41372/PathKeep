//! Responsibilities: interpret Codex surface attribution without inferring it from installation.
//! Not responsible for: thread identity, source capture or native session execution.
use serde_json::Value;

pub(super) fn origin(payload: &Value) -> &'static str {
    if payload["originator"]
        .as_str()
        .is_some_and(|origin| origin.eq_ignore_ascii_case("Codex Desktop"))
    {
        "desktop"
    } else if matches!(payload["source"].as_str(), Some("cli" | "exec")) {
        "cli"
    } else {
        "unknown"
    }
}
