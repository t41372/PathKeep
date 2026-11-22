//! Facts from native producer records. Display mirrors and inherited rows may
//! still render through the reader; they do not represent new work here.
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::collections::{BTreeMap, BTreeSet};

use crate::read_model::usage::Counter;
pub(crate) use crate::read_model::usage::Tokens;
fn add_known<T: std::ops::AddAssign + Default>(to: &mut Option<T>, v: Option<T>) {
    if let Some(v) = v {
        *to.get_or_insert_with(Default::default) += v
    }
}
#[derive(Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub(crate) struct PullRequest {
    pub repository: Option<String>,
    pub number: Option<u64>,
    pub url: Option<String>,
}
impl PullRequest {
    fn from_record(v: &Value) -> Option<Self> {
        let url = v["prUrl"].as_str().and_then(|raw| url::Url::parse(raw).ok()).filter(|u| {
            u.scheme() == "https"
                && u.host_str().is_some()
                && u.username().is_empty()
                && u.password().is_none()
        });
        let path = url.as_ref().and_then(|u| u.path_segments()).map(|p| p.collect::<Vec<_>>());
        let github = path.as_deref().and_then(|segments| match segments {
            [owner, repo, "pull", number] => {
                Some((format!("{owner}/{repo}"), number.parse::<u64>().ok()?))
            }
            _ => None,
        });
        let repository = v["prRepository"]
            .as_str()
            .map(str::to_owned)
            .or_else(|| github.as_ref().map(|(repo, _)| repo.clone()));
        let number = v["prNumber"].as_u64().or_else(|| github.map(|(_, n)| n));
        (url.is_some() || repository.is_some() && number.is_some()).then(|| Self {
            repository,
            number,
            url: url.map(String::from),
        })
    }
}
#[derive(Default, Serialize, Deserialize, Clone)]
#[serde(default, rename_all = "camelCase")]
pub(crate) struct Totals {
    pub messages: i64,
    pub turns: i64,
    pub calls: i64,
    pub failed_calls: i64,
    pub added_lines: i64,
    pub removed_lines: i64,
    pub edited_files: BTreeSet<String>,
    pub pull_requests: BTreeSet<PullRequest>,
    pub compactions: i64,
    pub api_errors: i64,
    pub retried_errors: i64,
    pub retry_unknown: i64,
    pub tokens: Option<Tokens>,
    pub reasoning_tokens: Option<i64>,
    pub recorded_usd: Option<f64>,
    pub estimated_usd: Option<f64>,
    pub unpriced_responses: i64,
    pub models: BTreeMap<String, i64>,
    pub agent_ms: Option<i64>,
    pub recorded_turns: i64,
    pub derived_turns: i64,
    pub unknown_duration_turns: i64,
    pub usage_responses: i64,
    pub conversations: i64,
    pub usage_conversations: i64,
    pub measured_models: BTreeSet<String>,
    pub ownership_partial: bool,
    pub unknown_edit_counts: i64,
}
impl Totals {
    pub(crate) fn wire(&self) -> Value {
        json!({
            "messages":self.messages,"turns":self.turns,"calls":self.calls,"failedCalls":self.failed_calls,
            "addedLines":self.added_lines,"removedLines":self.removed_lines,"editedFileCount":self.edited_files.len(),"unknownEditCounts":self.unknown_edit_counts,
            "pullRequests":self.pull_requests,"compactions":self.compactions,"apiErrors":self.api_errors,
            "tokens":self.tokens,"reasoningTokens":self.reasoning_tokens,"recordedUsd":self.recorded_usd,"estimatedUsd":self.estimated_usd,"unpricedResponses":self.unpriced_responses,
            "agentMs":self.agent_ms,"recordedTurns":self.recorded_turns,"derivedTurns":self.derived_turns,"unknownDurationTurns":self.unknown_duration_turns,
            "usageResponses":self.usage_responses,"ownershipPartial":self.ownership_partial})
    }
    pub fn merge(&mut self, x: &Self) {
        self.conversations += x.conversations;
        self.usage_conversations += x.usage_conversations;
        self.ownership_partial |= x.ownership_partial;
        self.measured_models.extend(x.measured_models.iter().cloned());
        self.messages += x.messages;
        self.turns += x.turns;
        self.calls += x.calls;
        self.failed_calls += x.failed_calls;
        self.added_lines += x.added_lines;
        self.removed_lines += x.removed_lines;
        self.edited_files.extend(x.edited_files.iter().cloned());
        self.pull_requests.extend(x.pull_requests.iter().cloned());
        self.compactions += x.compactions;
        self.api_errors += x.api_errors;
        self.retried_errors += x.retried_errors;
        self.retry_unknown += x.retry_unknown;
        if let Some(n) = x.tokens {
            self.tokens.get_or_insert_with(Tokens::default).add(n);
        }
        add_known(&mut self.reasoning_tokens, x.reasoning_tokens);
        add_known(&mut self.recorded_usd, x.recorded_usd);
        add_known(&mut self.estimated_usd, x.estimated_usd);
        self.unpriced_responses += x.unpriced_responses;
        for (m, n) in &x.models {
            *self.models.entry(m.clone()).or_default() += n;
        }
        add_known(&mut self.agent_ms, x.agent_ms);
        self.recorded_turns += x.recorded_turns;
        self.derived_turns += x.derived_turns;
        self.unknown_duration_turns += x.unknown_duration_turns;
        self.usage_responses += x.usage_responses;
        self.unknown_edit_counts += x.unknown_edit_counts;
    }
}
#[derive(Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct Turn {
    pub ordinal: i64,
    pub last_ordinal: i64,
    pub footer_ordinal: i64,
    pub prompt: Option<String>,
    pub started_at: Option<i64>,
    pub ended_at: Option<i64>,
    pub end_reason: &'static str,
    pub duration_source: &'static str,
    pub model: Option<String>,
    pub facts: Totals,
    #[serde(skip)]
    native_turn: bool,
    #[serde(skip)]
    pub responses: BTreeMap<String, Response>,
    #[serde(skip)]
    recorded_ms: Option<i64>,
    #[serde(skip)]
    pub(crate) edits: BTreeMap<String, (Option<i64>, Value)>,
    #[serde(skip)]
    pub(crate) named_calls: Vec<(Option<i64>, String)>,
    #[serde(skip)]
    pub(crate) answering_models: BTreeMap<String, (Option<i64>, String)>,
    #[serde(skip)]
    pub(crate) prompt_at: Option<i64>,
}
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct Response {
    pub ordinal: i64,
    pub at: Option<i64>,
    pub(crate) estimated_usd: Option<f64>,
    pub(crate) tokens: Tokens,
    #[serde(rename = "reasoningTokens")]
    reasoning: Option<i64>,
    #[serde(rename = "recordedUsd")]
    pub(crate) usd: Option<f64>,
    duration_ms: Option<i64>,
    pub(crate) model: Option<String>,
}
impl Turn {
    pub(crate) fn wire(&self) -> Value {
        json!({"ordinal":self.ordinal,"lastOrdinal":self.last_ordinal,"footerOrdinal":self.footer_ordinal,"prompt":self.prompt,
            "startedAt":self.started_at,"endedAt":self.ended_at,"endReason":self.end_reason,"durationSource":self.duration_source,
            "model":self.model,"facts":self.facts.wire()})
    }
    pub fn finish(mut self) -> Self {
        for r in self.responses.values() {
            self.facts.tokens.get_or_insert_with(Tokens::default).add(r.tokens);
            self.facts.usage_responses += 1;
            add_known(&mut self.facts.reasoning_tokens, r.reasoning);
            add_known(&mut self.facts.recorded_usd, r.usd);
            add_known(&mut self.facts.estimated_usd, r.estimated_usd);
            self.facts.unpriced_responses +=
                i64::from(r.usd.is_none() && r.estimated_usd.is_none());
            if let Some(m) = &r.model {
                self.facts.measured_models.insert(m.clone());
                *self.facts.models.entry(m.clone()).or_default() += r.tokens.output;
            }
        }
        for (_, edit) in self.edits.values() {
            for path in edit["files"].as_array().into_iter().flatten().filter_map(Value::as_str) {
                self.facts.edited_files.insert(path.to_string());
            }
            self.facts.added_lines += edit["addedLines"].as_i64().unwrap_or(0);
            self.facts.removed_lines += edit["removedLines"].as_i64().unwrap_or(0);
            self.facts.unknown_edit_counts += i64::from(edit["addedLines"].is_null());
        }
        self.facts.turns = i64::from(self.prompt.is_some() || self.native_turn);
        self.facts.messages = i64::from(self.prompt.is_some());
        self.facts.agent_ms = self.recorded_ms.or_else(|| {
            self.ended_at.zip(self.started_at).and_then(|(e, s)| {
                (e >= s && (self.last_ordinal != self.ordinal || self.end_reason == "nextPrompt"))
                    .then_some(e - s)
            })
        });
        self.duration_source = if self.recorded_ms.is_some() {
            self.facts.recorded_turns = 1;
            "recorded"
        } else if self.facts.agent_ms.is_some() {
            self.facts.derived_turns = 1;
            "derived"
        } else {
            self.facts.unknown_duration_turns = 1;
            "unknown"
        };
        self
    }
}

pub(crate) struct Interpreter {
    pub harness: String,
    pub turn: Option<Turn>,
    pub model: Option<String>,
    counter: Counter,
    pub details: Value,
    pub assignment: Option<String>,
    pub structure: Totals,
    child: bool,
    context: crate::read_model::RecordContext,
    pending_start: Option<i64>,
}
impl Interpreter {
    pub fn new(harness: String, child: bool, counter: Counter) -> Self {
        let context =
            crate::read_model::RecordContext::new(harness.parse().expect("published harness"));
        Self {
            context,
            harness,
            child,
            pending_start: None,
            counter,
            turn: None,
            model: None,
            structure: Totals::default(),
            details: json!({}),
            assignment: None,
        }
    }
    pub fn record(
        &mut self,
        n: i64,
        kind: &str,
        role: Option<&str>,
        time: Option<i64>,
        text: &str,
        v: &Value,
    ) -> (Option<Turn>, Vec<Value>) {
        let codex = self.harness == "codex";
        let deep = self.harness == "deepseek";
        let p = &v["payload"];
        let m = if deep {
            &v["data"]["message"]
        } else if codex {
            p
        } else {
            &v["message"]
        };
        let content = m.get("content").unwrap_or(&v["content"]);
        let tool_result =
            content.as_array().is_some_and(|a| a.iter().any(|b| b["type"] == "tool_result"));
        let prompt = role == Some("user")
            && !tool_result
            && match self.harness.as_str() {
                "codex" => matches!(
                    kind,
                    "response_item.message"
                        | "event_msg.user_message"
                        | "event_msg.item_completed.UserMessage"
                ),
                "deepseek" => kind == "user/message" && v["data"]["source"]["kind"] == "user",
                "claude-code" => {
                    v["isMeta"] != true && !text.trim_start().starts_with("[Request interrupted")
                }
                _ => true,
            };
        // A parent's assignment names the child without counting as a human message.
        if self.child && prompt && self.assignment.is_none() && !text.trim().is_empty() {
            self.assignment = Some(text.chars().take(160).collect());
        }
        let prompt = prompt && !self.child;
        let mut finished = None;
        if kind == "turn/start" {
            self.pending_start = time;
        }
        if prompt || self.child && kind == "turn/start" {
            if let Some(mut old) = self.turn.take() {
                if old.end_reason == "open" {
                    old.end_reason = "nextPrompt";
                    old.ended_at = time;
                }
                finished = Some(old.finish());
            }
            self.turn = Some(Turn {
                ordinal: n,
                last_ordinal: n,
                footer_ordinal: n,
                prompt_at: if prompt { time } else { None },
                prompt: prompt.then(|| {
                    text.lines().take(2).collect::<Vec<_>>().join("\n").chars().take(240).collect()
                }),
                native_turn: true,
                started_at: self.pending_start.take().or(time),
                ended_at: time,
                end_reason: "open",
                ..Default::default()
            });
        }
        if matches!(kind, "session_meta" | "turn_context" | "model_change" | "request/header") {
            self.model = p
                .get("model")
                .or(v.get("modelId"))
                .or(v.pointer("/data/header/config/model"))
                .and_then(Value::as_str)
                .map(String::from)
                .or(self.model.take());
        }
        for (field, path) in [
            ("startCommit", "/payload/git/commit_hash"),
            ("effort", "/payload/effort"),
            ("permissionMode", "/permissionMode"),
            ("launchCallId", "/sourceToolUseID"),
            ("launchAssistantId", "/sourceToolAssistantUUID"),
            ("agentType", "/agentType"),
            ("agentId", "/agentId"),
            ("agentType", "/subagentType"),
        ] {
            if let Some(s) = v.pointer(path).and_then(Value::as_str)
                && (field != "startCommit" || self.details[field].is_null())
            {
                self.details[field] = json!(s);
            }
        }
        if kind == "pr-link"
            && let Some(pr) = PullRequest::from_record(v)
        {
            self.structure.pull_requests.insert(pr);
        }
        if kind == "session"
            && let crate::read_model::RecordContext::Deepseek { version } = &mut self.context
        {
            *version = v["version"].as_u64();
        }
        if self
            .context
            .boundary(kind, v)
            .is_some_and(|b| b.kind == crate::read_model::BoundaryKind::Compaction)
        {
            self.structure.compactions += 1;
        }
        if matches!(super::operations::notice(kind, v), Some(("apiError", _))) {
            self.structure.api_errors += 1;
            match v.get("retried").or(v.pointer("/payload/retried")).and_then(Value::as_bool) {
                Some(true) => self.structure.retried_errors += 1,
                None => self.structure.retry_unknown += 1,
                _ => {}
            }
        }
        let assistant = role == Some("assistant") && (!deep || kind == "assistant/message");
        let work = assistant
            || role == Some("tool")
            || self.child && role == Some("user")
            || kind == "event_msg.token_count";
        if self.turn.is_none() && work {
            self.turn = Some(Turn {
                ordinal: n,
                last_ordinal: n,
                footer_ordinal: n,
                native_turn: self.child || self.pending_start.is_some(),
                started_at: self
                    .pending_start
                    .take()
                    .or_else(|| self.child.then_some(time).flatten()),
                ended_at: time,
                end_reason: "open",
                ..Default::default()
            });
        }
        let Some(t) = self.turn.as_mut() else { return (finished, vec![]) };
        if role.is_some() && (!text.is_empty() || content.is_array()) {
            t.footer_ordinal = n;
        }
        let ended =
            matches!(kind, "turn/end" | "event_msg.task_complete" | "event_msg.turn_complete");
        let interrupted = matches!(kind, "event_msg.turn_aborted" | "turn/abort")
            || v.pointer("/data/reason/kind") == Some(&json!("interrupted"))
            || text.starts_with("[Request interrupted");
        let duration =
            (v["subtype"] == "turn_duration").then(|| v["durationMs"].as_i64()).flatten();
        if prompt || work || ended || interrupted || duration.is_some() || kind == "tool/call" {
            t.last_ordinal = n;
            t.ended_at = time;
        }
        if duration.is_some() {
            t.recorded_ms = duration;
        }
        if interrupted {
            t.footer_ordinal = n;
            t.end_reason = "interrupted";
        } else if ended
            || duration.is_some()
            || assistant
                && (matches!(
                    m["stopReason"].as_str().or(m["stop_reason"].as_str()),
                    Some("stop" | "end_turn")
                ) || p.pointer("/item/phase").and_then(Value::as_str) == Some("final_answer")
                    || !matches!(
                        m["stopReason"].as_str().or(m["stop_reason"].as_str()),
                        Some("toolUse" | "tool_use")
                    ) && (content.is_string()
                        || content.as_array().is_some_and(|blocks| {
                            blocks.iter().any(|b| {
                                matches!(b["type"].as_str(), Some("text" | "output_text" | "Text"))
                            })
                        })))
        {
            t.end_reason = "completed";
        }
        let model = m
            .get("model")
            .or(m.pointer("/source/model"))
            .and_then(Value::as_str)
            .map(String::from)
            .or_else(|| self.model.clone());
        let response_key = || {
            m["responseId"]
                .as_str()
                .or(m["id"].as_str())
                .map(str::to_owned)
                .unwrap_or_else(|| n.to_string())
        };
        if assistant && let Some(m) = &model {
            t.answering_models.insert(response_key(), (time, m.clone()));
            t.model = Some(m.clone());
            t.facts.models.entry(m.clone()).or_default();
        }
        let u = if deep { &v["data"]["usage"] } else { &m["usage"] };
        let (amount, reasoning) = match &self.context {
            crate::read_model::RecordContext::Codex if kind == "event_msg.token_count" => {
                crate::adapters::codex::usage(&mut self.counter, &p["info"])
            }
            crate::read_model::RecordContext::Pi { .. } if assistant => {
                crate::adapters::pi::usage(u)
            }
            crate::read_model::RecordContext::Claude if assistant => {
                crate::adapters::claude::usage(u)
            }
            crate::read_model::RecordContext::Deepseek { .. } if assistant => {
                crate::adapters::deepseek::usage(u)
            }
            _ => (None, None),
        };
        if let Some(tokens) = amount {
            let key = response_key();
            let usd = u.pointer("/cost/total").and_then(Value::as_f64).or(v["costUSD"].as_f64());
            let local = m["provider"]
                .as_str()
                .or(m.pointer("/source/provider").and_then(Value::as_str))
                .is_some_and(|p| matches!(p, "ollama" | "local" | "llama.cpp" | "lmstudio"));
            let estimated_usd = if usd.is_none() && !local {
                super::pricing::estimate(
                    model.as_deref(),
                    tokens,
                    u.pointer("/cache_creation/ephemeral_1h_input_tokens")
                        .and_then(Value::as_i64)
                        .unwrap_or(0),
                )
            } else {
                None
            };
            t.responses.insert(
                key,
                Response {
                    tokens,
                    ordinal: n,
                    at: time,
                    duration_ms: m["durationMs"].as_i64(),
                    estimated_usd,
                    usd,
                    reasoning,
                    model,
                },
            );
        }
        let ops = super::operations::record(&mut t.facts, kind, v, content, codex, deep);
        for op in &ops {
            if op["kind"] == "call"
                && let Some(name) = op["name"].as_str()
            {
                t.named_calls.push((time, name.into()));
            }
            if let Some(edit) = op.get("edit") {
                let key =
                    op["callId"].as_str().map(str::to_owned).unwrap_or_else(|| format!("#{n}"));
                t.edits.insert(key, (time, edit.clone()));
            }
            if matches!(op["status"].as_str(), Some("error" | "denied"))
                && let Some(id) = op["callId"].as_str()
            {
                t.edits.remove(id);
            }
        }
        (finished, ops)
    }
}
