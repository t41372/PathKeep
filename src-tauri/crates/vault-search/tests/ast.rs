//! Responsibilities: required AST match sets through the real encrypted directory.
//! Not responsible for: testing the application's language normalization.
//! Failure modes: prefix expansion truncates, literals turn into syntax, fields leak,
//! phrase order/adjacency is lost, exclusions fail alone, grams lose CJK or typo recall.
mod support;
use support::*;
use vault_search::*;

#[test]
fn every_ast_form_has_an_exact_match_set() {
    let tmp = tempfile::tempdir().unwrap();
    let mut other_field = page(7, "unrelated");
    other_field.text[TextField::Notes as usize] = "alpha beta".into();
    let index = create(
        tmp.path(),
        vec![
            page(1, "alpha beta 421"),
            page(2, "beta alpha 42"),
            page(3, "alpha middle beta 429"),
            page(4, "beta 4200"),
            page(5, "literal* a+b"),
            page(6, "alphabet 4219"),
            other_field,
        ],
    );
    let cases = vec![
        (term("alpha"), vec![1, 2, 3, 7]),
        (Ast::Term { token: "alpha".into(), field: Some(TextField::Notes) }, vec![7]),
        (Ast::Prefix { token: "42".into(), field: None }, vec![1, 2, 3, 4, 6]),
        (Ast::Prefix { token: "a+".into(), field: Some(TextField::Title) }, vec![5]),
        (term("literal*"), vec![5]),
        (Ast::Phrase { tokens: vec!["alpha".into(), "beta".into()], field: None }, vec![1, 7]),
        (Ast::Phrase { tokens: vec!["alpha".into()], field: None }, vec![1, 2, 3, 7]),
        (Ast::Phrase { tokens: vec![], field: None }, vec![]),
        (Ast::Not(Box::new(term("alpha"))), vec![4, 5, 6]),
        (
            Ast::And(vec![
                term("alpha"),
                Ast::Not(Box::new(Ast::Prefix { token: "42".into(), field: None })),
            ]),
            vec![7],
        ),
        (Ast::Or(vec![term("unrelated"), term("alphabet")]), vec![6, 7]),
        (Ast::And(vec![]), vec![1, 2, 3, 4, 5, 6, 7]),
        (Ast::Or(vec![]), vec![]),
    ];
    for (ast, expected) in cases {
        assert_eq!(ids(&session(&index, ast.clone())), expected, "{ast:?}");
    }
}

#[test]
fn prefix_reaches_every_expansion_including_single_numeric_terms() {
    let tmp = tempfile::tempdir().unwrap();
    let index = create(tmp.path(), (0..1200).map(|i| page(i, &format!("42{i:04}"))).collect());
    let ast = Ast::Prefix { token: "42".into(), field: Some(TextField::Title) };
    assert_eq!(ids(&session(&index, ast)), (0..1200).collect::<Vec<_>>());
}

#[test]
fn typo_and_unsegmented_chinese_use_unicode_trigrams_without_a_candidate_cap() {
    let tmp = tempfile::tempdir().unwrap();
    let mut docs: Vec<_> = (0..350).map(|i| page(i, "searching")).collect();
    docs.extend([page(351, "資料庫搜尋指南"), page(352, "unrelated")]);
    let index = create(tmp.path(), docs);
    assert_eq!(
        ids(&session(&index, Ast::Trigrams { text: "searxhing".into(), minimum_match: 2 })),
        (0..350).collect::<Vec<_>>()
    );
    assert_eq!(
        ids(&session(&index, Ast::Trigrams { text: "資料庫搜尋".into(), minimum_match: 3 })),
        vec![351]
    );
    assert!(
        ids(&session(&index, Ast::Trigrams { text: "資料".into(), minimum_match: 1 })).is_empty()
    );
    assert!(
        index
            .search(Ast::Trigrams { text: "search".into(), minimum_match: 0 }, Filters::default())
            .is_err()
    );
}

#[test]
fn prefix_uses_bm25_and_internal_token_names_never_collide() {
    let tmp = tempfile::tempdir().unwrap();
    let index = create(
        tmp.path(),
        vec![page(1, "421 421 421"), page(2, "429 other other"), page(3, "^42 =421")],
    );
    let search = session(&index, Ast::Prefix { token: "42".into(), field: Some(TextField::Title) });
    let hits = search.page(10, None).unwrap().hits;
    assert_eq!(hits.iter().map(|h| h.page_id).collect::<Vec<_>>(), vec![1, 2]);
    assert!(hits[0].score > hits[1].score, "prefix term frequency must affect BM25");
    assert_eq!(ids(&session(&index, term("^42"))), vec![3]);
    assert_eq!(ids(&session(&index, term("=421"))), vec![3]);
    let raw = raw(tmp.path());
    let reader = raw.reader().unwrap();
    let field = raw.schema().get_field("title").unwrap();
    let query = tantivy::query::BoostQuery::new(
        Box::new(tantivy::query::TermQuery::new(
            tantivy::Term::from_field_text(field, "^42"),
            tantivy::schema::IndexRecordOption::WithFreqs,
        )),
        12.,
    );
    let scores = reader
        .searcher()
        .search(&query, &tantivy::collector::TopDocs::with_limit(10).order_by_score())
        .unwrap();
    assert_eq!(
        hits.iter().map(|h| h.score).collect::<Vec<_>>(),
        scores.iter().map(|h| h.0).collect::<Vec<_>>()
    );
}

#[test]
fn unicode_prefixes_and_phrases_preserve_token_and_field_boundaries() {
    let tmp = tempfile::tempdir().unwrap();
    let mut split = page(1, "alpha");
    split.text[TextField::Notes as usize] = "beta".into();
    let index = create(
        tmp.path(),
        vec![split, page(2, "資料庫搜尋 🚀launch alpha beta"), page(3, "alpha gap beta")],
    );
    assert_eq!(
        ids(&session(
            &index,
            Ast::Phrase { tokens: vec!["alpha".into(), "beta".into()], field: None }
        )),
        vec![2]
    );
    assert_eq!(ids(&session(&index, Ast::Prefix { token: "資料".into(), field: None })), vec![2]);
    assert_eq!(ids(&session(&index, Ast::Prefix { token: "🚀l".into(), field: None })), vec![2]);
    assert_eq!(
        ids(&session(&index, Ast::Prefix { token: String::new(), field: Some(TextField::Notes) })),
        vec![1]
    );
}

#[test]
fn a_prefix_can_occupy_any_phrase_position_without_expansion_limits() {
    let tmp = tempfile::tempdir().unwrap();
    let index = create(
        tmp.path(),
        vec![
            page(1, "alpha 421 final"),
            page(2, "alphabet 429 finale"),
            page(3, "alpha gap 421 final"),
            page(4, "final alpha 421"),
        ],
    );
    let ast = Ast::Phrase {
        tokens: vec![
            PhraseTerm::Prefix("alph".into()),
            PhraseTerm::Prefix("42".into()),
            "final".into(),
        ],
        field: Some(TextField::Title),
    };
    assert_eq!(ids(&session(&index, ast)), vec![1]);
    let ast = Ast::Phrase {
        tokens: vec![
            PhraseTerm::Prefix("alph".into()),
            PhraseTerm::Prefix("42".into()),
            PhraseTerm::Prefix("fin".into()),
        ],
        field: None,
    };
    assert_eq!(ids(&session(&index, ast)), vec![1, 2]);
}
