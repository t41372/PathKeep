//! Responsibilities: commit agent runs, steps and evidence before observation/replay.
//! Not responsible for: providers, transcript replacement or storage lifecycle.
//! Failure modes: missing first-turn parent, duplicate tool result, sequence races,
//! lost token accounting, replacement deleting journal evidence or orphan traces.

use super::*;

/// Creates a run and, for a lazy first turn, its conversation parent atomically.
pub fn begin_agent_run(vault: &Vault, run: &BeginAgentRun) -> Result<()> {
    anyhow::ensure!(!run.id.trim().is_empty(), "an agent run id is required");
    let run = run.clone();
    vault.write(Db::Agent, Priority::Interactive, move |s| {
        let now = now_rfc3339();
        if let Some(id) = run.conversation_id.as_deref() {
            s.execute(
                "INSERT INTO conversations(id, title, provider_id, created_at, updated_at,
                 message_count) VALUES(?1, ?2, ?3, ?4, ?4, 0) ON CONFLICT(id) DO NOTHING",
                params![id, UNTITLED_CONVERSATION_TITLE, run.provider_id.as_deref(), now.as_str()],
            )?;
        }
        s.execute(
            "INSERT INTO agent_runs(id, conversation_id, message_id, provider_id,
             embedding_provider_id, status, iterations, prompt_tokens, completion_tokens, error,
             created_at, updated_at) VALUES(?1, ?2, ?3, ?4, ?5, 'running', 0, 0, 0, NULL, ?6, ?6)
             ON CONFLICT(id) DO UPDATE SET conversation_id=excluded.conversation_id,
             message_id=excluded.message_id, provider_id=excluded.provider_id,
             embedding_provider_id=excluded.embedding_provider_id, status='running',
             iterations=0, prompt_tokens=0, completion_tokens=0, error=NULL,
             updated_at=excluded.updated_at",
            params![
                run.id.as_str(),
                run.conversation_id.as_deref(),
                run.message_id.as_deref(),
                run.provider_id.as_deref(),
                run.embedding_provider_id.as_deref(),
                now.as_str()
            ],
        )?;
        Ok(())
    })
}

/// Serializes sequence assignment and idempotent tool-result replay on the writer.
pub fn append_agent_step(vault: &Vault, step: &AppendAgentStep) -> Result<i64> {
    anyhow::ensure!(!step.run_id.trim().is_empty(), "an agent run id is required");
    let step = step.clone();
    vault.write(Db::Agent, Priority::Interactive, move |s| {
        if let Some(call) = step.tool_call_id.as_deref()
            && let Some(seq) = s
                .query(
                    "SELECT seq FROM agent_steps WHERE run_id=?1 AND kind=?2 AND tool_call_id=?3",
                    params![step.run_id.as_str(), step.kind.as_str(), call],
                    |r| Ok(r.get::<i64>(0)?),
                )?
                .pop()
        {
            return Ok(seq);
        }
        let seq: i64 = s.query_one(
            "SELECT COALESCE(MAX(seq)+1, 0) FROM agent_steps WHERE run_id=?1",
            [step.run_id.as_str()],
            |r| Ok(r.get(0)?),
        )?;
        let now = now_rfc3339();
        s.execute(
            "INSERT INTO agent_steps(id, run_id, seq, turn, kind, tool_name, tool_call_id,
             payload, created_at) VALUES(?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
            params![
                format!("{}:{seq}", step.run_id.as_str()),
                step.run_id.as_str(),
                seq,
                step.turn,
                step.kind.as_str(),
                step.tool_name.as_deref(),
                step.tool_call_id.as_deref(),
                step.payload.as_str(),
                now.as_str()
            ],
        )?;
        s.execute(
            "UPDATE agent_runs SET updated_at=?2 WHERE id=?1",
            params![step.run_id.as_str(), now.as_str()],
        )?;
        Ok(seq)
    })
}

/// Persists the final outcome and token accounting together.
pub fn finalize_agent_run(
    vault: &Vault,
    run_id: &str,
    status: AgentRunStatus,
    iterations: i64,
    prompt_tokens: i64,
    completion_tokens: i64,
    error: Option<&str>,
) -> Result<()> {
    let id = run_id.to_owned();
    let error = error.map(str::to_owned);
    vault.write(Db::Agent, Priority::Interactive, move |s| {
        s.execute(
            "UPDATE agent_runs SET status=?2, iterations=?3, prompt_tokens=?4,
             completion_tokens=?5, error=?6, updated_at=?7 WHERE id=?1",
            params![
                id.as_str(),
                status.as_str(),
                iterations,
                prompt_tokens,
                completion_tokens,
                error.as_deref(),
                now_rfc3339()
            ],
        )?;
        Ok(())
    })
}

/// Replaces one run's pinned evidence atomically and without duplicate history IDs.
pub fn record_agent_citations(
    vault: &Vault,
    run_id: &str,
    citations: &[AgentCitationRecord],
) -> Result<()> {
    let id = run_id.to_owned();
    let citations = citations.to_vec();
    vault.write(Db::Agent, Priority::Interactive, move |s| {
        s.execute("DELETE FROM agent_citations WHERE run_id=?1", [id.as_str()])?;
        for c in citations {
            s.execute(
                "INSERT INTO agent_citations(run_id, history_id, canonical_url, url, title,
                 visited_at, score, profile_id) VALUES(?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)
                 ON CONFLICT(run_id, history_id) DO UPDATE SET
                 canonical_url=excluded.canonical_url, url=excluded.url, title=excluded.title,
                 visited_at=excluded.visited_at, score=excluded.score,
                 profile_id=excluded.profile_id",
                params![
                    id.as_str(),
                    c.history_id,
                    c.canonical_url.as_str(),
                    c.url.as_str(),
                    c.title.as_deref(),
                    c.visited_at.as_deref(),
                    c.score.map(f64::from),
                    c.profile_id.as_deref()
                ],
            )?;
        }
        s.execute(
            "UPDATE agent_runs SET updated_at=?2 WHERE id=?1",
            params![id.as_str(), now_rfc3339()],
        )?;
        Ok(())
    })
}

/// Loads durable evidence for replay, including unfinished runs after a crash.
pub fn load_agent_run(vault: &Vault, run_id: &str) -> Result<Option<AgentRunTrace>> {
    let id = run_id.to_owned();
    vault.read(Db::Agent, move |s| load(s, &id))
}

fn load(s: &Session<'_>, id: &str) -> Result<Option<AgentRunTrace>> {
    let header = s
        .query(
            "SELECT conversation_id, message_id, provider_id, embedding_provider_id, status,
             iterations, prompt_tokens, completion_tokens, error FROM agent_runs WHERE id=?1",
            [id],
            |r| {
                Ok(AgentRunTrace {
                    id: id.to_owned(),
                    conversation_id: r.get(0)?,
                    message_id: r.get(1)?,
                    provider_id: r.get(2)?,
                    embedding_provider_id: r.get(3)?,
                    status: AgentRunStatus::from_tag(&r.get::<String>(4)?),
                    iterations: r.get(5)?,
                    prompt_tokens: r.get(6)?,
                    completion_tokens: r.get(7)?,
                    error: r.get(8)?,
                    steps: Vec::new(),
                    citations: Vec::new(),
                })
            },
        )?
        .pop();
    let Some(mut run) = header else { return Ok(None) };
    run.steps = s.query(
        "SELECT id, seq, turn, kind, tool_name, tool_call_id, payload FROM agent_steps WHERE
         run_id=?1 ORDER BY seq",
        [id],
        |r| {
            Ok(AgentStepRecord {
                id: r.get(0)?,
                seq: r.get(1)?,
                turn: r.get(2)?,
                kind: r.get(3)?,
                tool_name: r.get(4)?,
                tool_call_id: r.get(5)?,
                payload: r.get(6)?,
            })
        },
    )?;
    run.citations = s.query(
        "SELECT history_id, canonical_url, url, title, visited_at, score, profile_id
         FROM agent_citations WHERE run_id=?1 ORDER BY history_id",
        [id],
        |r| {
            Ok(AgentCitationRecord {
                history_id: r.get(0)?,
                canonical_url: r.get(1)?,
                url: r.get(2)?,
                title: r.get(3)?,
                visited_at: r.get(4)?,
                score: r.get::<Option<f64>>(5)?.map(|v| v as f32),
                profile_id: r.get(6)?,
            })
        },
    )?;
    Ok(Some(run))
}
