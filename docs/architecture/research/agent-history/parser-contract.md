# Adapter implementation contract

Working interface for the independently testable `agent-history::adapters` layer.
Tests must be written before its implementation; execute them only during final
verification. These names are shared by adapter implementers and archive code.

```rust
pub enum Harness { ClaudeCode, Codex, Pi, Deepseek }
// serde names: claude-code, codex, pi, deepseek; FromStr accepts these.
pub enum ArtifactKind { Transcript, Attachment, Metadata, Sqlite }
pub struct SourceRoot { pub harness: Harness, pub root: PathBuf }
pub struct Artifact {
    pub path: PathBuf,
    pub relative_path: String,
    pub kind: ArtifactKind,
    pub project_hint: Option<String>,
    pub generation: Option<u64>,
    pub current: bool, // older DeepSeek generations remain raw evidence
}
pub struct Diagnostic {
    pub code: String,
    pub path: Option<String>,
    pub ordinal: Option<u64>,
    pub detail: Option<String>, // never conversation text/credentials
}
pub struct SessionMetadata {
    pub native_id: String,
    pub project_path: Option<String>,
    pub title: Option<String>,
    pub created_at: Option<i64>, // milliseconds, source raw precision retained
    pub updated_at: Option<i64>,
    pub origin: String, // cli | desktop | cowork | unknown
    pub harness_version: Option<String>,
    pub parent_session_id: Option<String>,
    pub model: Option<String>,
}
pub struct NativeEvent {
    pub ordinal: u64, // physical JSONL record ordinal, zero based
    pub native_id: Option<String>,
    pub parent_id: Option<String>,
    pub kind: String,
    pub role: Option<String>,
    pub timestamp: Option<i64>,
    pub text: String,
    pub payload: serde_json::Value,
}
pub struct Reference {
    pub kind: String, // attachment | lineage | tool-output
    pub target: String, // native reference; resolver validates authority
    pub end_byte_offset: Option<u64>,
    pub end_ordinal_exclusive: Option<u64>,
}
pub struct ParseContext {
    pub relative_path: String,
    pub project_hint: Option<String>,
    pub max_record_bytes: usize,
}
pub struct ParseReport {
    pub session: Option<SessionMetadata>,
    pub event_count: u64,
    pub diagnostics: Vec<Diagnostic>,
    pub references: Vec<Reference>,
    pub complete: bool,
}
pub trait Adapter: Send + Sync {
    fn harness(&self) -> Harness;
    fn discover(&self, source: &SourceRoot,
        emit: &mut dyn FnMut(Artifact) -> crate::Result<()>,
        issues: &mut Vec<Diagnostic>) -> crate::Result<()>;
    // Input has already been decompressed by the shared transport layer.
    fn parse(&self, input: &mut dyn std::io::BufRead,
        context: &ParseContext,
        emit: &mut dyn FnMut(NativeEvent) -> crate::Result<()>)
        -> crate::Result<ParseReport>;
}
pub fn adapter(harness: Harness) -> &'static dyn Adapter;
```

Shared `adapters::common` supplies bounded JSONL record iteration (an oversized
record is drained without allocating it and reported; its raw bytes remain
archived), timestamp conversion, text extraction from known message content,
safe filesystem traversal and bounded diagnostic aggregation. All raw JSON
objects, including unknown event kinds, are emitted. Native content is never
logged. No adapter invokes the harness executable or imports its session manager.

`adapters::{claude,codex,pi,deepseek}` each exposes its zero-sized adapter struct
(`ClaudeAdapter`, `CodexAdapter`, `PiAdapter`, `DeepseekAdapter`) implementing this
trait. Parsers do not read referenced files; reference resolution belongs to
capture. Directory symlinks are not followed by default. Discovery errors are
reported rather than silently swallowed. Explicit source roots allow custom
profiles without changing process-global HOME/environment values.

The core retains all raw revisions, and normalization records are tied to the
physical source/revision. Native session IDs are grouping facts, not globally
unique primary keys. Source-specific branches and lineage survive in payloads
and references; a linear presentation must not pretend they are a single transcript.
