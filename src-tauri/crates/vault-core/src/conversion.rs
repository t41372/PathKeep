//! Responsibilities: give offline conversion the same identity and search rules as ingest.
//! Not responsible for: decoding SQLCipher, progress persistence or installing files.
//! Failure modes: legacy normalization differs from live writes; documents/outbox are
//! missing; cached counts omit reverted rows. Tests: vault-convert-sqlcipher acceptance.
use crate::Vault;
use vault_store::{Db, Priority, Session};
mod enrichment;
pub use enrichment::{
    LegacyEnrichment, embedding_summaries, import_enrichments, project as project_enrichments,
    refresh_embedding_totals, verify_enrichments, visit_urls,
};

/// Converters must derive keys from source URLs, never trust stored derived strings.
pub fn canonical_url_key(url: &str) -> String {
    crate::pages::canonical_key(url)
}

/// Rebuild the enrichment queue identity from its typed payload, not its saved key.
/// Same rule as `intelligence_runtime::enqueue`: one job per plugin and page.
pub fn enrichment_job_key(plugin: &str, payload: &str) -> anyhow::Result<String> {
    let payload: crate::intelligence_runtime::EnrichmentJobPayload = serde_json::from_str(payload)?;
    Ok(format!("{plugin}:{}", canonical_url_key(&payload.url)))
}

/// Preserve ingest's exact-URL identities without compiling an upsert for every URL.
pub fn pages_for_urls(
    s: &Session<'_>,
    urls: Vec<String>,
) -> vault_store::Result<std::collections::HashMap<String, i64>> {
    use vault_store::Value;
    let values = urls
        .iter()
        .map(|url| {
            vec![
                Value::Text(url.clone()),
                crate::visit_taxonomy::registrable_domain_for_url(url)
                    .map(Value::Text)
                    .unwrap_or(Value::Null),
                Value::Text(canonical_url_key(url)),
            ]
        })
        .collect();
    s.execute_rows("INSERT INTO pages(url,registrable_domain,canonical_url) VALUES(?1,?2,?3) ON CONFLICT(url) DO UPDATE SET url=excluded.url",values)?;
    let mut ids = std::collections::HashMap::new();
    for urls in urls.chunks(512) {
        let binds = (1..=urls.len()).map(|i| format!("?{i}")).collect::<Vec<_>>().join(",");
        s.for_each(
            &format!("SELECT url,id FROM pages WHERE url IN ({binds})"),
            urls.to_vec(),
            |r| {
                ids.insert(r.get::<String>(0)?, r.get::<i64>(1)?);
                Ok(())
            },
        )?;
    }
    Ok(ids)
}

/// Build one bounded page of documents and outbox events with area 1's writer.
pub fn project_pages(vault: &Vault, after: i64, limit: i64) -> anyhow::Result<Option<i64>> {
    Ok(vault.store().write(Db::Archive, Priority::Batch, move |s| {
        let pages = s.query(
            "SELECT id FROM pages WHERE id>?1 ORDER BY id LIMIT ?2",
            [after, limit],
            |r| Ok(r.get::<i64>(0)?),
        )?;
        for &id in &pages {
            crate::archive::search_documents::refresh(s, id)?;
        }
        Ok(pages.last().copied())
    })?)
}

/// Gives each source-evidence row of an old-format archive its `content_key` and drops the copies
/// the old format stored once per import of the same records, keeping the first
/// (`archive::source_evidence::FAMILIES`). A row without a key is one this has not reached yet, and
/// each page commits on its own, so a conversion stopped partway carries on where it was. Run
/// after the evidence tables are copied and verified: it changes what they hold.
pub fn dedupe_source_evidence(
    vault: &Vault,
    cancel: &vault_store::CancelToken,
) -> anyhow::Result<()> {
    use crate::archive::source_evidence::{FAMILIES, content_key};
    for family in &FAMILIES {
        loop {
            cancel.check()?;
            let reached = vault.store().write(Db::SourceEvidence, Priority::Batch, |s| {
                let rows = s.query(
                    &format!(
                        "SELECT id, source_profile_id, {} FROM {} WHERE content_key IS NULL
                         ORDER BY id LIMIT 2000",
                        family.content.join(","),
                        family.table
                    ),
                    (),
                    |r| {
                        let values = (0..family.content.len())
                            .map(|index| r.get_value(index + 2))
                            .collect::<Result<Vec<_>, _>>()?;
                        Ok((r.get::<i64>(0)?, r.get::<i64>(1)?, values))
                    },
                )?;
                for (id, profile, values) in &rows {
                    let key = content_key(family.table, *profile, values);
                    let held = s.query_one(
                        &format!(
                            "SELECT EXISTS(SELECT 1 FROM {} WHERE content_key=?1)",
                            family.table
                        ),
                        vault_store::params![key.clone()],
                        |r| Ok(r.get::<i64>(0)? != 0),
                    )?;
                    if held {
                        s.execute(
                            &format!("DELETE FROM {} WHERE id=?1", family.table),
                            vault_store::params![*id],
                        )?;
                    } else {
                        s.execute(
                            &format!("UPDATE {} SET content_key=?2 WHERE id=?1", family.table),
                            vault_store::params![*id, key],
                        )?;
                    }
                }
                Ok(rows.len())
            })?;
            if reached == 0 {
                break;
            }
        }
    }
    Ok(())
}

/// Converted facts supply the same cached totals that normal batches maintain.
/// Links the same visit recorded by two sources (`archive::same_visit`) in an archive written
/// before linking existed: each Takeout source's counted visits, best source first, are hidden
/// behind another source's copy or take its place. Run before `refresh_totals` and
/// `project_pages`, which count only what is left counted.
pub fn link_same_visits(vault: &Vault) -> anyhow::Result<()> {
    use crate::archive::same_visit;
    let mut takeouts = vault.store().read(Db::Archive, |s| {
        // v0.2 archives can hold a profile without a key; it is not a Takeout source.
        s.query("SELECT id, COALESCE(profile_key, '') FROM source_profiles", (), |r| {
            Ok((r.get::<i64>(0)?, r.get::<String>(1)?))
        })
    })?;
    takeouts.retain(|(_, key)| same_visit::rank(key) > 0);
    if takeouts.is_empty() {
        return Ok(());
    }
    takeouts.sort_by_key(|(id, key)| (same_visit::rank(key), *id));
    let run = vault.store().write(Db::Archive, Priority::Batch, |s| {
        crate::archive::write_runs::start_in(s, "same-visit", "conversion", &[])
            .map_err(|e| vault_store::Error::Operation(e.into_boxed_dyn_error()))
    })?;
    for (profile, _) in takeouts {
        let mut after = 0;
        loop {
            let last = vault.store().write(Db::Archive, Priority::Batch, move |s| {
                let rows = s.query(
                    "SELECT v.id, u.url, v.visit_time_ms FROM visits v JOIN urls u ON u.id=v.url_id
                     WHERE v.source_profile_id=?1 AND v.reverted_at IS NULL AND v.id>?2
                     ORDER BY v.id LIMIT 2000",
                    vault_store::params![profile, after],
                    |r| Ok((r.get::<i64>(0)?, r.get::<String>(1)?, r.get::<i64>(2)?)),
                )?;
                same_visit::link_existing(s, run, profile, &rows)?;
                Ok(rows.last().map(|(id, _, _)| *id))
            })?;
            match last {
                Some(last) => after = last,
                None => break,
            }
        }
    }
    vault.store().write(Db::Archive, Priority::Batch, move |s| {
        s.execute(
            "UPDATE runs SET status='success', finished_at=?2 WHERE id=?1",
            vault_store::params![run, crate::utils::now_rfc3339()],
        )?;
        Ok(())
    })?;
    Ok(())
}

/// Treats the copied visits and runs as version 1's: the new archive's migrations ran while it
/// was empty (`store_schema::backup_identity`). The visits get their openers by archive id, and
/// the runs mark which source evidence version 1 captured. Bounded transactions; idempotent.
pub fn adopt_version_one_facts(vault: &Vault) -> anyhow::Result<()> {
    vault.store().write(vault_store::Db::Archive, vault_store::Priority::Batch, |s| {
        crate::store_schema::backup_identity::adopt_copied_runs(s)?;
        crate::store_schema::backup_identity::restart_legacy_openers(s)
    })?;
    while vault
        .store()
        .write(vault_store::Db::Archive, vault_store::Priority::Batch, |s| {
            crate::store_schema::backup_identity::link_legacy_openers(s)
        })?
        .is_some_and(|progress| !progress.complete)
    {}
    Ok(())
}

pub fn refresh_totals(vault: &Vault) -> anyhow::Result<()> {
    Ok(vault.store().write(Db::Archive, Priority::Batch, |s| {
        for (name, table, predicate) in [
            ("sourceProfiles", "source_profiles", "enabled=1"),
            ("urls", "urls", "1"),
            ("visits", "visits", "reverted_at IS NULL"),
            ("downloads", "downloads", "reverted_at IS NULL"),
            ("manifests", "manifests", "1"),
            ("snapshots", "snapshots", "1"),
        ] {
            s.execute(
                &format!("UPDATE archive_totals SET value=(SELECT count(*) FROM {table} WHERE {predicate}) WHERE name=?1"),
                [name],
            )?;
        }
        s.execute(
            "INSERT INTO archive_totals(name,value) SELECT 'sourceVisits:'||p.id,(SELECT count(*) FROM visits v WHERE v.source_profile_id=p.id AND v.reverted_at IS NULL) FROM source_profiles p WHERE 1 ON CONFLICT(name) DO UPDATE SET value=excluded.value",
            (),
        )?;
        s.execute(
            "UPDATE import_batches SET visible_items=(SELECT count(*) FROM visits WHERE import_batch_id=import_batches.id AND reverted_at IS NULL)",
            (),
        )?;
        Ok(())
    })?)
}

use serde::{Deserialize, Serialize};
use std::path::PathBuf;
/// Counts are source rows; sizes describe the additional staged copy and index.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct Assessment {
    pub required: bool,
    pub source_encrypted: bool,
    pub source_format: String,
    pub target_format: String,
    pub source_bytes: u64,
    pub required_free_bytes: u64,
    pub files_touched: Vec<PathBuf>,
    pub files_retained: Vec<PathBuf>,
    /// Old files the given password doesn't open although it opens the archive. An install
    /// that changed its password in v0.1–v0.3 can have these: the old app re-encrypted only
    /// the archive. Conversion leaves them where they are and doesn't carry them over, so
    /// the upgrade screen asks before it starts. Empty until a password is given.
    pub files_locked: Vec<LockedFile>,
    pub tables: Vec<TableCount>,
    pub resumable: bool,
}
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct LockedFile {
    /// `SourceEvidence`, `Intelligence` or `Agent`.
    pub database: String,
    pub path: PathBuf,
}
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct TableCount {
    pub database: String,
    pub table: String,
    pub rows: u64,
}
