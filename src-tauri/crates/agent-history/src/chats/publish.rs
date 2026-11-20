//! Materialize list keys, native turn intervals and operation annotations while
//! the new reader publication is private. The head switch makes them visible.
use super::facts::{Interpreter, Tokens, Totals, Turn};
use crate::{query::operation, read_model::publication::SourceMember};
use serde_json::{Value, json};
use std::collections::BTreeSet;
use vault_store::{OptionalRow, Result, Session, params};

pub(super) fn order_key(value: Option<i64>, id: &str) -> String {
    match value {
        Some(n) => format!("1{:016x}/{id}", (n as u64) ^ (1 << 63)),
        None => format!("0/{id}"),
    }
}
fn turn(
    tx: &Session<'_>,
    publication: &str,
    source: &str,
    t: Turn,
    total: &mut Totals,
    first: &mut Option<String>,
) -> Result<()> {
    if first.is_none() {
        *first = t.prompt.as_ref().filter(|text| !text.trim().is_empty()).cloned();
    }
    total.merge(&t.facts);
    for response in t.responses.values() {
        tx.execute(
            "INSERT INTO ah_chat_response VALUES(?1,?2,?3,?4,?5)",
            params![
                publication,
                source,
                response.ordinal,
                t.ordinal,
                serde_json::to_string(response).map_err(operation)?
            ],
        )?;
    }
    tx.execute(
        "INSERT INTO ah_chat_turn VALUES(?1,?2,?3,?4,?5,?6,?7,?8)",
        params![
            publication,
            source,
            t.ordinal,
            t.last_ordinal,
            t.started_at,
            t.ended_at,
            t.footer_ordinal,
            t.wire().to_string()
        ],
    )?;
    Ok(())
}
fn initial_counter(
    tx: &Session<'_>,
    publication: &str,
    m: &SourceMember,
) -> Result<crate::read_model::usage::Counter> {
    if m.relation.base_native_id.is_none() {
        return Ok(crate::read_model::usage::Counter {
            from_start: true,
            reasoning: Some(0),
            ..Default::default()
        });
    }
    let sample=tx.query_one(
        "SELECT e.payload FROM ah_reader_line l JOIN ah_sessions s ON s.id=l.source
         JOIN ah_events e ON e.session_id=s.id AND e.projection_id=s.projection_id AND e.ordinal<l.records
         WHERE l.publication=?1 AND l.branch=?2 AND l.source<>?2 AND e.kind='event_msg.token_count'
         ORDER BY l.first_rank DESC,e.ordinal DESC LIMIT 1",
        params![publication,m.source_id.as_str()],|r|Ok(r.get::<String>(0)?)
    ).optional()?;
    let Some(sample) = sample else { return Ok(Default::default()) };
    let record: Value = serde_json::from_str(&sample).map_err(operation)?;
    let u = &record["payload"]["info"]["total_token_usage"];
    Ok(crate::read_model::usage::Counter {
        tokens: crate::adapters::codex::counter(u),
        reasoning: u["reasoning_output_tokens"].as_i64(),
        from_start: false,
    })
}
pub(crate) fn stage_summary(
    tx: &Session<'_>,
    id: &str,
    publication: &str,
    current: &str,
    cancel: &vault_store::CancelToken,
) -> Result<()> {
    let (harness, folder, native_title, mut started, mut last) = tx.query_one(
        "SELECT harness,project_path,title,created_at,updated_at FROM ah_sessions WHERE id=?1",
        [current],
        |r| {
            Ok((
                r.get::<String>(0)?,
                r.get::<Option<String>>(1)?,
                r.get::<Option<String>>(2)?,
                r.get::<Option<i64>>(3)?,
                r.get::<Option<i64>>(4)?,
            ))
        },
    )?;
    let members = tx.query(
        "SELECT descriptor FROM ah_reader_member WHERE publication=?1 ORDER BY source",
        [publication],
        |r| serde_json::from_str::<SourceMember>(&r.get::<String>(0)?).map_err(operation),
    )?;
    let elected = members.iter().find(|m| m.source_id == current).expect("elected membership");
    let mut own = Totals::default();
    let mut activity = std::collections::BTreeMap::<i64, (i64, i64)>::new();
    let mut first_prompt: Option<(i64, String)> = None;
    let mut detail = json!({});
    let mut assignment = None;
    for m in &members {
        // A superseded DeepSeek generation and a byte-identical copy contribute no new work.
        let mut contributes =
            (harness != "deepseek" || m.source_id == current) && m.copy_of.is_none();
        if m.awaiting_rebuild {
            continue;
        }
        let source = m.source_id.as_str();
        let projection =
            tx.query_one("SELECT projection_id FROM ah_sessions WHERE id=?1", [source], |r| {
                Ok(r.get::<i64>(0)?)
            })?;
        if harness == "pi" {
            super::variants::publish(tx, publication, source, projection)?;
        }
        let mut after = -1;
        if harness == "deepseek" {
            after=tx.query_one("SELECT coalesce(max(ordinal),-1) FROM ah_events WHERE session_id=?1 AND projection_id=?2 AND kind='session/end-seed' AND json_extract(payload,'$.data.inherited')=1",params![source,projection],|r|Ok(r.get(0)?))?;
        }
        let unresolved_fork = m.fork_id.is_none()
            && (m.relation.fork_path.is_some() || m.relation.forked_from_native_id.is_some());
        let unresolved_seed=harness=="deepseek" && after<0 && tx.query_one("SELECT EXISTS(SELECT 1 FROM ah_events WHERE session_id=?1 AND projection_id=?2 AND kind='session' AND json_extract(payload,'$.isSeeded')=1)",params![source,projection],|r|Ok(r.get::<i64>(0)?!=0))?;
        if contributes && (unresolved_fork || unresolved_seed) {
            own.ownership_partial = true;
            // Claude envelopes identify the originating session even when its
            // transcript has not arrived. Other missing boundaries stay unknown.
            if harness != "claude-code" || unresolved_seed {
                contributes = false;
            }
        }
        if harness == "codex" {
            super::mirrors::publish(tx, publication, source, projection)?;
        }
        let initial = if harness == "codex" {
            initial_counter(tx, publication, m)?
        } else {
            Default::default()
        };
        let child = m.parent_id.is_some()
            || m.relation.parent_native_id.is_some()
            || m.relation.parent_path.is_some();
        let mut parser = Interpreter::new(harness.clone(), child, initial);
        // Preserve the complete displayed prefix; charge only records produced
        // by this source. Native identity in the resolved origin proves a copy.
        let mut owned_parser = (m.fork_id.is_some() || unresolved_fork || after >= 0)
            .then(|| Interpreter::new(harness.clone(), child, initial));
        let mut insight = crate::insights::Writer::new(publication, source);
        let mut displayed = Totals::default();
        let mut display_title = None;
        let mut q=tx.prepare("SELECT e.ordinal,e.kind,e.role,e.timestamp,substr(e.text,1,1024),e.payload,
            ?3 IS NOT NULL AND e.native_id IS NOT NULL AND EXISTS(SELECT 1 FROM ah_sessions origin JOIN ah_events old
             ON old.session_id=origin.id AND old.projection_id=origin.projection_id AND old.ordinal=e.ordinal
             WHERE origin.conversation_id=?3 AND old.native_id=e.native_id)
            FROM ah_events e WHERE e.session_id=?1 AND e.projection_id=?2 AND NOT EXISTS(SELECT 1 FROM ah_chat_mirror mirror WHERE mirror.publication=?4 AND mirror.source=?1 AND mirror.ordinal=e.ordinal) ORDER BY e.ordinal")?;
        let mut rows = q.query(params![source, projection, m.fork_id.as_deref(), publication])?;
        while let Some(r) = rows.next()? {
            cancel.check()?;
            let n: i64 = r.get(0)?;
            let time: Option<i64> = r.get(3)?;
            let v: Value = serde_json::from_str(&r.get::<String>(5)?).map_err(operation)?;
            let kind: String = r.get(1)?;
            let role: Option<String> = r.get(2)?;
            let text: String = r.get(4)?;
            let inherited = r.get::<i64>(6)? != 0
                || n <= after
                || unresolved_fork
                    && harness == "claude-code"
                    && v["sessionId"].as_str().is_some_and(|id| id != m.native_id);
            let (finished, mut ops) = parser.record(n, &kind, role.as_deref(), time, &text, &v);
            if contributes
                && !inherited
                && let Some(original) = owned_parser.as_mut()
            {
                let (prior, _) = original.record(n, &kind, role.as_deref(), time, &text, &v);
                if let Some(t) = prior {
                    insight.turn(tx, &t)?;
                    own.merge(&t.facts);
                }
            }
            if let Some((code, tone)) = super::operations::notice(&kind, &v) {
                let display = n;
                tx.execute(
                    "INSERT INTO ah_chat_notice VALUES(?1,?2,?3,?4,?5,?6,?7)",
                    params![publication, source, display, n, code, tone, time],
                )?;
            }
            ops.extend(super::attachments::inline(&v).into_iter().map(|image| image.descriptor()));
            if ops.iter().any(|o| o["kind"] == "attachment")
                || super::operations::notice(&kind, &v).is_some()
            {
                tx.execute(
                    "INSERT INTO ah_chat_extra VALUES(?1,?2,?3)",
                    params![publication, source, n],
                )?;
            }
            if let Some(t) = finished {
                if contributes && owned_parser.is_none() {
                    insight.turn(tx, &t)?;
                }
                turn(tx, publication, source, t, &mut displayed, &mut display_title)?;
            }
            if contributes
                && !inherited
                && let Some(at) = time
            {
                let hour = activity.entry(at.div_euclid(3_600_000)).or_insert((at, at));
                hour.0 = hour.0.min(at);
                hour.1 = hour.1.max(at);
                started = Some(started.map_or(at, |s| s.min(at)));
                last = Some(last.map_or(at, |s| s.max(at)));
            }
            for op in &ops {
                let Some(call) = op["callId"].as_str() else { continue };
                if op["kind"] == "call" {
                    tx.execute("INSERT INTO ah_chat_call(publication,source,call_id,name,ordinal) VALUES(?1,?2,?3,?4,?5)
                        ON CONFLICT(publication,source,call_id) DO UPDATE SET name=excluded.name,ordinal=excluded.ordinal",
                        params![publication,source,call,op["name"].as_str(),n])?;
                } else if op["kind"] == "result" {
                    tx.execute("INSERT INTO ah_chat_call(publication,source,call_id,result_ordinal,status,duration,child_ref) VALUES(?1,?2,?3,?4,?5,?6,?7)
                        ON CONFLICT(publication,source,call_id) DO UPDATE SET result_ordinal=excluded.result_ordinal,status=excluded.status,duration=excluded.duration,child_ref=excluded.child_ref",
                        params![publication,source,call,n,op["status"].as_str(),op["durationMs"].as_i64(),op["agentId"].as_str()])?;
                }
            }
            if !ops.is_empty() {
                tx.execute(
                    "INSERT INTO ah_chat_operation VALUES(?1,?2,?3,?4)",
                    params![
                        publication,
                        source,
                        n,
                        serde_json::to_string(&ops).map_err(operation)?
                    ],
                )?;
            }
        }
        if let Some(t) = parser.turn.take() {
            let t = t.finish();
            if contributes && owned_parser.is_none() {
                insight.turn(tx, &t)?;
            }
            turn(tx, publication, source, t, &mut displayed, &mut display_title)?;
        }
        tx.execute("INSERT INTO ah_chat_attachment
            SELECT ?1,?2,CASE WHEN e.kind='attachment' THEN coalesce(
              (SELECT max(t.ordinal) FROM ah_chat_turn t WHERE t.publication=?1 AND t.source=?2 AND t.ordinal<=e.ordinal),
              (SELECT min(t.ordinal) FROM ah_chat_turn t WHERE t.publication=?1 AND t.source=?2),e.ordinal) ELSE e.ordinal END,
              e.ordinal,r.reference_key,coalesce(r.display_name,json_extract(e.payload,'$.attachment.filename')),r.display_kind,o.reason
            FROM ah_event_references r JOIN ah_events e ON e.session_id=?2 AND e.projection_id=r.projection_id AND e.ordinal=r.ordinal
            LEFT JOIN ah_reference_outcomes o ON o.parent_revision=e.revision_id AND o.reference_key=r.reference_key
            WHERE r.projection_id=?3",params![publication,source,projection])?;
        if contributes {
            if let Some(mut original) = owned_parser {
                if let Some(t) = original.turn.take() {
                    let t = t.finish();
                    insight.turn(tx, &t)?;
                    own.merge(&t.facts);
                }
                own.merge(&original.structure);
            } else {
                own.merge(&displayed);
                own.merge(&parser.structure);
            }
        }
        insight.flush(tx)?;
        if (harness != "deepseek" || source == current)
            && let Some(title) = display_title
        {
            let at=tx.query_one("SELECT started FROM ah_chat_turn WHERE publication=?1 AND source=?2 AND json_extract(facts,'$.prompt') IS NOT NULL ORDER BY ordinal LIMIT 1",
                params![publication,source],|r|Ok(r.get::<Option<i64>>(0)?)).optional()?.flatten().unwrap_or(i64::MAX);
            if first_prompt.as_ref().is_none_or(|(prior, _)| at < *prior) {
                first_prompt = Some((at, title));
            }
        }
        if source == current {
            detail = parser.details;
            assignment = parser.assignment;
        }
    }
    for (hour, (lo, hi)) in activity {
        tx.execute(
            "INSERT INTO ah_chat_activity VALUES(?1,?2,?3,?4)",
            params![publication, hour, lo, hi],
        )?;
    }
    if let Some(call) = &elected.launch_call {
        detail["launchCallId"] = json!(call);
    }
    let first_prompt = first_prompt.map(|(_, title)| title).or(assignment);
    let title_source = if native_title.is_some() {
        "tool"
    } else if first_prompt.is_some() {
        "firstPrompt"
    } else {
        "unknown"
    };
    let title =
        native_title.or_else(|| first_prompt.map(|title| title.chars().take(160).collect()));
    let bytes = members.iter().map(|m| m.bytes as i64).sum::<i64>();
    let saved = members.iter().map(|m| m.observation.captured_at).max();
    let missing = members.iter().all(|m| m.observation.missing_since.is_some());
    let root = elected.parent_id.is_none()
        && elected.relation.parent_native_id.is_none()
        && elected.relation.parent_path.is_none();
    own.conversations = 1;
    own.usage_conversations = i64::from(own.tokens.is_some());
    let totals = serde_json::to_string(&own).map_err(operation)?;
    if let Some(path) = folder.as_deref().filter(|p| !p.is_empty()) {
        tx.execute("INSERT OR IGNORE INTO ah_project_paths VALUES(?1)", [path])?;
    }
    tx.execute("INSERT INTO ah_chat_summary(publication,id,root,parent,harness,folder,title,title_source,started,last_activity,missing,bytes,saved,own,totals,recent_key,start_key,agent_key,tokens_key,turns_key,detail,messages,turns,agent_ms,tokens_total,has_edits,has_pr,project_id)
        VALUES(?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11,?12,?13,?14,?14,?15,?16,?17,?18,?19,?20,?21,?22,?23,?24,?25,?26,(SELECT project FROM ah_project_folder WHERE path=?6))",
        params![publication,id,i64::from(root),elected.parent_id.as_deref(),harness,folder,title,title_source,started,last,i64::from(missing),bytes,saved,totals,order_key(last,id),order_key(started,id),order_key(own.agent_ms,id),order_key(own.tokens.map(Tokens::total),id),order_key(Some(own.turns),id),detail.to_string(),own.messages,own.turns,own.agent_ms,own.tokens.map(Tokens::total),i64::from(!own.edited_files.is_empty()),i64::from(!own.pull_requests.is_empty())])?;
    tx.execute("UPDATE ah_chat_summary SET insight_first_usage=(SELECT min(first_usage) FROM ah_insight_page WHERE publication=?1),insight_first=(SELECT min(lo) FROM ah_insight_page WHERE publication=?1),
        insight_last=(SELECT max(hi) FROM ah_insight_page WHERE publication=?1) WHERE publication=?1",[publication])?;
    Ok(())
}

pub(crate) fn switch_summaries(tx: &Session<'_>) -> Result<()> {
    crate::insights::work::switching(tx)?;
    tx.execute(
        "INSERT OR IGNORE INTO ah_search_dirty SELECT conversation FROM ah_reader_build",
        (),
    )?;
    tx.execute("UPDATE ah_chat_clock SET publication=publication+1 WHERE id=1 AND EXISTS(SELECT 1 FROM ah_reader_build)",())?;
    tx.execute("INSERT OR IGNORE INTO ah_project_dirty SELECT s.folder FROM ah_reader_build b JOIN ah_chat_summary s ON s.id=b.conversation
        WHERE s.folder IS NOT NULL AND (s.live=1 OR s.publication=b.publication)",())?;
    let parents=tx.query("SELECT DISTINCT s.parent FROM ah_reader_build b JOIN ah_chat_summary s ON s.id=b.conversation WHERE s.parent IS NOT NULL AND (s.live=1 OR s.publication=b.publication)",(),|r|Ok(r.get::<String>(0)?))?;
    tx.execute("UPDATE ah_chat_summary SET live=0 WHERE live=1 AND id IN (SELECT conversation FROM ah_reader_build)",())?;
    tx.execute("UPDATE ah_chat_summary SET live=1 WHERE publication IN (SELECT publication FROM ah_reader_build)",())?;
    super::subagents::apply_launch_details(tx)?;
    let mut roots: BTreeSet<String> = parents.into_iter().collect();
    roots.extend(tx.query("SELECT b.conversation FROM ah_reader_build b WHERE EXISTS(SELECT 1 FROM ah_chat_summary c WHERE c.live=1 AND c.parent=b.conversation)",(),|r|Ok(r.get::<String>(0)?))?);
    let mut affected = BTreeSet::new();
    for id in roots {
        let mut current = Some(id);
        while let Some(id) = current {
            if !affected.insert(id.clone()) {
                break;
            }
            current = tx
                .query_one("SELECT parent FROM ah_chat_summary WHERE live=1 AND id=?1", [id], |r| {
                    Ok(r.get::<Option<String>>(0)?)
                })
                .optional()?
                .flatten();
        }
    }
    for id in affected {
        crate::projects::rollups::dirty_conversation(tx, &id)?;
        let mut total = Totals::default();
        let mut children = 0;
        tx.for_each("WITH RECURSIVE tree(id) AS (SELECT ?1 UNION SELECT c.id FROM tree t JOIN ah_chat_summary c ON c.parent=t.id AND c.live=1)
          SELECT s.id,s.own FROM tree JOIN ah_chat_summary s ON s.id=tree.id AND s.live=1",[id.as_str()],|r|{
            let mut facts:Totals=serde_json::from_str(&r.get::<String>(1)?).map_err(operation)?;
            if r.get::<String>(0)?!=id {children+=1;facts.agent_ms=None;facts.messages=0;facts.turns=0;facts.recorded_turns=0;facts.derived_turns=0;facts.unknown_duration_turns=0;}
            total.merge(&facts);Ok(())
        })?;
        tx.execute(
            "UPDATE ah_chat_summary SET totals=?1,children=?2,tokens_key=?3,tokens_total=?5,has_edits=?6,has_pr=?7 WHERE id=?4 AND live=1",
            params![
                serde_json::to_string(&total).map_err(operation)?,
                children,
                order_key(total.tokens.map(Tokens::total), &id),
                id,total.tokens.map(Tokens::total),i64::from(!total.edited_files.is_empty()),i64::from(!total.pull_requests.is_empty())
            ],
        )?;
    }
    Ok(())
}
