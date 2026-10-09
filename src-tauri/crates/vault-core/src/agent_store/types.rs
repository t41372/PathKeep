//! Responsibilities: describe the durable agent journal independently of transport.
//! Not responsible for: persistence, provider calls or transcript pagination.
//! Failure modes: status tags or citation fields change across replay.

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AgentRunStatus {
    /// The run is in flight (or was interrupted by a crash and never finalized).
    Running,
    /// The run finished and produced a final answer.
    Completed,
    /// The run was cooperatively cancelled.
    Cancelled,
    /// The run ended in a terminal error.
    Failed,
}

impl AgentRunStatus {
    /// Returns the stable lowercase tag stored in `agent_runs.status`.
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Running => "running",
            Self::Completed => "completed",
            Self::Cancelled => "cancelled",
            Self::Failed => "failed",
        }
    }

    /// Parses a stored status tag back into the enum (unknown tags read as `Running`/interrupted).
    pub(super) fn from_tag(tag: &str) -> Self {
        match tag {
            "completed" => Self::Completed,
            "cancelled" => Self::Cancelled,
            "failed" => Self::Failed,
            _ => Self::Running,
        }
    }
}

/// Parameters that open one agent run header.
///
/// A struct (not a long positional arg list) so the harness call site stays readable and a new
/// optional field is additive. `conversation_id`/`message_id` link the run to the chat turn it
/// answers; both providers are recorded for observability + replay context.
#[derive(Debug, Clone, Default)]
pub struct BeginAgentRun {
    pub id: String,
    pub conversation_id: Option<String>,
    pub message_id: Option<String>,
    pub provider_id: Option<String>,
    pub embedding_provider_id: Option<String>,
}

/// One journaled step in an agent run, appended BEFORE the harness observes/emits it.
///
/// `seq` is the monotonic 0-based ordinal within the run (the replay order); `turn` is the model
/// turn the step belongs to. `kind` is a stable tag (`assistant-turn` / `tool-result` / ...).
/// `payload` is opaque JSON owned by the caller, so the journal schema does not couple to the
/// evolving tool/turn shapes. `tool_call_id` (+ `run_id`) is the idempotency key for tool-result
/// steps so a duplicate/replayed result is detectable.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AgentStepRecord {
    pub id: String,
    pub seq: i64,
    pub turn: i64,
    pub kind: String,
    pub tool_name: Option<String>,
    pub tool_call_id: Option<String>,
    pub payload: String,
}

/// A new step to append; `seq` is assigned by [`super::append_agent_step`] (the next ordinal for the run).
#[derive(Debug, Clone)]
pub struct AppendAgentStep {
    pub run_id: String,
    pub turn: i64,
    pub kind: String,
    pub tool_name: Option<String>,
    pub tool_call_id: Option<String>,
    pub payload: String,
}

/// One pinned citation row for a run (evidence that must survive context compaction).
///
/// `canonical_url` is the W-STAR star key, so WU-6 can star a cited page directly from a trace.
#[derive(Debug, Clone, PartialEq)]
pub struct AgentCitationRecord {
    pub history_id: i64,
    pub canonical_url: String,
    pub url: String,
    pub title: Option<String>,
    pub visited_at: Option<String>,
    pub score: Option<f32>,
    /// The browser profile of the cited visit; none on rows converted from before it was kept.
    pub profile_id: Option<String>,
}

/// A fully loaded run trace for replay/inspection: the header plus its ordered steps + citations.
#[derive(Debug, Clone)]
pub struct AgentRunTrace {
    pub id: String,
    pub conversation_id: Option<String>,
    pub message_id: Option<String>,
    pub provider_id: Option<String>,
    pub embedding_provider_id: Option<String>,
    pub status: AgentRunStatus,
    pub iterations: i64,
    pub prompt_tokens: i64,
    pub completion_tokens: i64,
    pub error: Option<String>,
    pub steps: Vec<AgentStepRecord>,
    pub citations: Vec<AgentCitationRecord>,
}
