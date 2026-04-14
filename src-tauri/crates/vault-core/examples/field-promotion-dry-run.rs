use std::{env, process};

use vault_core::{
    archive::open_source_evidence_connection, config::load_config, config::project_paths,
};

fn main() {
    if let Err(error) = run() {
        eprintln!("{error:#}");
        process::exit(1);
    }
}

fn run() -> anyhow::Result<()> {
    let args = env::args().skip(1).collect::<Vec<_>>();
    if args.is_empty() {
        anyhow::bail!(
            "usage: cargo run --manifest-path src-tauri/Cargo.toml -p vault-core --example field-promotion-dry-run -- <entity-kind>"
        )
    }

    let entity_kind = &args[0];
    let paths = project_paths()?;
    let config = load_config(&paths)?;
    let connection = open_source_evidence_connection(&paths, &config, None)?;

    let total_matching: i64 = connection.query_row(
        "SELECT COUNT(*)
         FROM native_entities
         WHERE entity_kind = ?1",
        [entity_kind],
        |row| row.get(0),
    )?;
    let distinct_batches: i64 = connection.query_row(
        "SELECT COUNT(DISTINCT source_batch_id)
         FROM native_entities
         WHERE entity_kind = ?1",
        [entity_kind],
        |row| row.get(0),
    )?;

    println!(
        "{}",
        serde_json::to_string_pretty(&serde_json::json!({
            "entityKind": entity_kind,
            "matchingRows": total_matching,
            "distinctBatches": distinct_batches,
            "note": "This dry-run only reports preserved native entity coverage. Promotion logic should still define typed evidence mapping, capability impact, and backfill policy before execution.",
        }))?
    );
    Ok(())
}
