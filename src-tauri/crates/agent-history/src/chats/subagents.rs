//! Resolve the child title from the elected parent publication at the head switch.
//! Capture owns the saved metadata that identifies the launching tool call.
use serde_json::Value;
use vault_store::{Result, Session, params};

pub(super) fn name_launched_children(tx: &Session<'_>) -> Result<()> {
    let mut launches = tx.prepare(
        "SELECT child.publication,c.call_id,e.payload FROM ah_reader_build b
         CROSS JOIN ah_chat_summary child ON child.publication=b.publication
         JOIN ah_chat_summary parent ON parent.id=child.parent AND parent.live=1
         JOIN ah_chat_call c ON c.publication=parent.publication
           AND c.call_id=json_extract(child.detail,'$.launchCallId')
         JOIN ah_sessions s ON s.id=c.source
         JOIN ah_events e ON e.session_id=s.id AND e.projection_id=s.projection_id AND e.ordinal=c.ordinal
         WHERE child.title_source<>'tool' AND c.name IN ('Task','Agent')",
    )?;
    let mut rows = launches.query(())?;
    while let Some(row) = rows.next()? {
        let publication: String = row.get(0)?;
        let call: String = row.get(1)?;
        let event: Value =
            serde_json::from_str(&row.get::<String>(2)?).map_err(crate::query::operation)?;
        let description = event["message"]["content"]
            .as_array()
            .into_iter()
            .flatten()
            .find(|part| part["id"] == call && part["type"] == "tool_use")
            .and_then(|part| part["input"]["description"].as_str())
            .map(str::trim)
            .filter(|s| !s.is_empty());
        if let Some(title) = description {
            tx.execute(
                "UPDATE ah_chat_summary SET title=?2,title_source='tool' WHERE publication=?1",
                params![publication, title.chars().take(160).collect::<String>()],
            )?;
        }
    }
    Ok(())
}
