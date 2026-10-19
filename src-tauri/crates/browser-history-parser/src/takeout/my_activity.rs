//! Google Takeout's "My Activity → Chrome" JSON: the pages Chrome reported to the Google
//! account's Web & App Activity (`docs/architecture/takeout-my-activity-tradeoff.md`).
//!
//! ## Responsibilities
//! - Read one record into a visit: the page address (out of Google's `google.com/url?q=` click
//!   redirect), the title without its `Visited ` prefix, the instant from `time`.
//! - Give each visit a key that stays the same when a later export of the same account is
//!   imported, so the same visit is not imported twice.
//!
//! ## Not responsible for
//! - Streaming the file and batching rows (`browser_history`'s accumulator does that for both
//!   Takeout formats).
//! - Recognizing the file (`source`).
//!
//! ## Failure modes, each tested
//! - A redirect kept as the address: every page would sit under google.com.
//! - Another Google address (a search, Maps) or a redirect without a page in `q` taken apart.
//! - A record of another product (Search, Maps activity) or without a page ("Used Chrome")
//!   becoming a visit.
//! - `Visited ` left in the title, or a title that is only the prefix kept as an empty one.
//! - A record without a usable `time` imported at the epoch.
//! - Keys tied to the position in the file: a later export, with newer records first, would
//!   import every visit again. Two records of the same page at the same instant must still both
//!   be kept.

use serde_json::Value;

/// One record of the export, read.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) enum ActivityRecord {
    Visit(ActivityVisit),
    /// "Used Chrome" and the like: no page.
    NoPage,
    /// Another Google product's activity.
    NotChrome,
    /// No `time`, or one that is not RFC 3339.
    NoTime,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct ActivityVisit {
    pub url: String,
    pub title: Option<String>,
    /// Unix microseconds.
    pub time_micros: i64,
}

/// The English prefix Google puts before every page title, whatever the account's language.
const VISITED: &str = "Visited ";

pub(super) fn read_record(record: &Value) -> ActivityRecord {
    let chrome = record
        .get("products")
        .and_then(Value::as_array)
        .is_some_and(|products| products.iter().any(|product| product.as_str() == Some("Chrome")));
    if !chrome {
        return ActivityRecord::NotChrome;
    }
    let Some(address) =
        record.get("titleUrl").and_then(Value::as_str).filter(|address| !address.is_empty())
    else {
        return ActivityRecord::NoPage;
    };
    let Some(time_micros) = record
        .get("time")
        .and_then(Value::as_str)
        .and_then(|time| chrono::DateTime::parse_from_rfc3339(time).ok())
        .map(|time| time.timestamp_micros())
    else {
        return ActivityRecord::NoTime;
    };
    let title = record
        .get("title")
        .and_then(Value::as_str)
        .map(|title| title.strip_prefix(VISITED).unwrap_or(title).trim())
        .filter(|title| !title.is_empty())
        .map(str::to_owned);
    ActivityRecord::Visit(ActivityVisit { url: page_address(address), title, time_micros })
}

/// The page behind Google's record of a click (`https://www.google.com/url?q=<page>&usg=…`).
/// Only that one redirect is taken apart, and only when `q` holds a web address; anything else
/// is the page itself.
fn page_address(address: &str) -> String {
    let Ok(parsed) = url::Url::parse(address) else {
        return address.to_owned();
    };
    let google = matches!(parsed.host_str(), Some("www.google.com" | "google.com"));
    if !google || parsed.path() != "/url" {
        return address.to_owned();
    }
    parsed
        .query_pairs()
        .find(|(name, _)| name == "q")
        .map(|(_, page)| page.into_owned())
        .filter(|page| {
            url::Url::parse(page).is_ok_and(|page| matches!(page.scheme(), "http" | "https"))
        })
        .unwrap_or_else(|| address.to_owned())
}

/// Tells apart records of the same page at the same instant, by how many came before in the
/// file. The export lists newest first, so they are next to each other: only the current
/// instant's pages are remembered, and the count does not depend on where in the file they are.
#[derive(Debug, Default)]
pub(super) struct SameInstant {
    time_micros: i64,
    pages: Vec<(String, u32)>,
}

impl SameInstant {
    /// The visit's identity: page, instant and which of the same page at that instant it is.
    pub fn key(&mut self, visit: &ActivityVisit) -> String {
        if visit.time_micros != self.time_micros {
            self.time_micros = visit.time_micros;
            self.pages.clear();
        }
        let seen = match self.pages.iter_mut().find(|(url, _)| *url == visit.url) {
            Some((_, count)) => {
                *count += 1;
                *count
            }
            None => {
                self.pages.push((visit.url.clone(), 0));
                0
            }
        };
        format!("{}:{}:{seen}", visit.url, visit.time_micros)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn visit(record: Value) -> ActivityVisit {
        match read_record(&record) {
            ActivityRecord::Visit(visit) => visit,
            other => panic!("not a visit: {other:?}"),
        }
    }

    fn record(title: &str, url: &str, time: &str) -> Value {
        json!({"header": "example.com", "title": title, "titleUrl": url, "time": time,
               "products": ["Chrome"]})
    }

    #[test]
    fn other_addresses_stay_as_they_are() {
        for address in [
            "https://docs.example/url?q=https://elsewhere.example/",
            "https://www.google.com/search?q=https://docs.example/",
            "https://www.google.com/url?usg=AOvVaw0",
            "https://www.google.com/url?q=not%20an%20address",
            "https://www.google.com/url?q=javascript:alert(1)",
        ] {
            assert_eq!(visit(record("Visited x", address, "2025-04-22T18:30:56Z")).url, address);
        }
    }

    #[test]
    fn records_without_a_chrome_page_or_a_time_are_not_visits() {
        let used = json!({"title": "Used Chrome", "time": "2025-04-22T18:30:56Z",
                          "products": ["Chrome"]});
        assert_eq!(read_record(&used), ActivityRecord::NoPage);
        let search = json!({"title": "Searched for x", "titleUrl": "https://www.google.com/search?q=x",
                            "time": "2025-04-22T18:30:56Z", "products": ["Search"]});
        assert_eq!(read_record(&search), ActivityRecord::NotChrome);
        for time in [json!(null), json!("yesterday"), json!(1_745_346_656)] {
            let mut timeless = record("Visited x", "https://docs.example/", "");
            timeless["time"] = time;
            assert_eq!(read_record(&timeless), ActivityRecord::NoTime);
        }
    }

    /// A newer export lists more records before the same ones; their keys must not move. Two
    /// records of one page at one instant stay two.
    #[test]
    fn keys_do_not_depend_on_where_a_record_is() {
        let keys = |records: &[(&str, &str)]| {
            let mut same = SameInstant::default();
            records
                .iter()
                .map(|(url, time)| same.key(&visit(record("Visited x", url, time))))
                .collect::<Vec<_>>()
        };
        let older = [
            ("https://a.example/", "2025-04-22T18:30:56Z"),
            ("https://a.example/", "2025-04-22T18:30:56Z"),
            ("https://b.example/", "2025-04-22T18:30:56Z"),
            ("https://a.example/", "2025-04-21T08:00:00Z"),
        ];
        let newer = [("https://c.example/", "2025-04-23T09:00:00Z")]
            .into_iter()
            .chain(older)
            .collect::<Vec<_>>();
        let (old_keys, new_keys) = (keys(&older), keys(&newer));
        assert_eq!(new_keys[1..], old_keys[..]);
        assert_ne!(old_keys[0], old_keys[1], "the same page twice at one instant is two visits");
    }
}
