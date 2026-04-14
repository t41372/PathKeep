use browser_history_parser::{ChromiumReadCursor, HistoryDatabaseSet, chromium, firefox, safari};
use serde_json::json;
use std::{env, path::PathBuf, process};

fn main() {
    if let Err(error) = run() {
        eprintln!("{error}");
        process::exit(1);
    }
}

fn run() -> Result<(), Box<dyn std::error::Error>> {
    let args = env::args().skip(1).collect::<Vec<_>>();
    if args.len() < 2 {
        return Err("usage: cargo run -p browser-history-parser --example source-report -- <chromium|firefox|safari> <history-path> [favicons-path]".into());
    }

    let family = args[0].as_str();
    let history_path = PathBuf::from(&args[1]);
    let report = match family {
        "chromium" => {
            let parsed = chromium::parse_history(
                &HistoryDatabaseSet { history_path, favicons_path: args.get(2).map(PathBuf::from) },
                ChromiumReadCursor::default(),
            )?;
            json!({
                "family": "chromium",
                "schemaObservation": parsed.schema_observation,
                "capabilitySnapshot": parsed.capability_snapshot,
                "counts": {
                    "urls": parsed.urls.len(),
                    "visits": parsed.visits.len(),
                    "downloads": parsed.downloads.len(),
                    "searchTerms": parsed.search_terms.len(),
                    "nativeEntities": parsed.native_entities.len(),
                },
                "warnings": parsed.warnings,
            })
        }
        "firefox" => {
            let parsed = firefox::parse_history(&history_path, 0, 0)?;
            json!({
                "family": "firefox",
                "schemaObservation": parsed.schema_observation,
                "capabilitySnapshot": parsed.capability_snapshot,
                "counts": {
                    "urls": parsed.urls.len(),
                    "visits": parsed.visits.len(),
                    "nativeEntities": parsed.native_entities.len(),
                },
                "warnings": parsed.warnings,
            })
        }
        "safari" => {
            let parsed = safari::parse_history(&history_path, 0, 0)?;
            json!({
                "family": "safari",
                "schemaObservation": parsed.schema_observation,
                "capabilitySnapshot": parsed.capability_snapshot,
                "counts": {
                    "urls": parsed.urls.len(),
                    "visits": parsed.visits.len(),
                    "nativeEntities": parsed.native_entities.len(),
                },
                "warnings": parsed.warnings,
            })
        }
        other => return Err(format!("unsupported family `{other}`").into()),
    };

    println!("{}", serde_json::to_string_pretty(&report)?);
    Ok(())
}
