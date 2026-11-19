//! Responsibilities: register independently maintained native-format readers.
//! Not responsible for: archive writes, reference authorization or presentation.

use std::io::BufRead;

pub use crate::model::*;

pub mod claude;
pub mod codex;
pub mod common;
pub mod deepseek;
pub mod identity;
pub mod pi;
pub mod session_json;

pub use claude::ClaudeAdapter;
pub use codex::CodexAdapter;
pub use deepseek::DeepseekAdapter;
pub use pi::PiAdapter;
pub use session_json::{ClineAdapter, ContinueAdapter, RooCodeAdapter};

/// Adapters interpret retained bytes without importing a harness's mutating loader.
pub trait Adapter: Send + Sync {
    fn harness(&self) -> Harness;

    /// Supplies a content-derived identity while leaving foreign identities as native links.
    fn conversation_key(
        &self,
        session: &SessionMetadata,
        source_id: &str,
        relative_path: &str,
        first_digest: &str,
        first: &serde_json::Value,
    ) -> identity::ConversationKey;

    /// JSONL identity is established only after the tool finishes its first record.
    fn needs_first_line_end(&self) -> bool {
        true
    }

    /// Some native formats replace older physical generations instead of querying them.
    fn current_source_only(&self) -> bool {
        false
    }

    /// Native filename time settles sources whose last recorded activity is equal.
    fn source_time(&self, _path: &str) -> Option<String> {
        None
    }

    /// Only a tool that actually stores lineage paths may authorize path resolution.
    fn lineage_is_path(&self) -> bool {
        false
    }

    /// Physical native references can name a file independently of its conversation.
    fn physical_source_id(&self, _path: &str) -> Option<String> {
        None
    }

    /// A failed captured generation can still be associated by its native owned folder.
    fn generation_group(&self, _path: &str) -> Option<String> {
        None
    }

    /// Classifies evidence by the native tool's write semantics, so retention does
    /// not depend on how successfully a parser understands its contents.
    fn retention(&self, artifact: &Artifact) -> &'static str {
        default_retention(artifact)
    }

    /// Stream known source artifacts; report unreadable areas rather than hiding them.
    fn discover(
        &self,
        source: &SourceRoot,
        emit: &mut dyn FnMut(Artifact) -> crate::Result<()>,
        issues: &mut Vec<Diagnostic>,
    ) -> crate::Result<()>;

    /// Desktop backups can stop a walk before the next matching file is found.
    fn discover_cancellable(
        &self,
        cancel: &crate::capture::Discovery,
        source: &SourceRoot,
        emit: &mut dyn FnMut(Artifact) -> crate::Result<()>,
        issues: &mut Vec<Diagnostic>,
    ) -> crate::Result<()> {
        cancel.check()?;
        self.discover(source, emit, issues)
    }

    /// Emit every native JSON object from already decompressed input.
    fn parse(
        &self,
        input: &mut dyn BufRead,
        context: &ParseContext,
        emit: &mut dyn FnMut(NativeEvent) -> crate::Result<()>,
    ) -> crate::Result<ParseReport>;
}

/// A single registry prevents discovery and parsing from choosing different readers.
pub fn adapter(harness: Harness) -> &'static dyn Adapter {
    match harness {
        Harness::ClaudeCode => &ClaudeAdapter,
        Harness::Codex => &CodexAdapter,
        Harness::Pi => &PiAdapter,
        Harness::Deepseek => &DeepseekAdapter,
        Harness::Continue => &ContinueAdapter,
        Harness::Cline => &ClineAdapter,
        Harness::RooCode => &RooCodeAdapter,
    }
}

/// Generic artifact semantics contain no knowledge of a particular tool's layout.
pub(crate) fn default_retention(artifact: &Artifact) -> &'static str {
    match artifact.kind {
        ArtifactKind::Transcript => "appended",
        ArtifactKind::Attachment => "write-once",
        ArtifactKind::Sqlite | ArtifactKind::Metadata => "state",
    }
}
