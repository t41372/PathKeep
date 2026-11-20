//! Range-local arithmetic. Distinct coverage is counted at the root; amounts keep their
//! producing model and harness. Presence clusters are joined across conversations before clipping.
use super::{
    Preferences,
    catalog::{Catalog, Source},
    clock::{self, Axis, Cluster, Span},
    facts::{Brief, Fact, Meter, add, tokens},
};
use crate::{Result, read_model::usage::Tokens};
use chrono::Datelike;
use chrono_tz::Tz;
use serde_json::{Value, json};
use std::collections::{BTreeMap, HashMap, HashSet};

#[derive(Default)]
struct Root {
    turns: u64,
    ms: Option<i64>,
    recorded: bool,
    derived: bool,
    unknown: bool,
    usage: Option<Tokens>,
    money: Meter,
    edited: bool,
    last: Option<i64>,
    primary: BTreeMap<String, i64>,
}
#[derive(Default)]
struct Bucket {
    busy: i64,
    summed: Option<i64>,
    present: i64,
    usage: Option<Tokens>,
    tools: BTreeMap<String, Option<Tokens>>,
    models: BTreeMap<String, Option<Tokens>>,
    active: HashSet<usize>,
    measured: HashSet<usize>,
}
#[derive(Default)]
struct Group {
    ms: Option<i64>,
    usage: Option<Tokens>,
    roots: u64,
    known: u64,
    timed: u64,
    harnesses: std::collections::BTreeSet<String>,
}
pub(super) struct Report {
    pub axis: Axis,
    precomputed_busy: Option<Vec<i64>>,
    timed_prompts: u64,
    untimed_prompts: u64,
    known_edits: u64,
    tz: Tz,
    roots: HashMap<usize, Root>,
    children: HashSet<usize>,
    buckets: Vec<Bucket>,
    spans: Vec<Span>,
    presence: Vec<Cluster>,
    located: bool,
    rhythm: [[u64; 7]; 24],
    tools: BTreeMap<(String, String), u64>,
    model_tokens: BTreeMap<String, Option<Tokens>>,
    harness_tokens: BTreeMap<String, Option<Tokens>>,
    first_usage: BTreeMap<String, i64>,
    added: i64,
    removed: i64,
    unknown_edits: u64,
}
impl Report {
    pub fn new(
        axis: Axis,
        tz: Tz,
        first_usage: BTreeMap<String, i64>,
        work: Option<&Value>,
    ) -> Self {
        let precomputed_busy = work
            .and_then(|w| w["buckets"].as_array())
            .filter(|rows| {
                rows.len() == axis.starts.len()
                    && rows.iter().zip(&axis.starts).all(|(b, s)| b["bucketStart"] == *s)
            })
            .map(|rows| rows.iter().map(|b| b["busyMs"].as_i64().unwrap_or(0)).collect());
        let buckets = (0..axis.starts.len()).map(|_| Default::default()).collect();
        Self {
            axis,
            precomputed_busy,
            timed_prompts: 0,
            untimed_prompts: 0,
            known_edits: 0,
            tz,
            buckets,
            roots: Default::default(),
            children: Default::default(),
            spans: vec![],
            presence: vec![],
            located: false,
            rhythm: [[0; 7]; 24],
            tools: Default::default(),
            model_tokens: Default::default(),
            harness_tokens: Default::default(),
            first_usage,
            added: 0,
            removed: 0,
            unknown_edits: 0,
        }
    }
    pub fn fast(&self, lo: Option<i64>, hi: Option<i64>, undated: bool) -> Result<bool> {
        let Some((lo, hi)) = lo.zip(hi) else { return Ok(false) };
        Ok(!undated
            && self.axis.bin(lo).is_some()
            && self.axis.bin(lo) == self.axis.bin(hi)
            && clock::hour(self.tz, lo)?.2 == clock::hour(self.tz, hi)?.2)
    }
    pub fn bulk(&mut self, idx: usize, src: &Source, b: &Brief, at: Option<i64>) -> Result<()> {
        let root = self.roots.entry(src.root).or_default();
        root.last = root.last.max(at);
        let child = src.root != idx;
        if child {
            self.children.insert(idx);
        } else {
            root.turns += u64::from(b.turns);
            root.recorded |= b.recorded > 0;
            root.derived |= b.derived > 0;
            root.unknown |= b.unknown > 0;
            add(&mut root.ms, b.summed);
            self.located |= !b.spans.is_empty();
            if self.precomputed_busy.is_none() {
                self.spans.extend(&b.spans);
            }
            if at.is_some() {
                self.timed_prompts += u64::from(b.prompts)
            } else {
                self.untimed_prompts += u64::from(b.prompts)
            }
            self.presence.extend(&b.presence);
            if b.prompts != 0
                && let Some(at) = at
            {
                let (date, h, _) = clock::hour(self.tz, at)?;
                self.rhythm[h as usize][date.weekday().num_days_from_monday() as usize] +=
                    u64::from(b.prompts);
            }
        }
        root.edited |= b.edits != 0;
        self.known_edits += u64::from(b.edits - b.unknown_edits);
        self.unknown_edits += u64::from(b.unknown_edits);
        self.added += b.added;
        self.removed += b.removed;
        for (name, n) in &b.tools {
            *self.tools.entry((src.harness.clone(), name.clone())).or_default() += n;
        }
        if let Some(at) = b.first_usage {
            let first = self.first_usage.entry(src.harness.clone()).or_insert(at);
            *first = (*first).min(at);
        }
        let bin = at.and_then(|t| self.axis.bin(t));
        if let Some(i) = bin {
            let bin = &mut self.buckets[i];
            bin.active.insert(src.root);
            if !child {
                add(&mut bin.summed, b.summed);
            }
        }
        for (model, charge) in &b.models {
            root.money.merge(charge);
            tokens(&mut root.usage, charge.tokens);
            if !child
                && let Some(t) = charge.tokens
                && !model.is_empty()
            {
                *root.primary.entry(model.clone()).or_default() += t.output;
            }
            tokens(self.model_tokens.entry(model.clone()).or_default(), charge.tokens);
            tokens(self.harness_tokens.entry(src.harness.clone()).or_default(), charge.tokens);
            if let Some(i) = bin {
                let b = &mut self.buckets[i];
                tokens(&mut b.usage, charge.tokens);
                tokens(b.tools.entry(src.harness.clone()).or_default(), charge.tokens);
                tokens(b.models.entry(model.clone()).or_default(), charge.tokens);
                if charge.tokens.is_some() {
                    b.measured.insert(src.root);
                }
            }
        }
        Ok(())
    }
    pub fn detail(&mut self, idx: usize, src: &Source, facts: &[Fact]) -> Result<()> {
        let (lo, hi) = self.axis.bounds();
        for fact in facts {
            let at = fact.at();
            if let Fact::Work { at: Some(start), end: Some(end), source, turn, .. } = fact
                && end > start
            {
                let (start, end) = ((*start).max(lo), (*end).min(hi));
                if end <= start {
                    continue;
                }
                let mut b = Brief::default();
                b.observe(&Fact::Work {
                    at: Some(start),
                    end: Some(end),
                    source: *source,
                    elapsed: Some(end - start),
                    turn: *turn,
                });
                // Each visible interval counts once at the root and contributes clock time to
                // every bucket it crosses. The normal bulk path handles one bucket.
                let duration = b.summed.take();
                self.bulk(idx, src, &b, Some(start))?;
                if idx == src.root {
                    add(&mut self.roots.get_mut(&idx).unwrap().ms, duration);
                }
                self.axis.pieces(start, end, |i, n| {
                    let b = &mut self.buckets[i];
                    b.active.insert(src.root);
                    if idx == src.root {
                        add(&mut b.summed, Some(n));
                    }
                });
            } else {
                let inside = at.map_or(self.axis.all, |t| self.axis.bin(t).is_some());
                if !inside {
                    if idx == src.root
                        && let Fact::Prompt { at: Some(t), end } = fact
                    {
                        self.presence.push(Cluster::new(*t, *end));
                    }
                    continue;
                }
                let mut b = Brief::default();
                b.observe(fact);
                self.bulk(idx, src, &b, at)?;
            }
        }
        Ok(())
    }
    pub fn finish(
        mut self,
        cat: &Catalog,
        prefs: &Preferences,
        cancel: &vault_store::CancelToken,
        mut progress: impl FnMut(usize) -> Result<()>,
    ) -> Result<Value> {
        let mut tick = std::time::Instant::now();
        let mut busy = 0;
        for (s, e) in clock::union(self.spans) {
            self.axis.pieces(s, e, |i, n| {
                self.buckets[i].busy += n;
                busy += n;
            });
        }
        if let Some(values) = self.precomputed_busy {
            for (b, ms) in self.buckets.iter_mut().zip(values) {
                b.busy = ms;
                busy += ms;
            }
        }
        let mut present = 0;
        for c in clock::merge_clusters(self.presence) {
            self.axis.pieces(c.first, c.end, |i, n| {
                self.buckets[i].present += n;
                present += n;
            });
        }
        let mut usage = None;
        let mut sum = None;
        let mut known = 0;
        let mut edited = 0;
        let mut recorded = 0;
        let mut derived = 0;
        let mut mixed = 0;
        let mut unknown = 0;
        let mut money = Meter::default();
        let (mut paid, mut estimated, mut unpriced, mut overlap) = (0, 0, 0, 0);
        let mut projects = BTreeMap::<&str, Group>::new();
        let mut harness = BTreeMap::<&str, Group>::new();
        let mut primaries = BTreeMap::<&str, u64>::new();
        let mut no_usage = BTreeMap::<&str, u64>::new();
        for (done, (&idx, r)) in self.roots.iter().enumerate() {
            if done % 512 == 0 {
                cancel.check().map_err(|_| crate::Error::Cancelled)?;
                if tick.elapsed().as_millis() >= 70 {
                    progress(done)?;
                    tick = std::time::Instant::now();
                }
            }
            let src = &cat.sources[idx];
            tokens(&mut usage, r.usage);
            add(&mut sum, r.ms);
            known += usize::from(r.usage.is_some());
            edited += usize::from(r.edited);
            match (r.recorded, r.derived, r.unknown) {
                (true, false, false) => recorded += 1,
                (false, true, false) => derived += 1,
                (false, false, _) => unknown += 1,
                _ => mixed += 1,
            }
            money.merge(&r.money);
            paid += usize::from(r.money.recorded.is_some());
            estimated += usize::from(r.money.estimated.is_some());
            unpriced += usize::from(r.money.unpriced || r.usage.is_none());
            overlap += usize::from(r.money.estimated.is_some() && r.money.recorded.is_some());
            if r.usage.is_none()
                && r.last
                    .zip(self.first_usage.get(&src.harness).copied())
                    .is_some_and(|(at, first)| at < first)
            {
                *no_usage.entry(&src.harness).or_default() += 1;
            }
            if let Some((primary, _)) =
                r.primary.iter().max_by(|a, b| a.1.cmp(b.1).then_with(|| b.0.cmp(a.0)))
            {
                *primaries.entry(primary).or_default() += 1;
            }
            let update = |g: &mut Group| {
                g.roots += 1;
                g.known += u64::from(r.usage.is_some());
                g.timed += u64::from(r.ms.is_some());
                add(&mut g.ms, r.ms);
                tokens(&mut g.usage, r.usage);
                g.harnesses.insert(src.harness.clone());
            };
            update(harness.entry(&src.harness).or_default());
            if let Some(id) = src.project.as_deref() {
                update(projects.entry(id).or_default());
            }
        }
        let n = self.roots.len();
        let note = no_usage
            .into_iter()
            .find(|(_, v)| *v as usize > (n - known) / 2)
            .map(|(h, _)| json!({"harness":h,"firstUsageAt":self.first_usage[h]}));
        let mut overview = json!({"agentBusyMs":self.located.then_some(busy),"agentSummedMs":sum,"presentMs":(self.timed_prompts>0 || self.untimed_prompts==0 || present>0).then_some(present),"timedHumanMessages":self.timed_prompts,"undatedHumanMessages":self.untimed_prompts,
            "concurrency":sum.filter(|_|busy>0).map(|s|s as f64/busy as f64),"recordedDurationConversations":recorded,"derivedDurationConversations":derived,
            "mixedDurationConversations":mixed,"unknownDurationConversations":unknown,"conversationCount":n,"subagentCount":self.children.len(),"projectCount":projects.len(),
            "tokens":usage,"averageTokensPerMeasuredConversation":usage.filter(|_|known>0).map(|t|t.total() as f64/known as f64),"conversationsWithUsage":known,"usageCoverageNote":note});
        if prefs.show_cost {
            let mut total = money.recorded;
            if prefs.estimate_cost {
                add(&mut total, money.estimated);
            }
            overview["cost"] = json!({"totalUsd":total,"availableEstimatedUsd":money.estimated,"recordedUsd":money.recorded,"estimatedUsd":prefs.estimate_cost.then_some(money.estimated).flatten(),
                "recordedConversations":paid,"estimatedConversations":estimated,"bothConversations":overlap,"unpricedConversations":unpriced,"includeEstimates":prefs.estimate_cost,
                "priceTableVersion":crate::chats::pricing::VERSION});
        }
        let mut longest: Vec<_> = self.roots.iter().filter(|(_, r)| r.ms.is_some()).collect();
        let order = |(a, x): &(&usize, &Root), (b, y): &(&usize, &Root)| {
            y.ms.cmp(&x.ms).then_with(|| cat.sources[**a].id.cmp(&cat.sources[**b].id))
        };
        if longest.len() > 6 {
            longest.select_nth_unstable_by(6, order);
            longest.truncate(6);
        }
        longest.sort_unstable_by(order);
        let longest:Vec<_>=longest.into_iter().take(6).map(|(&i,r)|{let s=&cat.sources[i];json!({"conversationId":s.id,"title":s.title,"titleSource":s.title_source,"harness":s.harness,
            "project":cat.project(s),"turnsInRange":r.turns,"agentMsInRange":r.ms})}).collect();
        let mut ranked: Vec<_> = projects.into_iter().collect();
        ranked.sort_by(|a, b| b.1.ms.cmp(&a.1.ms).then(a.0.cmp(b.0)));
        let projects:Vec<_>=ranked.into_iter().take(8).map(|(id,g)|json!({"project":cat.projects.get(id),"harnesses":g.harnesses,"conversations":g.roots,
            "summedMs":g.ms,"tokens":g.usage,"conversationsWithUsage":g.known,"conversationsWithDuration":g.timed})).collect();
        let by_harness:Vec<_>=harness.into_iter().map(|(h,g)|json!({"harness":h,"conversations":g.roots,"summedMs":g.ms,"tokens":self.harness_tokens.get(h).copied().flatten(),"conversationsWithUsage":g.known,"conversationsWithDuration":g.timed})).collect();
        let mut models: Vec<_> = self.model_tokens.into_iter().collect();
        models.sort_by(|a, b| {
            b.1.map(Tokens::total).cmp(&a.1.map(Tokens::total)).then(a.0.cmp(&b.0))
        });
        let model_top: Vec<_> = models.iter().take(5).map(|m| &m.0).collect();
        let model_rows:Vec<_>=models.iter().take(7).map(|(m,t)|json!({"model":(!m.is_empty()).then_some(m),"tokens":t,"primaryConversations":primaries.get(m.as_str()).copied().unwrap_or(0)})).collect();
        let mut tools: Vec<_> = self.tools.into_iter().collect();
        let calls: u64 = tools.iter().map(|(_, n)| n).sum();
        tools.sort_by(|a, b| b.1.cmp(&a.1).then(a.0.cmp(&b.0)));
        let tools: Vec<_> = tools
            .into_iter()
            .take(10)
            .map(|((h, n), count)| json!({"harness":h,"name":n,"count":count}))
            .collect();
        let mut series = Vec::new();
        for (i, b) in self.buckets.into_iter().enumerate() {
            let mut other = None;
            for (m, t) in &b.models {
                if !model_top.contains(&m) {
                    tokens(&mut other, *t)
                }
            }
            let ms = if b.active.is_empty() { Some(0) } else { b.summed };
            series.push(json!({"bucketStart":self.axis.starts[i],"bucketEnd":self.axis.starts.get(i+1).copied().unwrap_or(self.axis.end),"busyMs":ms.map(|_|b.busy),"summedMs":ms,"presentMs":b.present,
                "tokens":b.usage,"tokensByHarness":b.tools,"tokensByModel":model_top.iter().map(|m|b.models.get(*m).copied().flatten()).collect::<Vec<_>>(),"otherModelTokens":other,
                "conversationCount":b.active.len(),"conversationsWithUsage":b.measured.len()}));
        }
        Ok(
            json!({"overview":overview,"series":{"unit":self.axis.unit,"modelKeys":model_top,"otherModelCount":models.len().saturating_sub(5),"buckets":series},
            "breakdowns":{"hourOfWeek":self.rhythm,"byHarness":by_harness,"topProjects":projects,"models":model_rows,"longest":longest,"linesAdded":(self.known_edits>0||self.unknown_edits==0).then_some(self.added),"linesRemoved":(self.known_edits>0||self.unknown_edits==0).then_some(self.removed),
            "conversationsWithEdits":edited,"editsWithUnknownLineCounts":self.unknown_edits,"toolCalls":{"total":calls,"top":tools}}}),
        )
    }
}
