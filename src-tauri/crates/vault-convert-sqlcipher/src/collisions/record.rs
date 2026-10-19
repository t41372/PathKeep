//! Responsibilities: deterministic content, counter and set merging for conversion.
//! Not responsible for: SQL, queue execution, file recovery or live intelligence rules.
//! Failure modes: timestamp offsets choose stale content, IDs treated as counters,
//! ties depend on replay order, null clears lost, arrays duplicated or counters overwritten.
//! Tests: the generated-archive collision acceptance test.
use crate::source::Row;
use anyhow::Result;
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value as Json};
use vault_store::Value;

#[derive(Serialize, Deserialize)]
pub(super) struct Record {
    pub id: i64,
    winner: i64,
    values: Map<String, Json>,
}
impl Record {
    pub fn from_row(fields: &[String], row: Row) -> Self {
        Self {
            id: row.id,
            winner: row.id,
            values: fields.iter().cloned().zip(row.values.into_iter().map(json)).collect(),
        }
    }
    pub fn key(&self, table: &str) -> String {
        match table {
            "intelligence_jobs" => self.values["dedupe_key"].as_str().unwrap().into(),
            _ => self.id.to_string(),
        }
    }
    pub fn job_id(&self) -> i64 {
        self.values["job_id"].as_i64().unwrap()
    }
    pub fn set_job_id(&mut self, id: i64) {
        self.values.insert("job_id".into(), id.into());
    }
    pub fn row(&self, fields: &[String]) -> Row {
        Row { id: self.id, values: fields.iter().map(|name| cell(&self.values[name])).collect() }
    }
    pub fn merge(self, other: Self) -> Result<Self> {
        let time = "updated_at";
        let order = timestamp(&self.values[time])
            .cmp(&timestamp(&other.values[time]))
            .then(self.winner.cmp(&other.winner));
        let (mut latest, older) = if order.is_lt() { (other, self) } else { (self, other) };
        latest.id = latest.id.min(older.id);
        let (counters, json_fields): (&[&str], &[&str]) = {
            latest.values.insert("id".into(), latest.id.into());
            let priority = latest.values["priority"]
                .as_i64()
                .unwrap()
                .min(older.values["priority"].as_i64().unwrap());
            latest.values.insert("priority".into(), priority.into());
            if timestamp(&older.values["created_at"]) < timestamp(&latest.values["created_at"]) {
                latest.values.insert("created_at".into(), older.values["created_at"].clone());
            }
            (&["attempt"], &["payload_json", "artifact_json"])
        };
        for name in counters {
            latest.values.insert((*name).into(), sum(&latest.values[*name], &older.values[*name]));
        }
        for name in json_fields {
            let mut value: Json = serde_json::from_str(latest.values[*name].as_str().unwrap())?;
            let previous: Json = serde_json::from_str(older.values[*name].as_str().unwrap())?;
            merge_json(&mut value, previous, "", false);
            latest.values.insert((*name).into(), value.to_string().into());
        }
        Ok(latest)
    }
}

// Existing app timestamps are RFC3339. Old fixture/diagnostic labels retain their
// deterministic lexical order, while real timestamps compare as instants.
fn timestamp(value: &Json) -> (Option<i64>, &str) {
    let text = value.as_str().unwrap();
    match chrono::DateTime::parse_from_rfc3339(text) {
        Ok(time) => (Some(time.timestamp_micros()), ""),
        Err(_) => (None, text),
    }
}
fn json(value: Value) -> Json {
    match value {
        Value::Null => Json::Null,
        Value::Integer(v) => v.into(),
        Value::Real(v) => serde_json::json!(v),
        Value::Text(v) => v.into(),
        Value::Blob(_) => unreachable!("job schemas have no blob columns"),
    }
}
fn cell(value: &Json) -> Value {
    match value {
        Json::Null => Value::Null,
        Json::String(v) => Value::Text(v.clone()),
        Json::Number(v) if v.is_i64() => Value::Integer(v.as_i64().unwrap()),
        Json::Number(v) => Value::Real(v.as_f64().unwrap()),
        _ => unreachable!("SQL records contain scalar fields; JSON columns remain strings"),
    }
}
fn sum(a: &Json, b: &Json) -> Json {
    match (a.as_i64(), b.as_i64()) {
        (Some(a), Some(b)) => (a + b).into(),
        _ => serde_json::json!(a.as_f64().unwrap() + b.as_f64().unwrap()),
    }
}
fn counter(name: &str) -> bool {
    name.ends_with("Count")
        || name.ends_with("_count")
        || matches!(
            name,
            "count"
                | "completedSteps"
                | "totalSteps"
                | "processedItems"
                | "totalItems"
                | "processedVisits"
                | "completed_steps"
                | "total_steps"
                | "processed_items"
                | "total_items"
                | "processed_visits"
        )
}
fn merge_json(latest: &mut Json, older: Json, name: &str, counters: bool) {
    match (latest, older) {
        (Json::Object(a), Json::Object(b)) => {
            for (key, value) in b {
                match a.get_mut(&key) {
                    Some(current) => merge_json(
                        current,
                        value,
                        &key,
                        counters || matches!(name, "counts" | "counters"),
                    ),
                    None => {
                        a.insert(key, value);
                    }
                }
            }
        }
        (Json::Array(a), Json::Array(b)) => {
            let union = a
                .drain(..)
                .chain(b)
                .map(|value| (value.to_string(), value))
                .collect::<std::collections::BTreeMap<_, _>>();
            a.extend(union.into_values());
        }
        (a @ Json::Number(_), b @ Json::Number(_)) if counters || counter(name) => *a = sum(a, &b),
        // A scalar, including an explicit null, belongs to the newest content.
        _ => {}
    }
}
