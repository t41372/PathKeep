use serde_json::Value;
use std::{collections::BTreeSet, env, fs, process};

fn main() {
    if let Err(error) = run() {
        eprintln!("{error}");
        process::exit(1);
    }
}

fn run() -> Result<(), Box<dyn std::error::Error>> {
    let args = env::args().skip(1).collect::<Vec<_>>();
    if args.len() != 2 {
        return Err("usage: cargo run -p browser-history-parser --example source-diff -- <left-report.json> <right-report.json>".into());
    }

    let left: Value = serde_json::from_slice(&fs::read(&args[0])?)?;
    let right: Value = serde_json::from_slice(&fs::read(&args[1])?)?;

    let left_tables = table_names(&left);
    let right_tables = table_names(&right);
    let left_caps = capability_names(&left);
    let right_caps = capability_names(&right);

    let report = serde_json::json!({
        "leftOnlyTables": diff_set(&left_tables, &right_tables),
        "rightOnlyTables": diff_set(&right_tables, &left_tables),
        "leftOnlyCapabilities": diff_set(&left_caps, &right_caps),
        "rightOnlyCapabilities": diff_set(&right_caps, &left_caps),
    });

    println!("{}", serde_json::to_string_pretty(&report)?);
    Ok(())
}

fn table_names(report: &Value) -> BTreeSet<String> {
    report
        .get("schemaObservation")
        .and_then(|value| value.get("tables"))
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(|table| table.get("name").and_then(Value::as_str))
        .map(ToString::to_string)
        .collect()
}

fn capability_names(report: &Value) -> BTreeSet<String> {
    report
        .get("capabilitySnapshot")
        .and_then(|value| value.get("items"))
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(|item| item.get("key").and_then(Value::as_str))
        .map(ToString::to_string)
        .collect()
}

fn diff_set(left: &BTreeSet<String>, right: &BTreeSet<String>) -> Vec<String> {
    left.difference(right).cloned().collect()
}
