//! Failure modes: stale rows survive repair, visible rows are removed, cached embedding totals drift,
//! or one repair monopolizes the writer. Native health and plan regressions cover these.
//! Responsibilities: retain the existing intelligence health algorithms for area 4.
//! Not responsible for: native archive health checks.
use super::*;
/// Verifies that derived AI/insight tables only reference currently visible visits.
pub(crate) fn check_stale_derived_state(
    connection: &vault_store::Session<'_>,
) -> Result<HealthCheck> {
    let mut stale_details = Vec::new();

    for (table, column, label) in [
        ("ai_embeddings", "history_id", "stale AI embeddings"),
        ("search_trail_members", "visit_id", "stale search trail members"),
        ("visit_derived_facts", "visit_id", "stale visit-derived-fact rows"),
    ] {
        let stale = count_rows_without_visible_visit(connection, table, column)?;
        if stale > 0 {
            stale_details.push(format!("{stale} {label}"));
        }
    }

    Ok(if stale_details.is_empty() {
        HealthCheck {
            code: "derived-state-freshness".to_string(),
            name: "Derived state freshness".to_string(),
            ok: true,
            detail: "Derived AI and Core Intelligence tables match the visible visit set."
                .to_string(),
        }
    } else {
        HealthCheck {
            code: "derived-state-freshness".to_string(),
            name: "Derived state freshness".to_string(),
            ok: false,
            detail: stale_details.join(", "),
        }
    })
}

/// Counts rows of a derived `table` whose `column` no longer points at a visible visit (the
/// visit was rolled back or is gone). Every table is installed by the consolidated schema.
///
/// The check is a correlated `NOT EXISTS`, so SQLite looks each row's visit up by primary key.
/// The `NOT IN (SELECT id FROM archive.visits WHERE reverted_at IS NULL)` it replaced first
/// copied every visible visit id into a temporary index on each call: 14.4M ids at the product
/// target. Numbers in `docs/architecture/ipc-performance.md`.
pub(crate) fn count_rows_without_visible_visit(
    connection: &vault_store::Session<'_>,
    table: &str,
    column: &str,
) -> Result<usize> {
    let count: i64 =
        connection
            .query_one(&rows_without_visible_visit_sql(table, column), (), |row| Ok(row.get(0)?))?;
    Ok(count.max(0) as usize)
}

fn rows_without_visible_visit_sql(table: &str, column: &str) -> String {
    format!(
        "SELECT COUNT(*)
         FROM {table}
         WHERE NOT EXISTS (
           SELECT 1 FROM archive.visits AS visits
           WHERE visits.id = {table}.{column} AND visits.reverted_at IS NULL LIMIT 1 OFFSET 0
         )"
    )
}

/// Repairs metadata in bounded batches and keeps the status totals in the same transaction.
pub(crate) fn clear_stale_ai_embeddings(vault: &crate::Vault) -> Result<usize> {
    use vault_store::{Db, Priority, params_owned as params};
    let mut after = 0;
    let mut removed = 0;
    let upper = vault.read(Db::Intelligence, |s| {
        Ok(s.query_one("SELECT COALESCE(MAX(id),0) FROM ai_embeddings", (), |r| {
            Ok(r.get::<i64>(0)?)
        })?)
    })?;
    loop {
        let (next,count)=vault.write(Db::Intelligence,Priority::Batch,move |s| {
            let rows=s.query("SELECT id,provider_id,model,content_bytes,metadata_bytes,lineage,NOT EXISTS(SELECT 1 FROM archive.visits v WHERE v.id=e.history_id AND v.reverted_at IS NULL LIMIT 1 OFFSET 0) FROM ai_embeddings e WHERE id>?1 AND id<=?2 ORDER BY id LIMIT 512",params![after,upper],|r|Ok((r.get::<i64>(0)?,r.get::<String>(1)?,r.get::<String>(2)?,r.get::<i64>(3)?,r.get::<i64>(4)?,r.get::<String>(5)?,r.get::<i64>(6)?!=0)))?;
            let next=rows.last().map(|r|r.0);let mut count=0;
            let mut delete=s.prepare("DELETE FROM ai_embeddings WHERE id=?1")?;
            let mut totals=s.prepare("UPDATE ai_embedding_totals SET last_indexed_at=CASE WHEN item_count=1 THEN NULL ELSE last_indexed_at END,item_count=item_count-1,content_bytes=content_bytes-?3,metadata_bytes=metadata_bytes-?4 WHERE provider_id=?1 AND model=?2 AND lineage=?5")?;
            for (id,provider,model,content,metadata,lineage,stale) in rows {
                if stale {delete.execute([id])?;totals.execute(params![provider,model,content,metadata,lineage])?;count+=1;}
            }
            Ok((next,count))
        })?;
        removed += count;
        let Some(next) = next else {
            break;
        };
        after = next;
    }
    Ok(removed)
}

/// Checks the owned intelligence database against canonical archive visibility.
pub fn derived_health(vault: &crate::Vault) -> Result<HealthCheck> {
    vault.read(vault_store::Db::Intelligence, check_stale_derived_state)
}

/// Repairs derived state through the owner; canonical archive facts are never changed.
pub(crate) fn repair_derived(vault: &crate::Vault) -> Result<usize> {
    let stale = vault.read(vault_store::Db::Intelligence, |s| {
        Ok(count_rows_without_visible_visit(s, "search_trail_members", "visit_id")?
            + count_rows_without_visible_visit(s, "visit_derived_facts", "visit_id")?)
    })?;
    let embeddings = clear_stale_ai_embeddings(vault)?;
    if stale > 0 {
        crate::intelligence::clear_derived_intelligence_state(vault)?;
        crate::intelligence_runtime::mark_all_deterministic_modules_stale(
            vault,
            crate::models::DERIVED_STALE_VISIBILITY_OR_ROLLBACK_CHANGED,
            "Archive visibility or rollback state changed after the last Core Intelligence rebuild.",
        )?;
    }
    Ok(embeddings + stale)
}

#[cfg(test)]
mod tests {
    use super::*;
    use vault_store::{Db, Priority, params_owned as params};
    fn fixture(key: Option<[u8; 32]>) -> crate::Vault {
        let vault = crate::Vault::open_temp_with_key(key).unwrap();
        vault.write(Db::Archive,Priority::Batch,|s| {
            s.execute("INSERT INTO source_profiles(id,browser_kind,profile_name,profile_path,profile_key,discovered_at) VALUES(1,'chrome','Default','/fixture','profile','')",())?;
            for id in 1..=2_i64 {
                s.execute("INSERT INTO pages(id,url,canonical_url) VALUES(?1,?2,?2)",params![id,format!("https://example.com/{id}")])?;
                s.execute("INSERT INTO urls(id,page_id,url,source_profile_id,created_by_run_id,first_visit_ms,first_visit_iso,last_visit_ms,last_visit_iso) VALUES(?1,?1,?2,1,0,1,'',1,'')",params![id,format!("https://example.com/{id}")])?;
                s.execute("INSERT INTO visits(id,url_id,source_profile_id,created_by_run_id,visit_time_ms,visit_time_iso,reverted_at) VALUES(?1,?1,1,0,1,'',?2)",params![id,if id==2 {Some("2026-01-01")} else {None}])?;
            }Ok(())
        }).unwrap();
        vault.write(Db::Intelligence,Priority::Batch,|s| {
            s.execute("INSERT INTO intelligence_generations(profile_id) VALUES('profile')",())?;
            for id in 1..=3_i64 {
                s.execute("INSERT INTO visit_derived_facts_rows(visit_id,profile_id,canonical_url,registrable_domain,computed_at) VALUES(?1,'profile',?2,'example.com','')",params![id,format!("https://example.com/{id}")])?;
                s.execute("INSERT INTO ai_embeddings(history_id,profile_id,url,domain,visited_at,content_hash,content_bytes,metadata_bytes,provider_id,model,indexed_at,lineage) VALUES(?1,'profile',?2,'example.com','','hash',2,3,'provider','model','','legacy')",params![id,format!("https://example.com/{id}")])?;
            }
            s.execute("INSERT INTO ai_embedding_totals(provider_id,model,item_count,content_bytes,metadata_bytes,last_indexed_at,lineage) VALUES('provider','model',3,6,9,'','legacy')",())?;Ok(())
        }).unwrap();
        vault
    }
    #[test]
    fn stale_row_checks_look_visits_up_by_primary_key() {
        let vault = fixture(None);
        vault
            .read(Db::Intelligence, |s| {
                for (table, column) in [
                    ("ai_embeddings", "history_id"),
                    ("search_trail_members", "visit_id"),
                    ("visit_derived_facts", "visit_id"),
                ] {
                    let sql = format!(
                        "EXPLAIN QUERY PLAN {}",
                        rows_without_visible_visit_sql(table, column)
                    );
                    let plan = s.query(&sql, (), |r| Ok(r.get::<String>(3)?))?.join("\n");
                    assert!(
                        plan.contains("SEARCH visits USING INTEGER PRIMARY KEY"),
                        "{table}: {plan}"
                    );
                }
                Ok(())
            })
            .unwrap();
    }
}
