//! Shared schema-observation and native-row capture helpers.

use crate::{
    error::ParseError,
    types::{
        CapabilityCoverage, CapabilitySnapshot, NativeEntity, ObservedColumn, ObservedTable,
        SchemaObservation,
    },
};
use rusqlite::{Connection, Row, ToSql, types::ValueRef};
use serde_json::{Map, Value, json};
use std::collections::BTreeMap;

/// Inspects the current SQLite connection and returns a machine-readable schema observation.
pub fn inspect_schema(
    connection: &Connection,
    required_tables: &[&str],
) -> Result<SchemaObservation, ParseError> {
    let table_names = table_names(connection)?;
    let mut tables = Vec::new();
    for table_name in &table_names {
        tables.push(observed_table(
            connection,
            table_name,
            required_tables.contains(&table_name.as_str()),
        )?);
    }
    for table_name in required_tables {
        if !table_names.iter().any(|existing| existing == table_name) {
            tables.push(ObservedTable {
                name: (*table_name).to_string(),
                present: false,
                required: true,
                row_count: None,
                columns: Vec::new(),
            });
        }
    }
    tables.sort_by(|left, right| left.name.cmp(&right.name));
    Ok(SchemaObservation { tables })
}

/// Captures one full native table row as JSON payloads.
pub fn capture_native_rows(
    connection: &Connection,
    sql: &str,
    params: &[&dyn ToSql],
    entity_kind: &str,
    primary_key_column: &str,
    parent_key_column: Option<&str>,
) -> Result<Vec<NativeEntity>, ParseError> {
    let mut statement = connection.prepare(sql)?;
    let column_names =
        statement.column_names().iter().map(|name| name.to_string()).collect::<Vec<_>>();
    let rows = statement.query_map(params, |row| {
        native_entity_from_row(
            row,
            &column_names,
            entity_kind,
            primary_key_column,
            parent_key_column,
        )
    })?;
    Ok(rows.collect::<rusqlite::Result<Vec<_>>>()?)
}

/// Converts the current SQLite row into one preserved native-entity payload.
///
/// Streamed ingest paths use this helper to build source-native evidence while
/// they are already scanning canonical rows, avoiding a second full query over
/// the same table just to reconstruct raw-row JSON later.
pub fn capture_native_row(
    row: &Row<'_>,
    column_names: &[String],
    entity_kind: &str,
    primary_key_column: &str,
    parent_key_column: Option<&str>,
) -> rusqlite::Result<NativeEntity> {
    native_entity_from_row(row, column_names, entity_kind, primary_key_column, parent_key_column)
}

/// Builds a capability snapshot from a list of coarse coverage entries.
pub fn capability_snapshot(items: Vec<CapabilityCoverage>) -> CapabilitySnapshot {
    CapabilitySnapshot { items }
}

fn table_names(connection: &Connection) -> Result<Vec<String>, ParseError> {
    let mut statement = connection.prepare(
        "SELECT name
         FROM sqlite_master
         WHERE type = 'table'
           AND name NOT LIKE 'sqlite_%'
         ORDER BY name",
    )?;
    Ok(statement
        .query_map([], |row| row.get::<_, String>(0))?
        .collect::<rusqlite::Result<Vec<_>>>()?)
}

fn observed_table(
    connection: &Connection,
    table_name: &str,
    required: bool,
) -> Result<ObservedTable, ParseError> {
    let pragma_sql = format!("PRAGMA table_info({})", quoted_identifier(table_name));
    let mut statement = connection.prepare(&pragma_sql)?;
    let columns = statement
        .query_map([], |row| {
            Ok(ObservedColumn {
                name: row.get(1)?,
                data_type: row.get(2)?,
                not_null: row.get::<_, i64>(3)? != 0,
                primary_key_ordinal: row.get(5)?,
            })
        })?
        .collect::<rusqlite::Result<Vec<_>>>()?;
    let row_count_sql = format!("SELECT COUNT(*) FROM {}", quoted_identifier(table_name));
    let row_count = connection.query_row(&row_count_sql, [], |row| row.get(0)).ok();
    Ok(ObservedTable { name: table_name.to_string(), present: true, required, row_count, columns })
}

fn native_entity_from_row(
    row: &Row<'_>,
    column_names: &[String],
    entity_kind: &str,
    primary_key_column: &str,
    parent_key_column: Option<&str>,
) -> rusqlite::Result<NativeEntity> {
    let payload = row_payload_json(row, column_names)?;
    let primary_key =
        payload.get(primary_key_column).map(value_to_key).unwrap_or_else(|| "unknown".to_string());
    let parent_primary_key =
        parent_key_column.and_then(|column| payload.get(column).map(value_to_key));
    Ok(NativeEntity {
        entity_kind: entity_kind.to_string(),
        native_primary_key: primary_key,
        parent_native_primary_key: parent_primary_key,
        payload_json: Value::Object(payload.clone()).to_string(),
        metadata: BTreeMap::from([
            ("primaryKeyColumn".to_string(), primary_key_column.to_string()),
            ("columnCount".to_string(), column_names.len().to_string()),
        ]),
    })
}

fn row_payload_json(
    row: &Row<'_>,
    column_names: &[String],
) -> rusqlite::Result<Map<String, Value>> {
    let mut payload = Map::new();
    for (index, column_name) in column_names.iter().enumerate() {
        let value = row.get_ref(index)?;
        payload.insert(column_name.clone(), value_ref_to_json(value));
    }
    Ok(payload)
}

fn value_ref_to_json(value: ValueRef<'_>) -> Value {
    match value {
        ValueRef::Null => Value::Null,
        ValueRef::Integer(integer) => json!(integer),
        ValueRef::Real(real) => json!(real),
        ValueRef::Text(text) => Value::String(String::from_utf8_lossy(text).to_string()),
        ValueRef::Blob(blob) => Value::String(hex::encode(blob)),
    }
}

fn value_to_key(value: &Value) -> String {
    match value {
        Value::Null => "null".to_string(),
        Value::Bool(boolean) => boolean.to_string(),
        Value::Number(number) => number.to_string(),
        Value::String(string) => string.clone(),
        Value::Array(_) | Value::Object(_) => value.to_string(),
    }
}

fn quoted_identifier(identifier: &str) -> String {
    format!("\"{}\"", identifier.replace('"', "\"\""))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn schema_observation_marks_required_missing_tables_and_column_nullability() {
        let connection = Connection::open_in_memory().expect("open memory db");
        connection
            .execute(
                "CREATE TABLE present_required (
                    id INTEGER PRIMARY KEY,
                    title TEXT NOT NULL,
                    optional_note TEXT
                )",
                [],
            )
            .expect("create required table");
        connection
            .execute("CREATE TABLE optional_table (flag INTEGER)", [])
            .expect("create optional table");
        connection
            .execute(
                "INSERT INTO present_required (title, optional_note) VALUES ('Example', NULL)",
                [],
            )
            .expect("insert required row");

        let observation = inspect_schema(&connection, &["present_required", "missing_required"])
            .expect("inspect schema");

        let table = |name: &str| {
            observation.tables.iter().find(|table| table.name == name).expect("table observed")
        };
        let present_required = table("present_required");
        assert!(present_required.present);
        assert!(present_required.required);
        assert_eq!(present_required.row_count, Some(1));
        let title = present_required
            .columns
            .iter()
            .find(|column| column.name == "title")
            .expect("title column");
        assert!(title.not_null);
        assert_eq!(title.primary_key_ordinal, 0);
        let id =
            present_required.columns.iter().find(|column| column.name == "id").expect("id column");
        assert_eq!(id.primary_key_ordinal, 1);

        let optional_table = table("optional_table");
        assert!(optional_table.present);
        assert!(!optional_table.required);
        assert_eq!(optional_table.row_count, Some(0));
        assert!(!optional_table.columns[0].not_null);

        let missing_required = table("missing_required");
        assert!(!missing_required.present);
        assert!(missing_required.required);
        assert_eq!(missing_required.row_count, None);
        assert!(missing_required.columns.is_empty());
    }

    #[test]
    fn native_value_conversion_preserves_non_text_keys() {
        assert_eq!(value_ref_to_json(ValueRef::Blob(&[0xab, 0xcd])), json!("abcd"));
        assert_eq!(value_to_key(&Value::Null), "null");
        assert_eq!(value_to_key(&json!(true)), "true");
        assert_eq!(value_to_key(&json!(["a", 2])), r#"["a",2]"#);
        assert_eq!(value_to_key(&json!({"kind": "native"})), r#"{"kind":"native"}"#);
    }
}
