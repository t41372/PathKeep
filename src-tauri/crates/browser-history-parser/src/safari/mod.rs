use crate::{ParseError, types::DatabaseInspection};
use std::path::Path;

pub fn inspect_history(_path: &Path) -> Result<DatabaseInspection, ParseError> {
    Err(ParseError::UnsupportedProvider { provider: "safari" })
}
