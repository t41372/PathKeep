//! Time-local summaries keep the common report path small. The underlying timestamped
//! contributions remain available when a range or local hour cuts through a page.
use super::clock::{Cluster, Span, merge_clusters, union};
use crate::{chats::facts::Turn, query::operation, read_model::usage::Tokens};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use vault_store::{Session, params};

#[derive(Clone, Copy, Serialize, Deserialize)]
pub(super) enum DurationKind {
    Recorded,
    Derived,
    Unknown,
}
#[derive(Clone, Serialize, Deserialize)]
pub(super) enum Fact {
    Work {
        at: Option<i64>,
        end: Option<i64>,
        source: DurationKind,
        elapsed: Option<i64>,
        turn: bool,
    },
    Prompt {
        at: Option<i64>,
        end: Option<i64>,
    },
    Reply {
        at: Option<i64>,
        model: String,
        tokens: Option<Tokens>,
        recorded: Option<f64>,
        estimated: Option<f64>,
    },
    Call {
        at: Option<i64>,
        name: String,
    },
    Edit {
        at: Option<i64>,
        added: Option<i64>,
        removed: Option<i64>,
    },
}
impl Fact {
    pub fn at(&self) -> Option<i64> {
        match self {
            Self::Work { at, .. }
            | Self::Prompt { at, .. }
            | Self::Reply { at, .. }
            | Self::Call { at, .. }
            | Self::Edit { at, .. } => *at,
        }
    }
    fn extent(&self) -> (Option<i64>, Option<i64>) {
        let at = self.at();
        let end = match self {
            Self::Work { end, .. } => *end,
            Self::Prompt { end, .. } => {
                at.map(|t| end.unwrap_or(t).clamp(t, t.saturating_add(900000)))
            }
            _ => at,
        };
        (at, end.or(at))
    }
}
pub(super) fn add<T: Copy + Default + std::ops::AddAssign>(a: &mut Option<T>, b: Option<T>) {
    if let Some(b) = b {
        *a.get_or_insert_with(Default::default) += b
    }
}
pub(super) fn tokens(a: &mut Option<Tokens>, b: Option<Tokens>) {
    if let Some(t) = b {
        a.get_or_insert_with(Default::default).add(t)
    }
}
#[derive(Default, Serialize, Deserialize, Clone)]
pub(super) struct Meter {
    pub tokens: Option<Tokens>,
    pub recorded: Option<f64>,
    pub estimated: Option<f64>,
    pub unpriced: bool,
}
impl Meter {
    pub fn merge(&mut self, other: &Self) {
        tokens(&mut self.tokens, other.tokens);
        add(&mut self.recorded, other.recorded);
        add(&mut self.estimated, other.estimated);
        self.unpriced |= other.unpriced;
    }
}
#[derive(Default, Serialize, Deserialize)]
pub(super) struct Brief {
    pub turns: u32,
    pub prompts: u32,
    pub recorded: u32,
    pub derived: u32,
    pub unknown: u32,
    pub summed: Option<i64>,
    pub spans: Vec<Span>,
    pub presence: Vec<Cluster>,
    pub models: BTreeMap<String, Meter>,
    pub first_usage: Option<i64>,
    pub tools: BTreeMap<String, u64>,
    pub edits: u32,
    pub unknown_edits: u32,
    pub added: i64,
    pub removed: i64,
}
impl Brief {
    pub fn observe(&mut self, f: &Fact) {
        match f {
            Fact::Work { at, end, source, elapsed, turn } => {
                self.turns += u32::from(*turn);
                match source {
                    DurationKind::Recorded => self.recorded += 1,
                    DurationKind::Derived => self.derived += 1,
                    DurationKind::Unknown => self.unknown += 1,
                }
                add(&mut self.summed, *elapsed);
                if let Some((s, e)) = at.zip(*end) {
                    self.spans.push((s, e.max(s)));
                }
            }
            Fact::Prompt { at, end } => {
                self.prompts += 1;
                if let Some(t) = *at {
                    self.presence.push(Cluster::new(t, *end));
                }
            }
            Fact::Reply { at, model, tokens: amount, recorded, estimated } => {
                self.models.entry(model.clone()).or_default().merge(&Meter {
                    tokens: *amount,
                    recorded: *recorded,
                    estimated: *estimated,
                    unpriced: amount.is_some() && recorded.is_none() && estimated.is_none(),
                });
                if amount.is_some() {
                    self.first_usage = self.first_usage.into_iter().chain(*at).min();
                }
            }
            Fact::Call { name, .. } => *self.tools.entry(name.clone()).or_default() += 1,
            Fact::Edit { added, removed, .. } => {
                self.edits += 1;
                self.added += added.unwrap_or(0);
                self.removed += removed.unwrap_or(0);
                self.unknown_edits += u32::from(added.is_none() || removed.is_none());
            }
        }
    }
}

pub(crate) struct Writer<'a> {
    publication: &'a str,
    source: &'a str,
    page: i64,
    lo: Option<i64>,
    hi: Option<i64>,
    undated: bool,
    events: Vec<Fact>,
}
impl<'a> Writer<'a> {
    pub fn new(publication: &'a str, source: &'a str) -> Self {
        Self { publication, source, page: 0, lo: None, hi: None, undated: false, events: vec![] }
    }
    fn push(&mut self, s: &Session<'_>, f: Fact) -> vault_store::Result<()> {
        let (lo, hi) = f.extent();
        if self.events.len() >= 512
            || self.lo.map(|n| n.div_euclid(3600000)) != lo.map(|n| n.div_euclid(3600000))
        {
            self.flush(s)?;
        }
        self.lo = self.lo.into_iter().chain(lo).min();
        self.hi = self.hi.max(hi);
        self.undated |= lo.is_none();
        self.events.push(f);
        Ok(())
    }
    pub fn turn(&mut self, s: &Session<'_>, t: &Turn) -> vault_store::Result<()> {
        let duration = match t.duration_source {
            "recorded" => DurationKind::Recorded,
            "derived" => DurationKind::Derived,
            _ => DurationKind::Unknown,
        };
        let at = t
            .started_at
            .or_else(|| t.ended_at.zip(t.facts.agent_ms).map(|(e, n)| e.saturating_sub(n)));
        let end = at.zip(t.facts.agent_ms).map(|(t, n)| t.saturating_add(n.max(0)));
        if t.facts.turns > 0 || end.is_some() {
            self.push(
                s,
                Fact::Work {
                    at,
                    end,
                    source: duration,
                    elapsed: t.facts.agent_ms,
                    turn: t.facts.turns > 0,
                },
            )?;
        }
        if t.prompt.is_some() {
            self.push(s, Fact::Prompt { at: t.prompt_at, end: end.or(t.ended_at) })?;
        }
        for r in t.responses.values() {
            self.push(
                s,
                Fact::Reply {
                    at: r.at,
                    model: r.model.clone().unwrap_or_default(),
                    tokens: Some(r.tokens),
                    recorded: r.usd,
                    estimated: r.estimated_usd,
                },
            )?;
        }
        for (key, (at, model)) in &t.answering_models {
            if !t.responses.contains_key(key) {
                self.push(
                    s,
                    Fact::Reply {
                        at: *at,
                        model: model.clone(),
                        tokens: None,
                        recorded: None,
                        estimated: None,
                    },
                )?;
            }
        }
        for (at, name) in &t.named_calls {
            self.push(s, Fact::Call { at: *at, name: name.clone() })?;
        }
        for (at, edit) in t.edits.values() {
            self.push(
                s,
                Fact::Edit {
                    at: *at,
                    added: edit["addedLines"].as_i64(),
                    removed: edit["removedLines"].as_i64(),
                },
            )?;
        }
        Ok(())
    }
    pub fn flush(&mut self, s: &Session<'_>) -> vault_store::Result<()> {
        if self.events.is_empty() {
            return Ok(());
        }
        let mut b = Brief::default();
        for f in &self.events {
            b.observe(f);
        }
        b.spans = union(b.spans);
        b.presence = merge_clusters(b.presence);
        let brief = postcard::to_allocvec(&b).map_err(operation)?;
        let detail = postcard::to_allocvec(&self.events).map_err(operation)?;
        s.execute("INSERT INTO ah_insight_page(publication,source,page,lo,hi,undated,first_usage,brief,detail) VALUES(?1,?2,?3,?4,?5,?6,?7,?8,?9)",
            params![self.publication,self.source,self.page,self.lo,self.hi,i64::from(self.undated),b.first_usage,brief,detail])?;
        for (hour, input) in super::work::split(&self.events) {
            s.execute(
                "INSERT INTO ah_insight_clock_input VALUES(?1,?2,?3,?4,?5)",
                params![
                    self.publication,
                    self.source,
                    self.page,
                    hour,
                    postcard::to_allocvec(&input).map_err(operation)?
                ],
            )?;
        }
        self.page += 1;
        self.events.clear();
        self.lo = None;
        self.hi = None;
        self.undated = false;
        Ok(())
    }
}
