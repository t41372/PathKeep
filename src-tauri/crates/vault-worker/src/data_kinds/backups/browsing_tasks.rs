//! Browsing contributions to Tasks. The native run ledger, intelligence queue and index queue
//! remain authoritative. The cursor advances each producer only through rows actually delivered.
use super::{
    Refusal, VaultSession, admitted_config, check_reply,
    runs::{decode, limit, page},
    tasks::{Group, ListTasks},
};
use anyhow::Result;
use chrono::{DateTime, Local};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use vault_core::KindState;
use vault_store::{Db, Value as Sql};

#[derive(Default, Serialize, Deserialize)]
struct Cursor {
    positions: [Option<(String, i64)>; 3],
}
struct Row {
    table: usize,
    at: String,
    id: i64,
    value: Value,
}

pub(super) fn list(session: &VaultSession, req: ListTasks) -> Result<Value> {
    let paths = vault_core::project_paths()?;
    let config = admitted_config(&paths)?;
    if config.capture_policy.browsing == KindState::Off {
        return page(vec![], None);
    }
    let vault = session.vault().map_err(anyhow::Error::msg)?;
    let n = limit(req.limit)?;
    let mut cursor: Cursor = decode(req.cursor.as_deref())?;
    let mut rows = Vec::new();
    let today = Local::now().date_naive();
    let (lo, hi) = super::tasks::local_day(today)?;
    // Each SELECT exposes the same narrow columns. Private run payloads and message content
    // never cross this boundary. Native job IDs address the existing action commands.
    let tables = [
        (
            "run",
            Db::Archive,
            "SELECT id,run_type AS operation,status AS state,started_at AS created,finished_at AS ended,NULL AS artifact,0 AS stopped FROM runs WHERE run_type NOT IN ('assistant','mcp_query','ai_index','system','doctor','retention_prune','rekey','archive_restore')",
        ),
        (
            "intelligence",
            Db::Intelligence,
            "SELECT id,job_type AS operation,state,created_at AS created,finished_at AS ended,artifact_json AS artifact,stop_requested AS stopped FROM intelligence_jobs",
        ),
        (
            "index",
            Db::Intelligence,
            "SELECT id,job_type AS operation,state,created_at AS created,finished_at AS ended,NULL AS artifact,0 AS stopped FROM ai_jobs WHERE job_type IN ('index-build','index-clear')",
        ),
    ];
    for (table, (native, db, source)) in tables.into_iter().enumerate() {
        let (before, id) =
            cursor.positions[table].clone().unwrap_or_else(|| ("9999".into(), i64::MAX));
        let (predicate, time) = match req.group {
            Group::Running => ("state IN ('running','started')", "created"),
            Group::Waiting => ("state IN ('queued','paused','stale')", "created"),
            Group::Attention => ("state IN ('failed','partial')", "coalesce(ended,created)"),
            Group::Today => ("ended IS NOT NULL AND ended>=?4 AND ended<?5", "ended"),
        };
        let sql=format!("SELECT id,operation,state,{time},ended,artifact,stopped FROM ({source}) WHERE {predicate}
            AND ({time},id)<(?1,?2) ORDER BY {time} DESC,id DESC LIMIT ?3");
        let mut args = vec![Sql::Text(before), Sql::Integer(id), Sql::Integer(n + 1)];
        if matches!(req.group, Group::Today) {
            for bound in [lo, hi] {
                args.push(Sql::Text(
                    DateTime::from_timestamp_millis(bound)
                        .ok_or(Refusal::InvalidRequest)?
                        .to_rfc3339(),
                ));
            }
        }
        rows.extend(vault.store().read(db, move |s| {
            let origin =
                if native == "run" { Some(super::browsing_runs::origin(s)?) } else { None };
            s.query(&sql, args, |r| read_row(r, table, native, origin.as_deref()))
        })?);
    }
    rows.sort_by(|a, b| {
        b.at.cmp(&a.at).then_with(|| b.table.cmp(&a.table)).then_with(|| b.id.cmp(&a.id))
    });
    let more = rows.len() > req.limit;
    rows.truncate(req.limit);
    let items = rows
        .into_iter()
        .map(|r| {
            cursor.positions[r.table] = Some((r.at, r.id));
            r.value
        })
        .collect();
    if check_reply(&vault, session, true)?.browsing == KindState::Off {
        return Err(Refusal::Off.into());
    }
    page(items, more.then(|| serde_json::to_string(&cursor)).transpose()?)
}
fn read_row(
    r: &vault_store::Row,
    table: usize,
    native: &str,
    origin: Option<&str>,
) -> vault_store::Result<Row> {
    let id = r.get::<i64>(0)?;
    let operation = r.get::<String>(1)?;
    let state = r.get::<String>(2)?;
    let at = r.get::<String>(3)?;
    let ended = r.get::<Option<String>>(4)?;
    let artifact = r
        .get::<Option<String>>(5)?
        .and_then(|s| serde_json::from_str::<Value>(&s).ok())
        .unwrap_or_default();
    let done = artifact.get("processedItems").or(artifact.get("completedSteps"));
    let total = artifact.get("totalItems").or(artifact.get("totalSteps"));
    let progress = done.map(|done| json!({"done":done,"total":total}));
    let commands = actions(native, &state, r.get::<i64>(6)? != 0);
    let millis = |s: &str| DateTime::parse_from_rfc3339(s).ok().map(|t| t.timestamp_millis());
    let display_state = match state.as_str() {
        "success" | "complete" => "succeeded",
        "started" => "running",
        s => s,
    };
    let value = json!({
        "taskId":format!("browsing:{native}/{id}"),"jobId":id,"kind":"browsing",
        "operation":operation,"state":display_state,"at":millis(&at),"endedAt":ended.as_deref().and_then(millis),
        "progress":progress,"attention":matches!(state.as_str(),"failed"|"partial"),
        "runRef":origin.map(|o|format!("browsing:{o}/{id}")),"commands":commands
    });
    Ok(Row { table, at, id, value })
}
fn actions(owner: &str, state: &str, stopped: bool) -> Vec<&'static str> {
    match (owner, state) {
        ("intelligence", "failed" | "cancelled") => vec!["retry_intelligence_job"],
        ("intelligence", "queued") => vec!["cancel_intelligence_job"],
        ("intelligence", "running") if !stopped => vec!["cancel_intelligence_job"],
        ("index", "running" | "queued") => vec!["pause_ai_job", "cancel_ai_job"],
        ("index", "paused" | "stale" | "failed" | "cancelled") => vec!["replay_ai_job"],
        _ => vec![],
    }
}
