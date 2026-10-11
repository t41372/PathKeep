//! Responsibilities: keep each page's stored site (`pages.registrable_domain`) split by the public
//! suffix list this build carries. The archive records the list's version; an archive whose pages
//! were split by another list (a release brought a newer one) is split again, once.
//! Not responsible for: the split itself (`visit_taxonomy::registrable_domain_for_host`), or
//! Insights, which build anew when the list changes (`visit_taxonomy::url_rules_version`).
//! Failure modes: pages left under the old split (a whole-site star misses them, a page's details
//! name the old site); the walk repeated on every open (the version is recorded when it ends, and
//! a walk stopped half way starts over, rewriting only what still differs); one long write that
//! holds the archive (a batch per transaction).
//! Tests: `sites_tests.rs`.

use crate::Vault;
use crate::visit_taxonomy::{public_suffix_list_version, registrable_domain_for_url};
use anyhow::Result;
use vault_store::{Db, Priority, params};

const LIST_VERSION: &str = "publicSuffixList";
const PAGES_PER_BATCH: i64 = 5_000;

/// Records that the archive's pages are split by this build's list: a new archive, or one just
/// converted, whose every page was written with it.
pub fn record_list_version(vault: &Vault) -> Result<()> {
    Ok(vault.store().write(Db::Archive, Priority::Batch, |s| {
        s.execute(
            "INSERT INTO archive_meta(name,value) VALUES(?1,?2)
             ON CONFLICT(name) DO UPDATE SET value=excluded.value",
            params![LIST_VERSION, public_suffix_list_version()],
        )?;
        Ok(())
    })?)
}

/// Splits again the pages written under another list's sites; returns how many moved. Nothing
/// to do, and nothing read, when the archive's pages are already split by this build's list.
pub fn resplit_if_list_changed(vault: &Vault) -> Result<usize> {
    let recorded = vault.store().read(Db::Archive, |s| {
        s.query("SELECT value FROM archive_meta WHERE name=?1", [LIST_VERSION], |r| {
            Ok(r.get::<String>(0)?)
        })
    })?;
    if recorded.first().map(String::as_str) == Some(public_suffix_list_version()) {
        return Ok(0);
    }
    let (mut after, mut moved) = (0_i64, 0_usize);
    loop {
        let (last, batch_moved) = vault.store().write(Db::Archive, Priority::Batch, move |s| {
            let pages = s.query(
                "SELECT id,url,registrable_domain FROM pages WHERE id>?1 ORDER BY id LIMIT ?2",
                params![after, PAGES_PER_BATCH],
                |r| Ok((r.get::<i64>(0)?, r.get::<String>(1)?, r.get::<Option<String>>(2)?)),
            )?;
            let mut moved = 0;
            for (id, url, stored) in &pages {
                let site = registrable_domain_for_url(url);
                if site != *stored {
                    s.execute(
                        "UPDATE pages SET registrable_domain=?2 WHERE id=?1",
                        params![*id, site],
                    )?;
                    moved += 1;
                }
            }
            Ok((pages.last().map(|(id, ..)| *id), moved))
        })?;
        moved += batch_moved;
        match last {
            Some(id) => after = id,
            None => break,
        }
    }
    record_list_version(vault)?;
    Ok(moved)
}

#[cfg(test)]
#[path = "sites_tests.rs"]
mod tests;
