//! Resolve the child title and type from the elected parent publication at the head switch.
//! Capture owns the saved metadata that identifies the launching tool call.
use serde_json::Value;
use vault_store::{Result, Session, params};

pub(super) fn apply_launch_details(tx: &Session<'_>) -> Result<()> {
    let mut launches = tx.prepare(
        "SELECT child.publication,c.call_id,e.payload FROM ah_reader_build b
         CROSS JOIN ah_chat_summary child ON child.publication=b.publication
         JOIN ah_chat_summary parent ON parent.id=child.parent AND parent.live=1
         JOIN ah_chat_call c ON c.publication=parent.publication
           AND c.call_id=json_extract(child.detail,'$.launchCallId')
         JOIN ah_sessions s ON s.id=c.source
         JOIN ah_events e ON e.session_id=s.id AND e.projection_id=s.projection_id AND e.ordinal=c.ordinal
         WHERE c.name IN ('Task','Agent') AND (child.title_source<>'tool'
            OR nullif(trim(json_extract(child.detail,'$.agentType')),'') IS NULL)",
    )?;
    let mut rows = launches.query(())?;
    while let Some(row) = rows.next()? {
        let publication: String = row.get(0)?;
        let call: String = row.get(1)?;
        let event: Value =
            serde_json::from_str(&row.get::<String>(2)?).map_err(crate::query::operation)?;
        let Some(input) = event["message"]["content"]
            .as_array()
            .into_iter()
            .flatten()
            .find(|part| part["id"] == call && part["type"] == "tool_use")
            .and_then(|part| part["input"].as_object())
        else {
            continue;
        };
        let text = |name: &str| input.get(name)?.as_str().map(str::trim).filter(|s| !s.is_empty());
        if let Some(title) = text("description") {
            tx.execute(
                "UPDATE ah_chat_summary SET title=?2,title_source='tool'
                 WHERE publication=?1 AND title_source<>'tool'",
                params![publication.as_str(), title.chars().take(160).collect::<String>()],
            )?;
        }
        if let Some(agent_type) = text("subagent_type") {
            tx.execute(
                "UPDATE ah_chat_summary SET detail=json_set(detail,'$.agentType',?2)
                 WHERE publication=?1 AND nullif(trim(json_extract(detail,'$.agentType')),'') IS NULL",
                params![publication.as_str(), agent_type],
            )?;
        }
    }
    Ok(())
}
