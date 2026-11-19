//! Responsibilities: the form a path inside the data folder takes in the records PathKeep writes
//! there to finish or undo an operation after a crash: the install journal, the record of stores
//! being made, and the delete-all-data intent. A path is plain names joined by `/`, written and
//! read the same on every system, because a data folder on an external drive moves between
//! Windows, macOS and Linux with its records.
//! Not responsible for: which paths a record may name (each record checks its own), or the native
//! absolute paths in the journal of PathKeep 0.5 (`install::LegacyJournal` reads them as they
//! are).
//!
//! How this could fail, and what guards it:
//! - Written with the system's separator, a record made on Windows names
//!   `archive\history-vault.db`; macOS reads that as one file with a backslash in its name, and
//!   recovery acts on another file than the one meant, or on none. Names are joined by `/` on
//!   every system.
//! - A name one system reads as something else: `\` separates names on Windows, `C:` is a drive
//!   there and `a:b` a stream of the file `a`. No name holds `/`, `\`, `:` or NUL, on any system,
//!   and each must also be one plain name to the system reading it.
//! - A record carried in from elsewhere or edited names `..`, an absolute path or an empty name,
//!   and recovery moves or deletes something outside the data folder. Only plain names decode; a
//!   record with anything else cannot be read, and the operation it belongs to stops with nothing
//!   changed.
//! - A path is recorded that cannot be read back, and after a crash the record that should finish
//!   the operation is refused. Writing follows the reading rule: a path that breaks it is refused
//!   before its record is written, so the operation never starts.
//!
//! Tests: `tests` below.

use serde::{Deserialize, Deserializer, Serialize, Serializer};
use std::path::{Component, Path, PathBuf};

/// A path inside the data folder, relative to it, as the crash records name it.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub(crate) struct RecordedPath(PathBuf);

impl RecordedPath {
    /// `path`, relative to the data folder, in the form a record keeps. A path a record could not
    /// read back the same on every system is refused here, before anything is recorded.
    pub(crate) fn new(path: &Path) -> anyhow::Result<Self> {
        let refused = || {
            anyhow::anyhow!(
                "{} cannot be recorded: a crash record holds only names that read the same on \
                 every system, without /, \\, : or NUL",
                path.display()
            )
        };
        let mut names = Vec::new();
        for part in path.components() {
            let Component::Normal(name) = part else { return Err(refused()) };
            names.push(name.to_str().filter(|name| is_plain(name)).ok_or_else(refused)?);
        }
        if names.is_empty() {
            return Err(refused());
        }
        Ok(Self(names.into_iter().collect()))
    }

    fn parse(text: &str) -> Option<Self> {
        let names: Vec<&str> = text.split('/').collect();
        names.iter().all(|name| is_plain(name)).then(|| Self(names.into_iter().collect()))
    }

    fn text(&self) -> String {
        self.0
            .components()
            .map(|part| part.as_os_str().to_str().expect("a plain name, checked when it was made"))
            .collect::<Vec<_>>()
            .join("/")
    }
}

/// One name, and the same one, to every system: not empty, not `.` or `..`, without `/`, `\`, `:`
/// or NUL, and one plain name to this system as well.
pub(crate) fn is_plain(name: &str) -> bool {
    !matches!(name, "" | "." | "..")
        && !name.contains(['/', '\\', ':', '\0'])
        && Path::new(name).components().eq([Component::Normal(name.as_ref())])
}

impl std::ops::Deref for RecordedPath {
    type Target = Path;

    fn deref(&self) -> &Path {
        &self.0
    }
}

impl AsRef<Path> for RecordedPath {
    fn as_ref(&self) -> &Path {
        &self.0
    }
}

impl Serialize for RecordedPath {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(&self.text())
    }
}

impl<'de> Deserialize<'de> for RecordedPath {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let text = String::deserialize(deserializer)?;
        Self::parse(&text).ok_or_else(|| {
            serde::de::Error::custom(format!("{text:?} is not plain names joined by /"))
        })
    }
}

impl schemars::JsonSchema for RecordedPath {
    fn schema_name() -> std::borrow::Cow<'static, str> {
        "RecordedPath".into()
    }

    fn json_schema(_: &mut schemars::SchemaGenerator) -> schemars::Schema {
        schemars::json_schema!({
            "type": "string",
            "pattern": "^[^/\\\\:\\u0000]+(/[^/\\\\:\\u0000]+)*$"
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn decodes(text: &str) -> bool {
        serde_json::from_str::<RecordedPath>(&serde_json::to_string(text).unwrap()).is_ok()
    }

    #[test]
    fn a_path_is_written_as_names_joined_by_slashes_and_read_back_the_same() {
        let path = RecordedPath::new(&Path::new("archive").join("history-vault.db")).unwrap();
        let text = serde_json::to_string(&path).unwrap();
        assert_eq!(text, r#""archive/history-vault.db""#);
        let read: RecordedPath = serde_json::from_str(&text).unwrap();
        assert_eq!(read, path);
        assert_eq!(Path::new("root").join(&read), Path::new("root/archive/history-vault.db"));
    }

    // Each of these is another path on some system, or a way out of the data folder.
    #[test]
    fn a_name_any_system_would_read_otherwise_does_not_decode() {
        for text in [
            "",
            "/",
            "a/",
            "/a",
            "a//b",
            ".",
            "..",
            "./a",
            "a/./b",
            "a/../b",
            "../a",
            "archive\\history-vault.db",
            "C:",
            "C:/a",
            "a:b",
            "a/b:c",
            "a\u{0}b",
            "//server/share",
        ] {
            assert!(!decodes(text), "{text:?}");
        }
        assert!(decodes("staging/maintenance-1aB_c.d"));
    }

    // What could not be read back is never recorded: the operation stops before its record.
    #[test]
    fn a_path_that_could_not_be_read_back_is_not_recorded() {
        for path in ["", "/a", "../a", "a/../b", "./a"] {
            assert!(RecordedPath::new(Path::new(path)).is_err(), "{path:?}");
        }
        #[cfg(unix)]
        {
            use std::os::unix::ffi::OsStrExt;
            for name in ["a\\b", "a:b", "C:"] {
                assert!(RecordedPath::new(&Path::new("archive").join(name)).is_err(), "{name:?}");
            }
            let not_utf8 = std::ffi::OsStr::from_bytes(b"a\xffb");
            assert!(RecordedPath::new(&Path::new("archive").join(not_utf8)).is_err());
        }
    }

    #[cfg(windows)]
    #[test]
    fn a_windows_path_is_written_with_slashes() {
        let path = RecordedPath::new(Path::new(r"staging\maintenance-x")).unwrap();
        assert_eq!(serde_json::to_string(&path).unwrap(), r#""staging/maintenance-x""#);
        for path in [r"C:\a", r"C:a", r"\\server\share\a", r"\a"] {
            assert!(RecordedPath::new(Path::new(path)).is_err(), "{path:?}");
        }
    }
}
