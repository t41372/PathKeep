//! Responsibilities: transform bounded source pages and verify every copied value.
//! Not responsible for: source opening, progress journals or installation.
//! Failure modes: legacy derived keys reused, rows lost/repeated, omitted nullable
//! values, digest mismatch accepted. Tests: tests/conversion.rs.
mod stars;
use crate::source::{self, Row, quote};
use anyhow::Result;
use sha2::{Digest, Sha256};
use vault_core::{Vault, conversion::canonical_url_key};
use vault_store::{Db, Priority, Value};

pub(crate) fn fields(
    vault: &Vault,
    db: Db,
    source: &rusqlite::Connection,
    table: &str,
) -> Result<Vec<String>> {
    let name = table.to_owned();
    let target = vault.store().read(db, move |s| s.table_columns(&name))?;
    Ok(source::columns(source, table)?
        .into_iter()
        .filter(|c| {
            target.contains(c)
                && !(matches!(table, "ai_embeddings" | "ai_embedding_totals") && c == "lineage")
        })
        .collect())
}

fn text<'a>(fields: &[String], values: &'a [Value], name: &str) -> Option<&'a str> {
    match fields.iter().position(|f| f == name).map(|i| &values[i]) {
        Some(Value::Text(s)) => Some(s),
        _ => None,
    }
}

pub(crate) fn normalize(table: &str, fields: &[String], row: &mut Row) -> vault_store::Result<()> {
    if table == "star" {
        let kind = text(fields, &row.values, "entity_kind").unwrap();
        let raw = text(fields, &row.values, "entity_key").unwrap();
        let key = if kind == "url" {
            canonical_url_key(raw)
        } else {
            vault_core::visit_taxonomy::registrable_domain_for_host(raw)
        };
        let index = fields.iter().position(|f| f == "entity_key").unwrap();
        row.values[index] = Value::Text(key);
    }
    if let Some(index) = fields.iter().position(|f| f == "canonical_url") {
        let raw = text(fields, &row.values, "url")
            .or_else(|| text(fields, &row.values, "canonical_url"))
            .unwrap();
        row.values[index] = Value::Text(canonical_url_key(raw));
    }
    // Only Chromium ever filled the visit duration, and the old app kept Chromium's microseconds
    // in these millisecond columns. The conversion runs once, so it divides exactly once.
    let duration = match table {
        "visits" => fields.iter().position(|f| f == "visit_duration_ms"),
        "visit_engagement_evidence"
            if text(fields, &row.values, "metric_key") == Some("engagement.visit_duration_ms") =>
        {
            fields.iter().position(|f| f == "metric_value_int")
        }
        _ => None,
    };
    if let Some(index) = duration
        && let Value::Integer(micros) = row.values[index]
    {
        row.values[index] = Value::Integer(micros.div_euclid(1_000));
    }
    if table == "intelligence_jobs"
        && text(fields, &row.values, "job_type") == Some("enrichment-plugin")
    {
        let key = vault_core::conversion::enrichment_job_key(
            text(fields, &row.values, "plugin_id").unwrap(),
            text(fields, &row.values, "payload_json").unwrap(),
        )
        .map_err(|e| vault_store::Error::Operation(e.into_boxed_dyn_error()))?;
        let index = fields.iter().position(|f| f == "dedupe_key").unwrap();
        row.values[index] = Value::Text(key);
    }
    Ok(())
}

pub(crate) fn write(
    vault: &Vault,
    db: Db,
    table: &str,
    fields: &[String],
    mut rows: Vec<Row>,
) -> Result<()> {
    normalize_page(vault, table, fields, &mut rows)?;
    let table = table.to_owned();
    let fields = fields.to_vec();
    vault.store().write(db, Priority::Batch, move |s| {
        let mut names = fields.clone();
        let annotation = (table == "url_annotations" || table == "url_tags")
            && !names.iter().any(|f| f == "canonical_url");
        if annotation {
            names.push("canonical_url".into());
        }
        if table == "urls" {
            names.push("page_id".into());
        }
        let legacy_embeddings = table == "ai_embeddings" || table == "ai_embedding_totals";
        if legacy_embeddings {
            names.push("lineage".into());
        }
        let sql = format!(
            "INSERT OR REPLACE INTO {}(rowid,{}) VALUES(?1,{})",
            quote(&table),
            names.iter().map(|s| quote(s)).collect::<Vec<_>>().join(","),
            (2..=names.len() + 1).map(|i| format!("?{i}")).collect::<Vec<_>>().join(",")
        );
        let pages = if table == "urls" {
            let urls = rows
                .iter()
                .map(|row| text(&fields, &row.values, "url").unwrap().to_owned())
                .collect();
            vault_core::conversion::pages_for_urls(s, urls)?
        } else {
            Default::default()
        };
        let mut inserts = Vec::with_capacity(rows.len());
        for mut row in rows {
            if table == "star" {
                stars::upsert(s, "star", &fields, &row)?;
                continue;
            }
            if annotation {
                row.values.push(Value::Text(canonical_url_key(
                    text(&fields, &row.values, "url").unwrap(),
                )));
            }
            if table == "urls" {
                let page = pages[text(&fields, &row.values, "url").unwrap()];
                row.values.push(Value::Integer(page));
            }
            if legacy_embeddings {
                row.values.push(Value::Text("legacy".into()));
            }
            let mut values = vec![Value::Integer(row.id)];
            values.extend(row.values);
            inserts.push(values);
        }
        if !inserts.is_empty() {
            s.execute_rows(&sql, inserts)?;
        }
        Ok(())
    })?;
    Ok(())
}

pub(crate) fn verify(
    vault: &Vault,
    db: Db,
    source: &rusqlite::Connection,
    table: &str,
    fields: &[String],
    cancel: &vault_store::CancelToken,
    progress: &mut impl FnMut(u64),
) -> Result<()> {
    if table == "star" {
        return stars::verify(vault, source, fields, cancel);
    }
    let mut expected = Sha256::new();
    let mut cursor = None;
    let mut count = 0u64;
    loop {
        cancel.check()?;
        let mut rows = source::page(source, table, fields, cursor)?;
        if rows.is_empty() {
            break;
        }
        cursor = rows.last().map(|r| r.id);
        normalize_page(vault, table, fields, &mut rows)?;
        for row in rows {
            hash_row(&mut expected, &row);
            count += 1;
        }
        progress(count);
    }
    let mut actual = Sha256::new();
    let mut cursor = None;
    let mut actual_count = 0;
    loop {
        cancel.check()?;
        let rows = native_page(vault, db, table, fields, cursor)?;
        if rows.is_empty() {
            break;
        }
        cursor = rows.last().map(|r| r.id);
        for row in rows {
            hash_row(&mut actual, &row);
            actual_count += 1;
        }
        progress(count + actual_count);
    }
    anyhow::ensure!(
        actual_count == count && actual.finalize() == expected.finalize(),
        "conversion-digest-mismatch: {db:?}.{table}"
    );
    Ok(())
}

/// Both direct copies and merged-table proofs read the same bounded typed rows.
pub(crate) fn native_page(
    vault: &Vault,
    db: Db,
    table: &str,
    fields: &[String],
    cursor: Option<i64>,
) -> Result<Vec<Row>> {
    let names = fields.to_vec();
    let table = table.to_owned();
    Ok(vault.store().read(db, move |s| {
        let filter = cursor.map(|id| format!("WHERE rowid>{id}")).unwrap_or_default();
        s.query(
            &format!(
                "SELECT rowid,{} FROM {} {filter} ORDER BY rowid LIMIT {}",
                names.iter().map(|s| quote(s)).collect::<Vec<_>>().join(","),
                quote(&table),
                source::PAGE
            ),
            (),
            |r| {
                Ok(Row {
                    id: r.get(0)?,
                    values: (0..names.len())
                        .map(|i| r.get_value(i + 1).map_err(Into::into))
                        .collect::<vault_store::Result<_>>()?,
                })
            },
        )
    })?)
}

fn normalize_page(vault: &Vault, table: &str, fields: &[String], rows: &mut [Row]) -> Result<()> {
    for row in rows.iter_mut() {
        normalize(table, fields, row)?;
    }
    if table == "visit_derived_facts" {
        let visit = fields.iter().position(|f| f == "visit_id").unwrap();
        let key = fields.iter().position(|f| f == "canonical_url").unwrap();
        let ids = rows
            .iter()
            .map(|row| match row.values[visit] {
                Value::Integer(id) => id,
                _ => unreachable!(),
            })
            .collect();
        let urls = vault_core::conversion::visit_urls(vault, ids)?;
        for row in rows {
            let Value::Integer(id) = row.values[visit] else { unreachable!() };
            if let Some(url) = urls.get(&id) {
                row.values[key] = Value::Text(canonical_url_key(url));
            }
        }
    } else if table == "ai_embeddings" {
        let history = fields.iter().position(|f| f == "history_id").unwrap();
        let hash = fields.iter().position(|f| f == "content_hash").unwrap();
        let ids = rows
            .iter()
            .map(|row| match row.values[history] {
                Value::Integer(id) => id,
                _ => unreachable!(),
            })
            .collect::<Vec<_>>();
        let summaries = vault_core::conversion::embedding_summaries(vault, ids)?;
        for row in rows {
            let Value::Integer(id) = row.values[history] else { unreachable!() };
            let value = vault_core::ai::build_dedup_content_hash(
                text(fields, &row.values, "url").unwrap(),
                text(fields, &row.values, "title"),
                summaries.get(&id).map(String::as_str),
            );
            row.values[hash] = Value::Text(value);
        }
    }
    Ok(())
}

/// Compare logical values independently of either engine's page layout or cipher.
pub(crate) fn hash_row(hash: &mut Sha256, row: &Row) {
    hash.update(row.id.to_le_bytes());
    for value in &row.values {
        match value {
            Value::Null => hash.update([0]),
            Value::Integer(v) => {
                hash.update([1]);
                hash.update(v.to_le_bytes());
            }
            Value::Real(v) => {
                hash.update([2]);
                hash.update(v.to_bits().to_le_bytes());
            }
            Value::Text(v) => {
                hash.update([3]);
                hash.update((v.len() as u64).to_le_bytes());
                hash.update(v.as_bytes());
            }
            Value::Blob(v) => {
                hash.update([4]);
                hash.update((v.len() as u64).to_le_bytes());
                hash.update(v);
            }
        }
    }
}

/// Deleted IDs must remain retired after conversion, just as after a native copy.
pub(crate) fn sequences(
    vault: &Vault,
    db: Db,
    source: &rusqlite::Connection,
    tables: &[String],
) -> Result<()> {
    let exists = source.query_row(
        "SELECT count(*) FROM sqlite_schema WHERE name='sqlite_sequence'",
        [],
        |r| r.get::<_, i64>(0),
    )? != 0;
    if !exists {
        return Ok(());
    }
    let values = source
        .prepare("SELECT name,seq FROM sqlite_sequence")?
        .query_map([], |r| Ok((r.get::<_, String>(0)?, r.get::<_, i64>(1)?)))?
        .collect::<rusqlite::Result<Vec<_>>>()?;
    let values = values.into_iter().filter(|(name, _)| tables.contains(name)).collect::<Vec<_>>();
    vault.store().write(db, Priority::Batch, move |s| {
        for (name, seq) in values {
            let changed = s.execute(
                "UPDATE sqlite_sequence SET seq=MAX(seq,?2) WHERE name=?1",
                vault_store::params![name.as_str(), seq],
            )?;
            if changed == 0 {
                s.execute(
                    "INSERT INTO sqlite_sequence(name,seq) VALUES(?1,?2)",
                    vault_store::params![name, seq],
                )?;
            }
        }
        Ok(())
    })?;
    Ok(())
}
