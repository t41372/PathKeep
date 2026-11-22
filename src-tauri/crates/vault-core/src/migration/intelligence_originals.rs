//! Move-file transport for user-maintained rules and metadata that locates saved page text.
//! Derived intelligence tables are rebuilt by their owners on the destination.

use crate::Vault;
use anyhow::{Context, Result};
use std::io::{self, BufRead, BufReader};
use std::path::Path;
use vault_crypto::DataKey;
use vault_store::{CancelToken, Db, Priority, Value};

pub(super) const ENTRY: &str = "archive/intelligence-originals.pkf";

// Explicit columns keep the wire format independent of column order and future cache fields.
const TABLES: [(&str, &str); 3] = [
    (
        "site_dictionary_overrides",
        "id,target_kind,target_value,domain_category,page_category,interaction_kind,display_name,search_engine,is_noisy,note,created_at,updated_at",
    ),
    (
        "search_engine_rules",
        "rule_id,engine_id,display_name,host_pattern,path_prefix,query_param_key,enabled,note,example_url,created_at,updated_at",
    ),
    (
        "page_content_enrichments",
        "id,canonical_url,content_source,fetch_status,fetched_at,final_url,language,readable_title,readable_text_blob_path,readable_text_bytes,text_hash,snippet_json,extraction_json,pipeline_version,extractor_version,enrichment_summary,refetch_after,http_status",
    ),
];

pub(super) fn pack(vault: &Vault, root: &Path, key: &DataKey, cancel: &CancelToken) -> Result<()> {
    std::fs::create_dir_all(root.join("archive"))?;
    let root = root.to_owned();
    let key = DataKey::from_bytes(zeroize::Zeroizing::new(*key.expose_bytes()));
    let cancel = cancel.clone();
    // A single read transaction gives all three tables the same cut. The cursor and the
    // encrypted writer stream rows without a plaintext staging file.
    vault.store().read(Db::Intelligence, move |session| {
        vault_crypto::write_artifact(&root, ENTRY, &key, |out| {
            for (name, columns) in TABLES {
                writeln!(out, "{name}")?;
                let mut statement = session
                    .prepare(&format!("SELECT json_array({columns}) FROM {name} ORDER BY rowid"))
                    .map_err(io::Error::other)?;
                let mut rows = statement.query(()).map_err(io::Error::other)?;
                while let Some(row) = rows.next().map_err(io::Error::other)? {
                    cancel.check().map_err(io::Error::other)?;
                    writeln!(out, "{}", row.get::<String>(0).map_err(io::Error::other)?)?;
                }
                writeln!(out)?;
            }
            Ok(())
        })?;
        Ok(())
    })?;
    Ok(())
}

pub(super) fn unpack(
    vault: &Vault,
    root: &Path,
    key: &DataKey,
    cancel: &CancelToken,
) -> Result<()> {
    vault_crypto::read_artifact(root, ENTRY, key, |input| {
        install_rows(vault, BufReader::new(input), cancel)
            .map_err(|e| io::Error::other(e.into_boxed_dyn_error()))
    })?;
    Ok(())
}

fn install_rows(vault: &Vault, file: impl BufRead, cancel: &CancelToken) -> Result<()> {
    let mut lines = file.lines();
    for (name, columns) in TABLES {
        anyhow::ensure!(
            lines.next().transpose()?.as_deref() == Some(name),
            "missing portable table {name}"
        );
        let width = columns.split(',').count();
        let bindings = vec!["?"; width].join(",");
        let sql = format!("INSERT OR REPLACE INTO {name}({columns}) VALUES({bindings})");
        loop {
            cancel.check()?;
            let mut batch = Vec::new();
            let mut ended = false;
            for _ in 0..128 {
                let line = lines.next().context("incomplete portable table")??;
                if line.is_empty() {
                    ended = true;
                    break;
                }
                let row: Vec<serde_json::Value> = serde_json::from_str(&line)?;
                anyhow::ensure!(row.len() == width, "wrong number of fields in {name}");
                let values = row
                    .into_iter()
                    .map(|v| {
                        Ok(match v {
                            serde_json::Value::Null => Value::Null,
                            serde_json::Value::String(s) => Value::Text(s),
                            serde_json::Value::Number(n) => {
                                Value::Integer(n.as_i64().context("invalid portable integer")?)
                            }
                            _ => anyhow::bail!("invalid portable field"),
                        })
                    })
                    .collect::<Result<Vec<_>>>()?;
                batch.push(values);
            }
            if !batch.is_empty() {
                let sql = sql.clone();
                vault.store().write(Db::Intelligence, Priority::Batch, move |session| {
                    session.execute_rows(&sql, batch)?;
                    Ok(())
                })?;
            }
            if ended {
                break;
            }
        }
    }
    anyhow::ensure!(lines.next().is_none(), "unexpected trailing portable table");
    Ok(())
}
