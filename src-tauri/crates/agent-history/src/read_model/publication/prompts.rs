//! Build the prompt TOC in selected-line order. Its text is a two-line
//! preview and its rank can be sought directly even at the end of a long chat.
use super::{leaves, visible::RankCursor};
use vault_store::{OptionalRow, Session, params};
pub(crate) fn stage_prompts(s: &Session<'_>, publication: &str) -> vault_store::Result<()> {
    let lines=s.query("SELECT branch,source,version,records,first_visible FROM ah_reader_line WHERE publication=?1 ORDER BY branch,first_rank",
        [publication],|r|Ok((r.get::<String>(0)?,r.get::<String>(1)?,r.get::<String>(2)?,r.get::<i64>(3)?,r.get::<i64>(4)?)))?;
    let mut last = String::new();
    let mut next = 0;
    for (branch, source, version, records, base) in lines {
        if last != branch {
            next = 0;
            last.clone_from(&branch);
        }
        let selected = leaves::selection(s, &version)?;
        let mut rank = RankCursor::default();
        let mut statement=s.prepare("SELECT ordinal,json_extract(facts,'$.prompt') FROM ah_chat_turn WHERE publication=?1 AND source=?2 AND ordinal<?3 AND json_extract(facts,'$.prompt') IS NOT NULL ORDER BY ordinal")?;
        let origin=s.query_one("SELECT coalesce((SELECT ?1 FROM ah_reader_member WHERE publication=?1 AND source=?2),(SELECT c.publication FROM ah_sessions a JOIN ah_chat_summary c ON c.id=a.conversation_id AND c.live=1 WHERE a.id=?2))",params![publication,source.as_str()],|r|Ok(r.get::<Option<String>>(0)?))?;
        let mut rows = statement.query(params![origin, source.as_str(), records])?;
        while let Some(row) = rows.next()? {
            let n: i64 = row.get(0)?;
            let screen_rank = if let Some(offset) = rank.at(s, &version, n as u64)? {
                let visible = if let Some(sel) = &selected {
                    let Some(pos)=s.query_one(
                        "SELECT ?3-reverse_rank-count+?2-first_ordinal FROM ah_reader_walk_run WHERE owner=?1 AND leaf=-1 AND first_ordinal<=?2 AND first_ordinal+count>?2 ORDER BY first_ordinal DESC LIMIT 1",
                        params![version.as_str(),offset as i64,sel.visible as i64],|r|Ok(r.get::<i64>(0)?)).optional()? else{continue};
                    pos
                } else {
                    offset as i64
                };
                let native_rank = base + visible;
                let removed=s.query_one("SELECT skipped FROM ah_chat_mirror_rank WHERE publication=?1 AND branch=?2 AND native_rank<?3 ORDER BY native_rank DESC LIMIT 1",
                    params![publication,branch.as_str(),native_rank],|r|Ok(r.get::<i64>(0)?)).optional()?.unwrap_or(0);
                let rank = native_rank - removed;
                rank + super::supplement::added_before_native(s, publication, &branch, rank)?
            } else {
                let Some(extra)=s.query_one("SELECT rank FROM ah_chat_insert_rank WHERE publication=?1 AND branch=?2 AND source=?3 AND ordinal=?4",params![publication,branch.as_str(),source.as_str(),n],|r|Ok(r.get::<i64>(0)?)).optional()? else{continue};
                extra
            };
            s.execute(
                "INSERT INTO ah_chat_prompt VALUES(?1,?2,?3,?4,?5,?6,?7)",
                params![
                    publication,
                    branch.as_str(),
                    next,
                    source.as_str(),
                    n,
                    screen_rank,
                    row.get::<String>(1)?
                ],
            )?;
            next += 1;
        }
    }
    Ok(())
}
