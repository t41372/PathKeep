//! Backup inventory from committed evidence. Drawing this page does not resolve
//! native tool defaults, revisit directories, or attach a conversation read model.
use super::*;
use vault_store::OptionalRow;

#[derive(Deserialize)]
#[serde(rename_all = "lowercase")]
enum Purpose {
    Backup,
    Space,
}
/// Choose backup inventory or the space banner over the same committed facts.
#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Overview {
    purpose: Purpose,
    #[serde(default)]
    include_archive_bytes: bool,
}
/// Bound the source picker response independently of the number of custom roots.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ListSources {
    cursor: Option<String>,
    limit: usize,
}

#[derive(Clone, Hash, Eq, PartialEq)]
enum CountScope {
    All,
    Tool(String),
    Root(String, String),
}
#[derive(Default)]
pub(super) struct InventoryCache {
    token: Option<Arc<()>>,
    values: std::collections::HashMap<CountScope, Value>,
}
fn counts(
    vault: &Vault,
    runtime: &Runtime,
    root: Option<&selection::Root>,
    tool: Option<AgentTool>,
) -> Result<Value> {
    let token = vault.store().read_token(Db::AgentHistoryEvidence)?;
    let mut cache = runtime.inventory.lock().unwrap();
    if !cache.token.as_ref().is_some_and(|t| Arc::ptr_eq(t, &token)) {
        cache.values.clear();
        cache.token = Some(token);
    }
    let scope = match (root, tool) {
        (Some(r), _) => {
            CountScope::Root(r.tool.harness().to_string(), r.path.to_string_lossy().into_owned())
        }
        (_, Some(t)) => CountScope::Tool(t.harness().to_string()),
        _ => CountScope::All,
    };
    if let Some(facts) = cache.values.get(&scope) {
        return Ok(facts.clone());
    }
    let (facts, single_tool, single_root) = read_counts(vault, root, tool)?;
    // A scope containing exactly one tool or directory already computed its totals.
    // Reuse only narrower equivalent scopes, never infer global totals from a subset.
    if let Some(tool) = single_tool {
        if scope == CountScope::All {
            cache.values.insert(CountScope::Tool(tool.clone()), facts.clone());
        }
        if let Some(root) = single_root {
            cache.values.insert(CountScope::Root(tool, root), facts.clone());
        }
    }
    cache.values.insert(scope, facts.clone());
    Ok(facts)
}
fn read_counts(
    vault: &Vault,
    root: Option<&selection::Root>,
    tool: Option<AgentTool>,
) -> Result<(Value, Option<String>, Option<String>)> {
    let filter =
        root.map(|r| (r.source().harness.to_string(), r.path.to_string_lossy().into_owned()));
    let (clause, args) = if let Some((h, p)) = filter {
        ("WHERE s.harness=?1 AND s.root=?2", vec![h, p])
    } else if let Some(t) = tool {
        ("WHERE s.harness=?1", vec![t.harness().to_string()])
    } else {
        ("", vec![])
    };
    let files = format!(
        "SELECT coalesce(SUM(r.byte_len),0),
                COUNT(CASE WHEN s.kind='transcript' THEN 1 END),
                COUNT(CASE WHEN s.kind='attachment' THEN 1 END),
                COUNT(CASE WHEN s.kind='transcript' AND c.source_id IS NULL THEN 1 END),
                CASE WHEN MIN(s.harness)=MAX(s.harness) THEN MIN(s.harness) END,
                CASE WHEN MIN(s.root)=MAX(s.root) THEN MIN(s.root) END
         FROM ah_sources s JOIN ah_revisions r ON r.id=s.latest_revision_id
         LEFT JOIN ah_source_conversations c ON c.source_id=s.id AND c.revision_id=r.id
         {clause}"
    );
    let conversations = format!(
        "SELECT COUNT(CASE WHEN child=0 THEN 1 END),COUNT(CASE WHEN child=1 THEN 1 END),
                COUNT(CASE WHEN child=0 AND present=0 THEN 1 END)
         FROM (SELECT c.conversation_id,MAX(c.is_child) child,
               MAX(CASE WHEN s.missing_since IS NULL
                        THEN 1 ELSE 0 END) present
               FROM ah_source_conversations c JOIN ah_sources s ON s.id=c.source_id
               {clause} GROUP BY c.conversation_id)"
    );
    Ok(vault.store().read(Db::AgentHistoryEvidence, move |tx| {
        let (bytes, transcripts, attachments, unclassified, single_tool, single_root) = tx
            .query_one(&files, vault_store::params_from_iter(args.iter().cloned()), |row| {
                Ok((
                    row.get::<i64>(0)?,
                    row.get::<i64>(1)?,
                    row.get::<i64>(2)?,
                    row.get::<i64>(3)?,
                    row.get::<Option<String>>(4)?,
                    row.get::<Option<String>>(5)?,
                ))
            })?;
        let (roots, children, absent) =
            tx.query_one(&conversations, vault_store::params_from_iter(args), |row| {
                Ok((row.get::<i64>(0)?, row.get::<i64>(1)?, row.get::<i64>(2)?))
            })?;
        let mut facts = json!({
            "transcriptFiles":transcripts.to_string(),"attachmentFiles":attachments.to_string(),
            "roots":roots.to_string(),"children":children.to_string(),
            "onlyInPathkeep":absent.to_string(),"sourceBytes":bytes.to_string()
        });
        if unclassified > 0 {
            facts["unclassified"] = json!(unclassified.to_string());
        }
        Ok((facts, single_tool, single_root))
    })?)
}

fn source_row(
    vault: &Vault,
    runtime: &Runtime,
    policy: &CapturePolicy,
    r: &selection::Root,
) -> Result<Value> {
    let key = r.id.clone();
    let last=vault.store().read(Db::AgentHistoryEvidence,move |s|s.query_one(
        "SELECT o.state,r.public_id,r.id,json_type(r.report_json,'$.captureOutcome') FROM ah_run_roots o JOIN ah_backup_runs r ON r.id=o.run_id
         WHERE o.root_id=?1 ORDER BY o.run_id DESC LIMIT 1",[key],|r|Ok((r.get::<String>(0)?,r.get::<String>(1)?,r.get::<i64>(2)?,r.get::<Option<String>>(3)?))).optional())?;
    let enabled = policy.enabled_tools.contains(&r.tool);
    let state = if !enabled {
        "disabled"
    } else {
        last.as_ref().map_or("stale", |(s, _, _, _)| s.as_str())
    };
    let inventory = counts(vault, runtime, Some(r), None)?;
    let parts = [
        ("conversations", "transcriptFiles"),
        ("subagents", "children"),
        ("attachments", "attachmentFiles"),
        ("retained-after-removal", "onlyInPathkeep"),
    ]
    .into_iter()
    .filter(|(_, key)| inventory[*key] != "0")
    .map(|(part, _)| part)
    .collect::<Vec<_>>();
    let mut row = json!({"rootId":r.id,"directoryToken":preview::directory_token(&r.id,&r.directory),"tool":r.tool,"path":r.path,"enabled":enabled,"availability":state,
        "parts":parts,"counts":inventory,"sourceBytes":inventory["sourceBytes"],"autoDeletesByDefault":r.tool==AgentTool::ClaudeCode});
    if let Some((reason, run, id, measured)) = last {
        if measured.is_some() {
            super::notes::source(vault, id, r.id.clone(), &mut row)?;
        }
        row["lastRun"] = json!(format!("agent-history:{run}"));
        if reason != "available" {
            row["reason"] = json!(reason);
        }
    }
    Ok(row)
}
/// Show retained roots and disabled tools without a rescan.
pub fn sources(session: &VaultSession, req: ListSources) -> Result<Value> {
    let (vault, c) = admitted(session, false)?;
    let n = runs::limit(req.limit)? as usize;
    let after: String = runs::decode(req.cursor.as_deref())?;
    let mut rows = Vec::new();
    let roots = selection::device_roots(&vault, &c.capture_policy)?;
    for r in &roots {
        if r.id > after {
            rows.push((r.id.clone(), Some(r), r.tool));
        }
    }
    for tool in AgentTool::ALL {
        if !roots.iter().any(|r| r.tool == tool) {
            let key = format!("tool:{}", tool.harness().as_str());
            if key > after {
                rows.push((key, None, tool));
            }
        }
    }
    rows.sort_by(|a, b| a.0.cmp(&b.0));
    let next = (rows.len() > n).then(|| serde_json::to_string(&rows[n - 1].0)).transpose()?;
    let mut items = Vec::new();
    for (_, root, tool) in rows.into_iter().take(n) {
        items.push(match root {Some(r)=>source_row(&vault,&session.conversations,&c.capture_policy,r)?,None=>json!({"rootId":null,"tool":tool,"path":null,"enabled":false,
            "availability":"disabled","parts":[],"counts":null,"sourceBytes":null,"autoDeletesByDefault":tool==AgentTool::ClaudeCode})});
    }
    if check_reply(&vault, session, false)? != c.capture_policy {
        return Err(Refusal::PolicyChanged.into());
    }
    runs::page(items, next)
}
/// Read the inventory without probing native tool folders.
pub fn overview(session: &VaultSession, req: Overview) -> Result<Value> {
    let (vault, c) = admitted(session, false)?;
    let mut result = counts(&vault, &session.conversations, None, None)?;
    let roots = selection::device_roots(&vault, &c.capture_policy)?;
    let mut tools = Vec::new();
    for r in &roots {
        tools.push(source_row(&vault, &session.conversations, &c.capture_policy, r)?);
    }
    result["lookedIn"] = json!(roots.iter().map(|r| &r.path).collect::<Vec<_>>());
    result["sources"] = json!(tools);
    let mut breakdown = Vec::new();
    for tool in AgentTool::ALL {
        let mut counts = counts(&vault, &session.conversations, None, Some(tool))?;
        counts["tool"] = json!(tool);
        counts["enabled"] = json!(
            tools
                .iter()
                .any(|row| row["tool"] == tool.harness().as_str() && row["enabled"] == true)
        );
        breakdown.push(counts);
    }
    result["perTool"] = json!(breakdown);
    let complete =
        tools.iter().all(|r| matches!(r["availability"].as_str(), Some("available" | "disabled")));
    result["coverage"] = if result.get("unclassified").is_some() {
        json!({"state":"partial","reasons":["unclassified-transcripts"]})
    } else if complete {
        json!({"state":"complete","reasons":[]})
    } else {
        json!({"state":"partial","reasons":["incomplete-source-observations"]})
    };
    result["firstBackup"] = json!(result_name(&first_backup(&vault, &c.capture_policy)?));
    if let Some(last) = runs::records(&vault, None, 1)?.pop() {
        result["lastRun"] = last.row(&vault)?;
    }
    if let Some(a) = session.conversations.active.lock().unwrap().as_ref() {
        result["activeTask"] = a.intent.start();
    }
    if req.include_archive_bytes {
        result["archiveBytes"] = json!(vault.conversation_archive_bytes()?.to_string());
    }
    if matches!(req.purpose, Purpose::Space) {
        result["lag"] = Value::Null;
    }
    if check_reply(&vault, session, false)? != c.capture_policy {
        return Err(Refusal::PolicyChanged.into());
    }
    Ok(result)
}

pub(crate) fn first_backup(vault: &Vault, policy: &CapturePolicy) -> Result<String> {
    let Some(enable) = policy.selection_ref.clone() else { return Ok("not-started".into()) };
    let first = vault.store().read(Db::AgentHistoryEvidence, move |s| {
        s.query_one("WITH RECURSIVE family(task_id) AS (
             SELECT task_id FROM ah_desktop_backups WHERE trigger='enable' AND task_id=(SELECT first_task_id FROM ah_capture_selections WHERE id=?1)
             UNION SELECT d.task_id FROM ah_desktop_backups d JOIN family f ON d.retry_of=f.task_id)
             SELECT r.status FROM ah_backup_runs r JOIN ah_desktop_backups d ON d.run_id=r.id JOIN family f ON f.task_id=d.task_id
             ORDER BY (r.status='complete') DESC,r.id DESC LIMIT 1", [enable], |r| {
            Ok(r.get::<String>(0)?)
        })
        .optional()
    })?;
    Ok(first.unwrap_or_else(|| "not-started".into()))
}
