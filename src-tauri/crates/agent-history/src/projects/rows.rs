//! Reduce published roots by folder and tool. No outline, turn or transcript reads are needed.
use super::{
    Choices, D, Directory,
    grouping::{name, parent},
};
use crate::{Archive, Result};
use serde::Serialize;
use serde_json::Value;
use std::collections::{BTreeMap, BTreeSet};
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct Card {
    pub id: String,
    pub display_name: String,
    pub default_name: String,
    pub kind: &'static str,
    pub main_folder: Option<String>,
    pub parent_folder: Option<String>,
    pub folder_paths: Vec<String>,
    pub main_exists: Option<bool>,
    pub folder_count: usize,
    pub gone: bool,
    pub name_is_shared: bool,
    pub remotes: BTreeSet<String>,
    pub harnesses: BTreeSet<String>,
    pub conversation_count: i64,
    pub subagent_count: i64,
    pub summed_agent_ms: Option<i64>,
    pub tokens_total: Option<i64>,
    pub tokens_known_conversations: i64,
    pub first_activity: Option<i64>,
    pub last_activity: Option<i64>,
    pub latest_conversation_title: Option<String>,
    pub has_added_folders: bool,
}
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct Location {
    pub path: String,
    pub role: &'static str,
    pub reason: &'static str,
    pub rule_owner: &'static str,
    pub exists: Option<bool>,
    pub branch: Option<String>,
    pub remotes: Vec<String>,
    pub harnesses: BTreeSet<String>,
    pub conversation_count: i64,
    pub original_project_id: String,
    pub auto_project_id: String,
    added_from_project: Option<Origin>,
}
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct Origin {
    id: String,
    display_name: String,
    default_name: String,
}
pub(super) struct Snapshot {
    pub version: i64,
    pub home: Option<String>,
    pub view: String,
    pub cards: Vec<Card>,
    pub folders: BTreeMap<String, Vec<Location>>,
    pub absorbed: BTreeMap<String, String>,
    pub arrangement: Vec<Value>,
    pub suggestions: Vec<Value>,
    indices: BTreeMap<String, usize>,
}
impl Snapshot {
    pub fn card(&self, id: &str) -> Option<&Card> {
        self.indices.get(id).map(|i| &self.cards[*i])
    }
}
#[derive(Default)]
struct Stats {
    count: i64,
    children: i64,
    ms: Option<i64>,
    tokens: Option<i64>,
    known: i64,
    first: Option<i64>,
    last: Option<i64>,
    harnesses: BTreeSet<String>,
    latest: Option<(String, Option<String>)>,
}
fn sum(total: &mut Option<i64>, n: Option<i64>) {
    if let Some(n) = n {
        *total = Some(total.unwrap_or_default() + n)
    }
}
impl Archive {
    pub(super) fn project_rows(
        &self,
        dir: &Directory,
        choices: &Choices,
        version: i64,
        view: (i64, i64),
        home: Option<String>,
    ) -> Result<Snapshot> {
        let groups = self.store.read(D, |s| {
            s.query(
                "SELECT b.path,b.harness,b.roots,b.children,b.ms,b.tokens,b.known,b.first_at,b.last_at,b.latest,b.title FROM ah_project_rollup b",
                (),
                |r| {
                    Ok((
                        r.get::<String>(0)?,
                        r.get::<String>(1)?,
                        r.get::<i64>(2)?,
                        r.get::<i64>(3)?,
                        r.get::<Option<i64>>(4)?,
                        r.get::<Option<i64>>(5)?,
                        r.get::<i64>(6)?,
                        r.get::<Option<i64>>(7)?,
                        r.get::<Option<i64>>(8)?,
                        r.get::<String>(9)?,
                        r.get::<Option<String>>(10)?,
                    ))
                },
            )
        })?;
        let mut stats = BTreeMap::<String, Stats>::new();
        let mut counts = BTreeMap::<String, i64>::new();
        let mut tools = BTreeMap::<String, BTreeSet<String>>::new();
        for (path, h, n, ch, ms, tokens, known, first, last, key, title) in groups {
            let Some(f) = dir.folders.get(&path) else { continue };
            let row = stats.entry(f.project.clone()).or_default();
            row.count += n;
            row.children += ch;
            row.known += known;
            sum(&mut row.ms, ms);
            sum(&mut row.tokens, tokens);
            row.first = row.first.into_iter().chain(first).min();
            row.last = row.last.max(last);
            if row.latest.as_ref().is_none_or(|(k, _)| key > *k) {
                row.latest = Some((key, title));
            }
            row.harnesses.insert(h.clone());
            tools.entry(path.clone()).or_default().insert(h);
            *counts.entry(path).or_default() += n;
        }
        let mut folders = BTreeMap::<String, Vec<Location>>::new();
        let mut absorbed = BTreeMap::new();
        for f in dir.folders.values() {
            if f.own != f.project {
                absorbed.insert(f.own.clone(), f.project.clone());
            }
            let git = f.observation.git.as_ref();
            let rule = choices.rules.get(&super::grouping::key(&f.path));
            let added_from_project = rule.and_then(|r| r.added_from.as_ref()).map(|id| Origin {
                id: id.clone(),
                display_name: dir.labels[id].clone(),
                default_name: name(&dir.mains[id]).into(),
            });
            folders.entry(f.project.clone()).or_default().push(Location {
                path: f.path.clone(),
                role: f.role,
                reason: f.reason,
                rule_owner: if rule.is_some() { "user" } else { "pathkeep" },
                exists: f.observation.exists,
                branch: git.and_then(|g| g.branch.clone()),
                remotes: git.map(|g| g.remotes.clone()).unwrap_or_default(),
                harnesses: tools.remove(&f.path).unwrap_or_default(),
                conversation_count: counts.get(&f.path).copied().unwrap_or(0),
                original_project_id: f.own.clone(),
                auto_project_id: f.natural.clone(),
                added_from_project,
            });
        }
        let mut names = BTreeMap::new();
        for id in stats.keys() {
            *names.entry(&dir.labels[id]).or_insert(0) += 1;
        }
        let mut cards = Vec::new();
        for (id, row) in stats {
            let main = &dir.mains[&id];
            let paths = folders.get_mut(&id).expect("counted folder");
            paths.sort_by(|a, b| (a.path != *main, &a.path).cmp(&(b.path != *main, &b.path)));
            cards.push(Card {
                display_name: dir.labels[&id].clone(),
                default_name: name(main).into(),
                kind: dir.kinds[&id],
                main_folder: (!main.is_empty()).then(|| main.clone()),
                parent_folder: parent(main).map(str::to_owned),
                folder_paths: paths.iter().map(|f| f.path.clone()).collect(),
                main_exists: paths.iter().find(|f| f.path == *main).and_then(|f| f.exists),
                folder_count: paths.len(),
                gone: paths.iter().all(|f| f.exists == Some(false)),
                name_is_shared: names[&dir.labels[&id]] > 1,
                remotes: paths.iter().flat_map(|f| f.remotes.iter().cloned()).collect(),
                harnesses: row.harnesses,
                conversation_count: row.count,
                subagent_count: row.children,
                summed_agent_ms: row.ms,
                tokens_total: row.tokens,
                tokens_known_conversations: row.known,
                first_activity: row.first,
                last_activity: row.last,
                latest_conversation_title: row.latest.and_then(|(_, t)| t),
                has_added_folders: paths.iter().any(|f| f.role == "userAdded"),
                id,
            });
        }
        cards.sort_by(|a, b| b.last_activity.cmp(&a.last_activity).then_with(|| a.id.cmp(&b.id)));
        let indices = cards.iter().enumerate().map(|(i, c)| (c.id.clone(), i)).collect();
        let suggestions = super::suggestions::build(&cards, &folders, choices);
        let arrangement = super::edit::arrangement(&choices.nodes, &cards);
        Ok(Snapshot {
            home,
            version,
            view: format!("{}:{}:{version}", view.0, view.1),
            cards,
            indices,
            folders,
            absorbed,
            arrangement,
            suggestions,
        })
    }
}
