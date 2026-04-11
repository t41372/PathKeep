//! Google Takeout placeholder parser surface.
//!
//! PathKeep can import Takeout through higher-level logic, but this parser
//! crate does not currently expose a standalone Takeout database/file parser.
//! The explicit unsupported error keeps that boundary honest.

use crate::{ParseError, types::DatabaseInspection};
use std::path::Path;

/// Reports that standalone Takeout parsing is not implemented in this crate.
pub fn inspect_history(_path: &Path) -> Result<DatabaseInspection, ParseError> {
    Err(ParseError::UnsupportedProvider { provider: "google-takeout" })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn inspect_history_reports_takeout_as_an_unsupported_provider() {
        let error =
            inspect_history(Path::new("/tmp/takeout.zip")).expect_err("takeout should not inspect");
        match error {
            ParseError::UnsupportedProvider { provider } => assert_eq!(provider, "google-takeout"),
            other => panic!("expected unsupported provider error, got {other:?}"),
        }
    }
}
