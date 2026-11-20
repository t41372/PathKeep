//! Viewer choices shared by the Insights wall, project Insights and reader cost display.
use super::Range;
use crate::{Archive, Result, query::operation};
use serde::{Deserialize, Serialize};
use vault_store::{Db, Priority};
#[derive(Clone, Copy, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Equivalent {
    #[default]
    Nonstop,
    Workdays,
}
#[derive(Clone, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct Preferences {
    pub range: Range,
    pub equivalent: Equivalent,
    pub show_cost: bool,
    pub estimate_cost: bool,
}
impl Default for Preferences {
    fn default() -> Self {
        Self {
            range: Range::All,
            equivalent: Default::default(),
            show_cost: true,
            estimate_cost: true,
        }
    }
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct PreferencePatch {
    pub range: Option<Range>,
    pub equivalent: Option<Equivalent>,
    pub show_cost: Option<bool>,
    pub estimate_cost: Option<bool>,
}
fn get(s: &vault_store::Session<'_>) -> vault_store::Result<Preferences> {
    s.query_one("SELECT value FROM ah_insight_preferences WHERE id=1", (), |r| {
        serde_json::from_str(&r.get::<String>(0)?).map_err(operation)
    })
}
impl Archive {
    pub fn insight_preferences(&self) -> Result<Preferences> {
        let _job = self.jobs.enter(None)?;
        Ok(self.store.read(Db::AgentHistoryEvidence, get)?)
    }
    pub fn update_insight_preferences(&self, q: PreferencePatch) -> Result<Preferences> {
        let _job = self.jobs.enter(None)?;
        Ok(self.store.write(Db::AgentHistoryEvidence, Priority::Interactive, move |s| {
            let mut p = get(s)?;
            if let Some(v) = q.range {
                p.range = v
            }
            if let Some(v) = q.equivalent {
                p.equivalent = v
            }
            if let Some(v) = q.show_cost {
                p.show_cost = v
            }
            if let Some(v) = q.estimate_cost {
                p.estimate_cost = v
            }
            s.execute(
                "UPDATE ah_insight_preferences SET value=?1 WHERE id=1",
                [serde_json::to_string(&p).map_err(operation)?],
            )?;
            Ok(p)
        })?)
    }
}
