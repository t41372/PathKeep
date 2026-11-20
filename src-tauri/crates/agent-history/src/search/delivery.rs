//! Counts and backpressure for one ordered result stream. A Records group is an upsert;
//! find occurrences are append-only, including matches in collapsed parts.
use super::{catalog::Card, schema::EventDocument};
use crate::{Result, read_model::PartKind};
use serde_json::{Value, json};
use std::{
    ops::Range,
    time::{Duration, Instant},
};

pub(super) type Sink<'a> = dyn FnMut(&str, Value) -> Result<()> + 'a;
pub(super) struct Group<'a> {
    pub card: &'a Card,
    pub hits: u64,
    pub title_hit: bool,
    pub snippets: Vec<Value>,
}
impl<'a> Group<'a> {
    pub fn new(card: &'a Card) -> Self {
        Self { card, hits: 0, title_hit: false, snippets: vec![] }
    }
    fn card(&self) -> Value {
        let mut value = self.card.fields();
        value["hits"] = self.hits.into();
        value["titleHit"] = self.title_hit.into();
        value["snippets"] = json!(self.snippets);
        value
    }
}

pub(super) struct Delivery<'a> {
    pub find: bool,
    pub scanned: u64,
    pub total: i64,
    pub conversations: u64,
    pub hits: u64,
    sink: &'a mut Sink<'a>,
    pending: Vec<Value>,
    since: Instant,
}
impl<'a> Delivery<'a> {
    pub fn new(find: bool, total: i64, sink: &'a mut Sink<'a>) -> Self {
        Self {
            find,
            total,
            sink,
            scanned: 0,
            conversations: 0,
            hits: 0,
            pending: Vec::new(),
            since: Instant::now(),
        }
    }
    pub fn title(
        &mut self,
        group: &mut Group,
        id: &str,
        path: &[&str],
        range: Range<usize>,
    ) -> Result<()> {
        group.title_hit = true;
        group.hits += 1;
        if self.find {
            self.push_match(
                group,
                json!({"role":"title","address":null,"part":null,"range":[range.start,range.end],
                "path":path,"subagentId":id,"fold":"subagent","occurrence":group.hits}),
            )?;
        }
        Ok(())
    }
    pub fn record(
        &mut self,
        group: &mut Group,
        source: &super::schema::EventSource,
        path: &[&str],
        record: &EventDocument,
        i: usize,
        range: Range<usize>,
    ) -> Result<()> {
        group.hits += 1;
        if !self.find && group.snippets.len() == 3 {
            return Ok(());
        }
        let id = path.last().copied().unwrap_or(&group.card.id);
        let part = &record.parts[i];
        let fold = match part.kind {
            PartKind::Thinking => Some("thinking"),
            PartKind::CallInput | PartKind::CallResult => Some("tool"),
            PartKind::Text => None,
        };
        let (text, highlight) = snippet(&part.text, &range);
        let value = json!({"text":text,"highlight":highlight,"range":[range.start,range.end],"role":part.role,"part":i,
            "address":{"conversationId":id,"publication":source.publication,"sourceId":record.session_id,"ordinal":record.ordinal,"nativeId":record.native_id},
            "subagentId":path.last(),"path":path,"fold":fold,"occurrence":group.hits});
        if self.find {
            self.push_match(group, value)?
        } else {
            group.snippets.push(value);
        }
        Ok(())
    }
    fn push_match(&mut self, group: &Group, value: Value) -> Result<()> {
        self.pending.push(value);
        if self.pending.len() == 64 {
            self.flush(group.hits)?;
        }
        Ok(())
    }
    fn flush(&mut self, running: u64) -> Result<()> {
        if self.pending.is_empty() {
            return Ok(());
        }
        let items = std::mem::take(&mut self.pending);
        if self.find {
            (self.sink)("matches", json!({"items":items,"hits":running}))
        } else {
            (self.sink)("groups", json!({"items":items}))
        }
    }
    pub fn progress(&mut self, current: Option<&Group>) -> Result<()> {
        if self.since.elapsed() < Duration::from_millis(80) {
            return Ok(());
        }
        let partial = current.map_or(0, |g| g.hits);
        if !self.find
            && let Some(group) = current
            && partial != 0
        {
            self.pending.push(group.card());
        }
        let hits = self.hits + partial;
        self.flush(hits)?;
        (self.sink)(
            "progress",
            json!({"scanned":self.scanned,"total":self.total,"hits":hits,"conversations":self.conversations+u64::from(partial>0)}),
        )?;
        self.since = Instant::now();
        Ok(())
    }
    pub fn end_group(&mut self, group: Group) -> Result<()> {
        self.scanned += 1;
        if group.hits > 0 {
            self.hits += group.hits;
            self.conversations += 1;
            if !self.find {
                self.pending.push(group.card());
            }
            if self.pending.len() >= 32 || self.conversations == 1 {
                self.flush(self.hits)?;
            }
        }
        self.progress(None)
    }
    pub fn end(&mut self, filtered: bool, lag: i64, complete: bool) -> Result<()> {
        self.flush(self.hits)?;
        (self.sink)(
            "finished",
            json!({"searchedCount":self.total,"hits":self.hits,"conversations":self.conversations,"filtered":filtered,"indexLag":lag,"complete":complete}),
        )
    }
}

fn snippet(text: &str, range: &Range<usize>) -> (String, [usize; 2]) {
    let start = text[..range.start].char_indices().rev().nth(35).map_or(0, |(n, _)| n);
    let end = text[range.end..].char_indices().nth(60).map_or(text.len(), |(n, _)| range.end + n);
    let mut out = String::new();
    if start > 0 {
        out.push('…')
    }
    fn append(out: &mut String, s: &str) {
        for ch in s.chars() {
            if ch.is_whitespace() {
                if !out.ends_with(' ') {
                    out.push(' ')
                }
            } else {
                out.push(ch)
            }
        }
    }
    append(&mut out, &text[start..range.start]);
    let from = out.len();
    let matched = &text[range.clone()];
    if let Some((n, _)) = matched.char_indices().nth(120) {
        append(&mut out, &matched[..n]);
        out.push('…');
    } else {
        append(&mut out, matched);
    }
    let to = out.len();
    append(&mut out, &text[range.end..end]);
    if end < text.len() {
        out.push('…')
    }
    (out, [from, to])
}
