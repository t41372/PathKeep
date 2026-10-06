//! Failure modes: invalid cached run JSON masks usable totals or reverted facts enter maintenance counts. Tests: legacy maintenance and wipe suites.
//! Responsibilities: supply existing SQLCipher maintenance counters.
//! Not responsible for: native reads; removed with the maintenance port.
use super::*;
/// Extracts cached archive totals from recent successful runs when available.
pub(super) fn load_cached_archive_totals(
    connection: &Connection,
) -> Result<Option<ArchiveVisibleTotals>> {
    let mut statement = connection.prepare(
        "SELECT stats_json
         FROM runs
         WHERE status = 'success'
         ORDER BY id DESC
         LIMIT 24",
    )?;
    let rows = statement.query_map([], |row| row.get::<_, String>(0))?;
    for row in rows {
        let stats_json = row?;
        let Ok(stats) = serde_json::from_str::<Value>(&stats_json) else {
            continue;
        };
        if let Some(totals) = archive_totals_from_stats(&stats) {
            return Ok(Some(totals));
        }
    }
    Ok(None)
}

/// Rebuilds cached visible totals from a stored stats payload when possible.
fn archive_totals_from_stats(stats: &Value) -> Option<ArchiveVisibleTotals> {
    Some(ArchiveVisibleTotals {
        total_profiles: stats.get("totalProfiles")?.as_u64()? as usize,
        total_urls: stats.get("totalUrls")?.as_u64()? as usize,
        total_visits: stats.get("totalVisits")?.as_u64()? as usize,
        total_downloads: stats.get("totalDownloads")?.as_u64()? as usize,
        earliest_visit_at: stats
            .get("earliestVisitAt")
            .and_then(|value| value.as_str().map(str::to_string)),
        latest_visit_at: stats
            .get("latestVisitAt")
            .and_then(|value| value.as_str().map(str::to_string)),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    /// The maintenance area still consumes old run JSON until its own port.
    #[test]
    fn malformed_and_incomplete_run_json_do_not_hide_usable_maintenance_totals() {
        let root = tempfile::tempdir().unwrap();
        let paths = crate::project_paths_with_root(root.path());
        let config = AppConfig { initialized: true, ..Default::default() };
        let connection = open_archive_connection(&paths, &config, None).unwrap();
        for (id, stats) in [
            (100, "{malformed"),
            (99, r#"{"totalProfiles":1}"#),
            (98, r#"{"totalProfiles":2,"totalUrls":3,"totalVisits":5,"totalDownloads":7}"#),
        ] {
            connection.execute("INSERT INTO runs(id,run_type,trigger,started_at,status,stats_json) VALUES(?1,'backup','manual','','success',?2)",rusqlite::params![id,stats]).unwrap();
        }
        let totals = load_cached_archive_totals(&connection).unwrap().unwrap();
        assert_eq!(
            (totals.total_profiles, totals.total_urls, totals.total_visits, totals.total_downloads),
            (2, 3, 5, 7)
        );
    }
}
