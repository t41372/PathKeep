//! Run details page through individual file conditions. Source cards read only
//! the compact counts recorded alongside each occurrence.
use super::*;
use vault_store::params;

pub(super) fn detail(vault: &Vault, run: i64, after: i64) -> Result<Vec<(i64, Value)>> {
    Ok(vault.store().read(Db::AgentHistoryEvidence, move |s| {
        s.query(
            "SELECT n.id,n.code,o.path,o.ordinal,o.detail,n.root_id,n.tool,n.first_run=o.run_id
         FROM ah_run_notes o JOIN ah_capture_notes n ON n.id=o.note_id
         WHERE o.run_id=?1 AND o.note_id>?2 ORDER BY o.note_id LIMIT 51",
            params![run, after],
            |r| {
                Ok((
                    r.get(0)?,
                    json!({
                        "code":r.get::<String>(1)?, "path":r.get::<Option<String>>(2)?,
                        "ordinal":r.get::<Option<String>>(3)?, "detail":r.get::<Option<String>>(4)?,
                        "rootId":r.get::<String>(5)?, "tool":r.get::<Option<String>>(6)?,
                        "isNew":r.get::<i64>(7)?!=0
                    }),
                ))
            },
        )
    })?)
}

pub(super) fn source(vault: &Vault, run: i64, root: String, row: &mut Value) -> Result<()> {
    let groups = vault.store().read(Db::AgentHistoryEvidence, move |s| {
        s.query(
            "SELECT code,SUM(seen),SUM(fresh) FROM ah_run_note_counts
         WHERE run_id=?1 AND root_id=?2 GROUP BY code ORDER BY code",
            params![run, root],
            |r| Ok((r.get::<String>(0)?, r.get::<i64>(1)?, r.get::<i64>(2)?)),
        )
    })?;
    row["noteCount"] = json!(groups.iter().map(|(_, n, _)| n).sum::<i64>().to_string());
    row["newNoteCount"] = json!(groups.iter().map(|(_, _, n)| n).sum::<i64>().to_string());
    row["notes"]=json!(groups.into_iter().map(|(code,n,new)|json!({"code":code,"count":n.to_string(),"newCount":new.to_string()})).collect::<Vec<_>>());
    Ok(())
}
