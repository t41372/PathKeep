//! Translate screen anchors to ranks using the same publication and selection as the window.
use super::*;
use crate::{Archive, Error, Result};
use vault_store::{OptionalRow, params};
impl Archive {
    pub(crate) fn screen_length(
        &self,
        h: &ReaderPublication,
        branch: &str,
        leaf: Option<u64>,
        physical: bool,
        job: &JobControl,
    ) -> Result<u64> {
        if let Some(leaf) = leaf {
            let (owner, len) = self.member_version(h, branch)?;
            if leaf >= len {
                return Err(Error::NotFound("leaf ordinal".into()));
            }
            self.leaf_page(&owner, leaf, 0, 1, job)?;
            if !physical {
                return self.leaf_visible_size(&owner, leaf, job);
            }
            return Ok(self.store.read(DB, move |s| {
                s.query_one(
                    "SELECT depth FROM ah_reader_walk WHERE owner=?1 AND leaf=?2",
                    params![owner, leaf as i64],
                    |r| Ok(r.get::<i64>(0)? as u64),
                )
            })?);
        }
        if branch == h.current_source {
            return Ok(if physical { h.physical_records } else { h.visible_records });
        }
        let (publication, branch) = (h.publication.0.clone(), branch.to_owned());
        self.store
            .read(DB, move |s| {
                Ok(manifest::member(s, &publication, &branch)?
                    .map(|m| if physical { m.3 } else { m.7 } as u64))
            })?
            .ok_or_else(|| Error::NotFound("branch".into()))
    }
    pub(crate) fn screen_rank(
        &self,
        h: &ReaderPublication,
        branch: &str,
        leaf: Option<u64>,
        physical: bool,
        source: &str,
        ordinal: u64,
    ) -> Result<u64> {
        let (publication, branch, source) =
            (h.publication.0.clone(), branch.to_owned(), source.to_owned());
        let n = i64::try_from(ordinal)
            .map_err(|_| Error::InvalidInput("ordinal out of range".into()))?;
        self.store.read(DB,move|s|{
            if let Some(leaf)=leaf{
                if !physical {
                    if source!=branch{return Ok(None)}
                    let owner=s.query_one("SELECT version FROM ah_reader_member WHERE publication=?1 AND source=?2",params![publication,branch],|r|Ok(r.get::<String>(0)?))?;
                    let rank=visible::prefix(s,&owner,n)?;
                    if visible::prefix(s,&owner,n+1)?==rank{return Ok(None)}
                    return s.query_one("SELECT rank+?3-start FROM ah_chat_leaf_run WHERE owner=?1 AND leaf=?2 AND start<=?3 AND start+records>?3",params![owner,leaf as i64,rank],|r|Ok(r.get::<i64>(0)? as u64)).optional();
                }
                return s.query_one("SELECT w.depth-r.reverse_rank-r.count+?4-r.first_ordinal FROM ah_reader_member m JOIN ah_reader_walk w ON w.owner=m.version AND w.leaf=?3 JOIN ah_reader_walk_run r ON r.owner=w.owner AND r.leaf=w.leaf WHERE m.publication=?1 AND m.source=?2 AND m.source=?5 AND r.first_ordinal<=?4 AND r.first_ordinal+r.count>?4",params![publication,branch,leaf as i64,n,source],|r|Ok(r.get::<i64>(0)? as u64)).optional();
            }
            let Some((v,base,visible_base))=s.query_one("SELECT version,first_rank,first_visible FROM ah_reader_line WHERE publication=?1 AND branch=?2 AND source=?3 AND records>?4",params![publication,branch,source,n],|r|Ok((r.get::<String>(0)?,r.get::<i64>(1)?,r.get::<i64>(2)?))).optional()? else{return Ok(None)};
            if physical{return Ok(Some((base+n) as u64))}
            let rank=visible::prefix(s,&v,n)?;
            if visible::prefix(s,&v,n+1)?==rank{return Ok(None)}
            if let Some(sel)=leaves::selection(s,&v)?{
                return s.query_one("SELECT ?3-reverse_rank-count+?2-first_ordinal FROM ah_reader_walk_run WHERE owner=?1 AND leaf=-1 AND first_ordinal<=?2 AND first_ordinal+count>?2",params![v,rank,visible_base+sel.visible as i64],|r|Ok(r.get::<i64>(0)? as u64)).optional()
            }
            Ok(Some((visible_base+rank) as u64))
        })?.ok_or_else(||Error::NotFound("record on selected branch".into()))
    }
}

impl Archive {
    /// Add navigation to a raw window without reading a single body. Reuse
    /// decoded rank masks for neighboring records from the same source.
    pub(crate) fn screen_raw_metadata(
        &self,
        h: &ReaderPublication,
        branch: &str,
        leaf: Option<u64>,
        entries: Vec<(SourceId, OutlineEntry)>,
        job: &JobControl,
    ) -> Result<Vec<serde_json::Value>> {
        if leaf.is_some() {
            self.screen_length(h, branch, leaf, false, job)?;
        }
        let (publication, branch) = (h.publication.0.clone(), branch.to_owned());
        Ok(self.store.read(DB,move|s|{
            let mut source_info=std::collections::HashMap::new();
            let mirrors=s.query_one("SELECT EXISTS(SELECT 1 FROM ah_chat_mirror_rank WHERE publication=?1 AND branch=?2)",params![publication.as_str(),branch.as_str()],|r|Ok(r.get::<i64>(0)?!=0))?;
            let key=leaf.map_or_else(||branch.clone(),|n|super::line_facts::leaf_key(&branch,n));
            let extras=super::supplement::added_before_native(s,&publication,&key,i64::MAX)?!=0;
            let mut rows=Vec::with_capacity(entries.len());
            for (source,e) in entries {
                if !source_info.contains_key(&source.0){
                    let (owner,start)=s.query_one("SELECT version,first_visible FROM ah_reader_line WHERE publication=?1 AND branch=?2 AND source=?3 LIMIT 1",params![publication.as_str(),branch.as_str(),source.0.as_str()],|r|Ok((r.get::<String>(0)?,r.get::<i64>(1)?)))?;
                    let selected=leaves::selection(s,&owner)?;
                    source_info.insert(source.0.clone(),(owner,start,selected,visible::RankCursor::default()));
                }
                let (owner,base,selection,mask)=source_info.get_mut(&source.0).unwrap();
                let n=e.locator.ordinal.0;
                let mut displayed=if let Some(rank)=mask.at(s,owner,n)?{
                    if let Some(leaf)=leaf{
                        s.query_one("SELECT rank+?3-start FROM ah_chat_leaf_run WHERE owner=?1 AND leaf=?2 AND start<=?3 AND start+records>?3",params![owner.as_str(),leaf as i64,rank as i64],|r|Ok(r.get::<i64>(0)?)).optional()?
                    }else if let Some(sel)=selection{
                        s.query_one("SELECT ?3-reverse_rank-count+?2-first_ordinal FROM ah_reader_walk_run WHERE owner=?1 AND leaf=-1 AND first_ordinal<=?2 AND first_ordinal+count>?2 ORDER BY first_ordinal DESC LIMIT 1",params![owner.as_str(),rank as i64,*base+sel.visible as i64],|r|Ok(r.get::<i64>(0)?)).optional()?
                    }else{Some(*base+rank as i64)}
                }else{None};
                let mut mirror=None;
                if mirrors && let Some(native)=displayed{
                    let skipped=s.query_one("SELECT skipped FROM ah_chat_mirror_rank WHERE publication=?1 AND branch=?2 AND native_rank<=?3 ORDER BY native_rank DESC LIMIT 1",params![publication.as_str(),branch.as_str(),native],|r|Ok(r.get::<i64>(0)?)).optional()?.unwrap_or(0);
                    displayed=Some(native-skipped);
                    mirror=s.query_one("SELECT original FROM ah_chat_mirror WHERE publication=?1 AND source=?2 AND ordinal=?3",params![publication.as_str(),source.0.as_str(),n as i64],|r|Ok(r.get::<i64>(0)?)).optional()?;
                }
                if extras {
                    displayed=match displayed{
                        Some(rank)=>Some(rank+super::supplement::added_before_native(s,&publication,&key,rank)?),
                        None=>s.query_one("SELECT rank FROM ah_chat_insert_rank WHERE publication=?1 AND branch=?2 AND source=?3 AND ordinal=?4",params![publication.as_str(),key.as_str(),source.0.as_str(),n as i64],|r|Ok(r.get::<i64>(0)?)).optional()?,
                    };
                }
                rows.push(serde_json::json!({"sourceId":source,"ordinal":n,"nativeId":e.id,"parentId":e.parent,"kind":e.kind,"role":e.role,"at":e.timestamp,
                    "bytes":e.locator.length,"visible":displayed.is_some() && mirror.is_none(),"visibleRank":displayed,"mirrorOfOrdinal":mirror,"coverage":e.locator.coverage,"boundary":e.boundary}));
            }
            Ok(rows)
        })?)
    }
}
