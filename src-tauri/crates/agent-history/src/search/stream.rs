//! Stream search over the serving index and find over the open reader publication. Both
//! match complete readable parts. The delivery module owns previews and frame batching.
use super::{
    candidates::{self, Selection, Selections},
    catalog::{Batch, Card},
    delivery::{Delivery, Group as Hits},
    expression::Expression,
    index,
    schema::EventSource,
};
use crate::{
    Archive, Error, Result,
    chats::{Group, ListQuery, Sort},
};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use vault_store::CancelToken;

/// A Records search scope. Titles participate in every selection.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Role {
    User,
    Agent,
    Tool,
    Thinking,
}
fn all_roles() -> Vec<Role> {
    vec![Role::User, Role::Agent, Role::Tool, Role::Thinking]
}
/// Expression and scope shared by the Records box and reader find.
#[derive(Clone, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ScreenQuery {
    pub query: String,
    #[serde(default = "all_roles")]
    pub roles: Vec<Role>,
    #[serde(default)]
    pub filters: ListQuery,
}
impl ScreenQuery {
    pub fn all(query: String) -> Self {
        Self { query, roles: all_roles(), filters: ListQuery::default() }
    }
    pub fn validate(&self) -> Result<()> {
        self.expression().map(drop)
    }
    fn expression(&self) -> Result<Expression> {
        if self.roles.is_empty() {
            return Err(Error::InvalidInput("choose at least one role".into()));
        }
        let q = &self.filters;
        if q.cursor.is_some()
            || q.limit.is_some()
            || q.sort != Sort::LastActivity
            || q.group == Group::Project
        {
            return Err(Error::InvalidInput("search streams in last activity order".into()));
        }
        Expression::parse(&self.query)
    }
    fn filtered(&self) -> bool {
        let q = &self.filters;
        !all_roles().iter().all(|r| self.roles.contains(r))
            || !q.flags.is_empty()
            || !q.harnesses.is_empty()
            || q.project_id.is_some()
            || q.day.is_some()
            || q.range.as_deref().is_some_and(|r| r != "all")
    }
}

impl Archive {
    /// Stream the entire eligible corpus, or a conversation and its descendants. Offsets are
    /// half-open bytes in a readable part. A consumer discards its view on a terminal error.
    pub fn stream_conversations(
        &self,
        scope: Option<&str>,
        q: &ScreenQuery,
        cancel: &CancelToken,
        mut send: impl FnMut(&str, Value) -> Result<()>,
    ) -> Result<()> {
        let expr = q.expression()?;
        self.ensure_state_visible()?;
        let check = || cancel.check().map_err(|_| Error::Cancelled);
        send("progress", json!({"scanned":0,"total":null,"conversations":0,"hits":0}))?;
        let _worker = self.jobs.enter_window(cancel)?;
        check()?;
        self.ensure_projects()?;
        let _pin = self.search_reads.read().unwrap();
        let marks = self.search_marks_generation()?;
        let mut deliver = |ty: &str, p: Value| {
            check()?;
            if self.search_marks_generation()? != marks {
                return Err(Error::StaleCursor);
            }
            send(ty, p)
        };
        let scan = Scan { archive: self, expr: &expr, roles: &q.roles, cancel };
        if let Some(id) = scope {
            let _publication = self.jobs.pin_publication()?;
            let (tree, complete) = self.find_tree(id)?;
            deliver(
                "started",
                json!({"total":1,"indexedThrough":null,"indexLag":0,"filtered":q.filtered()}),
            )?;
            let mut out = Delivery::new(true, 1, &mut deliver);
            scan.group(&tree.roots[0], &tree, None, &mut out)?;
            return out.end(q.filtered(), 0, complete);
        }
        let (opened, searcher, mut catalog) = {
            let _switch = self.search_switch.lock().unwrap();
            let opened = index::open(self)?;
            let reader: tantivy::IndexReader = index::checked(
                opened
                    .index
                    .reader_builder()
                    .reload_policy(tantivy::ReloadPolicy::Manual)
                    .try_into(),
                &opened.directory,
            )?;
            let catalog = self.search_catalog(q, opened.watermark)?;
            (opened, reader.searcher(), catalog)
        };
        let total = catalog.total;

        deliver(
            "started",
            json!({"total":total,"indexLag":catalog.lag,"indexedThrough":opened.watermark,"filtered":q.filtered()}),
        )?;
        let required = candidates::RequiredWord::choose(&searcher, &opened, &expr, &q.roles)?;
        let mut selective = required
            .as_ref()
            .filter(|word| !word.common())
            .map(|word| word.postings(&searcher, &opened, cancel))
            .transpose()?;
        if let Some(map) = &selective
            && map.len() <= 128
            && let Some(titles) = candidates::titles(&searcher, &opened, &expr)?
        {
            catalog.restrict(self, map.keys().copied().collect(), titles)?;
        }
        let mut delivery = Delivery::new(false, total, &mut deliver);
        while let Some(batch) = catalog.next(self)? {
            for root in &batch.roots {
                scan.group(root, &batch, selective.as_ref(), &mut delivery)?;
                if delivery.scanned == 1
                    && let Some(word) = required.as_ref().filter(|w| w.common() && w.selectable())
                {
                    selective = Some(word.postings(&searcher, &opened, cancel)?);
                }
            }
        }
        delivery.end(q.filtered(), catalog.lag, self.normalization_complete()?)
    }
}

struct Scan<'a> {
    archive: &'a Archive,
    expr: &'a Expression,
    roles: &'a [Role],
    cancel: &'a CancelToken,
}
impl Scan<'_> {
    fn group(
        &self,
        root: &Card,
        batch: &Batch,
        selections: Option<&Selections>,
        out: &mut Delivery,
    ) -> Result<()> {
        let mut group = Hits::new(root);
        let mut members = vec![(root.id.as_str(), root.title.as_deref(), Vec::new())];
        while let Some((id, title, path)) = members.pop() {
            self.cancel.check().map_err(|_| Error::Cancelled)?;
            if let Some(title) = title.filter(|_| !out.find || id != root.id) {
                self.expr.visit(&[(0, title)], self.cancel, |_, range| {
                    out.title(&mut group, id, &path, range)
                })?;
            }
            for source in batch.sources.get(id).into_iter().flatten() {
                let selection = selections.and_then(|map| map.get(&source.projection_id));
                if selections.is_none() || selection.is_some() {
                    self.source(&path, source, selection, &mut group, out)?;
                }
            }
            if let Some(children) = batch.children.get(id) {
                for (child, title) in children.iter().rev() {
                    let mut next = path.clone();
                    next.push(child.as_str());
                    members.push((child, title.as_deref(), next));
                }
            }
        }
        out.end_group(group)
    }
    fn source(
        &self,
        path: &[&str],
        source: &EventSource,
        selection: Option<&Selection>,
        group: &mut Hits,
        out: &mut Delivery,
    ) -> Result<()> {
        let final_count = selection.and_then(|s| s.occurrences).map(|n| group.hits + n);
        let mut after = -1;
        loop {
            self.cancel.check().map_err(|_| Error::Cancelled)?;
            if final_count.is_some() && group.snippets.len() == 3 {
                break;
            }
            let wanted = if let Some(sel) = selection {
                let rest = &sel.ordinals[sel.ordinals.partition_point(|n| *n <= after)..];
                if rest.is_empty() {
                    break;
                }
                &rest[..rest.len().min(128)]
            } else {
                &[][..]
            };
            let page = self.archive.search_record_page(source, after, wanted)?;
            if page.is_empty() {
                break;
            }
            for record in page {
                after = record.ordinal;
                let parts: Vec<_> = record
                    .parts
                    .iter()
                    .enumerate()
                    .filter(|(_, p)| self.roles.contains(&p.role))
                    .map(|(i, p)| (i, p.text.as_str()))
                    .collect();
                self.expr.visit(&parts, self.cancel, |i, range| {
                    out.record(group, source, path, &record, i, range)
                })?;
            }
            out.progress(Some(group))?;
        }
        if let Some(total) = final_count {
            group.hits = total
        }
        Ok(())
    }
}
