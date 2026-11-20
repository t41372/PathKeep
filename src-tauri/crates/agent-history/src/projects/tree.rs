//! Folder view groups are computed from project cards, including matches beyond the page.
//! Collapse and expansion in this view belong to the requesting window.
use super::grouping::{key, parent};
use serde_json::{Value, json};
use std::collections::{BTreeMap, BTreeSet};
#[derive(Default)]
struct Group {
    parent: Option<String>,
    projects: Vec<String>,
    children: usize,
    count: usize,
    roots: i64,
    last: Option<i64>,
}
pub(super) fn groups(cards: &[&super::rows::Card], home: Option<&str>) -> Value {
    let mut groups = BTreeMap::<String, Group>::new();
    let mut top = Vec::new();
    let home = home.map(key);
    for card in cards {
        let id = card.id.as_str();
        let scratch = card.kind == "scratch";
        let mut at = if scratch { Some("") } else { card.parent_folder.as_deref() };
        let mut direct = true;
        while let Some(path) = at.filter(|p| *p != "/" && Some(key(p)) != home) {
            let next = parent(path);
            let g = groups.entry(path.to_owned()).or_default();
            g.parent = next.map(str::to_owned);
            g.count += 1;
            g.roots += card.conversation_count;
            g.last = g.last.max(card.last_activity);
            if direct {
                g.projects.push(id.into());
                direct = false;
            }
            at = next;
        }
        if direct {
            top.push(id)
        }
    }
    let parents = groups.values().filter_map(|g| g.parent.clone()).collect::<Vec<_>>();
    for p in parents {
        if let Some(g) = groups.get_mut(&p) {
            g.children += 1
        }
    }
    let visible: BTreeSet<_> = groups
        .iter()
        .filter(|(_, g)| !g.projects.is_empty() || g.children > 1)
        .map(|(p, _)| p.as_str())
        .collect();
    let mut rows = Vec::new();
    for path in &visible {
        let g = &groups[*path];
        let mut parent = g.parent.as_deref();
        while let Some(p) = parent
            && !visible.contains(p)
        {
            parent = groups.get(p).and_then(|g| g.parent.as_deref());
        }
        rows.push(
            json!({"id":path,"parent":parent,"kind":if path.is_empty(){"scratch"}else{"folder"},
            "projectIds":g.projects,"projectCount":g.count,"conversationCount":g.roots,"lastActivity":g.last}),
        );
    }
    rows.sort_by(|a, b| {
        b["lastActivity"]
            .as_i64()
            .cmp(&a["lastActivity"].as_i64())
            .then_with(|| a["id"].as_str().cmp(&b["id"].as_str()))
    });
    json!({"groups":rows,"topLevelProjects":top})
}
