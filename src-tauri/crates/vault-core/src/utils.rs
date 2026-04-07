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

pub fn now_rfc3339() -> String {
    Utc::now().to_rfc3339()
}

pub fn sha256_hex(bytes: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(bytes);
    hex::encode(hasher.finalize())
}

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

pub fn chrome_time_to_rfc3339(value: i64) -> String {
    let unix_micros = value.saturating_sub(CHROME_UNIX_EPOCH_OFFSET_MICROS);
    let secs = unix_micros.div_euclid(1_000_000);
    let nanos = (unix_micros.rem_euclid(1_000_000) * 1_000) as u32;
    DateTime::<Utc>::from_timestamp(secs, nanos)
        .unwrap_or_else(|| Utc.timestamp_opt(0, 0).single().expect("unix epoch"))
        .to_rfc3339()
}

pub fn iso_to_chrome_time_micros(value: &str) -> Option<i64> {
    let parsed = DateTime::parse_from_rfc3339(value).ok()?;
    let micros = parsed.timestamp_micros();
    Some(unix_micros_to_chrome_time(micros))
}

pub fn unix_micros_to_chrome_time(value: i64) -> i64 {
    value.saturating_add(CHROME_UNIX_EPOCH_OFFSET_MICROS)
}

pub fn url_domain(url: &str) -> String {
    url.split("://").nth(1).unwrap_or(url).split('/').next().unwrap_or(url).trim().to_string()
}

pub fn sqlite_value_to_json(value: ValueRef<'_>) -> Value {
    match value {
        ValueRef::Null => Value::Null,
        ValueRef::Integer(v) => Value::from(v),
        ValueRef::Real(v) => Value::from(v),
        ValueRef::Text(v) => Value::from(String::from_utf8_lossy(v).to_string()),
        ValueRef::Blob(v) => Value::from(format!("base64:{}", base64_blob(v))),
    }
}

pub fn sqlite_row_to_json(row: &rusqlite::Row<'_>) -> rusqlite::Result<Value> {
    let mut object = Map::new();
    for index in 0..row.as_ref().column_count() {
        let name = row.as_ref().column_name(index)?.to_string();
        let value = row.get_ref(index)?;
        object.insert(name, sqlite_value_to_json(value));
    }
    Ok(Value::Object(object))
}

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
