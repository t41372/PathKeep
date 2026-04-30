//! Small cross-cutting utility helpers.
//!
//! These helpers are intentionally boring and dependency-light: timestamps,
//! hashes, SQLite-to-JSON shaping, and URL/domain normalization that multiple
//! backend modules rely on.

use anyhow::{Context, Result};
use chrono::{DateTime, TimeZone, Utc};
use rusqlite::types::ValueRef;
use serde_json::{Map, Value};
use sha2::{Digest, Sha256};
use std::{
    fs::File,
    io::{BufReader, Read},
    path::Path,
};

const CHROME_UNIX_EPOCH_OFFSET_MICROS: i64 = 11_644_473_600_000_000;
const WINDOWS_RESERVED_FILE_NAMES: [&str; 25] = [
    "CON", "PRN", "AUX", "NUL", "CLOCK$", "CONIN$", "CONOUT$", "COM1", "COM2", "COM3", "COM4",
    "COM5", "COM6", "COM7", "COM8", "COM9", "LPT1", "LPT2", "LPT3", "LPT4", "LPT5", "LPT6", "LPT7",
    "LPT8", "LPT9",
];

/// Returns the current UTC timestamp in RFC3339 form.
pub fn now_rfc3339() -> String {
    Utc::now().to_rfc3339()
}

/// Computes a lowercase hex SHA-256 digest for arbitrary bytes.
pub fn sha256_hex(bytes: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(bytes);
    hex::encode(hasher.finalize())
}

/// Streams one file from disk and returns its lowercase hex SHA-256 digest.
pub fn file_sha256_hex(path: &Path) -> Result<String> {
    let file = File::open(path).with_context(|| format!("opening {}", path.display()))?;
    let mut reader = BufReader::new(file);
    let mut hasher = Sha256::new();
    let mut buffer = [0_u8; 8192];

    loop {
        let read = reader.read(&mut buffer)?;
        if read == 0 {
            break;
        }
        hasher.update(&buffer[..read]);
    }

    Ok(hex::encode(hasher.finalize()))
}

/// Encodes an app identifier as one reversible filesystem path segment.
///
/// Profile IDs contain product grammar such as `firefox:default-release`.
/// Keeping that exact string in SQLite and UI state is useful, but using it as
/// a directory or temp-file prefix breaks on Windows. This helper preserves
/// alphanumerics and `._-`, percent-encodes every other UTF-8 byte, and guards
/// the few Windows-reserved basenames so archive paths stay portable.
pub fn filesystem_safe_path_segment(identifier: &str) -> String {
    let mut encoded = String::new();
    for byte in identifier.as_bytes() {
        if byte.is_ascii_alphanumeric() || matches!(byte, b'.' | b'_' | b'-') {
            encoded.push(*byte as char);
        } else {
            encoded.push_str(&format!("%{byte:02X}"));
        }
    }

    if encoded.is_empty() {
        return "id-empty".to_string();
    }

    while encoded.ends_with('.') {
        encoded.truncate(encoded.len() - 1);
        encoded.push_str("%2E");
    }
    if is_windows_reserved_name(&encoded) {
        let first = encoded.as_bytes()[0];
        return format!("%{first:02X}{}", &encoded[1..]);
    }
    encoded
}

/// Decodes a path segment produced by `filesystem_safe_path_segment`.
///
/// Older macOS/Linux checkpoint directories may still contain raw profile IDs.
/// Invalid percent escapes are therefore left as literal text instead of making
/// snapshot restore fail on old artifacts.
pub fn identifier_from_filesystem_segment(segment: &str) -> String {
    let bytes = segment.as_bytes();
    let mut decoded = Vec::with_capacity(bytes.len());
    let mut index = 0;
    while index < bytes.len() {
        if bytes[index] == b'%' && index + 2 < bytes.len() {
            if let (Some(high), Some(low)) =
                (hex_nibble(bytes[index + 1]), hex_nibble(bytes[index + 2]))
            {
                decoded.push((high << 4) | low);
                index += 3;
                continue;
            }
        }
        decoded.push(bytes[index]);
        index += 1;
    }

    String::from_utf8(decoded).unwrap_or_else(|_| segment.to_string())
}

/// Converts a Chromium microsecond timestamp into RFC3339.
pub fn chrome_time_to_rfc3339(value: i64) -> String {
    let unix_micros = value.saturating_sub(CHROME_UNIX_EPOCH_OFFSET_MICROS);
    let secs = unix_micros.div_euclid(1_000_000);
    let nanos = (unix_micros.rem_euclid(1_000_000) * 1_000) as u32;
    DateTime::<Utc>::from_timestamp(secs, nanos)
        .unwrap_or_else(|| Utc.timestamp_opt(0, 0).single().expect("unix epoch"))
        .to_rfc3339()
}

/// Converts RFC3339 back into Chromium's microseconds-since-1601 format.
pub fn iso_to_chrome_time_micros(value: &str) -> Option<i64> {
    let parsed = DateTime::parse_from_rfc3339(value).ok()?;
    let micros = parsed.timestamp_micros();
    Some(unix_micros_to_chrome_time(micros))
}

/// Converts Unix microseconds into Chromium's epoch format.
pub fn unix_micros_to_chrome_time(value: i64) -> i64 {
    value.saturating_add(CHROME_UNIX_EPOCH_OFFSET_MICROS)
}

/// Extracts a normalized domain/host string from a URL-like input.
pub fn url_domain(url: &str) -> String {
    url.split("://").nth(1).unwrap_or(url).split('/').next().unwrap_or(url).trim().to_string()
}

/// Converts stored favicon bytes into a data URL when the image format is recognized.
pub fn image_data_to_data_url(bytes: &[u8]) -> Option<String> {
    let mime_type = sniff_image_mime_type(bytes)?;
    Some(format!("data:{mime_type};base64,{}", base64_blob(bytes)))
}

/// Converts one SQLite value into a JSON value for diagnostics and audit payloads.
pub fn sqlite_value_to_json(value: ValueRef<'_>) -> Value {
    match value {
        ValueRef::Null => Value::Null,
        ValueRef::Integer(v) => Value::from(v),
        ValueRef::Real(v) => Value::from(v),
        ValueRef::Text(v) => Value::from(String::from_utf8_lossy(v).to_string()),
        ValueRef::Blob(v) => Value::from(format!("base64:{}", base64_blob(v))),
    }
}

/// Converts an entire SQLite row into a JSON object keyed by column name.
pub fn sqlite_row_to_json(row: &rusqlite::Row<'_>) -> rusqlite::Result<Value> {
    let mut object = Map::new();
    for index in 0..row.as_ref().column_count() {
        let name = row.as_ref().column_name(index)?.to_string();
        let value = row.get_ref(index)?;
        object.insert(name, sqlite_value_to_json(value));
    }
    Ok(Value::Object(object))
}

/// Base64-encodes SQLite blob bytes for JSON transport.
fn base64_blob(bytes: &[u8]) -> String {
    const TABLE: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut output = String::new();
    let mut index = 0;
    while index < bytes.len() {
        let b0 = bytes[index];
        let b1 = *bytes.get(index + 1).unwrap_or(&0);
        let b2 = *bytes.get(index + 2).unwrap_or(&0);
        output.push(TABLE[(b0 >> 2) as usize] as char);
        output.push(TABLE[((b0 & 0b0000_0011) << 4 | (b1 >> 4)) as usize] as char);
        if index + 1 < bytes.len() {
            output.push(TABLE[((b1 & 0b0000_1111) << 2 | (b2 >> 6)) as usize] as char);
        } else {
            output.push('=');
        }
        if index + 2 < bytes.len() {
            output.push(TABLE[(b2 & 0b0011_1111) as usize] as char);
        } else {
            output.push('=');
        }
        index += 3;
    }
    output
}

fn is_windows_reserved_name(segment: &str) -> bool {
    let basename = segment.split('.').next().unwrap_or(segment).to_ascii_uppercase();
    WINDOWS_RESERVED_FILE_NAMES.contains(&basename.as_str())
}

fn hex_nibble(byte: u8) -> Option<u8> {
    match byte {
        b'0'..=b'9' => Some(byte - b'0'),
        b'a'..=b'f' => Some(byte - b'a' + 10),
        b'A'..=b'F' => Some(byte - b'A' + 10),
        _ => None,
    }
}

/// Identifies common favicon payload formats without needing a full image decoder.
fn sniff_image_mime_type(bytes: &[u8]) -> Option<&'static str> {
    if bytes.starts_with(&[0x89, b'P', b'N', b'G', 0x0D, 0x0A, 0x1A, 0x0A]) {
        return Some("image/png");
    }

    if bytes.starts_with(&[0xFF, 0xD8, 0xFF]) {
        return Some("image/jpeg");
    }

    if bytes.starts_with(b"GIF87a") || bytes.starts_with(b"GIF89a") {
        return Some("image/gif");
    }

    if bytes.starts_with(&[0x00, 0x00, 0x01, 0x00]) {
        return Some("image/x-icon");
    }

    if bytes.starts_with(b"RIFF") && bytes.get(8..12) == Some(b"WEBP") {
        return Some("image/webp");
    }

    let sniff_window = &bytes[..bytes.len().min(256)];
    let prefix = String::from_utf8_lossy(sniff_window);
    let prefix = prefix.trim_start_matches(|character: char| character.is_ascii_whitespace());
    if prefix.starts_with("<svg") || (prefix.starts_with("<?xml") && prefix.contains("<svg")) {
        return Some("image/svg+xml");
    }

    None
}

#[cfg(test)]
pub(crate) fn test_env_lock() -> &'static std::sync::Mutex<()> {
    static LOCK: std::sync::OnceLock<std::sync::Mutex<()>> = std::sync::OnceLock::new();
    LOCK.get_or_init(|| std::sync::Mutex::new(()))
}

#[cfg(test)]
pub(crate) fn restore_test_env_var(name: &str, value: Option<&std::ffi::OsStr>) {
    unsafe {
        if let Some(value) = value {
            std::env::set_var(name, value);
        } else {
            std::env::remove_var(name);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rusqlite::Connection;
    use tempfile::tempdir;

    #[test]
    fn sha_helpers_are_stable() {
        assert_eq!(
            sha256_hex(b"pathkeep"),
            "fb2062a6b0c6b2735d597e50e410d6510e6f081fca1f3eb7d7fff3b26f0fe64e"
        );

        let dir = tempdir().expect("tempdir");
        let file = dir.path().join("sample.txt");
        std::fs::write(&file, b"hello world").expect("write file");
        assert_eq!(
            file_sha256_hex(&file).expect("hash file"),
            "b94d27b9934d3e08a52e52d7da7dabfac484efe37a5380ee9088f7ace2efcde9"
        );
    }

    #[test]
    fn path_segment_encoding_is_windows_safe_and_reversible() {
        let encoded = filesystem_safe_path_segment("firefox:96xe8h3r.default-release");
        assert_eq!(encoded, "firefox%3A96xe8h3r.default-release");
        assert!(!encoded.chars().any(|character| matches!(
            character,
            '<' | '>' | ':' | '"' | '/' | '\\' | '|' | '?' | '*'
        )));
        assert_eq!(
            identifier_from_filesystem_segment(&encoded),
            "firefox:96xe8h3r.default-release"
        );
        assert_eq!(identifier_from_filesystem_segment("chrome:Default"), "chrome:Default");
        let reserved = filesystem_safe_path_segment("CON");
        assert_eq!(reserved, "%43ON");
        assert_eq!(identifier_from_filesystem_segment(&reserved), "CON");
        assert_eq!(
            identifier_from_filesystem_segment(&filesystem_safe_path_segment("profile.")),
            "profile."
        );
        assert_eq!(filesystem_safe_path_segment(""), "id-empty");
        assert_eq!(identifier_from_filesystem_segment("%3a%zz"), ":%zz");
    }

    #[test]
    fn chrome_time_roundtrip_is_lossless_for_valid_inputs() {
        let original = "2026-04-03T04:00:00+00:00";
        let chrome = iso_to_chrome_time_micros(original).expect("to chrome");
        let converted = chrome_time_to_rfc3339(chrome);
        assert_eq!(converted, original);

        let fractional = "2026-04-03T04:00:00.123456+00:00";
        let chrome = iso_to_chrome_time_micros(fractional).expect("to chrome fractional");
        let converted = chrome_time_to_rfc3339(chrome);
        assert_eq!(converted, fractional);
    }

    #[test]
    fn invalid_iso_returns_none_and_invalid_chrome_time_falls_back() {
        assert!(iso_to_chrome_time_micros("not-a-date").is_none());
        assert_eq!(chrome_time_to_rfc3339(i64::MIN), "1970-01-01T00:00:00+00:00");
    }

    #[test]
    fn domain_extraction_handles_urls_and_bare_hosts() {
        assert_eq!(url_domain("https://example.com/path?q=1"), "example.com");
        assert_eq!(url_domain("example.org/path"), "example.org");
    }

    #[test]
    fn image_data_to_data_url_detects_common_favicon_formats() {
        assert_eq!(
            image_data_to_data_url(&[0x89, b'P', b'N', b'G', 0x0D, 0x0A, 0x1A, 0x0A, 0x01]),
            Some("data:image/png;base64,iVBORw0KGgoB".to_string())
        );
        assert_eq!(
            image_data_to_data_url(&[0xFF, 0xD8, 0xFF, 0x00]),
            Some("data:image/jpeg;base64,/9j/AA==".to_string())
        );
        assert_eq!(
            image_data_to_data_url(b"GIF89a"),
            Some("data:image/gif;base64,R0lGODlh".to_string())
        );
        assert_eq!(
            image_data_to_data_url(&[0x00, 0x00, 0x01, 0x00]),
            Some("data:image/x-icon;base64,AAABAA==".to_string())
        );
        assert_eq!(
            image_data_to_data_url(b"RIFF\x00\x00\x00\x00WEBP"),
            Some("data:image/webp;base64,UklGRgAAAABXRUJQ".to_string())
        );
        assert_eq!(
            image_data_to_data_url(br#"<?xml version="1.0"?><svg viewBox="0 0 1 1"></svg>"#),
            Some(
                "data:image/svg+xml;base64,PD94bWwgdmVyc2lvbj0iMS4wIj8+PHN2ZyB2aWV3Qm94PSIwIDAgMSAxIj48L3N2Zz4="
                    .to_string()
            )
        );
        assert!(image_data_to_data_url(b"not-an-image").is_none());
    }

    #[test]
    fn sqlite_row_and_value_are_serialized_to_json() {
        let connection = Connection::open_in_memory().expect("db");
        connection
            .execute("CREATE TABLE sample (id INTEGER, title TEXT, payload BLOB, ratio REAL, missing TEXT)", [])
            .expect("create");
        connection
            .execute(
                "INSERT INTO sample (id, title, payload, ratio, missing) VALUES (1, 'hello', X'0102', 1.5, NULL)",
                [],
            )
            .expect("insert");

        let value =
            connection.query_row("SELECT * FROM sample", [], sqlite_row_to_json).expect("row json");

        assert_eq!(value["id"], 1);
        assert_eq!(value["title"], "hello");
        assert_eq!(value["ratio"], 1.5);
        assert_eq!(value["missing"], Value::Null);
        assert_eq!(value["payload"], "base64:AQI=");
    }

    #[test]
    fn blob_serialization_covers_base64_padding_edges() {
        assert_eq!(sqlite_value_to_json(ValueRef::Blob(&[0x01])), "base64:AQ==");
        assert_eq!(sqlite_value_to_json(ValueRef::Blob(&[0x01, 0x02, 0x03])), "base64:AQID");
    }
}
