//! Responsibilities: retain a consistent committed SQLite snapshot as streaming logical evidence.
//! Not responsible for: native SQLite byte copies, restoring a harness, encryption or path consent.
//!
//! One read transaction includes committed WAL rows without copying live sidecars. Evidence
//! preserves schema, storage classes and addressable rowids; it is deliberately not a native
//! `.sqlite` file. SQLite may coordinate through its WAL shared-memory sidecar despite a
//! read-only connection. It never receives permission to change source conversation data.
//! TEXT bytes come from SQLite's UTF-8 API, not the original physical encoding of UTF-16 cells.

use std::{
    io::{self, Write},
    path::Path,
    time::{Duration, Instant},
};

use rusqlite::{Connection, OpenFlags, Row, config::DbConfig, limits::Limit, types::ValueRef};
use serde::Serialize;
use serde_json::{Value, json};

use crate::{Error, Result, model::Diagnostic};

/// Identifies the logical codec so callers cannot mislabel its output as native database bytes.
pub const SQLITE_EVIDENCE_FORMAT: &str = "pathkeep-sqlite-evidence-v2";

/// Explicit limits make a pathological source fail honestly instead of allocating without bound.
#[derive(Clone, Debug)]
pub struct SqliteCaptureOptions {
    pub max_row_bytes: usize,
    pub max_duration: Duration,
    pub busy_timeout: Duration,
}

impl Default for SqliteCaptureOptions {
    fn default() -> Self {
        Self {
            max_row_bytes: 64 * 1024 * 1024,
            max_duration: Duration::from_secs(15 * 60),
            busy_timeout: Duration::from_secs(2),
        }
    }
}

/// Describes the retained logical snapshot and any facts SQLite could not expose through SQL.
#[derive(Clone, Debug, Serialize)]
pub struct SqliteCaptureReport {
    pub format: &'static str,
    pub tables: u64,
    pub rows: u64,
    pub complete: bool,
    pub diagnostics: Vec<Diagnostic>,
}

/// Captures source evidence without plaintext staging or source writes. The caller owns output
/// publication: an error can leave an incomplete stream that must never become a valid revision.
pub fn capture_sqlite(
    path: &Path,
    output: &mut dyn Write,
    cancel: &dyn Fn() -> bool,
) -> Result<SqliteCaptureReport> {
    capture_sqlite_with_options(path, output, cancel, &SqliteCaptureOptions::default())
}

/// Allows an embedding application to select explicit source limits. Cancellation is checked
/// between rows; an owned deadline also interrupts long SQLite virtual-machine operations.
pub fn capture_sqlite_with_options(
    path: &Path,
    output: &mut dyn Write,
    cancel: &dyn Fn() -> bool,
    options: &SqliteCaptureOptions,
) -> Result<SqliteCaptureReport> {
    if options.max_row_bytes < 1024 || options.max_row_bytes > i32::MAX as usize {
        return Err(Error::InvalidInput("sqlite_row_limit_out_of_range".into()));
    }
    let deadline = Instant::now()
        .checked_add(options.max_duration)
        .ok_or_else(|| Error::InvalidInput("sqlite_deadline_out_of_range".into()))?;
    check_control(cancel, deadline)?;
    let connection = open_source(path, options, deadline)?;
    let transaction = connection.unchecked_transaction().map_err(source_error)?;
    // BEGIN alone is deferred: acquire the actual read snapshot before yielding output to a
    // caller whose writer may block while a harness commits another version.
    transaction
        .query_row("SELECT count(*) FROM main.sqlite_schema", [], |row| row.get::<_, i64>(0))
        .map_err(source_error)?;
    write_header(&transaction, output)?;
    write_schema(&transaction, output, cancel, deadline)?;
    let mut report = SqliteCaptureReport {
        format: SQLITE_EVIDENCE_FORMAT,
        tables: 0,
        rows: 0,
        complete: true,
        diagnostics: Vec::new(),
    };
    {
        // The outer cursor remains bounded; there is no in-memory list of every table or row.
        let mut statement = transaction
            .prepare("SELECT name FROM main.sqlite_schema WHERE type='table' ORDER BY rowid")
            .map_err(source_error)?;
        let mut tables = statement.query([]).map_err(source_error)?;
        while let Some(table) = tables.next().map_err(source_error)? {
            check_control(cancel, deadline)?;
            let name: String = table.get(0).map_err(source_error)?;
            capture_table(&transaction, &name, output, cancel, deadline, &mut report)?;
        }
    }
    check_control(cancel, deadline)?;
    transaction.commit().map_err(source_error)?;
    write_record(
        output,
        &json!({"type":"end", "tables":report.tables, "rows":report.rows,
            "complete":report.complete, "diagnostics":report.diagnostics}),
    )?;
    Ok(report)
}

fn open_source(
    path: &Path,
    options: &SqliteCaptureOptions,
    deadline: Instant,
) -> Result<Connection> {
    // SQLite's NOFOLLOW rejects a symlink anywhere in the supplied pathname. macOS's
    // ordinary temporary directory contains the OS-managed /var alias. Establish the
    // explicit file authority first: ambient ancestors may resolve, the selected file
    // itself may not be a link. NOFOLLOW then protects the canonical open against a
    // subsequently introduced symlink rather than rejecting normal host paths.
    let canonical =
        crate::capture::files::ensure_source_path(path, path).map_err(|error| match error {
            Error::Io(_) => Error::InvalidInput("sqlite_source_path_unavailable".into()),
            other => other,
        })?;
    let connection = Connection::open_with_flags(
        canonical,
        OpenFlags::SQLITE_OPEN_READ_ONLY
            | OpenFlags::SQLITE_OPEN_NO_MUTEX
            | OpenFlags::SQLITE_OPEN_NOFOLLOW,
    )
    .map_err(source_error)?;
    connection
        .busy_timeout(options.busy_timeout.min(options.max_duration))
        .map_err(source_error)?;
    connection
        .set_limit(Limit::SQLITE_LIMIT_LENGTH, options.max_row_bytes as i32)
        .map_err(source_error)?;
    connection
        .set_db_config(DbConfig::SQLITE_DBCONFIG_TRUSTED_SCHEMA, false)
        .map_err(source_error)?;
    connection.set_db_config(DbConfig::SQLITE_DBCONFIG_DEFENSIVE, true).map_err(source_error)?;
    connection
        .set_db_config(DbConfig::SQLITE_DBCONFIG_NO_CKPT_ON_CLOSE, true)
        .map_err(source_error)?;
    connection
        .execute_batch("PRAGMA query_only=ON; PRAGMA temp_store=MEMORY;")
        .map_err(source_error)?;
    connection
        .progress_handler(1000, Some(move || Instant::now() >= deadline))
        .map_err(source_error)?;
    Ok(connection)
}

fn write_header(connection: &Connection, output: &mut dyn Write) -> Result<()> {
    let integer =
        |name| connection.pragma_query_value(Some("main"), name, |row| row.get::<_, i64>(0));
    let encoding: String = connection
        .pragma_query_value(Some("main"), "encoding", |row| row.get(0))
        .map_err(source_error)?;
    write_record(
        output,
        &json!({"type":"header", "format":SQLITE_EVIDENCE_FORMAT,
            "native_bytes":false, "text_encoding":"sqlite-utf8-api",
            "sqlite_version":rusqlite::version(),
            "user_version":integer("user_version").map_err(source_error)?,
            "application_id":integer("application_id").map_err(source_error)?,
            "page_size":integer("page_size").map_err(source_error)?, "encoding":encoding}),
    )
}

fn write_schema(
    connection: &Connection,
    output: &mut dyn Write,
    cancel: &dyn Fn() -> bool,
    deadline: Instant,
) -> Result<()> {
    let mut statement = connection
        .prepare(
            "SELECT rowid, type, name, tbl_name, rootpage, sql FROM main.sqlite_schema ORDER BY
                rowid",
        )
        .map_err(source_error)?;
    let mut rows = statement.query([]).map_err(source_error)?;
    while let Some(row) = rows.next().map_err(source_error)? {
        check_control(cancel, deadline)?;
        write_record(
            output,
            &json!({"type":"schema",
            "rowid":row.get::<_,i64>(0).map_err(source_error)?,
            "object_type":row.get::<_,String>(1).map_err(source_error)?,
            "name":row.get::<_,String>(2).map_err(source_error)?,
            "table_name":row.get::<_,String>(3).map_err(source_error)?,
            "root_page":row.get::<_,i64>(4).map_err(source_error)?,
            "sql":row.get::<_,Option<String>>(5).map_err(source_error)?}),
        )?;
    }
    Ok(())
}

#[derive(Serialize)]
struct Column {
    name: String,
    declared_type: String,
    primary_key_position: i64,
    hidden: i64,
}

fn columns(connection: &Connection, name: &str) -> Result<Vec<Column>> {
    let mut statement = connection
        .prepare("SELECT name, type, pk, hidden FROM pragma_table_xinfo(?1, 'main' ) ORDER BY cid")
        .map_err(source_error)?;
    let mapped = statement
        .query_map([name], |row| {
            Ok(Column {
                name: row.get(0)?,
                declared_type: row.get(1)?,
                primary_key_position: row.get(2)?,
                hidden: row.get(3)?,
            })
        })
        .map_err(source_error)?;
    mapped.collect::<rusqlite::Result<Vec<_>>>().map_err(source_error)
}

fn rowid_column(
    connection: &Connection,
    name: &str,
    columns: &[Column],
    without_rowid: bool,
) -> Result<Option<String>> {
    if without_rowid {
        return Ok(None);
    }
    for alias in ["rowid", "_rowid_", "oid"] {
        if !columns.iter().any(|column| column.name.eq_ignore_ascii_case(alias)) {
            return Ok(Some(alias.into()));
        }
    }
    let primary: Vec<_> = columns.iter().filter(|column| column.primary_key_position > 0).collect();
    if primary.len() == 1 && primary[0].declared_type.eq_ignore_ascii_case("INTEGER") {
        // INTEGER PRIMARY KEY DESC is the historical exception: its separate PK index proves
        // this is not an alias for rowid. Table-level PRIMARY KEY(id DESC) still aliases rowid.
        let has_index: bool = connection
            .query_row(
                "SELECT EXISTS(SELECT 1 FROM pragma_index_list(?1, 'main' ) WHERE origin='pk')",
                [name],
                |row| row.get(0),
            )
            .map_err(source_error)?;
        if !has_index {
            return Ok(Some(primary[0].name.clone()));
        }
    }
    Ok(None)
}

fn capture_table(
    connection: &Connection,
    name: &str,
    output: &mut dyn Write,
    cancel: &dyn Fn() -> bool,
    deadline: Instant,
    report: &mut SqliteCaptureReport,
) -> Result<()> {
    let (kind, without_rowid): (String, bool) = connection
        .query_row(
            "SELECT type, wr FROM pragma_table_list WHERE schema='main' AND name=?1",
            [name],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .map_err(source_error)?;
    let columns = columns(connection, name)?;
    let rowid_column = rowid_column(connection, name, &columns, without_rowid)?;
    if !without_rowid && rowid_column.is_none() {
        report.complete = false;
        crate::adapters::common::push_diagnostic(
            &mut report.diagnostics,
            Diagnostic {
                code: "sqlite_hidden_rowid_unavailable".into(),
                path: None,
                ordinal: None,
                detail: Some(name.into()),
            },
        );
    }
    write_record(
        output,
        &json!({"type":"table", "name":name, "table_kind":kind,
        "without_rowid":without_rowid, "rowid_column":rowid_column, "columns":columns}),
    )?;
    // Explicit columns preserve generated/hidden columns too. NOT INDEXED avoids choosing a
    // covering secondary index and makes ordinary-table traversal use its native key order.
    let mut selected = Vec::with_capacity(columns.len() + 1);
    if let Some(alias) = &rowid_column {
        selected.push(quote(alias));
    }
    selected.extend(columns.iter().map(|column| quote(&column.name)));
    if selected.is_empty() {
        return Err(Error::Unsupported("sqlite_table_has_no_exposed_columns".into()));
    }
    let sql = format!("SELECT {} FROM main.{} NOT INDEXED", selected.join(","), quote(name));
    let mut statement = connection.prepare(&sql).map_err(source_error)?;
    let mut rows = statement.query([]).map_err(source_error)?;
    let offset = usize::from(rowid_column.is_some());
    while let Some(row) = rows.next().map_err(source_error)? {
        check_control(cancel, deadline)?;
        let rowid =
            if offset == 1 { Some(row.get::<_, i64>(0).map_err(source_error)?) } else { None };
        let values = (0..columns.len())
            .map(|index| cell(row, index + offset))
            .collect::<Result<Vec<_>>>()?;
        write_record(output, &json!({"type":"row", "table":name, "rowid":rowid, "values":values}))?;
        report.rows += 1;
    }
    report.tables += 1;
    Ok(())
}

fn cell(row: &Row<'_>, index: usize) -> Result<Value> {
    Ok(match row.get_ref(index).map_err(source_error)? {
        ValueRef::Null => json!({"type":"null"}),
        ValueRef::Integer(value) => json!({"type":"integer", "value":value}),
        ValueRef::Real(value) => json!({"type":"real", "bits":format!("{:016x}",value.to_bits())}),
        ValueRef::Text(value) => match std::str::from_utf8(value) {
            Ok(text) => json!(text),
            Err(_) => json!({"type":"text", "hex":hex::encode(value)}),
        },
        ValueRef::Blob(value) => json!({"type":"blob", "hex":hex::encode(value)}),
    })
}

fn quote(identifier: &str) -> String {
    format!("\"{}\"", identifier.replace('"', "\"\""))
}

fn check_control(cancel: &dyn Fn() -> bool, deadline: Instant) -> Result<()> {
    if cancel() {
        return Err(Error::Cancelled);
    }
    if Instant::now() >= deadline {
        return Err(Error::LimitExceeded("sqlite_capture_deadline".into()));
    }
    Ok(())
}

fn write_record(output: &mut dyn Write, value: &impl Serialize) -> Result<()> {
    serde_json::to_writer(&mut *output, value).map_err(|error| match error.io_error_kind() {
        Some(kind) => Error::Io(io::Error::new(kind, "SQLite evidence output failed")),
        None => Error::Json(error),
    })?;
    output.write_all(b"\n")?;
    Ok(())
}

fn source_error(error: rusqlite::Error) -> Error {
    match error.sqlite_error_code() {
        Some(rusqlite::ErrorCode::TooBig) => Error::LimitExceeded("sqlite_source_row_limit".into()),
        Some(rusqlite::ErrorCode::OperationInterrupted) => {
            Error::LimitExceeded("sqlite_capture_deadline".into())
        }
        // Engine messages can contain schema identifiers or source content. The caller gets a
        // stable source-read classification; destination writer failures remain Error::Io.
        code => Error::InvalidInput(format!("sqlite_source_read_failed:{code:?}")),
    }
}
