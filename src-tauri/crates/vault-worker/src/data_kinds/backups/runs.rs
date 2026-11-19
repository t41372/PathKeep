//! Read the two independent ledgers in bounded pages. Cursors record the last
//! consumed key in each ledger so an unconsumed sibling is returned next time.
use super::*;
use serde::de::DeserializeOwned;
use vault_store::{OptionalRow, params};

/// Request one ordered page across the ledgers visible in a space.
#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ListRuns {
    space: Option<DataKind>,
    include_archive_wide: bool,
    cursor: Option<String>,
    limit: usize,
}
/// Resolve a stable deep link independently of the recent-runs page.
#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct GetRun {
    run_ref: String,
    detail_cursor: Option<String>,
}
#[derive(Serialize, Deserialize, Default)]
struct Cursor {
    agent: Option<(i64, i64)>,
    browsing: Option<(String, i64)>,
    scope: Option<String>,
}
#[derive(Default, Serialize, Deserialize)]
struct DetailCursor {
    run: String,
    warnings: usize,
    roots: usize,
}

pub(super) fn limit(n: usize) -> Result<i64> {
    if !(1..=100).contains(&n) {
        return Err(Refusal::InvalidRequest.into());
    }
    Ok(n as i64)
}
pub(super) fn decode<T: DeserializeOwned + Default>(cursor: Option<&str>) -> Result<T> {
    cursor
        .map(|v| serde_json::from_str(v).map_err(|_| Refusal::InvalidCursor.into()))
        .unwrap_or_else(|| Ok(T::default()))
}
pub(super) fn page(items: Vec<Value>, next: Option<String>) -> Result<Value> {
    let token = hash(&items)?;
    let mut result =
        json!({"items":items,"viewToken":token,"coverage":{"state":"complete","reasons":[]}});
    if let Some(cursor) = next {
        result["nextCursor"] = json!(cursor);
    }
    Ok(result)
}

const AGENT_SELECT: &str = "SELECT r.id,r.public_id,r.status,r.started_at,r.finished_at,
    d.trigger,d.scope_json,d.retry_of,d.acknowledged,r.report_json,d.task_id,d.stop_reason
    FROM ah_backup_runs r LEFT JOIN ah_desktop_backups d ON d.run_id=r.id";

pub(super) struct Record {
    pub id: i64,
    pub public: String,
    pub status: String,
    pub started: i64,
    pub finished: Option<i64>,
    pub trigger: String,
    pub scope: Option<Scope>,
    pub retry_of: Option<String>,
    pub ack: bool,
    pub report: Value,
    pub task: Option<String>,
    pub stop_reason: Option<String>,
}

impl Record {
    pub(super) fn read(r: &vault_store::Row) -> vault_store::Result<Self> {
        let scope: Option<String> = r.get(6)?;
        let report: Option<String> = r.get(9)?;
        fn parse<T: DeserializeOwned>(text: &str) -> vault_store::Result<T> {
            serde_json::from_str(text).map_err(|e| vault_store::Error::Operation(Box::new(e)))
        }
        Ok(Self {
            id: r.get(0)?,
            public: r.get(1)?,
            status: r.get(2)?,
            started: r.get(3)?,
            finished: r.get(4)?,
            trigger: r.get::<Option<String>>(5)?.unwrap_or_else(|| "manual".into()),
            scope: scope.as_deref().map(parse).transpose()?,
            retry_of: r.get(7)?,
            ack: r.get::<Option<i64>>(8)?.unwrap_or_default() != 0,
            report: report.as_deref().map(parse).transpose()?.unwrap_or_default(),
            task: r.get(10)?,
            stop_reason: r.get(11)?,
        })
    }
    pub fn row(&self, vault: &Vault) -> Result<Value> {
        let summary = totals(vault, self.id, None, None)?;
        let warnings = self.report["issues"]
            .as_array()
            .into_iter()
            .flatten()
            .map(|issue| {
                if issue["code"] == "diagnostics_omitted" {
                    issue["detail"].as_str().and_then(|s| s.parse::<u64>().ok()).unwrap_or(1)
                } else {
                    1
                }
            })
            .sum::<u64>();
        let mut row = json!({"runRef":format!("agent-history:{}",self.public),"scope":"agent-history","operation":"backup","trigger":self.trigger,
            "result":result_name(&self.status),"startedAt":self.started,"summary":summary,"warningCount":warnings.to_string()});
        if let Some(end) = self.finished {
            row["finishedAt"] = json!(end);
            row["durationMs"] = json!(end.saturating_sub(self.started));
        }
        if let Some(batch) = &self.task {
            row["batchId"] = json!(batch);
        }
        if self.task.is_none() {
            for (field, key) in [
                ("filesCaptured", "captured"),
                ("filesUnchanged", "unchanged"),
                ("sourceBytes", "bytes"),
            ] {
                row["summary"][field] = json!(self.report[key].as_u64().map(|v| v.to_string()));
            }
            for field in [
                "filesVisited",
                "filesAbsent",
                "newRoots",
                "updatedRoots",
                "newChildren",
                "unknownFormat",
                "unavailable",
                "unstable",
                "unclassified",
                "attachmentsChanged",
            ] {
                row["summary"][field] = Value::Null;
            }
        }
        Ok(row)
    }
}
pub(super) fn record(vault: &Vault, public: &str) -> Result<Record> {
    let public = public.to_owned();
    vault
        .store()
        .read(Db::AgentHistoryEvidence, move |s| {
            s.query_one(&format!("{AGENT_SELECT} WHERE r.public_id=?1"), [public], Record::read)
                .optional()
        })?
        .ok_or_else(|| Refusal::Missing.into())
}

pub(super) fn records(vault: &Vault, after: Option<(i64, i64)>, n: i64) -> Result<Vec<Record>> {
    let (time, id) = after.unwrap_or((i64::MAX, i64::MAX));
    Ok(vault.store().read(Db::AgentHistoryEvidence, move |s| {
        s.query(
            &format!(
                "{AGENT_SELECT}
        WHERE (r.started_at,r.id)<(?1,?2) ORDER BY r.started_at DESC,r.id DESC LIMIT ?3"
            ),
            params![time, id, n],
            Record::read,
        )
    })?)
}

/// Merge only the eligible page from each indexed ledger.
pub fn list_runs(session: &VaultSession, req: ListRuns) -> Result<Value> {
    let paths = vault_core::project_paths()?;
    let policy = admitted_config(&paths)?.capture_policy;
    if req.space == Some(DataKind::Browsing) && policy.browsing == KindState::Off {
        return Err(Refusal::Off.into());
    }
    let browsing = req.space != Some(DataKind::AgentHistory) && policy.browsing != KindState::Off;
    let vault = if req.space == Some(DataKind::Browsing) {
        session.vault().map_err(anyhow::Error::msg)?
    } else {
        // An unfiltered ledger must disclose a locally unavailable conversation role.
        admitted(session, req.space.is_none())?.0
    };
    let agents =
        req.space != Some(DataKind::Browsing) && vault.agent_visibility(&policy)? != KindState::Off;
    let n = limit(req.limit)?;
    let mut cursor: Cursor = decode(req.cursor.as_deref())?;
    let filter = hash(&(req.space, req.include_archive_wide, policy.token()))?;
    if req.cursor.is_some() && cursor.scope.as_ref() != Some(&filter) {
        return Err(Refusal::InvalidCursor.into());
    }
    cursor.scope = Some(filter);
    let agent = if agents { records(&vault, cursor.agent, n + 1)? } else { vec![] };
    let browser = if browsing || req.include_archive_wide {
        super::browsing_runs::page(
            &vault,
            browsing,
            req.include_archive_wide,
            cursor.browsing.clone(),
            n + 1,
        )?
    } else {
        vec![]
    };
    let mut a = agent.into_iter().peekable();
    let mut b = browser.into_iter().peekable();
    let mut items = Vec::new();
    while items.len() < req.limit && (a.peek().is_some() || b.peek().is_some()) {
        if a.peek().is_some_and(|r| {
            b.peek()
                .is_none_or(|(_, _, v)| r.started >= v["startedAt"].as_i64().unwrap_or_default())
        }) {
            let row = a.next().unwrap();
            cursor.agent = Some((row.started, row.id));
            items.push(row.row(&vault)?);
        } else if let Some((date, id, row)) = b.next() {
            cursor.browsing = Some((date, id));
            items.push(row);
        }
    }
    let next = (a.peek().is_some() || b.peek().is_some())
        .then(|| serde_json::to_string(&cursor))
        .transpose()?;
    if check_reply(&vault, session, !agents)? != policy {
        return Err(Refusal::PolicyChanged.into());
    }
    page(items, next)
}

/// Return the detail for one public reference or legacy browsing ID.
pub fn get_run(session: &VaultSession, req: GetRun) -> Result<Value> {
    if let Some(public) = req.run_ref.strip_prefix("agent-history:") {
        let (vault, _) = admitted(session, false)?;
        let record = record(&vault, public)?;
        let mut row = record.row(&vault)?;
        let mut cursor: DetailCursor = decode(req.detail_cursor.as_deref())?;
        if req.detail_cursor.is_some() && cursor.run != public {
            return Err(Refusal::InvalidCursor.into());
        }
        cursor.run = public.to_owned();
        let skip = cursor.warnings;
        let warnings = record.report["issues"].as_array().map(Vec::as_slice).unwrap_or_default();
        if skip > warnings.len() {
            return Err(Refusal::InvalidCursor.into());
        }
        let mut slice = warnings.iter().skip(skip).take(50).cloned().collect::<Vec<_>>();
        for (index, warning) in slice.iter_mut().enumerate() {
            if let Some(origin) = record.report["warningSources"].get((skip + index).to_string()) {
                warning["rootId"] = origin["rootId"].clone();
                warning["tool"] = origin["tool"].clone();
            }
            if let Some(ordinal) = warning["ordinal"].as_u64() {
                warning["ordinal"] = json!(ordinal.to_string());
            }
        }
        cursor.warnings += slice.len();
        row["warnings"] = json!(slice);
        let mut more = cursor.warnings < warnings.len();
        if let Some(scope) = record.scope {
            row["readGit"] = json!(scope.read_git);
            let mut outcomes = Vec::new();
            if cursor.roots > scope.roots.len() {
                return Err(Refusal::InvalidCursor.into());
            }
            for root in scope.roots.iter().skip(cursor.roots).take(50) {
                let id = root.id.clone();
                let run = record.id;
                let availability = vault.store().read(Db::AgentHistoryEvidence, move |tx| {
                    tx.query_one(
                        "SELECT state FROM ah_run_roots WHERE run_id=?1 AND root_id=?2",
                        params![run, id],
                        |r| Ok(r.get::<String>(0)?),
                    )
                    .optional()
                })?;
                outcomes.push(json!({"rootId":root.id,"tool":root.tool,"path":root.path,"availability":availability.as_deref().unwrap_or("unobserved"),
                    "counts":totals(&vault,run,Some(root),None)?}));
            }
            cursor.roots += outcomes.len();
            more |= cursor.roots < scope.roots.len();
            row["roots"] = json!(outcomes);
            let tools =
                scope.roots.iter().map(|root| root.tool).collect::<std::collections::BTreeSet<_>>();
            let mut per_tool = Vec::new();
            for tool in tools {
                let run = record.id;
                let states=vault.store().read(Db::AgentHistoryEvidence,move |s|s.query(
                    "SELECT state,COUNT(*) FROM ah_run_roots WHERE run_id=?1 AND tool=?2 GROUP BY state",
                    params![run,tool.harness().as_str()],|r|Ok((r.get::<String>(0)?,r.get::<i64>(1)?.to_string()))))?;
                let availability = match states.as_slice() {
                    [] => "stale",
                    [(state, _)] => state.as_str(),
                    _ => "partial",
                };
                let outcomes = states.iter().cloned().collect::<std::collections::BTreeMap<_, _>>();
                per_tool.push(json!({"tool":tool,"availability":availability,"rootOutcomes":outcomes,"counts":totals(&vault,run,None,Some(tool))?}));
            }
            row["perTool"] = json!(per_tool);
        }
        if more {
            row["nextCursor"] = json!(serde_json::to_string(&cursor)?);
        }
        row["coverage"] = match record.status.as_str() {
            "complete" => json!({"state":"complete","reasons":[]}),
            "running" => json!({"state":"building","reasons":[]}),
            other => json!({"state":"partial","reasons":[result_name(other)]}),
        };
        check_reply(&vault, session, false)?;
        return Ok(row);
    }
    super::browsing_runs::detail(session, &req.run_ref)
}

/// Only the small per-root totals participate in a run list or a detail page.
/// Their publication transaction also publishes the source/revision they count.
fn totals(
    vault: &Vault,
    run: i64,
    root: Option<&selection::Root>,
    tool: Option<AgentTool>,
) -> Result<Value> {
    let filter =
        root.map(|r| (r.source().harness.to_string(), r.path.to_string_lossy().into_owned()));
    let tool =
        tool.map(serde_json::to_value).transpose()?.and_then(|v| v.as_str().map(str::to_owned));
    let rows=vault.store().read(Db::AgentHistoryEvidence,move |s|{
        let read=|r:&vault_store::Row|Ok((r.get::<String>(0)?,r.get::<i64>(1)?));
        if let Some((tool,path))=filter{
            s.query("SELECT metric,quantity FROM ah_run_totals WHERE run_id=?1 AND harness=?2 AND root=?3",params![run,tool,path],read)
        }else if let Some(tool)=tool {
            s.query("SELECT metric,SUM(quantity) FROM ah_run_totals WHERE run_id=?1 AND harness=?2 GROUP BY metric",params![run,tool],read)
        }else{
            s.query("SELECT metric,SUM(quantity) FROM ah_run_totals WHERE run_id=?1 GROUP BY metric",[run],read)
        }
    })?;
    let mut summary = json!({"type":"agent-history","retainedBytes":null});
    for metric in [
        "filesVisited",
        "filesAbsent",
        "filesCaptured",
        "filesUnchanged",
        "sourceBytes",
        "newRoots",
        "updatedRoots",
        "newChildren",
        "unavailable",
        "unstable",
        "unknownFormat",
        "unclassified",
        "attachmentsChanged",
    ] {
        summary[metric] = json!("0");
    }
    for (metric, value) in rows {
        summary[metric] = json!(value.to_string());
    }
    Ok(summary)
}
