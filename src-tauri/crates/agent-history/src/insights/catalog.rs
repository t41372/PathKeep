//! Freeze the visible root/descendant relationships and project labels for a report.
use super::{Query, query::D};
use crate::{Archive, Error, Result, chats::list::view};
use serde_json::Value;
use std::{collections::HashMap, sync::Arc};
use vault_store::CancelToken;
pub(super) struct Source {
    pub root: usize,
    pub id: String,
    pub title: Option<String>,
    pub title_source: String,
    pub harness: String,
    pub project: Option<String>,
    pub(super) lo: Option<i64>,
    pub(super) hi: Option<i64>,
    pub(super) first_usage: Option<i64>,
}
#[derive(Default)]
pub(super) struct Catalog {
    pub(super) view: (i64, i64),
    pub(super) saved_roots: usize,
    pub sources: Vec<Source>,
    pub projects: HashMap<String, Value>,
    pub(super) publications: HashMap<String, usize>,
}
impl Catalog {
    pub fn project(&self, s: &Source) -> Option<&Value> {
        s.project.as_ref().and_then(|p| self.projects.get(p))
    }
    pub(super) fn eligible(&self, i: usize, q: &Query) -> bool {
        let Some(s) = self.sources.get(self.sources[i].root) else { return false };
        (q.harnesses.is_empty() || q.harnesses.iter().any(|h| h.as_str() == s.harness))
            && q.project_id.as_ref().is_none_or(|p| Some(p) == s.project.as_ref())
    }
}
struct Meta {
    pub_id: String,
    id: String,
    parent: Option<String>,
    root: bool,
    hidden: bool,
    harness: String,
    title: Option<String>,
    label: String,
    project: Option<String>,
    lo: Option<i64>,
    hi: Option<i64>,
    first_usage: Option<i64>,
}
impl Archive {
    pub(super) fn insight_catalog(
        &self,
        version: (i64, i64),
        cancel: &CancelToken,
        mut progress: impl FnMut(usize) -> Result<()>,
    ) -> Result<Arc<Catalog>> {
        if let Some((v, c)) = &self.insights.lock().unwrap().catalog
            && *v == version
        {
            return Ok(c.clone());
        }
        // Read the roster in one store snapshot. The progress channel never blocks the
        // database reader, even if the window pauses consumption.
        let (snapshot, rows) = std::thread::scope(|scope| -> Result<_> {
            let (count, counts) = std::sync::mpsc::sync_channel(1);
            let token = cancel.clone();
            let task=scope.spawn(move||self.store.read(D,move|s|{
                let version=view(s)?;
                let mut rows=Vec::new();
                s.for_each("SELECT c.publication,c.id,c.parent,c.root,u.trashed_at IS NOT NULL,c.harness,
                    coalesce(u.title,c.title),CASE WHEN u.title IS NULL THEN c.title_source ELSE 'user' END,c.project_id,c.insight_first,c.insight_last,c.insight_first_usage
                    FROM ah_chat_summary c LEFT JOIN ah_user_state u ON u.conversation_id=c.id WHERE c.live=1",(),|r|{
                    if rows.len()%512==0 {token.check()?;let _=count.try_send(rows.len());}
                    rows.push(Meta{pub_id:r.get(0)?,id:r.get(1)?,parent:r.get(2)?,root:r.get::<i64>(3)?!=0,hidden:r.get::<i64>(4)?!=0,
                        harness:r.get(5)?,title:r.get(6)?,label:r.get(7)?,project:r.get(8)?,lo:r.get(9)?,hi:r.get(10)?,first_usage:r.get(11)?});Ok(())
                })?;
                Ok((version,rows))
            }));
            for n in counts {
                progress(n)?;
            }
            Ok(task
                .join()
                .map_err(|_| Error::Integrity("insights roster interrupted".into()))??)
        })?;
        let mut children = HashMap::<&str, Vec<usize>>::new();
        let mut stack = Vec::new();
        for (i, m) in rows.iter().enumerate().filter(|(_, r)| !r.hidden) {
            if m.root {
                stack.push((i, i));
            } else if let Some(parent) = m.parent.as_deref() {
                children.entry(parent).or_default().push(i);
            }
        }
        let mut parents = vec![usize::MAX; rows.len()];
        while let Some((i, root)) = stack.pop() {
            parents[i] = root;
            for child in children.remove(rows[i].id.as_str()).unwrap_or_default() {
                stack.push((child, root));
            }
        }
        drop(children);
        let mut cat = Catalog {
            view: snapshot,
            saved_roots: rows.iter().filter(|r| r.root).count(),
            ..Default::default()
        };
        for (m, root) in rows.into_iter().zip(parents) {
            let i = cat.sources.len();
            if root != usize::MAX {
                cat.publications.insert(m.pub_id, i);
            }
            cat.sources.push(Source {
                root,
                id: m.id,
                title: m.title,
                title_source: m.label,
                project: m.project,
                harness: m.harness,
                lo: m.lo,
                hi: m.hi,
                first_usage: m.first_usage,
            });
        }
        cat.projects = self.insight_projects()?;
        let cat = Arc::new(cat);
        self.insights.lock().unwrap().catalog = Some((snapshot, cat.clone()));
        Ok(cat)
    }
}
