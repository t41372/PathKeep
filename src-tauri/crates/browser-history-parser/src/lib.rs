pub mod chromium;
pub mod error;
pub mod firefox;
pub mod safari;
pub mod takeout;
pub mod types;

pub use error::ParseError;
pub use types::{
    ChromiumHistory, ChromiumReadCursor, DatabaseInspection, HistoryDatabaseSet, ParsedDownload,
    ParsedFavicon, ParsedSearchTerm, ParsedUrl, ParsedVisit, ParserWarning,
};
