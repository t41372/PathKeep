//! Responsibilities: append source sightings and changed folder facts beside raw evidence.
//! Not responsible for: source discovery, git reads or inferring deletion from unreadable roots.
use super::Archive;
use crate::{Result, SourceRoot};
use serde::Serialize;
use vault_store::{Db, OptionalRow, Priority, params};

/// Folder history remains readable after its native working directory disappears.
#[derive(Debug, Serialize)]
pub struct FolderObservation {
    pub path: String,
    pub exists: bool,
    pub observed_at: i64,
    pub git: Option<crate::capture::git::GitObservation>,
}

impl Archive {
    pub(crate) fn observe_source(&self, run: i64, id: &str, seen: bool) -> Result<()> {
        let source = id.to_owned();
        let id = id.to_owned();
        let now = chrono::Utc::now().timestamp_millis();
        let changed = self.store.write(Db::AgentHistoryEvidence, Priority::Batch, move |tx| {
            let missing = tx.query_one(
                "SELECT missing_since IS NOT NULL FROM ah_sources WHERE id=?1",
                [id.as_str()],
                |r| Ok(r.get::<i64>(0)? != 0),
            )?;
            tx.execute(
                "
                INSERT OR IGNORE INTO ah_source_observations(run_id, source_id, seen, observed_at)
                VALUES(?1, ?2, ?3, ?4)",
                params![run, id.clone(), i64::from(seen), now],
            )?;
            if seen {
                tx.execute(
                    "
                UPDATE ah_sources SET last_seen_at=?1, missing_since=NULL WHERE id=?2",
                    params![now, id],
                )?;
            } else {
                tx.execute(
                    "
                UPDATE ah_sources SET missing_since=coalesce(missing_since, ?1) WHERE id=?2",
                    params![now, id],
                )?;
            }
            Ok(missing == seen)
        })?;
        if changed {
            let _gate = self
                .publication_gate
                .lock()
                .map_err(|_| crate::Error::Integrity("publication lock".into()))?;
            self.store.write(Db::AgentHistoryReadModel, Priority::Batch, move |tx| {
                crate::read_model::publication::refresh(tx, &source, None)
            })?;
            self.flush_reader_heads()?;
        }
        Ok(())
    }

    pub(crate) fn observe_missing(&self, run: i64, root: &SourceRoot) -> Result<()> {
        let mut after = String::new();
        loop {
            let key = after.clone();
            let path = root.root.to_string_lossy().into_owned();
            let harness = root.harness.to_string();
            let page = self.store.read(Db::AgentHistoryEvidence, move |tx| {
                tx.query(
                    "
                SELECT id, relative_path FROM ah_sources WHERE root=?1 AND harness=?2 AND id>?3
                ORDER BY id LIMIT 128",
                    params![path, harness, key],
                    |r| Ok((r.get::<String>(0)?, r.get::<String>(1)?)),
                )
            })?;
            if page.is_empty() {
                break;
            }
            for (id, relative) in page {
                let seen = self.store.read(Db::AgentHistoryEvidence, {
                    let id = id.clone();
                    move |tx| {
                        tx.query_one(
                            "
                    SELECT EXISTS(SELECT 1 FROM ah_source_observations WHERE run_id=?1 AND
                    source_id=?2)",
                            params![run, id],
                            |r| Ok(r.get::<i64>(0)? != 0),
                        )
                    }
                })?;
                if !seen {
                    let path = if root.root.is_file() {
                        root.root.clone()
                    } else {
                        root.root.join(relative)
                    };
                    match std::fs::symlink_metadata(path) {
                        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                            self.observe_source(run, &id, false)?
                        }
                        Ok(_) => self.observe_source(run, &id, true)?,
                        _ => {}
                    }
                }
                after = id;
            }
        }
        Ok(())
    }

    pub(crate) fn observe_folder(
        &self,
        path: &str,
        exists: bool,
        git: Option<crate::capture::git::GitObservation>,
    ) -> Result<()> {
        let path = path.to_owned();
        let git = git.map(|g| serde_json::to_string(&g)).transpose()?;
        let now = chrono::Utc::now().timestamp_millis();
        self.store.write(Db::AgentHistoryEvidence, Priority::Batch, move |tx| {
            let previous = tx
                .query_one(
                    "
                SELECT exists_on_disk, git_json FROM ah_folder_observations WHERE path=?1 ORDER BY
                id DESC LIMIT 1",
                    [path.as_str()],
                    |r| Ok((r.get::<i64>(0)? != 0, r.get::<Option<String>>(1)?)),
                )
                .optional()?;
            if previous != Some((exists, git.clone())) {
                tx.execute(
                    "
                INSERT INTO ah_folder_observations(path, exists_on_disk, observed_at, git_json)
                VALUES(?1, ?2, ?3, ?4)",
                    params![path, i64::from(exists), now, git],
                )?;
            }
            Ok(())
        })?;
        Ok(())
    }

    /// Provides bounded pages of irreplaceable folder observations, newest first.
    pub fn folders(
        &self,
        path: Option<&str>,
        before: Option<i64>,
        limit: usize,
    ) -> Result<crate::query::Page<FolderObservation>> {
        let _job = self.jobs.enter(None)?;
        let count = crate::query::page_limit(limit)?;
        let path = path.map(str::to_owned);
        let mut rows = self.store.read(Db::AgentHistoryEvidence, move |tx| {
            tx.query(
                "
            SELECT id, path, exists_on_disk, observed_at, git_json FROM ah_folder_observations
            WHERE (?1 IS NULL OR path=?1) AND id<?2 ORDER BY id DESC LIMIT ?3",
                params![path, before.unwrap_or(i64::MAX), count + 1],
                |r| {
                    Ok((
                        r.get::<i64>(0)?,
                        FolderObservation {
                            path: r.get(1)?,
                            exists: r.get::<i64>(2)? != 0,
                            observed_at: r.get(3)?,
                            git: r
                                .get::<Option<String>>(4)?
                                .map(|s| serde_json::from_str(&s))
                                .transpose()
                                .map_err(crate::query::operation)?,
                        },
                    ))
                },
            )
        })?;
        let more = rows.len() > limit;
        rows.truncate(limit);
        let next_cursor = more.then(|| rows.last().unwrap().0.to_string());
        Ok(crate::query::Page {
            items: rows.into_iter().map(|(_, item)| item).collect(),
            next_cursor,
        })
    }
}
