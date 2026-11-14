//! Responsibilities: stable, loss-aware values shared by capture and adapters.
//! Not responsible for: locating files, interpreting native formats or storage.

use std::{fmt, path::PathBuf, str::FromStr};

use serde::{Deserialize, Serialize};
use serde_json::Value;

/// A storage format, independent of which model provider generated a response.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Harness {
    ClaudeCode,
    Codex,
    Pi,
    Deepseek,
    Continue,
    Cline,
    RooCode,
}

impl Harness {
    /// The formats registered in this build, used for capability enumeration.
    pub const ALL: [Self; 7] = [
        Self::ClaudeCode,
        Self::Codex,
        Self::Pi,
        Self::Deepseek,
        Self::Continue,
        Self::Cline,
        Self::RooCode,
    ];

    /// The portable identifier used in source declarations and exported evidence.
    pub fn as_str(self) -> &'static str {
        match self {
            Self::ClaudeCode => "claude-code",
            Self::Codex => "codex",
            Self::Pi => "pi",
            Self::Deepseek => "deepseek",
            Self::Continue => "continue",
            Self::Cline => "cline",
            Self::RooCode => "roo-code",
        }
    }
}

impl fmt::Display for Harness {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.as_str())
    }
}

impl FromStr for Harness {
    type Err = crate::Error;

    fn from_str(value: &str) -> crate::Result<Self> {
        match value {
            "claude-code" | "claude" => Ok(Self::ClaudeCode),
            "codex" => Ok(Self::Codex),
            "pi" => Ok(Self::Pi),
            "deepseek" | "deepseek-harness" | "dsh" => Ok(Self::Deepseek),
            "continue" => Ok(Self::Continue),
            "cline" => Ok(Self::Cline),
            "roo-code" | "roo" => Ok(Self::RooCode),
            _ => Err(crate::Error::InvalidInput("unknown harness identifier".into())),
        }
    }
}

/// Distinguishes parseable transcripts from associated evidence and native stores.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ArtifactKind {
    Transcript,
    Attachment,
    Metadata,
    Sqlite,
}

/// Explicit roots keep independent profiles separate without changing HOME.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct SourceRoot {
    pub harness: Harness,
    pub root: PathBuf,
}

/// An adapter's inventory claim; capture must validate the path again before use.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Artifact {
    pub path: PathBuf,
    pub relative_path: String,
    pub kind: ArtifactKind,
    pub project_hint: Option<String>,
    pub generation: Option<u64>,
    pub current: bool,
}

/// A content-free explanation of a coverage limitation, suitable for translation.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Diagnostic {
    pub code: String,
    pub path: Option<String>,
    pub ordinal: Option<u64>,
    pub detail: Option<String>,
}

/// Native grouping facts never replace the physical source's archive identity.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct SessionMetadata {
    pub native_id: String,
    pub project_path: Option<String>,
    pub title: Option<String>,
    pub created_at: Option<i64>,
    pub updated_at: Option<i64>,
    pub origin: String,
    pub harness_version: Option<String>,
    pub parent_session_id: Option<String>,
    pub forked_from_session_id: Option<String>,
    pub model: Option<String>,
}

impl Default for SessionMetadata {
    fn default() -> Self {
        Self {
            native_id: String::new(),
            project_path: None,
            title: None,
            created_at: None,
            updated_at: None,
            origin: "unknown".into(),
            harness_version: None,
            parent_session_id: None,
            forked_from_session_id: None,
            model: None,
        }
    }
}

/// One native object plus searchable facts; payload retains unknown fields/types.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct NativeEvent {
    /// Transient occurrence evidence streams to the projection without growing ParseReport.
    #[serde(skip)]
    pub references: Vec<Reference>,
    pub ordinal: u64,
    pub native_id: Option<String>,
    pub parent_id: Option<String>,
    pub kind: String,
    pub role: Option<String>,
    pub timestamp: Option<i64>,
    pub text: String,
    pub payload: Value,
}

/// An untrusted native pointer which capture resolves only inside approved roots.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Reference {
    pub kind: String,
    pub target: String,
    pub end_byte_offset: Option<u64>,
    pub end_ordinal_exclusive: Option<u64>,
}

/// Per-file parser limits can degrade a projection without dropping raw evidence.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ParseContext {
    pub relative_path: String,
    pub project_hint: Option<String>,
    pub max_record_bytes: usize,
}

impl Default for ParseContext {
    fn default() -> Self {
        Self {
            relative_path: String::new(),
            project_hint: None,
            max_record_bytes: 16 * 1024 * 1024,
        }
    }
}

/// Describes interpretation coverage separately from successful raw capture.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ParseReport {
    pub session: Option<SessionMetadata>,
    pub event_count: u64,
    pub diagnostics: Vec<Diagnostic>,
    pub references: Vec<Reference>,
    pub complete: bool,
}

impl Default for ParseReport {
    fn default() -> Self {
        Self {
            session: None,
            event_count: 0,
            diagnostics: Vec::new(),
            references: Vec::new(),
            complete: true,
        }
    }
}
