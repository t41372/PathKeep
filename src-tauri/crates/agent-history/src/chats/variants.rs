//! Prompt versions in a Pi tree. A user entry points to its latest recorded
//! descendant so the version picker can request the corresponding leaf window.
use std::collections::HashMap;
use vault_store::{Session, params};

pub(super) fn publish(
    s: &Session<'_>,
    publication: &str,
    source: &str,
    attempt: i64,
) -> vault_store::Result<()> {
    let mut latest = HashMap::<String, i64>::new();
    let mut events=s.prepare("SELECT ordinal,native_id,parent_id,role FROM ah_events WHERE session_id=?1 AND projection_id=?2 AND kind<>'session' AND native_id IS NOT NULL ORDER BY ordinal DESC")?;
    let mut rows = events.query(params![source, attempt])?;
    while let Some(r) = rows.next()? {
        let ordinal: i64 = r.get(0)?;
        let id: String = r.get(1)?;
        let parent: Option<String> = r.get(2)?;
        let tip = latest.remove(&id).unwrap_or(ordinal);
        if r.get::<Option<String>>(3)?.as_deref() == Some("user") {
            s.execute(
                "INSERT INTO ah_chat_variant VALUES(?1,?2,?3,?4,?5,?6)",
                params![publication, source, ordinal, id, parent.as_deref(), tip],
            )?;
        }
        if let Some(parent) = parent {
            let highest = latest.entry(parent).or_insert(tip);
            *highest = (*highest).max(tip);
        }
    }
    Ok(())
}
