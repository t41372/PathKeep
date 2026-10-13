//! Responsibilities: complete agent row fixtures for maintenance round trips.
//! Not responsible for: producing migrations or exercising production query code.
//! Failure modes: a table or column the round trip should carry has no fixture row.
use super::Fixture;

pub(super) const ROWS: &[Fixture] = &[
    Fixture {
        table: "conversations",
        columns: "id, title, provider_id, created_at, updated_at, message_count",
        values: "'sample', 'sample', 'sample', 'sample', 'sample', 1",
    },
    Fixture {
        table: "messages",
        columns: "id, conversation_id, seq, role, content, reasoning, tool_calls_json, status, created_at",
        values: "'sample', 'sample', 1, 'sample', 'sample', 'sample', 'sample', 'sample', 'sample'",
    },
    Fixture {
        table: "agent_runs",
        columns: "id, conversation_id, message_id, provider_id, embedding_provider_id, status, iterations, prompt_tokens, completion_tokens, error, created_at, updated_at",
        values: "'sample', 'sample', 'sample', 'sample', 'sample', 'sample', 1, 1, 1, 'sample', 'sample', 'sample'",
    },
    Fixture {
        table: "agent_steps",
        columns: "id, run_id, seq, turn, kind, tool_name, tool_call_id, payload, created_at",
        values: "'sample', 'sample', 1, 1, 'sample', 'sample', 'sample', 'sample', 'sample'",
    },
    Fixture {
        table: "agent_citations",
        columns: "run_id, history_id, canonical_url, url, title, visited_at, score",
        values: "'sample', 1, 'sample', 'sample', 'sample', 'sample', 1.5",
    },
];
