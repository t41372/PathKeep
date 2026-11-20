//! Adapt the existing browsing ledger to the cross-space run list. Numeric
//! browsing links retain their audit payload. Private MCP audit stays separate.
use super::*;
use vault_store::{OptionalRow, Row, params};

const WIDE: &str = "'doctor','retention_prune','rekey','archive_restore'";
const PRIVATE: &str = "'system','mcp_query','ai_index','assistant'";
const COLUMNS: &str = "id,started_at,finished_at,run_type,trigger,status,stats_json,warnings_json";

pub(super) fn origin(tx: &vault_store::Session<'_>) -> vault_store::Result<String> {
    tx.query_one("SELECT value FROM archive_meta WHERE name='runOrigin'", (), |row| Ok(row.get(0)?))
}

fn row(r: &Row, origin: &str) -> vault_store::Result<(String, i64, Value)> {
    let id: i64 = r.get(0)?;
    let date: String = r.get(1)?;
    let operation: String = r.get(3)?;
    let wide = r.get::<i64>(8)? != 0;
    let scope = if wide { "archive" } else { "browsing" };
    let kind = if wide {
        "maintenance"
    } else if operation == "import" {
        "import"
    } else {
        "browsing"
    };
    let timestamp =
        |s: &str| chrono::DateTime::parse_from_rfc3339(s).ok().map(|t| t.timestamp_millis());
    let start = timestamp(&date);
    let stats = r.get::<Option<String>>(6)?.and_then(|v| serde_json::from_str::<Value>(&v).ok());
    let warnings =
        r.get::<Option<String>>(7)?.and_then(|v| serde_json::from_str::<Vec<Value>>(&v).ok());
    let mut row = json!({
        "runRef":format!("browsing:{origin}/{id}"),"operation":operation,"scope":scope,
        "trigger":r.get::<String>(4)?,"result":r.get::<String>(5)?,"startedAt":start,
        "summary":{"type":kind,"stats":stats},"warningCount":warnings.map_or(0,|v|v.len()).to_string()
    });
    if let Some(end) = r.get::<Option<String>>(2)?.as_deref().and_then(timestamp) {
        row["finishedAt"] = json!(end);
        if let Some(start) = start.filter(|s| *s <= end) {
            row["durationMs"] = json!(end - start);
        }
    }
    Ok((date, id, row))
}

pub(super) fn page(
    vault: &Vault,
    browsing: bool,
    wide: bool,
    after: Option<(String, i64)>,
    limit: i64,
) -> Result<Vec<(String, i64, Value)>> {
    let (date, id) = after.unwrap_or_else(|| ("9999".into(), i64::MAX));
    let sql = format!(
        "SELECT {COLUMNS},run_type IN ({WIDE}) FROM runs WHERE run_type NOT IN ({PRIVATE})
        AND ((?1 AND run_type NOT IN ({WIDE})) OR (?2 AND run_type IN ({WIDE})))
        AND (started_at,id)<(?3,?4) ORDER BY started_at DESC,id DESC LIMIT ?5"
    );
    Ok(vault.store().read(Db::Archive, move |s| {
        let origin = origin(s)?;
        s.query(&sql, params![i64::from(browsing), i64::from(wide), date, id, limit], |r| {
            row(r, &origin)
        })
    })?)
}

pub(super) fn detail(session: &VaultSession, reference: &str) -> Result<Value> {
    admitted_config(&vault_core::project_paths()?)?;
    let vault = session.vault().map_err(anyhow::Error::msg)?;
    let policy = check_reply(&vault, session, true)?;
    let origin = vault.store().read(Db::Archive, origin)?;
    let id = if let Some(reference) = reference.strip_prefix("browsing:") {
        let (saved, id) = reference.split_once('/').ok_or(Refusal::Missing)?;
        if saved != origin {
            return Err(Refusal::Missing.into());
        }
        id
    } else {
        reference
    }
    .parse::<i64>()
    .map_err(|_| Refusal::Missing)?;
    let (.., row) = vault
        .store()
        .read(Db::Archive, move |tx| {
            let sql =
                format!("SELECT {COLUMNS},run_type IN ({WIDE}) FROM runs WHERE id=?1 AND run_type NOT IN ({PRIVATE})");
            tx.query_one(&sql, [id], |r| row(r, &origin)).optional()
        })?
        .ok_or(Refusal::Missing)?;
    if row["scope"] == "browsing" && policy.browsing == KindState::Off {
        return Err(Refusal::Off.into());
    }
    let mut value = serde_json::to_value(vault_core::load_audit_run_detail(&vault, id)?)?;
    if let (Value::Object(detail), Value::Object(fields)) = (&mut value, row) {
        detail.extend(fields);
    }
    if policy != check_reply(&vault, session, true)? {
        return Err(Refusal::PolicyChanged.into());
    }
    Ok(value)
}
