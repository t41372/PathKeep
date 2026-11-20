//! Explain uncertain relationships using saved facts. A pair of stable IDs identifies the
//! user's answer even if later observations change which folder is the likely destination.
use super::{
    edit::Choices,
    rows::{Card, Location},
};
use serde_json::{Value, json};
use std::collections::{BTreeMap, BTreeSet};

pub(super) fn build(
    cards: &[Card],
    folders: &BTreeMap<String, Vec<Location>>,
    choices: &Choices,
) -> Vec<Value> {
    let mut ordered: Vec<_> = cards.iter().filter(|p| p.kind != "scratch").collect();
    ordered.sort_by_key(|p| (p.gone, p.first_activity, &p.main_folder));
    let mut repos = BTreeMap::<&str, Option<&Card>>::new();
    for p in ordered.iter().filter(|p| p.kind == "repo") {
        repos.entry(&p.default_name).and_modify(|v| *v = None).or_insert(Some(p));
    }
    let mut by_remote = BTreeMap::<&str, &Card>::new();
    let mut by_name = BTreeMap::<&str, &Card>::new();
    let mut proposals = BTreeMap::new();
    for card in ordered {
        let Some(path) = &card.main_folder else { continue };
        let mut suggestions = Vec::new();
        if card.gone
            && let Some(&into) = by_name.get(card.default_name.as_str())
        {
            suggestions.push((into, "moved", json!({"oldPath":path,"newPath":into.main_folder})));
        }
        if !card.gone {
            by_name.entry(&card.default_name).or_insert(card);
        }
        for remote in &card.remotes {
            if let Some(&into) = by_remote.get(remote.as_str()) {
                suggestions.push((into, "sameRemote", json!({"remote":remote})))
            } else {
                by_remote.insert(remote, card);
            }
        }
        let normalized = path.replace('\\', "/");
        if card.kind == "folder"
            && normalized.contains("/.codex/worktrees/")
            && let Some(Some(into)) = repos.get(card.default_name.as_str())
        {
            suggestions.push((
                *into,
                "toolWorktree",
                json!({"harness":"codex","folderName":card.default_name,"branch":folders[&card.id][0].branch}),
            ));
        }
        let mut targets = BTreeSet::new();
        for (into, kind, evidence) in suggestions {
            if !targets.insert(&into.id) {
                continue;
            }
            let mut pair = [card.id.as_str(), into.id.as_str()];
            pair.sort_unstable();
            let id = pair.join("/");
            proposals.insert(
                id.clone(),
                json!({"id":id,"kind":kind,"fromFolder":path,"fromProjectId":card.id,"intoProjectId":into.id,
                "intoDisplayName":into.display_name,"fromConversationCount":card.conversation_count,"fromFolderGone":card.gone,"evidence":evidence,"decision":"pending"}),
            );
        }
    }
    let by_id: BTreeMap<_, _> = cards.iter().map(|p| (p.id.as_str(), p)).collect();
    for (id, answer) in &choices.answers {
        let mut row = answer.proposal.clone();
        row["decision"] = answer.decision.clone().into();
        if let Some(p) = row["intoProjectId"].as_str().and_then(|id| by_id.get(id)) {
            row["intoDisplayName"] = p.display_name.clone().into();
        }
        if let Some(p) = row["fromProjectId"].as_str().and_then(|id| by_id.get(id)) {
            row["fromConversationCount"] = p.conversation_count.into();
            row["fromFolderGone"] = p.gone.into();
        }
        proposals.insert(id.clone(), row);
    }
    let mut items: Vec<_> = proposals.into_values().collect();
    items.sort_by_key(|s| match s["decision"].as_str() {
        Some("pending") => 0,
        Some("later") => 1,
        _ => 2,
    });
    items
}
