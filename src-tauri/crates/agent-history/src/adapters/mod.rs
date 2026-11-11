//! Responsibilities: register independently maintained native-format readers.
//! Not responsible for: archive writes, reference authorization or presentation.

use std::io::BufRead;

pub use crate::model::*;

pub mod claude;
pub mod codex;
pub mod common;
pub mod deepseek;
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

    /// Stream known source artifacts; report unreadable areas rather than hiding them.
    fn discover(
        &self,
        source: &SourceRoot,
        emit: &mut dyn FnMut(Artifact) -> crate::Result<()>,
        issues: &mut Vec<Diagnostic>,
    ) -> crate::Result<()>;

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
