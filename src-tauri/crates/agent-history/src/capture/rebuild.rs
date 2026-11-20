//! Rebuild an empty model from retained evidence, including already acknowledged marks.
//! Installation, source authorization and filesystem replacement belong to the data owner.
use super::{BackupOptions, check_cancel};
use crate::{Archive, ArtifactKind, Error, Result};
use vault_store::Db;

pub(super) fn budgets(
    archive: &Archive,
    revision: i64,
    options: &BackupOptions,
) -> Result<BackupOptions> {
    let mut record = options.max_record_bytes;
    let mut decoded = options.max_decoded_bytes;
    let mut after = String::new();
    loop {
        check_cancel(options)?;
        let cursor = after.clone();
        let versions=archive.store.read(Db::AgentHistoryEvidence,move |s|s.query(
            "SELECT parser_version FROM ah_parse_reports WHERE revision_id=?1 AND parser_version>?2 ORDER BY parser_version LIMIT 32",
            vault_store::params![revision,cursor],|r|Ok(r.get::<String>(0)?)))?;
        let Some(last) = versions.last() else { break };
        after = last.clone();
        for version in versions {
            if let Some((_, limits)) = version.split_once(":record=")
                && let Some((r, d)) = limits.split_once(":decoded=")
                && let (Ok(r), Ok(d)) = (r.parse::<usize>(), d.parse::<u64>())
            {
                if r > 64 * 1024 * 1024 {
                    return Err(Error::Integrity("invalid retained parser budget".into()));
                }
                record = record.max(r);
                decoded = decoded.max(d);
            }
        }
    }
    Ok(BackupOptions {
        max_record_bytes: record,
        max_decoded_bytes: decoded,
        cancel: options.cancel.clone(),
        read_git: false,
        ..Default::default()
    })
}

impl Archive {
    /// Reconstructs a new read model offline using a disposable working copy of evidence.
    /// The caller keeps the original evidence, including its run ledger and capture grants.
    pub fn rebuild_from_retained_evidence(&self, options: &BackupOptions) -> Result<()> {
        self.rebuild_model(options, false)
    }

    /// A damaged ledger can supply a revision hint but cannot veto a verified
    /// transcript. Record identities from the new projection in the salvaged ledger.
    pub fn rebuild_salvaged_model(&self, options: &BackupOptions) -> Result<()> {
        self.rebuild_model(options, true)
    }

    fn rebuild_model(&self, options: &BackupOptions, salvage: bool) -> Result<()> {
        use vault_store::{Db, OptionalRow, Priority, params};
        if self.projection_generation()? != 0 {
            return Err(Error::InvalidInput("cold rebuild requires an empty projection".into()));
        }
        let mut query = crate::query::SourceQuery {
            kind: Some(ArtifactKind::Transcript),
            ..Default::default()
        };
        loop {
            let page = self.list_sources(&query)?;
            for source in page.items {
                let harness = match source.harness.parse() {
                    Ok(h) => h,
                    Err(_) if salvage => continue,
                    Err(e) => return Err(e),
                };
                if source.latest_revision_id.is_some()
                    && !self.retired_fallback(&source)?
                    && crate::adapters::adapter(harness).publishes_transcript(&source.relative_path)
                {
                    // A capture can commit a raw head without publishing its new interpretation.
                    // Use the published revision the canonical ledger identifies. Only this
                    // disposable working copy ever changes which revision is presented as head.
                    let id = source.id.clone();
                    let published=self.store.read(Db::AgentHistoryEvidence,move |s|
                        s.query_one("SELECT revision_id,conversation_id FROM ah_source_conversations WHERE source_id=?1",
                            [id],|r|Ok((r.get::<i64>(0)?,r.get::<String>(1)?))).optional())?;
                    if let Some((revision, _)) = &published
                        && Some(*revision) != source.latest_revision_id
                    {
                        let (id, revision) = (source.id.clone(), *revision);
                        self.store.write(Db::AgentHistoryEvidence, Priority::Batch, move |s| {
                            s.execute(
                                "UPDATE ah_sources SET latest_revision_id=?1 WHERE id=?2",
                                params![revision, id],
                            )?;
                            Ok(())
                        })?;
                    }
                    match self.reinterpret(&source.id, options, true) {
                        Err(Error::Unsupported(_) | Error::LimitExceeded(_)) if salvage => {}
                        result => {
                            result?;
                        }
                    }
                    if salvage {
                        if published
                            .as_ref()
                            .is_some_and(|(id, _)| Some(*id) != source.latest_revision_id)
                        {
                            self.set_coverage(&source.id, false)?;
                        }
                        self.record_salvaged_projection(source.id, source.latest_revision_id)?;
                    } else if let Some((revision, expected)) = published {
                        if Some(revision) != source.latest_revision_id {
                            self.set_coverage(&source.id, false)?;
                        }
                        let id = source.id;
                        let actual=self.store.read(Db::AgentHistoryReadModel,move |s|
                            s.query_one("SELECT conversation_id FROM ah_sessions WHERE id=?1 AND revision_id=?2",
                                params![id,revision],|r|Ok(r.get::<String>(0)?)).optional())?;
                        if actual.as_ref() != Some(&expected) {
                            return Err(Error::Integrity(
                                "rebuilt source did not reproduce its recorded conversation".into(),
                            ));
                        }
                    }
                }
            }
            let Some(next) = page.next_cursor else { break };
            query.after = Some(next);
        }
        // Ordinary attachment only replays pending edits. A new model needs every
        // canonical mark, including acknowledged edits and marks on trashed sessions.
        let mut after = String::new();
        loop {
            check_cancel(options)?;
            let cursor = after.clone();
            let rows = self.store.read(Db::AgentHistoryEvidence,move |s| s.query(
                "SELECT conversation_id,starred,put_away,trashed_at,title FROM ah_user_state WHERE conversation_id>?1 ORDER BY conversation_id LIMIT 128",
                [cursor], |r| Ok((r.get::<String>(0)?,r.get::<i64>(1)?,r.get::<i64>(2)?,r.get::<Option<i64>>(3)?,r.get::<Option<String>>(4)?))))?;
            let Some(last) = rows.last() else { break };
            after = last.0.clone();
            self.store.write(Db::AgentHistoryReadModel, Priority::Batch, move |s| {
                for (id, star, away, trash, title) in rows {
                    s.execute(
                        "INSERT OR REPLACE INTO ah_user_state VALUES(?1,?2,?3,?4,?5)",
                        params![id, star, away, trash, title],
                    )?;
                }
                Ok(())
            })?;
        }
        self.ensure_projects()?;
        self.refresh_project_rollups()?;
        self.refresh_insight_hours(&options.cancel)?;
        self.synchronize_search()?;
        Ok(())
    }
    fn record_salvaged_projection(&self, id: String, head: Option<i64>) -> Result<()> {
        use vault_store::{OptionalRow, Priority, params};
        let source = id.clone();
        let parsed=self.store.read(Db::AgentHistoryReadModel,move|s|s.query_one(
            "SELECT conversation_id,revision_id,parent_session_id IS NOT NULL OR parent_path IS NOT NULL
             FROM ah_sessions WHERE id=?1 AND native_id<>''",[source],
            |r|Ok((r.get::<String>(0)?,r.get::<i64>(1)?,r.get::<i64>(2)?))
        ).optional())?;
        self.store.write(Db::AgentHistoryEvidence,Priority::Batch,move|s|{
            // Interpreting the previously published revision never changes which
            // verified raw revision was observed last.
            s.execute("UPDATE ah_sources SET latest_revision_id=?1 WHERE id=?2",params![head,id.clone()])?;
            s.execute("DELETE FROM ah_source_conversations WHERE source_id=?1",[id.as_str()])?;
            if let Some((conversation,revision,child))=parsed{
                s.execute("INSERT INTO ah_source_conversations(source_id,conversation_id,revision_id,is_child) VALUES(?1,?2,?3,?4)",params![id,conversation,revision,child])?;
            }
            Ok(())
        })?;
        Ok(())
    }
}
