//! Responsibilities: deterministic encrypted fixtures and independent result helpers.
//! Not responsible for: replacing the real index or mocking the directory.
//! Failure modes: a fixture hides field drift, duplicate pages, or plaintext writes;
//! the tests inspect raw schema, independent queries, and on-disk bytes.
#![allow(dead_code)]
use std::path::Path;
use vault_crypto::{DataKey, EncryptedDirectory};
use vault_search::*;

pub fn key() -> DataKey {
    DataKey::from_bytes([0x52; 32].into())
}
pub fn config() -> Config {
    Config {
        writer_threads: 1,
        writer_memory_bytes: 20_000_000,
        cache_bytes: 2 * 1024 * 1024,
        ..Config::default()
    }
}
pub fn page(id: u64, title: &str) -> PageDocument {
    let mut page = PageDocument::new(id);
    page.text[TextField::Title as usize] = title.into();
    page.compact_text = title.into();
    page.newest_visit_ms = (id % 7) as i64;
    page.domain = if id % 3 == 0 { "other.test" } else { "docs.test" }.into();
    page.profile_ids = if id % 2 == 0 { vec![1, 9] } else { vec![9] };
    page
}
pub fn create(path: &Path, docs: Vec<PageDocument>) -> SearchIndex {
    let index = SearchIndex::create(path, Some(&key()), config()).unwrap();
    index.apply(Batch { upserts: docs, deletes: vec![], last_seq: 1 }).unwrap();
    index
}
pub fn session(index: &SearchIndex, ast: Ast) -> SearchSession {
    index.search(ast, Filters::default()).unwrap()
}
pub fn term(token: &str) -> Ast {
    Ast::Term { token: token.into(), field: None }
}
pub fn ids(session: &SearchSession) -> Vec<u64> {
    let mut ids: Vec<_> =
        session.page(10_000, None).unwrap().hits.into_iter().map(|h| h.page_id).collect();
    ids.sort_unstable();
    ids
}
pub fn raw(path: &Path) -> tantivy::Index {
    tantivy::Index::open(EncryptedDirectory::open(path, &key(), 1024 * 1024).unwrap()).unwrap()
}
pub fn assert_no_plaintext(path: &Path) {
    for entry in std::fs::read_dir(path).unwrap() {
        let data = std::fs::read(entry.unwrap().path()).unwrap();
        for needle in [b"sentinelprivate".as_slice(), b"normalized_title", b"last_seq"] {
            assert!(!data.windows(needle.len()).any(|s| s == needle));
        }
    }
}
