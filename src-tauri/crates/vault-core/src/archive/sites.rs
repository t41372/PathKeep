//! Responsibilities: keep each page's stored site (`pages.registrable_domain`) split by the public
//! suffix list this build carries. The archive records the list's version; an archive whose pages
//! were split by another list (a release brought a newer one) is split again, once. Whole-site
//! stars keep covering the pages they covered when a site is split apart.
//! Not responsible for: the split itself (`visit_taxonomy::registrable_domain_for_host`), or
//! Insights, which build anew when the list changes (`visit_taxonomy::url_rules_version`).
//! Failure modes: pages left under the old split (a whole-site star misses them, a page's details
//! name the old site); the walk repeated on every open (the version is recorded when it ends, and
//! a walk stopped half way starts over, rewriting only what still differs); one long write that
//! holds the archive (a batch per transaction); a star on a site the list split apart left
//! matching nothing, or carried to a site that only ends in the same letters; stars carried
//! while a conversion still checks the copied ones against the old file (they are carried when
//! the archive next opens, recorded apart from the list version).
//! Tests: `sites_tests.rs`.

use crate::Vault;
use crate::visit_taxonomy::{public_suffix_list_version, registrable_domain_for_url};
use anyhow::Result;
use std::collections::{HashMap, HashSet};
use vault_store::{Db, Priority, Session, params};

const LIST_VERSION: &str = "publicSuffixList";
/// The list whose split whole-site stars were last carried to (`carry_stars`).
const STARS_CARRIED: &str = "publicSuffixListStars";
const PAGES_PER_BATCH: i64 = 5_000;

/// Records that the archive's pages are split by this build's list: a new archive, or one just
/// converted, whose every page was written with it. Its stars are carried when it next opens.
pub fn record_list_version(vault: &Vault) -> Result<()> {
    record(vault, LIST_VERSION)
}

fn record(vault: &Vault, name: &'static str) -> Result<()> {
    Ok(vault.store().write(Db::Archive, Priority::Batch, move |s| {
        s.execute(
            "INSERT INTO archive_meta(name,value) VALUES(?1,?2)
             ON CONFLICT(name) DO UPDATE SET value=excluded.value",
            params![name, public_suffix_list_version()],
        )?;
        Ok(())
    })?)
}

fn is_current(vault: &Vault, name: &'static str) -> Result<bool> {
    let recorded = vault.store().read(Db::Archive, move |s| {
        s.query("SELECT value FROM archive_meta WHERE name=?1", [name], |r| Ok(r.get::<String>(0)?))
    })?;
    Ok(recorded.first().map(String::as_str) == Some(public_suffix_list_version()))
}

/// Splits again the pages written under another list's sites, then carries whole-site stars to
/// the sites they were split into; returns how many pages moved. Nothing to do, and next to
/// nothing read, when both already follow this build's list.
pub fn resplit_if_list_changed(vault: &Vault) -> Result<usize> {
    let moved = if is_current(vault, LIST_VERSION)? { 0 } else { split_again(vault)? };
    if !is_current(vault, STARS_CARRIED)? {
        vault.store().write(Db::Archive, Priority::Batch, carry_stars)?;
        record(vault, STARS_CARRIED)?;
    }
    Ok(moved)
}

fn split_again(vault: &Vault) -> Result<usize> {
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

/// Stars on sites the list split apart, carried to the sites they covered; returns how many
/// stars were carried. Earlier releases put every `*.github.io` page on one site, `github.io`, so
/// a star there covered all of them; now each is a site of its own and the star matches none. A
/// star is carried to every site under its own (`rust-lang.github.io`), with its time and
/// profile; a site the user had starred keeps its own star. The old star goes only when no page
/// is left on its site, and a star with nothing under it stays as it is.
fn carry_stars(s: &Session<'_>) -> vault_store::Result<usize> {
    let starred = s
        .query(
            "SELECT entity_key,starred_at,source_profile FROM star WHERE entity_kind='domain'",
            (),
            |r| Ok((r.get::<String>(0)?, (r.get::<String>(1)?, r.get::<Option<String>>(2)?))),
        )?
        .into_iter()
        .collect::<HashMap<_, _>>();
    if starred.is_empty() {
        return Ok(0);
    }
    // One walk over the archive's sites, on its index: which starred sites still have pages,
    // and which sites sit under a starred one.
    let sites = s.query(
        "SELECT DISTINCT registrable_domain FROM pages
         WHERE registrable_domain IS NOT NULL AND registrable_domain<>''",
        (),
        |r| Ok(r.get::<String>(0)?),
    )?;
    let mut kept = HashSet::new();
    let mut under: HashMap<&str, Vec<&str>> = HashMap::new();
    for site in &sites {
        if starred.contains_key(site) {
            kept.insert(site.as_str());
        }
        let mut rest = site.as_str();
        while let Some(dot) = rest.find('.') {
            rest = &rest[dot + 1..];
            if let Some((key, _)) = starred.get_key_value(rest) {
                under.entry(key.as_str()).or_default().push(site);
            }
        }
    }
    for (key, sites) in &under {
        let (at, profile) = &starred[*key];
        for site in sites {
            s.execute(
                "INSERT INTO star(entity_kind,entity_key,starred_at,source_profile)
                 VALUES('domain',?1,?2,?3) ON CONFLICT(entity_kind,entity_key) DO NOTHING",
                params![*site, at.as_str(), profile.clone()],
            )?;
        }
        if !kept.contains(key) {
            s.execute("DELETE FROM star WHERE entity_kind='domain' AND entity_key=?1", [*key])?;
        }
    }
    Ok(under.len())
}

#[cfg(test)]
#[path = "sites_tests.rs"]
mod tests;
