//! Responsibilities: compare indexed fuzzy recall with the existing complete text predicate.
//! Not responsible for: defining a new normalizer or edit-distance policy.
//! Failure modes: short edits destroy every shared trigram; a candidate prefilter drops a
//! valid document before validation.
use super::{
    SearchRequest, SearchSession,
    fixture::{Archive, CHROME},
};
use crate::archive::search_lexical::{FuzzyDocument, fuzzy_query, normalized_tokens};
use vault_store::Db;
#[test]
fn fuzzy_prefilters_equal_brute_force_even_without_a_shared_trigram() {
    for key in [None, Some([114; 32])] {
        let archive = Archive::with_key(key);
        for (offset, title) in [
            "abcd",
            "abxd",
            "axcd",
            "abcdefg",
            "abxdexg",
            "abcdefghij",
            "abxdezghyj",
            "abcdefghijklmnopq",
            "abxdezg hyjklmnopq",
            "unrelated",
        ]
        .into_iter()
        .enumerate()
        {
            archive.url(
                offset as i64 + 1,
                CHROME,
                &format!("https://zz.test/{offset}"),
                title,
                &[offset as i64 + 1],
            );
        }
        archive.index();
        let docs=archive.vault.store().read(Db::Archive,|s|s.query("SELECT page_id,normalized_url,normalized_title,normalized_search_terms,compact_text FROM search_documents",(),|r|Ok((r.get::<i64>(0)?,r.get::<String>(1)?,r.get::<String>(2)?,r.get::<String>(3)?,r.get::<String>(4)?)))).unwrap();
        for query in ["abcd", "abxd", "abcdefg", "abcdefghij", "abcdefghijklmnopq"] {
            let predicate = fuzzy_query(query).unwrap();
            let mut expected = docs
                .iter()
                .filter_map(|(id, url, title, terms, compact)| {
                    predicate
                        .score_document(&FuzzyDocument {
                            normalized_url: url,
                            normalized_title: title,
                            normalized_search_terms: terms,
                            compact_text: &normalized_tokens(compact),
                        })
                        .map(|_| *id)
                })
                .collect::<Vec<_>>();
            let mut session = SearchSession::start(
                &archive.vault,
                SearchRequest {
                    query: query.into(),
                    fuzzy: true,
                    page_size: 2,
                    ..Default::default()
                },
            )
            .unwrap();
            let mut actual = Vec::new();
            loop {
                let page = session.finish_page().unwrap();
                assert_eq!(page.count.value, expected.len() as u64, "{query}");
                actual.extend(page.items.into_iter().map(|hit| hit.page_id));
                let Some(cursor) = page.cursor else { break };
                session.next_page(&cursor).unwrap();
            }
            actual.sort_unstable();
            expected.sort_unstable();
            assert_eq!(actual, expected, "{query}");
        }
    }
}
