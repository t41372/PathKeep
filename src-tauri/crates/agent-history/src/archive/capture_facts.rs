//! Record committed file and conversation units. File facts share the raw-head
//! transaction. Identity becomes countable only after a native parse publishes.
use crate::{Archive, Result};
use vault_store::{Db, OptionalRow, Priority, Session, params};

pub(crate) fn file_commit(
    tx: &Session<'_>,
    run: i64,
    source: &str,
    revision: i64,
    bytes: u64,
    unchanged: bool,
) -> vault_store::Result<()> {
    let prior = tx
        .query_one(
            "SELECT outcome,bytes FROM ah_run_files WHERE run_id=?1 AND source_id=?2",
            params![run, source],
            |r| Ok((r.get::<String>(0)?, r.get::<i64>(1)?)),
        )
        .optional()?;
    if prior.as_ref().is_none_or(|(state, _)| state == "visited") {
        tally(tx, run, source, if unchanged { "filesUnchanged" } else { "filesCaptured" }, 1)?;
        let kind: String =
            tx.query_one("SELECT kind FROM ah_sources WHERE id=?1", [source], |r| Ok(r.get(0)?))?;
        if kind == "transcript" {
            tally(tx, run, source, "unclassified", 1)?;
        }
        if !unchanged {
            tally(tx, run, source, "sourceBytes", bytes as i64)?;
            if kind == "attachment" {
                tally(tx, run, source, "attachmentsChanged", 1)?;
            }
        }
    } else if let Some((state, previous_bytes)) = &prior
        && !unchanged
    {
        if state == "unchanged" {
            tally(tx, run, source, "filesCaptured", 1)?;
            tally(tx, run, source, "filesUnchanged", -1)?;
            tally(tx, run, source, "sourceBytes", bytes as i64)?;
        } else {
            tally(tx, run, source, "sourceBytes", bytes as i64 - previous_bytes)?;
        }
    }
    tx.execute("INSERT INTO ah_run_files(run_id,source_id,revision_id,bytes,outcome)
        VALUES(?1,?2,?3,?4,?5) ON CONFLICT(run_id,source_id) DO UPDATE SET
        revision_id=excluded.revision_id,bytes=excluded.bytes,
        outcome=CASE WHEN ah_run_files.outcome='captured' THEN 'captured' ELSE excluded.outcome END",
        params![run,source,revision,bytes as i64,if unchanged {"unchanged"} else {"captured"}])?;
    Ok(())
}

pub(super) fn tally(
    tx: &Session<'_>,
    run: i64,
    source: &str,
    metric: &str,
    delta: i64,
) -> vault_store::Result<()> {
    tx.execute(
        "INSERT INTO ah_run_totals(run_id,harness,root,metric,quantity)
        SELECT ?1,harness,root,?3,?4 FROM ah_sources WHERE id=?2
        ON CONFLICT(run_id,harness,root,metric) DO UPDATE SET quantity=quantity+excluded.quantity",
        params![run, source, metric, delta],
    )?;
    Ok(())
}

impl Archive {
    pub(crate) fn visit_file(&self, run: i64, source: &str) -> Result<()> {
        let source = source.to_owned();
        self.store.write(Db::AgentHistoryEvidence,Priority::Batch,move |tx|{
            let added=tx.execute("INSERT OR IGNORE INTO ah_run_files(run_id,source_id,bytes,outcome) VALUES(?1,?2,0,'visited')",params![run,source.clone()])?;
            if added>0{tally(tx,run,&source,"filesVisited",1)?;}
            Ok(())
        })?;
        Ok(())
    }
    pub(crate) fn file_problem(&self, run: i64, source: &str, metric: &str) -> Result<()> {
        let (source, metric) = (source.to_owned(), metric.to_owned());
        self.store.write(Db::AgentHistoryEvidence,Priority::Batch,move |tx|{
            let added=tx.execute("INSERT OR IGNORE INTO ah_run_file_problems(run_id,source_id,metric) VALUES(?1,?2,?3)",params![run,source.clone(),metric.clone()])?;
            if added>0{tally(tx,run,&source,&metric,1)?;}
            Ok(())
        })?;
        Ok(())
    }

    /// Retain an observation even when a reviewed directory could not be opened.
    pub fn root_outcome(
        &self,
        run: i64,
        id: &str,
        source: &crate::SourceRoot,
        state: &str,
    ) -> Result<()> {
        let (id, state) = (id.to_owned(), state.to_owned());
        let (tool, path) = (source.harness.to_string(), source.root.to_string_lossy().into_owned());
        self.store.write(Db::AgentHistoryEvidence,Priority::Batch,move |s|{
            s.execute("INSERT OR REPLACE INTO ah_run_roots(run_id,root_id,tool,state) VALUES(?1,?2,?3,?4)",params![run,id,tool.clone(),state.clone()])?;
            s.execute("INSERT OR REPLACE INTO ah_root_state(harness,root,state,run_id) VALUES(?1,?2,?3,?4)",params![tool,path,state,run])?;
            Ok(())
        })?;
        Ok(())
    }
    pub(crate) fn unchanged_file(
        &self,
        run: i64,
        source: &str,
        revision: i64,
        bytes: u64,
    ) -> Result<()> {
        let source = source.to_owned();
        Ok(self.store.write(Db::AgentHistoryEvidence, Priority::Batch, move |tx| {
            file_commit(tx, run, &source, revision, bytes, true)
        })?)
    }
    /// An archive from before desktop receipts can already serve this identity.
    pub(crate) fn published_identity(&self, source: &str) -> Result<Option<String>> {
        let source = source.to_owned();
        Ok(self.store.read(Db::AgentHistoryReadModel, move |s| {
            s.query_one(
                "SELECT conversation_id FROM ah_sessions WHERE id=?1 AND native_id<>''",
                [source],
                |r| Ok(r.get::<String>(0)?),
            )
            .optional()
        })?)
    }
    pub(crate) fn observed_identity(
        &self,
        run: i64,
        source: &str,
        revision: i64,
        changed: bool,
        prior: Option<String>,
    ) -> Result<Option<&'static str>> {
        let src = source.to_owned();
        let identity = self.store.read(Db::AgentHistoryReadModel, move |tx| {
            tx.query_one(
                "SELECT conversation_id,parent_session_id IS NOT NULL OR parent_path IS NOT NULL
            FROM ah_sessions WHERE id=?1 AND revision_id=?2 AND native_id<>''",
                params![src, revision],
                |r| Ok((r.get::<String>(0)?, r.get::<i64>(1)? != 0)),
            )
            .optional()
        })?;
        let Some((id, child)) = identity else { return Ok(None) };
        let source = source.to_owned();
        self.store.write(Db::AgentHistoryEvidence,Priority::Batch,move |tx|{
            if tx.execute("UPDATE ah_run_files SET classified=1 WHERE run_id=?1 AND source_id=?2 AND classified=0",params![run,source.clone()])?>0 {
                tally(tx,run,&source,"unclassified",-1)?;
            }
            let known=prior.as_deref()==Some(&id)||tx.query_one("SELECT EXISTS(SELECT 1 FROM ah_source_conversations WHERE conversation_id=?1)",
                [id.as_str()],|r|Ok(r.get::<i64>(0)?!=0))?;
            tx.execute("INSERT INTO ah_source_conversations(source_id,conversation_id,is_child,revision_id) VALUES(?1,?2,?3,?4)
                ON CONFLICT(source_id) DO UPDATE SET conversation_id=excluded.conversation_id,is_child=excluded.is_child,revision_id=excluded.revision_id",
                params![source.clone(),id.clone(),i64::from(child),revision])?;
            let novel_digest = tx.execute(
                "INSERT OR IGNORE INTO ah_conversation_digests(conversation_id,digest)
                 SELECT ?1,digest FROM ah_revisions WHERE id=?2",params![id.clone(),revision])?>0;
            let outcome=match (known,changed && novel_digest,child) {
                (false,_,true)=>"new-child",
                (false,_,false)=>"new-root",
                (true,true,false)=>"updated-root",
                _=>return Ok(None)
            };
            let inserted=tx.execute("INSERT OR IGNORE INTO ah_run_conversations(run_id,conversation_id,source_id,outcome) VALUES(?1,?2,?3,?4)",
                params![run,id,source.clone(),outcome])?;
            if inserted>0{tally(tx,run,&source,match outcome{"new-root"=>"newRoots","new-child"=>"newChildren",_=>"updatedRoots"},1)?;}
            Ok((inserted>0).then_some(outcome))
        }).map_err(Into::into)
    }
}
