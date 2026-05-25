//! Google Takeout `BrowserHistory.json` / `.jsonl` payload generator.
//!
//! ## Responsibilities
//! - Emit Takeout-format JSON or JSONL files containing browser-history
//!   records in the shape `browser_history_parser::takeout` recognizes.
//! - Stay faithful to the field names Google actually ships (`time_usec`,
//!   `page_transition`, `client_id`, `favicon_url`) so the parser exercises
//!   its real classifier and record-extraction paths.
//! - Make the time-unit contract testable: the writer takes Unix
//!   milliseconds and converts to the unit the parser currently assumes
//!   (microseconds-since-Unix-epoch). The audit's open question B6 about
//!   whether Google really ships Chrome epoch or Unix epoch can be pinned
//!   by writing fixtures in both unit interpretations and observing which
//!   one yields the expected Unix-ms output through the parser.
//!
//! ## Not responsible for
//! - Other Takeout payloads (TypedURL, Sessions, MyActivity HTML/JSON);
//!   those are out of scope until scenarios call for them.
//! - Zip packaging — the parser supports zipped Takeout sources but the
//!   first fixture slice writes plain files only. A `write_zip` helper
//!   will be added when a scenario needs it.

use std::fs::File;
use std::io::{BufWriter, Write};
use std::path::Path;

/// One Takeout `Browser History` record.
#[derive(Debug, Clone)]
pub struct TakeoutBrowserRecord {
    /// The page URL. Serialized as the `url` field.
    pub url: String,
    /// The page title. Serialized as the `title` field; omitted when `None`.
    pub title: Option<String>,
    /// Visit time in Unix milliseconds; serialized as `time_usec` in microseconds.
    pub visit_time_unix_ms: i64,
    /// Chrome transition tag, e.g. `LINK`, `TYPED`. Serialized as `page_transition`.
    pub page_transition: Option<String>,
    /// Stable client id; serialized as `client_id`. Captured as
    /// context evidence by the parser.
    pub client_id: Option<String>,
    /// Optional favicon URL; serialized as `favicon_url`. Captured as
    /// context evidence by the parser.
    pub favicon_url: Option<String>,
}

/// Which on-disk layout to emit for the Takeout payload.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TakeoutPayloadFormat {
    /// Standard Google Takeout layout: `{ "Browser History": [...] }`.
    StandardBrowserHistoryJson,
    /// Older / alternate Takeout layout using the `BrowserHistory` (no space) key.
    AlternateBrowserHistoryJson,
    /// JSONL: one JSON record per line, no wrapping object.
    JsonLines,
}

/// Builder for one Takeout `BrowserHistory.*` fixture.
#[derive(Debug)]
pub struct TakeoutBrowserHistoryFixture {
    format: TakeoutPayloadFormat,
    records: Vec<TakeoutBrowserRecord>,
}

impl TakeoutBrowserHistoryFixture {
    /// Creates an empty builder using the standard `Browser History` key.
    pub fn new() -> Self {
        Self { format: TakeoutPayloadFormat::StandardBrowserHistoryJson, records: Vec::new() }
    }

    /// Switches the writer to a different payload format.
    pub fn with_format(mut self, format: TakeoutPayloadFormat) -> Self {
        self.format = format;
        self
    }

    /// Adds one record to the payload.
    pub fn add_record(mut self, record: TakeoutBrowserRecord) -> Self {
        self.records.push(record);
        self
    }

    /// Materializes the fixture at `path`. The conventional file name is
    /// `BrowserHistory.json` (or `.jsonl`) inside a `Chrome` subdirectory,
    /// since the Takeout source classifier looks at path segments — but the
    /// path is the caller's responsibility.
    pub fn write(&self, path: &Path) -> std::io::Result<()> {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let file = File::create(path)?;
        let mut writer = BufWriter::new(file);

        match self.format {
            TakeoutPayloadFormat::StandardBrowserHistoryJson => {
                self.write_wrapped_json(&mut writer, "Browser History")?;
            }
            TakeoutPayloadFormat::AlternateBrowserHistoryJson => {
                self.write_wrapped_json(&mut writer, "BrowserHistory")?;
            }
            TakeoutPayloadFormat::JsonLines => {
                for record in &self.records {
                    writer.write_all(serialize_record(record).as_bytes())?;
                    writer.write_all(b"\n")?;
                }
            }
        }

        writer.flush()?;
        Ok(())
    }

    fn write_wrapped_json<W: Write>(&self, writer: &mut W, key: &str) -> std::io::Result<()> {
        writer.write_all(b"{\n  \"")?;
        writer.write_all(key.as_bytes())?;
        writer.write_all(b"\": [")?;
        for (index, record) in self.records.iter().enumerate() {
            if index > 0 {
                writer.write_all(b",")?;
            }
            writer.write_all(b"\n    ")?;
            writer.write_all(serialize_record(record).as_bytes())?;
        }
        if !self.records.is_empty() {
            writer.write_all(b"\n  ")?;
        }
        writer.write_all(b"]\n}\n")?;
        Ok(())
    }
}

impl Default for TakeoutBrowserHistoryFixture {
    fn default() -> Self {
        Self::new()
    }
}

fn serialize_record(record: &TakeoutBrowserRecord) -> String {
    let mut fields: Vec<String> = Vec::with_capacity(6);
    if let Some(transition) = &record.page_transition {
        fields.push(format!("\"page_transition\": {}", json_string(transition)));
    }
    if let Some(title) = &record.title {
        fields.push(format!("\"title\": {}", json_string(title)));
    }
    fields.push(format!("\"url\": {}", json_string(&record.url)));
    fields.push(format!("\"time_usec\": {}", record.visit_time_unix_ms.saturating_mul(1_000)));
    if let Some(client_id) = &record.client_id {
        fields.push(format!("\"client_id\": {}", json_string(client_id)));
    }
    if let Some(favicon) = &record.favicon_url {
        fields.push(format!("\"favicon_url\": {}", json_string(favicon)));
    }
    format!("{{{}}}", fields.join(", "))
}

/// Minimal JSON string encoder. Handles the escape sequences the parser will
/// see in synthetic fixtures (quotes, backslashes, control chars) without
/// pulling in a full JSON serializer dependency.
fn json_string(value: &str) -> String {
    let mut buffer = String::with_capacity(value.len() + 2);
    buffer.push('"');
    for ch in value.chars() {
        match ch {
            '"' => buffer.push_str("\\\""),
            '\\' => buffer.push_str("\\\\"),
            '\n' => buffer.push_str("\\n"),
            '\r' => buffer.push_str("\\r"),
            '\t' => buffer.push_str("\\t"),
            ch if (ch as u32) < 0x20 => {
                buffer.push_str(&format!("\\u{:04x}", ch as u32));
            }
            ch => buffer.push(ch),
        }
    }
    buffer.push('"');
    buffer
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn json_string_escapes_control_and_special_characters() {
        assert_eq!(json_string("hello"), "\"hello\"");
        assert_eq!(json_string("with \"quotes\""), "\"with \\\"quotes\\\"\"");
        assert_eq!(json_string("with\\slash"), "\"with\\\\slash\"");
        assert_eq!(json_string("line1\nline2"), "\"line1\\nline2\"");
        assert_eq!(json_string("\u{0001}"), "\"\\u0001\"");
    }

    #[test]
    fn default_creates_empty_fixture() {
        let fixture = TakeoutBrowserHistoryFixture::default();
        assert_eq!(fixture.records.len(), 0);
    }

    #[test]
    fn json_string_escapes_tab_and_carriage_return() {
        assert_eq!(json_string("col1\tcol2"), "\"col1\\tcol2\"");
        assert_eq!(json_string("line\rend"), "\"line\\rend\"");
    }

    #[test]
    fn serialize_record_emits_field_order_the_parser_can_read() {
        let record = TakeoutBrowserRecord {
            url: "https://example.com".to_string(),
            title: Some("Example".to_string()),
            visit_time_unix_ms: 1_700_000_000_000,
            page_transition: Some("LINK".to_string()),
            client_id: None,
            favicon_url: None,
        };
        let serialized = serialize_record(&record);
        assert!(serialized.contains("\"url\": \"https://example.com\""));
        assert!(serialized.contains("\"title\": \"Example\""));
        assert!(serialized.contains("\"time_usec\": 1700000000000000"));
        assert!(serialized.contains("\"page_transition\": \"LINK\""));
        assert!(!serialized.contains("client_id"));
        assert!(!serialized.contains("favicon_url"));
    }
}
