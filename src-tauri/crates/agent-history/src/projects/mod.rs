//! Project screens share folder identity with Records. Evidence owns IDs and edits; membership
//! and project statistics are rebuilt from observations and published root summaries.
mod edit;
mod grouping;
mod list;
mod tree;
pub use list::{ProjectQuery, Status};
pub(crate) mod rollups;
mod rows;
pub(crate) mod schema;
mod suggestions;
use crate::{Archive, Result, query::operation};
use edit::Choices;
pub use edit::ProjectEdit;
use grouping::Directory;
pub use grouping::Grouping;
use rows::Snapshot;
use serde_json::{Value, json};
use std::sync::Arc;
use vault_store::Db;
const D: Db = Db::AgentHistoryReadModel;
const E: Db = Db::AgentHistoryEvidence;

#[derive(Default)]
pub(crate) struct Cache {
    policy: Grouping,
    stamp: Option<(i64, i64, i64)>,
    directory: Directory,
    choices: Choices,
    rows: Option<((i64, i64), Arc<Snapshot>)>,
}
impl Archive {
    /// Device settings affect automatic assignments. Manual assignments survive a regroup.
    pub fn set_project_grouping(&self, policy: Grouping) {
        let mut c = self.projects.lock().unwrap();
        if c.policy != policy {
            c.policy = policy;
            c.stamp = None;
        }
    }
    pub fn set_project_git(&self, read_git: bool) {
        let mut c = self.projects.lock().unwrap();
        if c.policy.read_git != read_git {
            c.policy.read_git = read_git;
            c.stamp = None;
        }
    }
    fn refresh_directory(&self, c: &mut Cache) -> Result<i64> {
        let (obs, version) = self.store.read(E, |s| {
            s.query_one(
                "SELECT coalesce((SELECT max(id) FROM ah_folder_observations),0),version FROM ah_project_choices WHERE singleton=1",
                (),
                |r| Ok((r.get(0)?, r.get(1)?)),
            )
        })?;
        let paths =
            self.store.read(D, |s| {
                s.query_one("SELECT coalesce(max(rowid),0) FROM ah_project_paths", (), |r| {
                    Ok(r.get(0)?)
                })
            })?;
        if c.stamp != Some((obs, version, paths)) {
            let choices: Choices = self.store.read(E, |s| {
                s.query_one("SELECT choices FROM ah_project_choices WHERE singleton=1", (), |r| {
                    serde_json::from_str(&r.get::<String>(0)?).map_err(operation)
                })
            })?;
            let dir = self.resolve_folders(&c.policy, &choices)?;
            self.publish_folders(&dir)?;
            c.directory = dir;
            c.choices = choices;
            c.stamp = Some((obs, version, paths));
            c.rows = None;
        }
        Ok(version)
    }
    pub(crate) fn ensure_projects(&self) -> Result<()> {
        self.refresh_directory(&mut self.projects.lock().unwrap()).map(drop)
    }
    fn project_snapshot(&self) -> Result<Arc<Snapshot>> {
        self.ensure_state_visible()?;
        let mut c = self.projects.lock().unwrap();
        let version = self.refresh_directory(&mut c)?;
        let view = self.store.read(D, crate::chats::list::view)?;
        if let Some((old, rows)) = &c.rows
            && *old == view
        {
            return Ok(rows.clone());
        }
        self.refresh_project_rollups()?;
        let snapshot = Arc::new(self.project_rows(
            &c.directory,
            &c.choices,
            version,
            view,
            c.policy.home.clone(),
        )?);
        c.rows = Some((view, snapshot.clone()));
        Ok(snapshot)
    }
    /// One row per project gives the picker its entire pool, including archived conversations.
    pub fn project_list(&self) -> Result<Value> {
        self.project_page(&Default::default())
    }
    pub fn project_detail(&self, id: &str) -> Result<Value> {
        let _job = self.jobs.enter(None)?;
        let data = self.project_snapshot()?;
        let Some(card) = data.card(id) else {
            return Ok(match data.absorbed.get(id) {
                Some(target) => json!({"status":"addedInto","into":target,"version":data.version}),
                None => json!({"status":"notFound","version":data.version}),
            });
        };
        Ok(
            json!({"status":"ok","project":card,"folders":data.folders.get(id),"version":data.version,"viewToken":data.view}),
        )
    }
    pub fn project_suggestions(&self, into: Option<&str>) -> Result<Value> {
        let _job = self.jobs.enter(None)?;
        let data = self.project_snapshot()?;
        Ok(
            json!({"version":data.version,"items":data.suggestions.iter().filter(|s|into.is_none_or(|id|s["intoProjectId"]==id)).collect::<Vec<_>>()}),
        )
    }
}
