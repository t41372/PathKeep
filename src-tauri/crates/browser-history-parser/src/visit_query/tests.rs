//! Responsibilities: pin the row window and SQLite work of the production visit statements.
//! Not responsible for: archive identity or end-to-end backup throughput.
//! Failure modes: the recent window scans old history, especially after ANALYZE; joining pages
//! starts from the page index; split ranges duplicate or reorder rows, lose boundary or Sync
//! visits, or count orphans outside the window; Safari's REAL cutoff loses precision; a missing
//! time index fails to read; a full or continued read builds a list or sorts every visit id.
//! Safari has no time-leading index: an invented one would hide its covering-index scan.
use super::*;
use browser_history_fixtures::{
    ChromiumHistoryFixture, FirefoxPlacesFixture, SafariHistoryFixture,
};
use rusqlite::{Connection, StatementStatus, params, params_from_iter};

const CURSOR: i64 = 4_096;
const SINCE: i64 = 1_777_680_000_000;

fn assert_index(db: &Connection, name: &str, columns: &[(&str, bool)]) {
    let actual = db
        .prepare("SELECT name,desc FROM pragma_index_xinfo(?1) WHERE key=1 ORDER BY seqno")
        .unwrap()
        .query_map([name], |row| Ok((row.get::<_, String>(0)?, row.get::<_, bool>(1)?)))
        .unwrap()
        .collect::<rusqlite::Result<Vec<_>>>()
        .unwrap();
    assert_eq!(
        actual,
        columns.iter().map(|(name, desc)| (name.to_string(), *desc)).collect::<Vec<_>>()
    );
}

#[test]
fn chromium_fixture_has_the_real_visit_time_index() {
    let (_dir, db) = source(VisitTable::Chromium);
    assert_index(&db, "visits_time_index", &[("visit_time", false)]);
}

#[test]
fn firefox_fixture_has_the_real_visit_indexes() {
    let (_dir, db) = source(VisitTable::Firefox);
    assert_index(&db, "moz_historyvisits_dateindex", &[("visit_date", false)]);
    assert_index(
        &db,
        "moz_historyvisits_placedateindex",
        &[("place_id", false), ("visit_date", false)],
    );
}

#[test]
fn safari_fixture_has_its_page_time_index_and_no_time_leading_index() {
    let (_dir, db) = source(VisitTable::Safari);
    let indexes = db
        .prepare("SELECT name FROM pragma_index_list('history_visits')")
        .unwrap()
        .query_map([], |row| row.get::<_, String>(0))
        .unwrap()
        .collect::<rusqlite::Result<Vec<_>>>()
        .unwrap();
    assert_eq!(indexes, ["history_visits__last_visit"]);
    assert_index(
        &db,
        "history_visits__last_visit",
        &[("history_item", false), ("visit_time", true), ("synthesized", false)],
    );
}

fn source(table: VisitTable) -> (tempfile::TempDir, Connection) {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("history.sqlite");
    match table {
        VisitTable::Chromium => ChromiumHistoryFixture::new().write(&path),
        VisitTable::Firefox => FirefoxPlacesFixture::new().write(&path),
        VisitTable::Safari => SafariHistoryFixture::new().write(&path),
    }
    .unwrap();
    let db = Connection::open(path).unwrap();
    let (visits, page, pages, time) = table.names();
    db.execute_batch(&format!(
        "INSERT INTO {pages}(id,url) VALUES(1,'https://example.com/a'),(2,'https://example.com/b');"
    ))
    .unwrap();
    // Safari's page-first index cannot skip-seek time when visits span many pages.
    let page_value = if table == VisitTable::Safari {
        db.execute_batch(&format!(
            "WITH RECURSIVE rows(id) AS (SELECT 1 UNION ALL SELECT id+1 FROM rows WHERE id<{CURSOR})
             INSERT INTO {pages}(id,url) SELECT id+10000,'https://example.com/'||id FROM rows"
        ))
        .unwrap();
        "id+10000"
    } else {
        "id%2+1"
    };
    let step = match table {
        VisitTable::Safari => Value::Real(0.000_125),
        _ => Value::Integer(1),
    };
    db.execute(
        &format!(
            "WITH RECURSIVE rows(id) AS (SELECT 1 UNION ALL SELECT id+1 FROM rows WHERE id<?3)
             INSERT INTO {visits}(id,{page},{time})
               SELECT id,CASE WHEN id%101=0 THEN 999 ELSE {page_value} END,?1+id*?2 FROM rows"
        ),
        params![table.time_floor(Some(SINCE - 3 * 86_400_000)), step, CURSOR],
    )
    .unwrap();
    (dir, db)
}

fn read(db: &Connection, sql: &str, values: &[Value]) -> (Vec<Vec<Value>>, i32) {
    let mut statement = db.prepare(sql).unwrap();
    let columns = statement.column_count();
    let rows = statement
        .query_map(params_from_iter(values), |row| (0..columns).map(|i| row.get(i)).collect())
        .unwrap()
        .collect::<rusqlite::Result<Vec<_>>>()
        .unwrap();
    (rows, statement.get_status(StatementStatus::FullscanStep))
}

fn plan(db: &Connection, sql: &str, values: &[Value]) -> Vec<String> {
    db.prepare(&format!("EXPLAIN QUERY PLAN {sql}"))
        .unwrap()
        .query_map(params_from_iter(values), |r| r.get(3))
        .unwrap()
        .collect::<rusqlite::Result<_>>()
        .unwrap()
}

fn legacy_visits(table: VisitTable, columns: &str) -> String {
    let (visits, page, pages, time) = table.names();
    format!(
        "SELECT {columns} FROM {visits} JOIN {pages} ON {pages}.id={visits}.{page}
         WHERE {visits}.id > ?1 OR {visits}.{time} >= ?2 ORDER BY {visits}.id"
    )
}

fn legacy_orphans(table: VisitTable) -> String {
    let (visits, page, pages, time) = table.names();
    format!(
        "SELECT COUNT(*) FROM {visits}
         WHERE ({visits}.id > ?1 OR {visits}.{time} >= ?2)
           AND NOT EXISTS (SELECT 1 FROM {pages} WHERE {pages}.id={visits}.{page})"
    )
}

fn shifted(time: &Value, amount: i64) -> Value {
    match time {
        Value::Integer(time) => Value::Integer(time + amount),
        Value::Real(time) => Value::Real(time + amount as f64 * 0.000_001),
        _ => unreachable!(),
    }
}

/// Compare all projected values and id order against the original OR on indexed and unindexed
/// browser files. Explicit expected ids keep a shared error in either query from passing.
pub(crate) fn rows_match(table: VisitTable, columns: &str, sql: impl Fn(VisitWindow) -> String) {
    for indexed in [true, false] {
        let (_dir, db) = source(table);
        let (visits, page, _, time) = table.names();
        let floor = table.time_floor(Some(SINCE));
        for (id, at) in [
            (2, floor.clone()),
            (3, shifted(&floor, 1)),
            (4, shifted(&floor, -1)),
            (101, floor.clone()),
            (CURSOR, floor.clone()),
        ] {
            db.execute(&format!("UPDATE {visits} SET {time}=?1 WHERE id=?2"), params![at, id])
                .unwrap();
        }
        // Old-time Sync after the cursor, overlap above it, and an orphan above it. Another
        // orphan at id 101 is exactly on the time boundary below the cursor.
        for (id, parent, at) in [
            (CURSOR + 1, 1, shifted(&floor, -10_000)),
            (CURSOR + 2, 2, floor.clone()),
            (CURSOR + 3, 999, shifted(&floor, -10_000)),
        ] {
            db.execute(
                &format!("INSERT INTO {visits}(id,{page},{time}) VALUES(?1,?2,?3)"),
                params![id, parent, at],
            )
            .unwrap();
        }
        if !indexed {
            let index = match table {
                VisitTable::Chromium => "visits_time_index",
                VisitTable::Firefox => "moz_historyvisits_dateindex",
                VisitTable::Safari => "history_visits__last_visit",
            };
            db.execute_batch(&format!("DROP INDEX {index}")).unwrap();
        }
        for analyzed in [false, true] {
            if analyzed {
                db.execute_batch("ANALYZE").unwrap();
            }
            for window in [
                VisitWindow { after_id: CURSOR, recent_since_ms: Some(SINCE) },
                VisitWindow::default(),
                VisitWindow { after_id: CURSOR / 2, recent_since_ms: None },
            ] {
                let old_values =
                    vec![Value::Integer(window.after_id), table.time_floor(window.recent_since_ms)];
                let values = parameters(table, window);
                let actual = read(&db, &sql(window), &values).0;
                assert_eq!(
                    actual,
                    read(&db, &legacy_visits(table, columns), &old_values).0,
                    "{table:?}: indexed={indexed}, analyzed={analyzed}, {window:?}"
                );
                let orphans = read(&db, &orphans(table, window), &values).0;
                assert_eq!(orphans, read(&db, &legacy_orphans(table), &old_values).0);
                assert_eq!(
                    crate::native_visits::orphan_visits(
                        std::path::Path::new(db.path().unwrap()),
                        table,
                        window.after_id,
                        window.recent_since_ms,
                    )
                    .unwrap(),
                    match orphans[0][0] {
                        Value::Integer(count) => count as usize,
                        _ => unreachable!(),
                    },
                );
                if window.recent_since_ms.is_some() {
                    assert_eq!(
                        actual.iter().map(|r| r[0].clone()).collect::<Vec<_>>(),
                        [2, 3, CURSOR, CURSOR + 1, CURSOR + 2].map(Value::Integer)
                    );
                    assert_eq!(orphans, vec![vec![Value::Integer(2)]]);
                }
            }
        }
    }
}

/// Chromium and Firefox seek both ranges. Safari seeks rowids and scans its narrower
/// page/time index for recent visits, doing no more fullscan work than the old OR query.
/// Keep the original plans in the output so a linked SQLite upgrade is easy to diagnose.
pub(crate) fn recent_read_work(
    table: VisitTable,
    columns: &str,
    sql: impl Fn(VisitWindow) -> String,
) {
    let (_dir, db) = source(table);
    let window = VisitWindow { after_id: CURSOR, recent_since_ms: Some(SINCE) };
    let values = parameters(table, window);
    for analyzed in [false, true] {
        if analyzed {
            db.execute_batch("ANALYZE").unwrap();
        }
        for (label, original, current) in [
            ("stream", legacy_visits(table, columns), sql(window)),
            ("orphans", legacy_orphans(table), orphans(table, window)),
        ] {
            let (rows, scan) = read(&db, &current, &values);
            if label == "stream" {
                assert!(rows.is_empty());
            } else {
                assert_eq!(rows, vec![vec![Value::Integer(0)]]);
            }
            let old_values = [Value::Integer(CURSOR), table.time_floor(Some(SINCE))];
            let old_scan = read(&db, &original, &old_values).1;
            println!(
                "SQLite {} {table:?} analyzed={analyzed} {label}: before fullscan={old_scan} {:?}; after fullscan={scan} {:?}",
                rusqlite::version(),
                plan(&db, &original, &old_values),
                plan(&db, &current, &values)
            );
            assert_recent_work(&db, table, &current, &values, scan, old_scan);
        }
    }
    // A quiet backup can also have many already-saved visits in the last day. Statistics on
    // that distribution can make the orphan OR choose a scan even though no ids are new.
    let (visits, _, _, time) = table.names();
    db.execute(&format!("UPDATE {visits} SET {time}=?1"), [table.time_floor(Some(SINCE))]).unwrap();
    db.execute_batch("ANALYZE").unwrap();
    for (label, original, current) in [
        ("stream", legacy_visits(table, columns), sql(window)),
        ("orphans", legacy_orphans(table), orphans(table, window)),
    ] {
        let old_values = [Value::Integer(CURSOR), table.time_floor(Some(SINCE))];
        let (rows, scan) = read(&db, &current, &values);
        assert_eq!(rows, read(&db, &original, &old_values).0);
        let old_scan = read(&db, &original, &old_values).1;
        println!(
            "SQLite {} {table:?} analyzed=true recent-existing {label}: before fullscan={old_scan} {:?}; after fullscan={scan} {:?}",
            rusqlite::version(),
            plan(&db, &original, &old_values),
            plan(&db, &current, &values)
        );
        assert_recent_work(&db, table, &current, &values, scan, old_scan);
    }
}

fn assert_recent_work(
    db: &Connection,
    table: VisitTable,
    sql: &str,
    values: &[Value],
    scan: i32,
    old_scan: i32,
) {
    let steps = plan(db, sql, values);
    if table == VisitTable::Safari {
        assert!(
            steps
                .iter()
                .any(|step| step
                    .contains("SEARCH history_visits USING INTEGER PRIMARY KEY (rowid>?)")),
            "{steps:?}"
        );
        assert!(
            steps.iter().any(|step| step
                .contains("SCAN history_visits USING COVERING INDEX history_visits__last_visit")),
            "{steps:?}"
        );
        assert!(scan >= CURSOR as i32 - 1, "Safari reads each visit's time: {scan}");
        assert!(scan <= old_scan, "Safari: before={old_scan}, after={scan}, {steps:?}");
    } else {
        assert_eq!(scan, 0, "{table:?}: {steps:?}");
    }
}

/// Reading every row, or continuing by id, must not materialize or sort an id list.
pub(crate) fn full_read_seeks(table: VisitTable, sql: impl Fn(VisitWindow) -> String) {
    let (_dir, db) = source(table);
    for analyzed in [false, true] {
        if analyzed {
            db.execute_batch("ANALYZE").unwrap();
        }
        for after_id in [0, CURSOR / 2, CURSOR] {
            let window = VisitWindow { after_id, recent_since_ms: None };
            let values = parameters(table, window);
            for query in [sql(window), orphans(table, window)] {
                let plan = plan(&db, &query, &values);
                assert!(
                    !plan
                        .iter()
                        .any(|step| step.contains("LIST SUBQUERY") || step.contains("B-TREE")),
                    "{plan:?}"
                );
                assert!(!query.contains("UNION"), "a full read needs only its rowid range");
                assert_eq!(read(&db, &query, &values).1, 0, "{table:?}: {plan:?}");
            }
        }
    }
}
