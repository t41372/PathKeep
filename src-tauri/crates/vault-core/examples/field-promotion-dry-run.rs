use std::{env, process};

use vault_core::{ArchiveMode, Vault, config::load_config, config::project_paths};
use vault_store::Db;

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
            "usage: cargo run --manifest-path src-tauri/Cargo.toml -p vault-core --example field-promotion-dry-run -- <entity-kind>\n\
             An encrypted archive reads its password from PATHKEEP_PASSWORD."
        )
    }

    let entity_kind = args[0].clone();
    let paths = project_paths()?;
    let config = load_config(&paths)?;
    let vault = if config.archive_mode == ArchiveMode::Encrypted {
        let password = env::var("PATHKEEP_PASSWORD")
            .map_err(|_| anyhow::anyhow!("set PATHKEEP_PASSWORD to open an encrypted archive"))?;
        Vault::open_password(paths, config, &password, None)?.0
    } else {
        Vault::open(paths, config, None)?
    };

    let (total_matching, distinct_batches) = vault.store().read(Db::SourceEvidence, move |s| {
        s.query_one(
            "SELECT COUNT(*), COUNT(DISTINCT source_batch_id) FROM native_entities WHERE entity_kind = ?1",
            [entity_kind.as_str()],
            |row| Ok((row.get::<i64>(0)?, row.get::<i64>(1)?)),
        )
    })?;
    vault.close()?;

    println!(
        "{}",
        serde_json::to_string_pretty(&serde_json::json!({
            "entityKind": args[0],
            "matchingRows": total_matching,
            "distinctBatches": distinct_batches,
            "note": "This dry-run only reports preserved native entity coverage. Promotion logic should still define typed evidence mapping, capability impact, and backfill policy before execution.",
        }))?
    );
    Ok(())
}
