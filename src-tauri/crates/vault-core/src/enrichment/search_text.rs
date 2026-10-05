//! Responsibilities: resolve canonical pages on a reader and publish enrichment in a short write.
//! Not responsible for: fetching, candidate generation or legacy visit identifiers.
//! Failure modes: variants miss their canonical key, a prefix scan blocks interactive
//! work, or text and outbox diverge. Tests: pages::tests and content_fetch tests.
use crate::Vault;
use vault_store::{Db, Priority};
pub(super) fn pages(vault: &Vault, url: &str) -> anyhow::Result<Vec<i64>> {
    let key = crate::pages::canonical_key(url);
    Ok(vault.store().read(Db::Archive, move |s| crate::pages::ids_for_key(s, &key))?)
}
pub(super) fn publish(vault: &Vault, ids: Vec<i64>, text: String) -> anyhow::Result<()> {
    vault.write(Db::Archive, Priority::Interactive, move |s| {
        for id in ids {
            crate::archive::search_documents::set_enrichment(s, id, &text)?;
        }
        Ok(())
    })
}
