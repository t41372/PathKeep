//! Read the same project cards for the catalogue, picker and add-folder dialog. A complete
//! card set supports client-locale sorting. Recent pages can use a stable activity/id cursor.
use super::{
    Archive, Result,
    rows::{Card, Snapshot},
};
use crate::{Error, Harness};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use std::collections::BTreeSet;

#[derive(Clone, Default, Deserialize, Serialize)]
#[serde(default, deny_unknown_fields, rename_all = "camelCase")]
pub struct ProjectQuery {
    pub text: String,
    pub harnesses: Vec<Harness>,
    pub status: Status,
    pub time_zone: Option<String>,
    pub other_only: bool,
    pub candidate_for: Option<String>,
    pub limit: Option<usize>,
    pub cursor: Option<String>,
    pub folder_tree: bool,
}
#[derive(Clone, Copy, Default, Deserialize, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub enum Status {
    #[default]
    All,
    Recent,
    Gone,
}
#[derive(Serialize, Deserialize)]
struct Cursor {
    scope: String,
    activity: Option<i64>,
    id: String,
}
impl Archive {
    pub fn project_page(&self, q: &ProjectQuery) -> Result<Value> {
        let _job = self.jobs.enter(None)?;
        let s = self.project_snapshot()?;
        if let Some(id) = &q.candidate_for
            && s.card(id).is_none_or(|p| p.kind == "scratch")
        {
            return Err(Error::InvalidInput("ineligible destination".into()));
        }
        if q.limit.is_some_and(|n| !(1..=500).contains(&n)) {
            return Err(Error::InvalidInput("project limit must be 1..500".into()));
        }
        let mut shape = q.clone();
        shape.cursor = None;
        shape.limit = None;
        let scope = hex::encode(Sha256::digest(serde_json::to_vec(&shape)?));
        let cursor = q
            .cursor
            .as_ref()
            .map(|v| {
                hex::decode(v)
                    .ok()
                    .and_then(|b| serde_json::from_slice::<Cursor>(&b).ok())
                    .filter(|c| c.scope == scope)
                    .ok_or_else(|| Error::InvalidInput("invalid project cursor".into()))
            })
            .transpose()?;
        let needle = q.text.trim().to_lowercase();
        let pins: BTreeSet<_> = s.arrangement.iter().filter_map(|p| p["id"].as_str()).collect();
        let recent = (q.status == Status::Recent)
            .then(|| crate::chats::list::recent_days(q.time_zone.clone()))
            .transpose()?;
        let matches: Vec<_> = s
            .cards
            .iter()
            .filter(|p| {
                let id = p.id.as_str();
                if q.other_only && pins.contains(id) {
                    return false;
                }
                if let Some(target) = &q.candidate_for
                    && (target == id || p.kind == "scratch" || p.has_added_folders)
                {
                    return false;
                }
                if q.status == Status::Gone && !p.gone {
                    return false;
                }
                if recent
                    .is_some_and(|(lo, hi)| p.last_activity.is_none_or(|t| !(lo..hi).contains(&t)))
                {
                    return false;
                }
                if !q.harnesses.is_empty()
                    && !q.harnesses.iter().any(|h| p.harnesses.contains(h.as_str()))
                {
                    return false;
                }
                needle.is_empty()
                    || std::iter::once(&p.display_name)
                        .chain(&p.folder_paths)
                        .chain(&p.remotes)
                        .any(|v| v.to_lowercase().contains(&needle))
            })
            .collect();
        let start = cursor.as_ref().map_or(0, |c| {
            matches.partition_point(|p| {
                let at = p.last_activity;
                at > c.activity || (at == c.activity && p.id.as_str() <= c.id.as_str())
            })
        });
        let end = start + q.limit.unwrap_or(matches.len()).min(matches.len() - start);
        let next = (end < matches.len()).then(|| {
            let p = matches[end - 1];
            hex::encode(
                serde_json::to_vec(&Cursor {
                    scope,
                    activity: p.last_activity,
                    id: p.id.as_str().into(),
                })
                .expect("scalar cursor"),
            )
        });
        let tree = q.folder_tree.then(|| super::tree::groups(&matches, s.home.as_deref()));
        Ok(response(&s, &matches[start..end], matches.len(), next, tree))
    }
}
fn response(
    s: &Snapshot,
    items: &[&Card],
    matching: usize,
    next: Option<String>,
    tree: Option<Value>,
) -> Value {
    json!({"version":s.version,"viewToken":s.view,"total":s.cards.len(),"matching":matching,"items":items,"nextCursor":next,
        "arrangement":s.arrangement,"homeFolder":s.home,"tree":tree,"pendingSuggestions":s.suggestions.iter().filter(|s|s["decision"]=="pending").count()})
}
