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
use vault_core::{KindState, Vault};
use vault_store::{Db, OptionalRow, Value as Sql};

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
    let with_browsing = config.capture_policy.browsing != KindState::Off && !req.archive_only();
    let vault = session.vault().map_err(anyhow::Error::msg)?;
    let n = limit(req.limit)?;
    let mut cursor: Cursor = decode(req.cursor.as_deref())?;
    let mut rows = Vec::new();
    let today = Local::now().date_naive();
    let (lo, hi) = super::tasks::local_day(today)?;
    for (table, (native, db, source)) in sources(with_browsing).into_iter().enumerate() {
        let (before, id) =
            cursor.positions[table].clone().unwrap_or_else(|| ("9999".into(), i64::MAX));
        let (predicate, time) = match req.group {
            Group::Running => ("state IN ('running','started')", "created"),
            Group::Waiting => ("state IN ('queued','paused','stale')", "created"),
            Group::Attention => {
                ("state IN ('failed','partial') AND NOT dismissed", "coalesce(ended,created)")
            }
            Group::Today => ("ended IS NOT NULL AND ended>=?4 AND ended<?5", "ended"),
        };
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
        let (source, marker) = if matches!(req.group, Group::Attention | Group::Today) {
            (with_dismissals(&vault, native, &source, &mut args)?, "dismissed")
        } else {
            (source, "0")
        };
        let sql=format!("SELECT id,operation,state,{time},ended,artifact,stopped,wide,{marker} FROM ({source}) WHERE {predicate}
            AND ({time},id)<(?1,?2) ORDER BY {time} DESC,id DESC LIMIT ?3");
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
    if check_reply(&vault, session, true)?.browsing == KindState::Off && with_browsing {
        return Err(Refusal::Off.into());
    }
    page(items, more.then(|| serde_json::to_string(&cursor)).transpose()?)
}
// These projections serve list, summary and dismissal with one ownership rule. Archive
// maintenance remains visible when this installation has switched browsing off.
fn sources(with_browsing: bool) -> Vec<(&'static str, Db, String)> {
    use super::browsing_runs::{PRIVATE, WIDE};
    let filter = if with_browsing {
        format!("run_type NOT IN ({PRIVATE})")
    } else {
        format!("run_type IN ({WIDE})")
    };
    let mut sources = vec![(
        "run",
        Db::Archive,
        format!(
            "SELECT id,run_type AS operation,status AS state,started_at AS created,
        finished_at AS ended,NULL AS artifact,0 AS stopped,run_type IN ({WIDE}) AS wide,
        json_array(id,started_at,coalesce(finished_at,started_at)) AS stamp FROM runs WHERE {filter}"
        ),
    )];
    if with_browsing {
        sources.extend([("intelligence",Db::Intelligence,"SELECT id,job_type AS operation,state,created_at AS created,
            finished_at AS ended,artifact_json AS artifact,stop_requested AS stopped,0 AS wide,
            json_array(id,created_at,attempt,coalesce(finished_at,updated_at)) AS stamp FROM intelligence_jobs".into()),
            ("index",Db::Intelligence,"SELECT id,job_type AS operation,state,created_at AS created,finished_at AS ended,
            NULL AS artifact,0 AS stopped,0 AS wide,json_array(id,created_at,attempt,coalesce(finished_at,updated_at)) AS stamp
            FROM ai_jobs WHERE job_type IN ('index-build','index-clear')".into())]);
    }
    sources
}
// Only explicit dismissals occupy metadata rows. Runs can seek their key in the same
// database. The queues receive just their family's acknowledgement keys as a bound set.
fn with_dismissals(v: &Vault, native: &str, source: &str, args: &mut Vec<Sql>) -> Result<String> {
    let prefix = dismissal_prefix(native);
    let predicate = if native == "run" {
        format!("EXISTS(SELECT 1 FROM archive_meta WHERE name='{prefix}' || task.stamp)")
    } else {
        let keys = v.store().read(Db::Archive, move |s| {
            s.query_one("SELECT json_group_array(substr(name,length(?1)+1)) FROM archive_meta WHERE name GLOB ?2",
                vault_store::params![prefix.as_str(), format!("{prefix}*")], |r| Ok(r.get::<String>(0)?))
        })?;
        args.push(Sql::Text(keys));
        format!("task.stamp IN (SELECT value FROM json_each(?{}))", args.len())
    };
    Ok(format!("SELECT task.*, {predicate} AS dismissed FROM ({source}) task"))
}

fn dismissal_prefix(native: &str) -> String {
    if native == "run" {
        "taskDismissal/run/".into()
    } else {
        format!("taskDismissal/job/{native}/")
    }
}

pub(super) fn summary(v: &Vault, with_browsing: bool) -> Result<(Vec<Value>, i64, i64)> {
    let mut running = Vec::new();
    let mut waiting = 0;
    let mut attention = 0;
    for (table, (native, db, source)) in sources(with_browsing).into_iter().enumerate() {
        let mut args = Vec::new();
        let counted = with_dismissals(v, native, &source, &mut args)?;
        let (active, queued, failed) = v.store().read(db, move |s| {
            let origin =
                if native == "run" { Some(super::browsing_runs::origin(s)?) } else { None };
            let active = s.query(
                &format!(
                    "SELECT id,operation,state,created,ended,artifact,stopped,wide,
                0 FROM ({source}) WHERE state IN ('started','running') ORDER BY created,id"
                ),
                (),
                |r| Ok(read_row(r, table, native, origin.as_deref())?.value),
            )?;
            let counts = s.query_one(
                &format!(
                    "SELECT count(CASE WHEN state IN ('queued','paused','stale') THEN 1 END),
                count(CASE WHEN state IN ('failed','partial') AND NOT dismissed THEN 1 END)
                FROM ({counted}) WHERE state IN ('queued','paused','stale','failed','partial')"
                ),
                args,
                |r| Ok((r.get::<i64>(0)?, r.get::<i64>(1)?)),
            )?;
            Ok((active, counts.0, counts.1))
        })?;
        running.extend(active);
        waiting += queued;
        attention += failed;
    }
    Ok((running, waiting, attention))
}

pub(super) fn dismiss(session: &VaultSession, task: &str) -> Result<Value> {
    let (kind, key) = task.split_once(':').ok_or(Refusal::InvalidRequest)?;
    let (native, id) = key.split_once('/').ok_or(Refusal::InvalidRequest)?;
    let id = id.parse::<i64>().map_err(|_| Refusal::InvalidRequest)?;
    let cfg = admitted_config(&vault_core::project_paths()?)?;
    if kind == "browsing" && cfg.capture_policy.browsing == KindState::Off {
        return Err(Refusal::Off.into());
    }
    let v = session.vault().map_err(anyhow::Error::msg)?;
    let wide = match kind {
        "archive" => true,
        "browsing" => false,
        _ => return Err(Refusal::InvalidRequest.into()),
    };
    let (_, db, source) = sources(!wide)
        .into_iter()
        .find(|(name, _, _)| *name == native)
        .ok_or(Refusal::InvalidRequest)?;
    let stamp = v.store().read(db, move |s| {
        s.query_one(&format!("SELECT stamp FROM ({source}) WHERE id=?1 AND wide=?2 AND state IN ('failed','partial')"),
            vault_store::params![id,i64::from(wide)], |r| Ok(r.get::<String>(0)?)).optional()
    })?.ok_or(Refusal::ActionUnavailable)?;
    let key = format!("{}{stamp}", dismissal_prefix(native));
    let changed = v.store().write(Db::Archive, vault_store::Priority::Interactive, move |s| {
        s.execute(
            "INSERT INTO archive_meta(name,value) VALUES(?1,'1') ON CONFLICT(name) DO NOTHING",
            [key],
        )
    })?;
    if changed > 0 {
        super::notify_browsing_tasks(&session.conversations, v.paths());
    }
    check_reply(&v, session, true)?;
    Ok(json!({"taskId":task,"dismissed":true}))
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
    let attention = matches!(state.as_str(), "failed" | "partial") && r.get::<i64>(8)? == 0;
    let kind = if r.get::<i64>(7)? == 0 { "browsing" } else { "archive" };
    let mut commands = actions(native, &state, r.get::<i64>(6)? != 0);
    if attention {
        commands.push("dismiss");
    }
    let millis = |s: &str| DateTime::parse_from_rfc3339(s).ok().map(|t| t.timestamp_millis());
    let display_state = match state.as_str() {
        "success" | "complete" => "succeeded",
        "started" => "running",
        s => s,
    };
    let value = json!({
        "taskId":format!("{kind}:{native}/{id}"),"jobId":id,"kind":kind,
        "operation":operation,"state":display_state,"at":millis(&at),"endedAt":ended.as_deref().and_then(millis),
        "progress":progress,"attention":attention,
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
