use std::path::PathBuf;

use thiserror::Error;

#[derive(Debug, Error)]
pub enum ParseError {
    #[error("failed to open SQLite database at {path}: {source}")]
    OpenDatabase {
        path: PathBuf,
        #[source]
        source: rusqlite::Error,
    },
    #[error("failed to inspect SQLite database: {0}")]
    Sqlite(#[from] rusqlite::Error),
    #[error("required table `{table}` is missing from the provided source")]
    MissingTable { table: &'static str },
    #[error("provider `{provider}` is not implemented yet")]
    UnsupportedProvider { provider: &'static str },
}
