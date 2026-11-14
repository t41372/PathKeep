//! Responsibilities: prove a real SQLite/WAL source is captured as consistent logical evidence.
//! Not responsible for: archive encryption, harness-specific interpretation or native restoration.
//!
//! Written before implementation. Failure modes: rows from different commits mix; WAL-only
//! data disappears; identifiers become SQL; blobs or invalid UTF-8 lose bytes; implicit rowids
//! disappear behind declared columns; WITHOUT ROWID keys change; source errors become output
//! errors; cancellation leaves a falsely complete artifact; symlinks redirect the capture.

use agent_history::{Error, sqlite::capture_sqlite};
use rusqlite::Connection;
use serde_json::Value;
use std::{cell::Cell, io, io::Write};

fn records(bytes: &[u8]) -> Vec<Value> {
    std::str::from_utf8(bytes)
        .unwrap()
        .lines()
        .map(|line| serde_json::from_str(line).unwrap())
        .collect()
}

fn rows<'a>(records: &'a [Value], table: &str) -> Vec<&'a Value> {
    records.iter().filter(|row| row["type"] == "row" && row["table"] == table).collect()
}

#[test]
fn committed_wal_rows_and_metadata_belong_to_one_snapshot() {
    let temp = tempfile::tempdir().unwrap();
    let path = temp.path().join("source.sqlite");
    let source = Connection::open(&path).unwrap();
    source
        .execute_batch(
            "PRAGMA journal_mode=WAL; PRAGMA wal_autocheckpoint=0;
             PRAGMA user_version=17; PRAGMA application_id=1146312784;
             CREATE TABLE messages(id INTEGER PRIMARY KEY, body TEXT);
             INSERT INTO messages VALUES(1,'before'),(2,'before');",
        )
        .unwrap();
    struct ConcurrentWriter<'a> {
        connection: &'a Connection,
        bytes: Vec<u8>,
        changed: bool,
    }
    impl Write for ConcurrentWriter<'_> {
        fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
            self.bytes.extend_from_slice(bytes);
            if !self.changed && self.bytes.contains(&b'\n') {
                self.connection
                    .execute_batch(
                        "BEGIN; UPDATE messages SET body='after';
                         INSERT INTO messages VALUES(3,'after');
                         PRAGMA user_version=18; COMMIT;",
                    )
                    .unwrap();
                self.changed = true;
            }
            Ok(bytes.len())
        }
        fn flush(&mut self) -> io::Result<()> {
            Ok(())
        }
    }
    let mut output = ConcurrentWriter { connection: &source, bytes: Vec::new(), changed: false };
    let report = capture_sqlite(&path, &mut output, &|| false).unwrap();
    let evidence = records(&output.bytes);
    assert!(output.changed);
    assert!(report.complete);
    assert_eq!(evidence[0]["format"], "pathkeep-sqlite-evidence-v2");
    assert_eq!(evidence[0]["native_bytes"], false);
    assert_eq!(evidence[0]["user_version"], 17);
    let messages = rows(&evidence, "messages");
    assert_eq!(messages.len(), 2);
    assert!(messages.iter().all(|row| row["values"][1] == "before"));
    assert_eq!(
        source.query_row("SELECT count(*) FROM messages", [], |r| r.get::<_, i64>(0)).unwrap(),
        3
    );
    assert_eq!(evidence.last().unwrap()["type"], "end");
}

#[test]
fn storage_classes_rowids_and_quoted_schema_survive() {
    let temp = tempfile::tempdir().unwrap();
    let path = temp.path().join("source.sqlite");
    let source = Connection::open(&path).unwrap();
    source
        .execute_batch(
            "CREATE TABLE \"odd\"\";DROP TABLE innocent;--\"(n, i, r, t, b);
             INSERT INTO \"odd\"\";DROP TABLE innocent;--\"(rowid,n,i,r,t,b)
             VALUES(912,NULL,-9223372036854775808,-1.5,CAST(X'80FF0041' AS TEXT),X'0001FF');
             CREATE TABLE innocent(value TEXT);
             INSERT INTO innocent VALUES('still here');
             CREATE TABLE keyed(a TEXT,b INTEGER,payload BLOB,PRIMARY KEY(a,b)) WITHOUT ROWID;
             INSERT INTO keyed VALUES('z',9,X'ABCD');
             CREATE TABLE shadowed(rowid TEXT, value TEXT);
             INSERT INTO shadowed(_rowid_,rowid,value) VALUES(42,'declared','data');",
        )
        .unwrap();
    let before = std::fs::read(&path).unwrap();
    let mut output = Vec::new();
    let report = capture_sqlite(&path, &mut output, &|| false).unwrap();
    assert!(report.complete);
    assert_eq!(std::fs::read(&path).unwrap(), before);
    let evidence = records(&output);
    assert_eq!(evidence[0]["text_encoding"], "sqlite-utf8-api");
    let item = rows(&evidence, "odd\";DROP TABLE innocent;--")[0];
    assert_eq!(item["rowid"], 912);
    assert_eq!(item["values"][0], serde_json::json!({"type":"null"}));
    assert_eq!(item["values"][1]["value"].as_i64(), Some(i64::MIN));
    assert_eq!(item["values"][2]["bits"], format!("{:016x}", (-1.5_f64).to_bits()));
    assert_eq!(item["values"][3]["hex"], "80ff0041");
    assert_eq!(item["values"][4]["hex"], "0001ff");
    assert_eq!(rows(&evidence, "shadowed")[0]["rowid"], 42);
    assert_eq!(rows(&evidence, "keyed")[0]["rowid"], Value::Null);
    assert!(evidence.iter().any(|item| item["type"] == "table"
        && item["name"] == "keyed"
        && item["without_rowid"] == true));
    assert!(evidence.iter().any(|item| item["type"] == "schema" && item["name"] == "innocent"));
    assert_eq!(rows(&evidence, "innocent").len(), 1);
}

#[test]
fn unaddressable_hidden_rowid_is_reported_instead_of_invented() {
    let temp = tempfile::tempdir().unwrap();
    let path = temp.path().join("source.sqlite");
    let source = Connection::open(&path).unwrap();
    source
        .execute_batch(
            "CREATE TABLE hidden(rowid TEXT,_rowid_ TEXT,oid TEXT,payload TEXT);
             INSERT INTO hidden VALUES('r','u','o','retained');
             CREATE TABLE aliased(id INTEGER PRIMARY KEY,rowid TEXT,_rowid_ TEXT,oid TEXT);
             INSERT INTO aliased VALUES(89,'r','u','o');",
        )
        .unwrap();
    let mut output = Vec::new();
    let report = capture_sqlite(&path, &mut output, &|| false).unwrap();
    let evidence = records(&output);
    assert!(!report.complete);
    assert!(report.diagnostics.iter().any(|issue| issue.code == "sqlite_hidden_rowid_unavailable"));
    assert_eq!(rows(&evidence, "hidden")[0]["values"][3], "retained");
    assert_eq!(rows(&evidence, "aliased")[0]["rowid"], 89);
    assert_eq!(evidence.last().unwrap()["complete"], false);
}

#[test]
fn an_explicit_cell_limit_fails_without_truncating_the_database() {
    use agent_history::sqlite::{SqliteCaptureOptions, capture_sqlite_with_options};
    let temp = tempfile::tempdir().unwrap();
    let path = temp.path().join("source.sqlite");
    let source = Connection::open(&path).unwrap();
    source
        .execute_batch("CREATE TABLE items(value BLOB); INSERT INTO items VALUES(zeroblob(16384));")
        .unwrap();
    let mut partial = Vec::new();
    let options = SqliteCaptureOptions { max_row_bytes: 4096, ..Default::default() };
    let result = capture_sqlite_with_options(&path, &mut partial, &|| false, &options);
    assert!(matches!(result, Err(Error::LimitExceeded(_))));
    assert!(!String::from_utf8_lossy(&partial).contains("\"type\":\"end\""));
    assert_eq!(
        source
            .query_row("SELECT length(value) FROM items", [], |row| row.get::<_, i64>(0))
            .unwrap(),
        16384
    );
}

#[test]
fn cancellation_and_destination_failure_never_emit_a_complete_footer() {
    let temp = tempfile::tempdir().unwrap();
    let path = temp.path().join("source.sqlite");
    let source = Connection::open(&path).unwrap();
    source
        .execute_batch("CREATE TABLE items(value TEXT); INSERT INTO items VALUES('secret');")
        .unwrap();
    let calls = Cell::new(0);
    let cancel = || {
        calls.set(calls.get() + 1);
        calls.get() > 3
    };
    let mut partial = Vec::new();
    assert!(matches!(capture_sqlite(&path, &mut partial, &cancel), Err(Error::Cancelled)));
    assert!(!String::from_utf8_lossy(&partial).contains("\"type\":\"end\""));
    struct FullDisk;
    impl Write for FullDisk {
        fn write(&mut self, _: &[u8]) -> io::Result<usize> {
            Err(io::Error::new(io::ErrorKind::StorageFull, "destination full"))
        }
        fn flush(&mut self) -> io::Result<()> {
            Ok(())
        }
    }
    let failure = capture_sqlite(&path, &mut FullDisk, &|| false).unwrap_err();
    assert!(matches!(failure, Error::Io(ref error) if error.kind() == io::ErrorKind::StorageFull));
    let missing = capture_sqlite(&temp.path().join("missing.sqlite"), &mut Vec::new(), &|| false)
        .unwrap_err();
    assert!(matches!(missing, Error::InvalidInput(_)));
    assert!(!temp.path().join("missing.sqlite").exists());
}

#[cfg(unix)]
#[test]
fn an_ambient_directory_alias_does_not_make_the_selected_file_unreadable() {
    let temp = tempfile::tempdir().unwrap();
    let directory = temp.path().join("real");
    std::fs::create_dir(&directory).unwrap();
    let source = Connection::open(directory.join("source.sqlite")).unwrap();
    source
        .execute_batch("CREATE TABLE items(value TEXT); INSERT INTO items VALUES('retained');")
        .unwrap();
    let alias = temp.path().join("alias");
    std::os::unix::fs::symlink(&directory, &alias).unwrap();
    let mut output = Vec::new();
    let report = capture_sqlite(&alias.join("source.sqlite"), &mut output, &|| false).unwrap();
    assert!(report.complete);
    assert_eq!(rows(&records(&output), "items").len(), 1);
}

#[cfg(unix)]
#[test]
fn final_component_symlink_is_not_followed() {
    let temp = tempfile::tempdir().unwrap();
    let path = temp.path().join("source.sqlite");
    let source = Connection::open(&path).unwrap();
    source.execute_batch("CREATE TABLE items(value TEXT);").unwrap();
    let link = temp.path().join("link.sqlite");
    std::os::unix::fs::symlink(&path, &link).unwrap();
    assert!(capture_sqlite(&link, &mut Vec::new(), &|| false).is_err());
}
