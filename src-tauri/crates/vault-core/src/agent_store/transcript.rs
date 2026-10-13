//! Responsibilities: atomic transcript saves and indexed, cursor-based chat/message reads.
//! Not responsible for: schema lifecycle, model execution or journal mutation.
//! Failure modes: partial saves, changed birth times, omitted messages/chats at page
//! boundaries, citations from a superseded run, or deleting only part of a chat.

use super::*;

/// Saves a complete transcript atomically, retaining creation times of existing turns.
pub fn save_conversation(
    vault: &Vault,
    request: &SaveAgentConversationRequest,
) -> Result<AgentConversationSummary> {
    anyhow::ensure!(!request.id.trim().is_empty(), "a conversation id is required");
    let request = request.clone();
    vault.write(Db::Agent, Priority::Interactive, move |s| save(s, &request))
}

fn save(
    s: &Session<'_>,
    request: &SaveAgentConversationRequest,
) -> Result<AgentConversationSummary> {
    let now = now_rfc3339();
    let title = request
        .title
        .as_deref()
        .map(str::trim)
        .filter(|t| !t.is_empty())
        .map(str::to_owned)
        .unwrap_or_else(|| derive_title_from_messages(&request.messages));
    let created_at =
        load_summary(s, &request.id)?.map(|c| c.created_at).unwrap_or_else(|| now.clone());
    let prior: std::collections::HashMap<String, String> = s
        .query(
            "SELECT id, created_at FROM messages WHERE conversation_id=?1",
            [request.id.as_str()],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )?
        .into_iter()
        .collect();
    s.execute(
        "INSERT INTO conversations(id, title, provider_id, created_at, updated_at, message_count)
         VALUES(?1, ?2, ?3, ?4, ?5, ?6) ON CONFLICT(id) DO UPDATE SET title=excluded.title,
         provider_id=excluded.provider_id, updated_at=excluded.updated_at,
         message_count=excluded.message_count",
        params![
            request.id.as_str(),
            title.as_str(),
            request.provider_id.as_deref(),
            created_at.as_str(),
            now.as_str(),
            request.messages.len() as i64
        ],
    )?;
    s.execute("DELETE FROM messages WHERE conversation_id=?1", [request.id.as_str()])?;
    for (seq, m) in request.messages.iter().enumerate() {
        s.execute(
            "INSERT INTO messages(id, conversation_id, seq, role, content, reasoning,
             tool_calls_json, status, created_at) VALUES(?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
            params![
                m.id.as_str(),
                request.id.as_str(),
                seq as i64,
                m.role.as_str(),
                m.content.as_str(),
                m.reasoning.as_deref(),
                m.tool_calls_json.as_deref(),
                m.status.as_deref(),
                prior.get(&m.id).unwrap_or(&now).as_str()
            ],
        )?;
    }
    Ok(AgentConversationSummary {
        id: request.id.clone(),
        title,
        provider_id: request.provider_id.clone(),
        created_at,
        updated_at: now,
        message_count: request.messages.len(),
    })
}

/// Enumerates every conversation using recency and ID to break timestamp ties.
pub fn list_conversations(
    vault: &Vault,
    limit: Option<u32>,
    cursor: Option<&str>,
) -> Result<AgentConversationListResponse> {
    let cursor: Option<(String, String)> = cursor.map(serde_json::from_str).transpose()?;
    let limit = page_size(limit);
    vault.read(Db::Agent, move |s| {
        let mut conversations = if let Some((time, id)) = cursor {
            // Two index seeks avoid Turso's residual tuple-inequality scan.
            let mut rows = s.query(
                "SELECT id, title, provider_id, created_at, updated_at, message_count FROM
                 conversations WHERE updated_at=?1 AND id<?2 ORDER BY updated_at DESC, id DESC
                 LIMIT ?3",
                params![time.as_str(), id.as_str(), limit + 1],
                summary,
            )?;
            if rows.len() <= limit as usize {
                rows.extend(s.query(
                    "SELECT id, title, provider_id, created_at, updated_at, message_count FROM
                     conversations WHERE updated_at<?1 ORDER BY updated_at DESC, id DESC LIMIT ?2",
                    params![time.as_str(), limit + 1 - rows.len() as i64],
                    summary,
                )?);
            }
            rows
        } else {
            s.query(
                "SELECT id, title, provider_id, created_at, updated_at, message_count FROM
                 conversations ORDER BY updated_at DESC, id DESC LIMIT ?1",
                [limit + 1],
                summary,
            )?
        };
        let next_cursor = if conversations.len() > limit as usize {
            conversations.pop();
            let last = conversations.last().unwrap();
            Some(serde_json::to_string(&(&last.updated_at, &last.id))?)
        } else {
            None
        };
        Ok(AgentConversationListResponse { conversations, next_cursor })
    })
}

/// Provides bounded transcript reads without treating the page size as a history cap.
pub fn list_conversation_messages(
    vault: &Vault,
    conversation_id: &str,
    limit: Option<u32>,
    cursor: Option<i64>,
) -> Result<AgentMessagePage> {
    let id = conversation_id.to_owned();
    vault.read(Db::Agent, move |s| message_page(s, &id, page_size(limit), cursor))
}

fn message_page(
    s: &Session<'_>,
    id: &str,
    limit: i64,
    cursor: Option<i64>,
) -> Result<AgentMessagePage> {
    let mut rows = s.query(
        "SELECT seq, id, role, content, reasoning, tool_calls_json, status FROM messages WHERE
         conversation_id=?1 AND seq>?2 ORDER BY seq ASC LIMIT ?3",
        params![id, cursor.unwrap_or(-1), limit + 1],
        |r| {
            Ok((
                r.get::<i64>(0)?,
                AgentMessage {
                    id: r.get(1)?,
                    role: r.get(2)?,
                    content: r.get(3)?,
                    reasoning: r.get(4)?,
                    tool_calls_json: r.get(5)?,
                    status: r.get(6)?,
                    ..Default::default()
                },
            ))
        },
    )?;
    let next_cursor = if rows.len() > limit as usize {
        rows.pop();
        Some(rows.last().unwrap().0)
    } else {
        None
    };
    let mut messages: Vec<_> = rows.into_iter().map(|(_, m)| m).collect();
    hydrate_messages(s, id, &mut messages)?;
    Ok(AgentMessagePage { messages, next_cursor })
}

/// Assembles the full transcript for replay and existing whole-transcript save clients.
/// Paged clients should use `list_conversation_messages` and never save a partial transcript.
pub fn load_conversation(
    vault: &Vault,
    conversation_id: &str,
) -> Result<Option<AgentConversationDetail>> {
    let id = conversation_id.to_owned();
    vault.read(Db::Agent, move |s| {
        let Some(summary) = load_summary(s, &id)? else { return Ok(None) };
        let mut messages = Vec::new();
        let mut cursor = None;
        loop {
            let page = message_page(s, &id, i64::from(MAX_AGENT_PAGE_SIZE), cursor)?;
            messages.extend(page.messages);
            cursor = page.next_cursor;
            if cursor.is_none() {
                break;
            }
        }
        Ok(Some(AgentConversationDetail { summary, messages }))
    })
}

/// Removes a conversation and its transcript/journal through the schema's cascades.
pub fn delete_conversation(vault: &Vault, conversation_id: &str) -> Result<bool> {
    let id = conversation_id.to_owned();
    vault.write(Db::Agent, Priority::Interactive, move |s| {
        Ok(s.execute("DELETE FROM conversations WHERE id=?1", [id])? > 0)
    })
}

/// Keeps a manual title and recency update in the same transaction.
pub fn rename_conversation(
    vault: &Vault,
    conversation_id: &str,
    title: &str,
) -> Result<Option<AgentConversationSummary>> {
    let title = title.trim().to_owned();
    anyhow::ensure!(!title.is_empty(), "a conversation title cannot be blank");
    let id = conversation_id.to_owned();
    vault.write(Db::Agent, Priority::Interactive, move |s| {
        s.execute(
            "UPDATE conversations SET title=?2, updated_at=?3 WHERE id=?1",
            params![id.as_str(), title.as_str(), now_rfc3339()],
        )?;
        load_summary(s, &id)
    })
}

fn hydrate_messages(
    s: &Session<'_>,
    conversation_id: &str,
    messages: &mut [AgentMessage],
) -> Result<()> {
    if messages.is_empty() {
        return Ok(());
    }
    let placeholders =
        (2..messages.len() + 2).map(|n| format!("?{n}")).collect::<Vec<_>>().join(",");
    let mut args = vec![vault_store::Value::Text(conversation_id.to_owned())];
    args.extend(messages.iter().map(|m| vault_store::Value::Text(m.id.clone())));
    let latest = format!(
        "SELECT r.id, r.message_id, r.prompt_tokens, r.completion_tokens, r.provider_id, r.model
         FROM agent_runs r WHERE
         r.conversation_id=?1 AND r.message_id IN ({placeholders}) AND NOT EXISTS (SELECT 1 FROM
         agent_runs newer WHERE newer.conversation_id=r.conversation_id AND
         newer.message_id=r.message_id AND (newer.updated_at>r.updated_at OR
         (newer.updated_at=r.updated_at AND newer.id>r.id)))"
    );
    let runs = s.query(&latest, args, |r| {
        Ok((
            r.get::<String>(0)?,
            r.get::<String>(1)?,
            r.get::<i64>(2)?,
            r.get::<i64>(3)?,
            r.get::<Option<String>>(4)?,
            r.get::<Option<String>>(5)?,
        ))
    })?;
    if runs.is_empty() {
        return Ok(());
    }
    let by_message: std::collections::HashMap<_, _> = runs
        .iter()
        .map(|(id, m, p, c, provider, model)| (m.as_str(), (id.as_str(), *p, *c, provider, model)))
        .collect();
    let placeholders = (1..=runs.len()).map(|n| format!("?{n}")).collect::<Vec<_>>().join(",");
    let args: Vec<_> = runs.iter().map(|r| vault_store::Value::Text(r.0.clone())).collect();
    let mut citations: std::collections::HashMap<String, Vec<AgentCitation>> =
        std::collections::HashMap::new();
    // In the order they were recorded, which is the order the answer cited them in: a reopened
    // answer lists its pages as they were shown, and its first page stays the one "Open in
    // History" goes to. (`record_agent_citations` replaces a run's rows in that order.)
    s.for_each(
        &format!(
            "SELECT run_id, history_id, canonical_url, url, title, visited_at, score, profile_id
             FROM agent_citations WHERE run_id IN ({placeholders}) ORDER BY run_id, rowid"
        ),
        args,
        |r| {
            citations.entry(r.get(0)?).or_default().push(AgentCitation {
                history_id: r.get(1)?,
                canonical_url: Some(r.get(2)?),
                url: r.get(3)?,
                title: r.get(4)?,
                visited_at: r.get::<Option<String>>(5)?.unwrap_or_default(),
                score: r.get::<Option<f64>>(6)?.map(|v| v as f32),
                profile_id: r.get::<Option<String>>(7)?.unwrap_or_default(),
            });
            Ok(())
        },
    )?;
    for m in messages {
        if let Some((id, prompt, completion, provider, model)) = by_message.get(m.id.as_str()) {
            m.provider_id.clone_from(provider);
            m.model.clone_from(model);
            if *prompt > 0 || *completion > 0 {
                m.usage = Some(AgentUsage {
                    prompt_tokens: (*prompt).max(0) as u64,
                    completion_tokens: (*completion).max(0) as u64,
                });
            }
            m.citations = citations.remove(*id).unwrap_or_default();
        }
    }
    Ok(())
}
