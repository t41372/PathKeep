//! Responsibilities: page oversized report text without dropping records or splitting UTF-8.
//! Not responsible for: querying intelligence or choosing a report's row cursor.
//! Failure modes: bytes disappear at a boundary, mutable reports splice different snapshots,
//! malformed offsets panic, or JSON escaping exceeds the model-output budget.
use anyhow::{Context, Result};
use serde_json::json;
const BODY_BYTES: usize = 7_000;

/// Keeps per-request view timers from invalidating otherwise identical report fragments.
pub(super) fn remove_volatile_diagnostics(value: &mut serde_json::Value) {
    if let Some(root) = value.as_object_mut() {
        root.remove("timings");
        root.remove("totalDurationMs");
        for section in root.values_mut() {
            if let Some(meta) = section.get_mut("meta").and_then(serde_json::Value::as_object_mut) {
                meta.remove("generatedAt");
            }
        }
    }
}

/// The cursor pins a byte position to the report content, so a changed report requests a restart.
pub(super) fn output(raw: &str, cursor: Option<&str>) -> Result<String> {
    if cursor.is_none() && raw.len() <= BODY_BYTES {
        return Ok(raw.to_owned());
    }
    let revision = crate::utils::sha256_hex(raw.as_bytes());
    let start = if let Some(cursor) = cursor {
        let (previous, offset): (String, usize) =
            serde_json::from_str(cursor).context("intelligence-report-cursor-invalid")?;
        if previous != revision {
            return Ok(
                json!({"restartRequired":true,"reportRevision":revision,"nextOutputCursor":null})
                    .to_string(),
            );
        }
        offset
    } else {
        0
    };
    let remaining = raw.get(start..).context("intelligence-report-cursor-invalid")?;
    let mut length = remaining.len().min(BODY_BYTES);
    loop {
        while !remaining.is_char_boundary(length) {
            length -= 1;
        }
        let end = start + length;
        let next = (end < raw.len())
            .then(|| serde_json::to_string(&(revision.as_str(), end)))
            .transpose()?;
        let page=json!({"jsonFragment":&remaining[..length],"reportRevision":revision,"nextOutputCursor":next,"totalBytes":raw.len()}).to_string();
        if page.len() <= BODY_BYTES {
            return Ok(page);
        }
        length /= 2;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn every_unicode_byte_is_reachable_and_changed_reports_request_a_restart() {
        let raw = json!({"text":"你好🦀\\\"".repeat(4000)}).to_string();
        let mut cursor = None;
        let mut joined = String::new();
        let mut first = None;
        loop {
            let page: serde_json::Value =
                serde_json::from_str(&output(&raw, cursor.as_deref()).unwrap()).unwrap();
            joined.push_str(page["jsonFragment"].as_str().unwrap());
            cursor = page["nextOutputCursor"].as_str().map(str::to_owned);
            if first.is_none() {
                first = cursor.clone();
            }
            if cursor.is_none() {
                break;
            }
        }
        assert_eq!(joined, raw);
        let changed: serde_json::Value =
            serde_json::from_str(&output("changed", first.as_deref()).unwrap()).unwrap();
        assert_eq!(changed["restartRequired"], true);
    }
}
