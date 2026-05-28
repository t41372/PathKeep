//! Deterministic browser-history fixtures for PathKeep ingest tests.
//!
//! ## Responsibilities
//! - Write real-format browser history files (Chromium `History` SQLite today;
//!   Firefox / Safari / Takeout to follow) from declarative record structs.
//! - Convert between human-readable Unix times and the on-disk epochs each
//!   browser uses, so fixture authors never write raw epoch math.
//! - Stay self-validating: every generator is paired with a round-trip test
//!   that proves PathKeep's real parser reads the fixture back as expected.
//!
//! ## Not responsible for
//! - Sampling real user data. Every fixture is programmatically synthesized;
//!   no URL or title is ever pulled from a live browser DB.
//! - Driving the canonical ingest pipeline. That belongs to integration tests
//!   in `vault-core`, which will consume the fixtures emitted here.
//! - Scenario orchestration (`Scenario` DSL, multi-profile composition,
//!   assertion API). That layer ships in the next slice once the per-family
//!   writers are verified.
//!
//! ## Dependencies
//! - `rusqlite` (bundled SQLCipher build inherited from the workspace) for
//!   writing real History databases.
//! - Epoch conversions are implemented in `time.rs` with plain integer
//!   arithmetic — no `chrono` dependency. The constants are pinned to
//!   `vault_core::utils::CHROME_UNIX_EPOCH_OFFSET_MICROS` and verified
//!   by round-trip tests against the production parser.
//!
//! ## Performance notes
//! - Fixture writes use a single transaction per database; bulk-loading a
//!   million-row scenario is bounded by SQLite's write throughput, not by
//!   per-row Rust overhead.

pub mod chromium;
pub mod firefox;
pub mod safari;
pub mod takeout;
pub mod time;

pub use chromium::{
    ChromiumDownloadRow, ChromiumFaviconRow, ChromiumHistoryFixture, ChromiumIconMappingRow,
    ChromiumKeywordSearchTermRow, ChromiumUrlRow, ChromiumVisitRow,
};
pub use firefox::{
    FirefoxPlaceRow, FirefoxPlacesFixture, FirefoxVisitRow, firefox_time_to_unix_ms,
    unix_ms_to_firefox_time,
};
pub use safari::{
    SafariHistoryFixture, SafariHistoryItemRow, SafariHistoryVisitRow, SafariSchemaVariant,
    safari_time_to_unix_ms, unix_ms_to_safari_time,
};
pub use takeout::{TakeoutBrowserHistoryFixture, TakeoutBrowserRecord, TakeoutPayloadFormat};
pub use time::{chrome_time_to_unix_ms, unix_ms_to_chrome_time};
