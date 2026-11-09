//! Responsibilities: compare indexed fuzzy recall with the existing complete text predicate.
//! Not responsible for: defining a new normalizer or edit-distance policy.
//! Failure modes: short edits destroy every shared trigram; a candidate prefilter drops a
//! valid document before validation; a word that cannot be misspelled (short, CJK, quoted)
//! falls out of the check, so a page lacking it passes, or a page matching it exactly is
//! lost beside a misspelled word; a run's edits delete a whole word; a misspelled run only
//! matches in the order it was typed.
use super::{
    SearchRequest, SearchSession, documents,
    fixture::{Archive, CHROME},
    query,
};
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
        let docs = archive
            .vault
            .store()
            .read(Db::Archive, |s| {
                s.query(
                    &format!("SELECT {} FROM search_documents d", documents::COLUMNS),
                    (),
                    documents::document,
                )
            })
            .unwrap();
        // Alone, beside a word that cannot be misspelled, in another order, and across an OR.
        for query in [
            "abcd",
            "abxd",
            "abcdefg",
            "abcdefghij",
            "abcdefghijklmnopq",
            "abxd zz",
            "unrelated abxd",
            "abcdefg abxd",
            "zz OR abxd",
            "abxdexg abcd OR nothing",
        ] {
            let predicate = query::build(query, true).fuzzy.unwrap();
            let mut expected = docs
                .iter()
                .filter(|document| predicate.matches(document))
                .map(|document| document.page_id as i64)
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

/// Pages whose URLs share no query word, so only their titles can answer.
fn typo_archive() -> Archive {
    let archive = Archive::with_key(None);
    for (offset, title) in [
        "JavaScript Vue",
        "JavaScript 中文教程",
        "TypeScript",
        "Rust",
        "Rust Go",
        "Rust Tokio",
        "Rust Lamg",
        "Python Tutorial",
        "Facebook",
        "E-commerce Guide",
        "Déjà vu",
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
    archive
}

/// Runs an ordinary search, so the typo retry happens the way it does in the app.
fn titles(archive: &Archive, query: &str) -> Vec<String> {
    let mut session = SearchSession::start(
        &archive.vault,
        SearchRequest { query: query.into(), page_size: 20, ..Default::default() },
    )
    .unwrap();
    let mut titles = session
        .finish_page()
        .unwrap()
        .items
        .into_iter()
        .map(|hit| hit.visit.title.unwrap_or_default())
        .collect::<Vec<_>>();
    titles.sort_unstable();
    titles
}

#[test]
fn a_word_spelled_right_answers_beside_a_misspelled_one() {
    let archive = typo_archive();
    assert_eq!(titles(&archive, "javascrippt vue"), ["JavaScript Vue"]);
    // An alternative must not cost a result: `vue` matches as written, `react` does not.
    assert_eq!(titles(&archive, "javascrippt react OR vue"), ["JavaScript Vue"]);
}

#[test]
fn an_or_of_short_words_still_has_to_match() {
    let archive = typo_archive();
    assert_eq!(titles(&archive, "ruste go OR js"), ["Rust Go"]);
}

#[test]
fn edits_cannot_delete_a_word_the_query_asked_for() {
    let archive = typo_archive();
    // Read as one word these are within their edits of the title, with `go` or `api` gone.
    assert!(titles(&archive, "javascript go").is_empty());
    assert!(titles(&archive, "javascrippt go").is_empty());
    assert!(titles(&archive, "typescript api").is_empty());
}

#[test]
fn a_misspelled_query_matches_in_any_word_order() {
    let archive = typo_archive();
    assert_eq!(titles(&archive, "tutorial pythn"), ["Python Tutorial"]);
}

#[test]
fn words_read_as_one_word_still_share_their_edits() {
    let archive = typo_archive();
    // `tokoi` alone is two edits from `tokio`, past what five letters allow; `bok` is too
    // short for typos. Together they are within what the whole run allows.
    assert_eq!(titles(&archive, "rust tokoi"), ["Rust Tokio"]);
    assert_eq!(titles(&archive, "face bok"), ["Facebook"]);
}

#[test]
fn a_page_with_accents_is_found_though_the_index_keeps_them() {
    let archive = typo_archive();
    // The index holds `déjàvu`, which shares no trigram with `dejavo`; the check folds it.
    assert_eq!(titles(&archive, "deja vo"), ["Déjà vu"]);
}

#[test]
fn a_hyphenated_word_is_one_word_to_misspell() {
    let archive = typo_archive();
    assert_eq!(titles(&archive, "e-comerce"), ["E-commerce Guide"]);
}

#[test]
fn a_misspelled_word_answers_beside_cjk() {
    let archive = typo_archive();
    assert_eq!(titles(&archive, "javascrippt 中文"), ["JavaScript 中文教程"]);
}

#[test]
fn a_quoted_phrase_is_never_matched_by_typo_distance() {
    let archive = typo_archive();
    assert_eq!(titles(&archive, "\"rust lang\" OR pythn"), ["Python Tutorial"]);
}

#[test]
fn operators_still_bind_a_misspelled_query() {
    let archive = typo_archive();
    assert_eq!(titles(&archive, "ruste intitle:tokio"), ["Rust Tokio"]);
    assert_eq!(titles(&archive, "ruste -tokio -lamg -go"), ["Rust"]);
}

#[test]
fn punctuation_between_words_is_not_a_word_to_match() {
    let archive = typo_archive();
    assert_eq!(titles(&archive, "tutorial & pythn"), ["Python Tutorial"]);
}
