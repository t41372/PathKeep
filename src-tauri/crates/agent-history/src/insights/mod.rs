//! Cancellable, archive-local reports over original turn, message and reply contributions.
//! Reading an Insights card never parses a transcript or opens a source folder.
mod calendar;
mod catalog;
mod clock;
mod facts;
mod pages;
mod prefs;
mod query;
mod report;
pub(crate) mod work;
use crate::Harness;
pub(crate) use facts::Writer;
pub use prefs::{PreferencePatch, Preferences};
pub(crate) use query::Cache;
use serde::{Deserialize, Serialize};

pub(crate) const MODEL: &[&str] = &[
    "CREATE TABLE ah_insight_page(publication TEXT NOT NULL,source TEXT NOT NULL,page INTEGER NOT NULL,lo INTEGER,hi INTEGER,undated INTEGER NOT NULL,first_usage INTEGER,brief BLOB NOT NULL,detail BLOB NOT NULL,PRIMARY KEY(publication,source,page))",
    "CREATE TABLE ah_insight_clock_input(publication TEXT NOT NULL,source TEXT NOT NULL,page INTEGER NOT NULL,hour INTEGER NOT NULL,data BLOB NOT NULL,PRIMARY KEY(publication,source,page,hour))",
    "CREATE INDEX ah_insight_clock_by_hour ON ah_insight_clock_input(hour,publication)",
    "CREATE TABLE ah_insight_clock_dirty(hour INTEGER PRIMARY KEY)",
    "CREATE TABLE ah_insight_clock_hour(hour INTEGER PRIMARY KEY,lo INTEGER NOT NULL,hi INTEGER NOT NULL,busy INTEGER NOT NULL,summed INTEGER NOT NULL,timed INTEGER NOT NULL,data BLOB NOT NULL)",
    "CREATE INDEX ah_insight_time ON ah_insight_page(hi)",
    "ALTER TABLE ah_chat_summary ADD COLUMN insight_first INTEGER",
    "ALTER TABLE ah_chat_summary ADD COLUMN insight_first_usage INTEGER",
    "ALTER TABLE ah_chat_summary ADD COLUMN insight_last INTEGER",
    "INSERT OR REPLACE INTO ah_reader_dirty SELECT conversation,lower(hex(randomblob(16))) FROM ah_reader_head",
];
pub(crate) const EVIDENCE: &[&str] = &[
    "CREATE TABLE ah_insight_preferences(id INTEGER PRIMARY KEY,value TEXT NOT NULL)",
    "INSERT INTO ah_insight_preferences VALUES(1,'{}')",
];
#[derive(Clone, Copy, Default, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Range {
    #[serde(rename = "today")]
    Today,
    #[serde(rename = "7d")]
    Week,
    #[serde(rename = "30d")]
    Month,
    #[serde(rename = "90d")]
    Quarter,
    #[serde(rename = "1y")]
    Year,
    #[serde(rename = "all")]
    #[default]
    All,
}
#[derive(Clone, Copy, PartialEq, Default, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum Part {
    Report,
    Calendar,
    #[default]
    Both,
}
/// Filters are transient. With no explicit range the viewer's saved range applies.
#[derive(Default, Deserialize, Serialize, Clone)]
#[serde(default, deny_unknown_fields, rename_all = "camelCase")]
pub struct Query {
    pub range: Option<Range>,
    pub year: Option<i32>,
    pub harnesses: Vec<Harness>,
    pub project_id: Option<String>,
    pub part: Part,
    pub include_estimates: Option<bool>,
}

#[cfg(test)]
mod measurement;
