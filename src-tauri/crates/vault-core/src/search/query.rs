//! Responsibilities: translate the existing syntax and normalization into a typed AST.
//! Not responsible for: SQL, ranking, candidate windows or query-string escaping.
//! Failure modes: quotes become unordered words; exclusions are truncated; Unicode
//! prefixes disappear; compact recall changes word intersections; OR becomes AND; OR
//! swallows the words around its neighbours (`rust tokio OR tauri` read as
//! "rust tokio, or tauri" instead of "rust, and tokio or tauri"); OR reaches past an
//! operator to a word further away.
//! Tests: search::tests, ported_queries and projection_tests.
use crate::archive::{
    search_lexical::{
        cjk_runs, contains_cjk, normalized_compact_filter_terms, normalized_filter_terms,
        normalized_tokens, query_variants,
    },
    search_query::{
        ParsedHistorySearchQuery, parse_history_search_query, split_operator, tokenize_query,
    },
};
use vault_search::{Ast, PhraseTerm, TextField};
pub(super) struct Query {
    pub ast: Ast,
    pub exact: Ast,
    pub filters: ParsedHistorySearchQuery,
    pub starred: bool,
    /// The typo predicate for the same clauses, when any has a Latin target: what a fuzzy
    /// search validates pages against, and whether a search without matches retries fuzzily.
    pub fuzzy: Option<crate::archive::search_lexical::FuzzyQuery>,
}
/// A query token as `OR` sees it: a word or quoted phrase it can join, an `OR`, or
/// anything else (an operator, an exclusion), which `OR` does not reach across.
enum Piece {
    Text { value: String, quoted: bool },
    Or,
    Other,
}
pub(super) fn build(raw: &str, fuzzy: bool) -> Query {
    let filters = parse_history_search_query(raw);
    let mut pieces = Vec::new();
    let mut clauses = Vec::new();
    let mut starred = false;
    for token in tokenize_query(raw) {
        if !token.quoted && token.value.eq_ignore_ascii_case("OR") {
            pieces.push(Piece::Or);
            continue;
        }
        let (negative, value) = token
            .value
            .strip_prefix('-')
            .filter(|value| !value.is_empty())
            .map_or((false, token.value.as_str()), |value| (true, value));
        let (operator, operand) = split_operator(value);
        if operator.as_deref() == Some("is") && operand.eq_ignore_ascii_case("starred") && !negative
        {
            starred = true;
            pieces.push(Piece::Other);
            continue;
        }
        let field = match operator.as_deref() {
            Some("intitle" | "title") => Some(TextField::Title),
            Some("inurl" | "url") => Some(TextField::Url),
            Some("tag" | "note" | "site" | "filetype" | "ext") => {
                pieces.push(Piece::Other);
                continue;
            }
            Some("after" | "before") if !negative => {
                pieces.push(Piece::Other);
                continue;
            }
            _ => None,
        };
        if let Some(field) = field {
            if !negative {
                clauses.push(field_candidates(operand, field));
            }
            pieces.push(Piece::Other);
            continue;
        }
        // Exclusions are canonical substring constraints, including short fragments.
        if negative {
            pieces.push(Piece::Other);
            continue;
        }
        pieces.push(Piece::Text { value: value.into(), quoted: token.quoted });
    }
    // `OR` joins the word or phrase right before it with the one right after it, as in
    // Google: `rust tokio OR tauri` is rust, and tokio or tauri. Everything else is AND.
    // An `OR` with no word or phrase on one side (at an end, doubled, next to an
    // operator or an exclusion) joins nothing.
    let mut lone = Vec::new();
    let mut alternatives = Vec::new();
    let mut at = 0;
    while at < pieces.len() {
        if !matches!(pieces[at], Piece::Text { .. }) {
            at += 1;
            continue;
        }
        let mut chain = vec![at];
        while let (Some(Piece::Or), Some(Piece::Text { .. })) =
            (pieces.get(at + 1), pieces.get(at + 2))
        {
            at += 2;
            chain.push(at);
        }
        at += 1;
        if chain.len() == 1 {
            lone.push(chain[0]);
        } else {
            alternatives.push(chain);
        }
    }
    let text = |index: usize| match &pieces[index] {
        Piece::Text { value, quoted } => (value.as_str(), *quoted),
        Piece::Or | Piece::Other => ("", false),
    };
    // The typo fallback groups words exactly as the exact search below does.
    let typo = crate::archive::search_lexical::fuzzy_clauses(
        &std::iter::once(lone.iter().map(|&index| text(index).0).collect::<Vec<_>>().join(" "))
            .filter(|run| !run.is_empty())
            .map(|run| vec![run])
            .chain(alternatives.iter().map(|chain| {
                chain.iter().map(|&index| text(index).0.to_owned()).collect::<Vec<_>>()
            }))
            .collect::<Vec<_>>(),
    );
    let fuzzy = fuzzy && typo.is_some();
    // Words outside any OR are searched together, as one run, so compact recall still
    // finds them written as one word; a phrase among them also has to match as written.
    let mut run = Vec::new();
    for &index in &lone {
        let (value, quoted) = text(index);
        run.push(value);
        if quoted {
            clauses.push(phrase(value, None));
        }
    }
    if !run.is_empty() {
        clauses.push(Ast::Or(vec![words(&run.join(" "), fuzzy)]));
    }
    for chain in alternatives {
        clauses.push(Ast::Or(
            chain
                .into_iter()
                .map(|index| match text(index) {
                    (value, true) => Ast::And(vec![words(value, fuzzy), phrase(value, None)]),
                    (value, false) => words(value, fuzzy),
                })
                .collect(),
        ));
    }
    // Fields and exclusions are constraints on every result, whatever OR joins.
    let mut ast = Ast::And(clauses);
    // Facets are global in the existing syntax, including across OR branches.
    // Positive tags can preselect posting candidates; canonical SQL checks exact labels.
    if !filters.required_tags.is_empty() {
        ast = Ast::And(
            std::iter::once(ast)
                .chain(
                    filters.required_tags.iter().map(|tag| field_words(tag, Some(TextField::Tags))),
                )
                .collect(),
        );
    }
    let exact = if fuzzy { build(raw, false).ast } else { ast.clone() };
    if fuzzy {
        ast = Ast::Or(vec![ast, exact.clone()]);
    }
    Query { ast, exact, filters, starred, fuzzy: typo }
}
fn words(raw: &str, fuzzy: bool) -> Ast {
    Ast::Or(
        query_variants(raw)
            .into_iter()
            .map(|variant| {
                let normalized = normalized_tokens(&variant);
                let compact =
                    normalized.chars().filter(|ch| ch.is_alphanumeric()).collect::<String>();
                if compact.is_empty() {
                    return Ast::Or(Vec::new());
                }
                if fuzzy {
                    let length = compact.chars().count();
                    let edits = crate::archive::search_lexical::max_allowed_distance(length);
                    let grams = Ast::Trigrams { text: compact, minimum_match: 1 };
                    // Each edit can destroy three original grams. Without a guaranteed
                    // survivor, score grams but visit their complement too; the complete
                    // text predicate decides membership. Nothing is sampled or capped.
                    return if length > edits * 3 + 2 {
                        grams
                    } else {
                        Ast::Or(vec![grams.clone(), Ast::Not(Box::new(grams))])
                    };
                }
                let mut terms = normalized
                    .split_whitespace()
                    .filter(|word| !contains_cjk(word) && *word != "|")
                    .map(|word| Ast::Prefix { token: word.into(), field: None })
                    .collect::<Vec<_>>();
                for run in cjk_runs(&compact) {
                    terms.push(phrase_tokens(
                        run.into_iter().map(|ch| ch.to_string()).collect(),
                        None,
                    ));
                }
                let words = Ast::And(terms);
                if compact.chars().count() >= 3 {
                    Ast::Or(vec![words, Ast::Substring { text: compact }])
                } else {
                    words
                }
            })
            .collect(),
    )
}
fn field_candidates(raw: &str, field: TextField) -> Ast {
    let mut candidates = vec![field_words(raw, Some(field))];
    for text in normalized_compact_filter_terms(raw)
        .into_iter()
        .filter(|value| value.chars().all(char::is_alphanumeric))
    {
        if text.chars().count() < 3 {
            return Ast::And(Vec::new());
        }
        candidates.push(Ast::Substring { text });
    }
    Ast::Or(candidates)
}
fn field_words(raw: &str, field: Option<TextField>) -> Ast {
    Ast::Or(
        query_variants(raw)
            .into_iter()
            .map(|variant| {
                Ast::And(
                    normalized_tokens(&variant)
                        .split_whitespace()
                        .map(|word| Ast::Prefix { token: word.into(), field })
                        .collect(),
                )
            })
            .collect(),
    )
}
fn phrase(raw: &str, field: Option<TextField>) -> Ast {
    Ast::Or(
        normalized_filter_terms(raw)
            .into_iter()
            .map(|variant| {
                phrase_tokens(
                    normalized_tokens(&variant).split_whitespace().map(str::to_owned).collect(),
                    field,
                )
            })
            .collect(),
    )
}
fn phrase_tokens(tokens: Vec<String>, field: Option<TextField>) -> Ast {
    if let Some(field) = field {
        return Ast::Phrase {
            tokens: tokens.into_iter().map(PhraseTerm::Exact).collect(),
            field: Some(field),
        };
    }
    let mut phrases = [
        TextField::Url,
        TextField::Title,
        TextField::SearchTerms,
        TextField::Enrichment,
        TextField::Notes,
        TextField::Tags,
    ]
    .into_iter()
    .map(|field| Ast::Phrase {
        tokens: tokens.iter().cloned().map(PhraseTerm::Exact).collect(),
        field: Some(field),
    })
    .collect::<Vec<_>>();
    // Grams are a set, so their stored order cannot prove a phrase. A whole CJK
    // gram remains useful for scoring when it genuinely exists in the projection.
    if !tokens.is_empty()
        && tokens.iter().all(|token| token.chars().count() == 1 && contains_cjk(token))
    {
        phrases.push(Ast::Term { token: tokens.concat(), field: Some(TextField::CjkGrams) });
    }
    Ast::Or(phrases)
}

/// Canonical validation of a stale posting uses the same normalized document contract.
/// It never computes a replacement score or makes candidates disappear behind a cap.
pub(super) fn matches_document(ast: &Ast, document: &vault_search::PageDocument) -> bool {
    let fields = |selected: Option<TextField>| {
        TextField::ALL
            .into_iter()
            .filter(move |field| selected.is_none_or(|wanted| *field == wanted))
    };
    match ast {
        Ast::Term { token, field } => fields(*field).any(|field| {
            document.text[field as usize].split_ascii_whitespace().any(|word| word == token)
        }),
        Ast::Prefix { token, field } => fields(*field).any(|field| {
            document.text[field as usize]
                .split_ascii_whitespace()
                .any(|word| word.starts_with(token))
        }),
        Ast::Phrase { tokens, field } => {
            !tokens.is_empty()
                && fields(*field).any(|field| {
                    let words =
                        document.text[field as usize].split_ascii_whitespace().collect::<Vec<_>>();
                    words.windows(tokens.len()).any(|words| {
                        words.iter().zip(tokens).all(|(word, term)| match term {
                            PhraseTerm::Exact(token) => *word == token,
                            PhraseTerm::Prefix(token) => word.starts_with(token),
                        })
                    })
                })
        }
        Ast::Substring { text } => {
            text.chars().count() >= 3 && document.compact_text.contains(text)
        }
        Ast::Trigrams { text, minimum_match } => {
            let chars = text.chars().collect::<Vec<_>>();
            let grams = chars
                .windows(3)
                .map(|w| w.iter().collect::<String>())
                .collect::<std::collections::HashSet<_>>();
            grams.into_iter().filter(|gram| document.compact_text.contains(gram)).count()
                >= *minimum_match
        }
        Ast::And(children) => children.iter().all(|child| matches_document(child, document)),
        Ast::Or(children) => children.iter().any(|child| matches_document(child, document)),
        Ast::Not(child) => !matches_document(child, document),
    }
}
