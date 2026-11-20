//! Version-checked user edits. Project names, per-path overrides, decisions and ordered pins
//! live in evidence. Undo retains the previous edit; suggestion answers can be reopened later.
use super::{
    E,
    rows::{Card, Snapshot},
};
use crate::{Archive, Error, Result, query::operation};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::collections::{BTreeMap, BTreeSet};
use vault_store::{Priority, params};

#[derive(Clone, Default, Serialize, Deserialize)]
#[serde(default)]
pub(super) struct Choices {
    pub names: BTreeMap<String, String>,
    pub rules: BTreeMap<String, Rule>,
    pub nodes: Vec<Node>,
    pub answers: BTreeMap<String, Answer>,
}
#[derive(Clone, Serialize, Deserialize)]
pub(super) struct Rule {
    pub project: String,
    pub added_from: Option<String>,
}
#[derive(Clone, Serialize, Deserialize)]
pub(super) struct Answer {
    pub decision: String,
    pub proposal: Value,
    prior: BTreeMap<String, Option<Rule>>,
}
#[derive(Clone, Serialize, Deserialize)]
pub(super) struct Node {
    id: String,
    parent: Option<String>,
    name: Option<String>,
    collapsed: bool,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ProjectEdit {
    version: i64,
    action: Action,
}
#[derive(Deserialize)]
#[serde(tag = "kind", rename_all = "camelCase", deny_unknown_fields)]
enum Action {
    Rename { id: String, name: String },
    Join { from: String, into: String },
    Detach { id: String, path: String },
    Answer { id: String, decision: Decision },
    CreateSection { parent: Option<String>, name: String },
    RenameSection { id: String, name: String },
    Collapse { id: Option<String>, collapsed: bool },
    DeleteSection { id: String },
    Move { id: String, parent: Option<String>, index: Option<usize> },
    Unpin { id: String },
    Undo,
}
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
enum Decision {
    Added,
    Declined,
    Later,
    Pending,
}

fn invalid(message: &str) -> Error {
    Error::InvalidInput(message.into())
}
fn card<'a>(s: &'a Snapshot, id: &str) -> Result<&'a Card> {
    s.card(id).ok_or_else(|| Error::NotFound("project".into()))
}
fn section<'a>(nodes: &'a mut [Node], id: &str) -> Result<&'a mut Node> {
    nodes
        .iter_mut()
        .find(|n| n.id == id && n.name.is_some())
        .ok_or_else(|| Error::NotFound("section".into()))
}
fn join(
    c: &mut Choices,
    s: &Snapshot,
    from: &str,
    into: &str,
) -> Result<BTreeMap<String, Option<Rule>>> {
    let source = card(s, from)?;
    let target = card(s, into)?;
    if from == into
        || source.kind == "scratch"
        || target.kind == "scratch"
        || source.has_added_folders
    {
        return Err(invalid("ineligible project join"));
    }
    let mut prior = BTreeMap::new();
    for folder in &s.folders[from] {
        let path = super::grouping::key(&folder.path);
        prior.insert(
            path.clone(),
            c.rules.insert(path, Rule { project: into.into(), added_from: Some(from.into()) }),
        );
    }
    Ok(prior)
}
fn place(nodes: &mut Vec<Node>, item: Node, index: Option<usize>, s: &Snapshot) {
    let siblings = nodes
        .iter()
        .enumerate()
        .filter(|(_, n)| n.parent == item.parent && (n.name.is_some() || s.card(&n.id).is_some()));
    let at = index.and_then(|i| siblings.map(|(n, _)| n).nth(i)).unwrap_or(nodes.len());
    nodes.insert(at, item);
}
impl Choices {
    fn apply(&mut self, action: Action, s: &Snapshot, new_id: &str) -> Result<()> {
        match action {
            Action::Rename { id, name } => {
                let p = card(s, &id)?;
                let label = name.trim();
                if label.is_empty() || p.default_name == label {
                    self.names.remove(&id);
                } else {
                    self.names.insert(id, label.into());
                }
            }
            Action::Join { from, into } => {
                join(self, s, &from, &into)?;
            }
            Action::Detach { id, path } => {
                let project = card(s, &id)?;
                if project.main_folder.as_deref() == Some(&path) || project.kind == "scratch" {
                    return Err(invalid("main or loose folder"));
                }
                let f = s.folders[&id]
                    .iter()
                    .find(|f| f.path == path)
                    .ok_or_else(|| invalid("folder does not belong to project"))?;
                self.rules.insert(
                    super::grouping::key(&path),
                    Rule { project: f.original_project_id.clone(), added_from: None },
                );
            }
            Action::Answer { id, decision } => {
                let proposal = s
                    .suggestions
                    .iter()
                    .find(|v| v["id"] == id)
                    .ok_or_else(|| Error::NotFound("suggestion".into()))?
                    .clone();
                let into = proposal["intoProjectId"].as_str().unwrap();
                if let Some(old) = self.answers.remove(&id) {
                    for (path, before) in old.prior {
                        if self.rules.get(&path).is_some_and(|rule| rule.project == into) {
                            if let Some(target) = before {
                                self.rules.insert(path, target);
                            } else {
                                self.rules.remove(&path);
                            }
                        }
                    }
                }
                let (decision, prior) = match decision {
                    Decision::Pending => return Ok(()),
                    Decision::Added => {
                        ("added", join(self, s, proposal["fromProjectId"].as_str().unwrap(), into)?)
                    }
                    Decision::Later => ("later", Default::default()),
                    Decision::Declined => ("declined", Default::default()),
                };
                self.answers.insert(id, Answer { decision: decision.into(), proposal, prior });
            }
            Action::CreateSection { parent, name } => {
                if name.trim().is_empty() {
                    return Err(invalid("empty section name"));
                }
                if let Some(p) = &parent {
                    section(&mut self.nodes, p)?.collapsed = false
                }
                let index = parent.is_none().then_some(0);
                place(
                    &mut self.nodes,
                    Node {
                        id: new_id.into(),
                        parent,
                        name: Some(name.trim().into()),
                        collapsed: false,
                    },
                    index,
                    s,
                );
            }
            Action::RenameSection { id, name } => {
                if name.trim().is_empty() {
                    return Err(invalid("empty section name"));
                }
                section(&mut self.nodes, &id)?.name = Some(name.trim().into());
            }
            Action::Collapse { id, collapsed } => {
                if let Some(id) = id {
                    section(&mut self.nodes, &id)?.collapsed = collapsed
                } else {
                    for n in &mut self.nodes {
                        if n.name.is_some() {
                            n.collapsed = collapsed
                        }
                    }
                }
            }
            Action::DeleteSection { id } => {
                section(&mut self.nodes, &id)?;
                let mut children = BTreeMap::<String, Vec<String>>::new();
                for node in &self.nodes {
                    if let Some(p) = &node.parent {
                        children.entry(p.clone()).or_default().push(node.id.clone());
                    }
                }
                let mut stack = vec![id];
                let mut deleted = BTreeSet::new();
                while let Some(id) = stack.pop() {
                    stack.extend(children.remove(&id).unwrap_or_default());
                    deleted.insert(id);
                }
                self.nodes.retain(|n| !deleted.contains(&n.id));
            }
            Action::Move { id, parent, index } => {
                let mut ancestor = parent.clone();
                while let Some(p) = ancestor {
                    if p == id {
                        return Err(invalid("section cycle"));
                    }
                    ancestor = section(&mut self.nodes, &p)?.parent.clone();
                }
                let mut item = if let Some(n) = self.nodes.iter().position(|n| n.id == id) {
                    self.nodes.remove(n)
                } else {
                    card(s, &id)?;
                    Node { id, parent: None, name: None, collapsed: false }
                };
                item.parent = parent;
                place(&mut self.nodes, item, index, s);
            }
            Action::Unpin { id } => {
                card(s, &id)?;
                self.nodes.retain(|n| n.id != id);
            }
            Action::Undo => unreachable!("undo restores the saved state"),
        }
        Ok(())
    }
}
pub(super) fn arrangement(nodes: &[Node], cards: &[Card]) -> Vec<Value> {
    let projects: BTreeMap<_, _> = cards.iter().map(|p| (p.id.as_str(), p)).collect();
    let parents: BTreeMap<_, _> = nodes
        .iter()
        .filter(|n| n.name.is_some())
        .map(|n| (n.id.as_str(), n.parent.as_deref()))
        .collect();
    let mut sums = BTreeMap::<&str, (usize, i64, Option<i64>)>::new();
    for n in nodes {
        if let Some(card) = projects.get(n.id.as_str()) {
            let mut parent = n.parent.as_deref();
            while let Some(id) = parent {
                let sum = sums.entry(id).or_default();
                sum.0 += 1;
                sum.1 += card.conversation_count;
                sum.2 = sum.2.max(card.last_activity);
                parent = parents.get(id).copied().flatten();
            }
        }
    }
    nodes
        .iter()
        .filter(|n| n.name.is_some() || projects.contains_key(n.id.as_str()))
        .map(|n| {
            let (projects, count, last) = sums.get(n.id.as_str()).copied().unwrap_or_default();
            json!({"kind":if n.name.is_some(){"section"}else{"project"},"id":n.id,"name":n.name,"parent":n.parent,"collapsed":n.collapsed,
            "projectCount":projects,"conversationCount":count,"lastActivity":last})
        })
        .collect()
}
impl Archive {
    pub fn edit_project(&self, request: ProjectEdit) -> Result<Value> {
        let _guard = self.jobs.enter(None)?;
        let snapshot = self.project_snapshot()?;
        let (version, created) = self.store.write(E, Priority::Interactive, move |s| {
            let (version, old, previous) = s.query_one(
                "SELECT version,choices,previous FROM ah_project_choices WHERE singleton=1",
                (),
                |r| Ok((r.get::<i64>(0)?, r.get::<String>(1)?, r.get::<Option<String>>(2)?)),
            )?;
            if version != request.version || snapshot.version != version {
                return Err(operation(Error::StaleCursor));
            }
            let created = if matches!(request.action, Action::CreateSection { .. }) { Some(s.query_one("SELECT 's-'||lower(hex(randomblob(16)))", (), |r| Ok(r.get::<String>(0)?))?) } else { None };
            let (next, undo) = if matches!(request.action, Action::Undo) {
                (previous.ok_or_else(|| operation(invalid("nothing to undo")))?, None)
            } else {
                let mut choices: Choices = serde_json::from_str(&old).map_err(operation)?;
                choices.apply(request.action, &snapshot, created.as_deref().unwrap_or("")).map_err(operation)?;
                (serde_json::to_string(&choices).map_err(operation)?, Some(old))
            };
            s.execute(
                "UPDATE ah_project_choices SET version=version+1,choices=?1,previous=?2 WHERE singleton=1",
                params![next, undo],
            )?;
            Ok((version + 1, created))
        })?;
        self.ensure_projects()?;
        Ok(json!({"version":version,"createdId":created,"viewToken":self.chat_view_token()?}))
    }
}
