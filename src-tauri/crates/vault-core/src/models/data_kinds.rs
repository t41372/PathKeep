//! Device capture policy in the guarded settings document. Approved root bindings
//! and captured evidence belong to the conversation store.
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::BTreeSet;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Deserialize, Serialize, JsonSchema)]
#[serde(rename_all = "kebab-case")]
pub enum DataKind {
    Browsing,
    AgentHistory,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Deserialize, Serialize, JsonSchema)]
#[serde(rename_all = "lowercase")]
pub enum KindState {
    On,
    Paused,
    Off,
}

/// Only these tools participate in desktop consent.
#[derive(
    Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Deserialize, Serialize, JsonSchema,
)]
#[serde(rename_all = "kebab-case")]
pub enum AgentTool {
    ClaudeCode,
    Codex,
    Pi,
    Deepseek,
}

#[derive(Clone, Debug, PartialEq, Eq, Deserialize, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct CapturePolicy {
    pub browsing: KindState,
    pub agent_history: KindState,
    pub enabled_tools: BTreeSet<AgentTool>,
    pub read_git: bool,
}

impl Default for CapturePolicy {
    fn default() -> Self {
        Self {
            browsing: KindState::On,
            agent_history: KindState::Off,
            enabled_tools: BTreeSet::new(),
            read_git: true,
        }
    }
}

#[derive(Debug, thiserror::Error)]
#[error("last-active-kind")]
pub struct LastActiveKind;

impl CapturePolicy {
    /// Compare the actual choices across windows and restarts.
    pub fn token(&self) -> String {
        hex::encode(Sha256::digest(serde_json::to_vec(self).expect("capture policy is JSON")))
    }

    /// Shared invariant for interactive changes, decoding a live control, and publishing any copy.
    pub fn validate(&self) -> Result<(), LastActiveKind> {
        if self.browsing == KindState::On || self.agent_history == KindState::On {
            Ok(())
        } else {
            Err(LastActiveKind)
        }
    }

    pub fn state(&self, kind: DataKind) -> KindState {
        match kind {
            DataKind::Browsing => self.browsing,
            DataKind::AgentHistory => self.agent_history,
        }
    }
}
