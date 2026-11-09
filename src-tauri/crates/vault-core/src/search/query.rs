//! Responsibilities: translate the existing syntax and normalization into a typed AST.
//! Not responsible for: SQL, ranking, candidate windows or query-string escaping.
//! Failure modes: quotes become unordered words; exclusions are truncated; Unicode
//! prefixes disappear; compact recall changes word intersections; OR becomes AND; OR
//! swallows the words around its neighbours (`rust tokio OR tauri` read as
//! "rust tokio, or tauri" instead of "rust, and tokio or tauri"); OR reaches past an
//! operator to a word further away; a typo search drops a word it cannot misspell, or
//! narrows its candidates past what it would accept.
//! Tests: query_tests, fuzzy_tests, search::tests and projection_tests.
use crate::archive::{
    search_lexical::{
        FuzzyDocument, FuzzyTarget, cjk_runs, contains_cjk, fuzzy_targets,
        normalized_compact_filter_terms, normalized_filter_terms, normalized_tokens,
        query_variants,
    },
    search_query::{
        ParsedHistorySearchQuery, parse_history_search_query, split_operator, tokenize_query,
    },
};
use vault_search::{Ast, PhraseTerm, TextField};
#[cfg(test)]
#[path = "query_tests.rs"]
mod tests;

pub(super) struct Query {
    pub ast: Ast,
    pub filters: ParsedHistorySearchQuery,
    pub starred: bool,
    /// What a typo search accepts, when some word could be misspelled: what it validates
    /// pages against, and whether a search without matches retries with typos allowed.
    pub fuzzy: Option<FuzzyQuery>,
}
/// What a typo search accepts, in the shape of the query itself. Every word stays in it,
/// matching as the exact search would or, where it can be misspelled, within its edits, so
/// a word that cannot be misspelled still has to be there. The candidates the index reads
/// and the check each page passes share this shape. That the candidates hold every page the
/// check accepts is still a separate promise for each kind of target, kept by
/// `FuzzyTarget::can_lose_every_gram` and known not to hold on some accented pages
/// (`WORK-SEARCH-ACCENT-GRAMS`).
#[derive(Debug, Clone)]
pub(super) enum FuzzyQuery {
    Exact(Ast),
    Near(FuzzyTarget),
    All(Vec<FuzzyQuery>),
    Any(Vec<FuzzyQuery>),
}
impl FuzzyQuery {
    fn word(value: &str, quoted: bool) -> Self {
        let exact = Self::Exact(alternative(value, quoted));
        if quoted {
            return exact;
        }
        Self::Any(
            std::iter::once(exact)
                .chain(fuzzy_targets(value).into_iter().map(Self::Near))
                .collect(),
        )
    }
    /// Words outside any OR: a page holds each of them, in any order, or holds them written
    /// as one word, sharing one budget of edits (`rust tokoi`, `face bok`).
    fn run(words: &[(&str, bool)]) -> Self {
        if let [(value, quoted)] = words {
            return Self::word(value, *quoted);
        }
        let run = words.iter().map(|(value, _)| *value).collect::<Vec<_>>().join(" ");
        let each = words
            .iter()
            .filter(|(value, _)| has_text(value))
            .map(|(value, quoted)| Self::word(value, *quoted))
            .collect();
        Self::Any(vec![Self::word(&run, false), Self::All(each)])
    }
    fn has_target(&self) -> bool {
        match self {
            Self::Exact(_) => false,
            Self::Near(_) => true,
            Self::All(parts) | Self::Any(parts) => parts.iter().any(Self::has_target),
        }
    }
    fn candidates(&self) -> Ast {
        match self {
            Self::Exact(ast) => ast.clone(),
            Self::Near(target) => {
                let grams = Ast::Trigrams { text: target.text.clone(), minimum_match: 1 };
                // Without a guaranteed survivor, score grams but visit their complement too;
                // the complete text predicate decides membership. Nothing is sampled or capped.
                if target.can_lose_every_gram() {
                    Ast::Or(vec![grams.clone(), Ast::Not(Box::new(grams))])
                } else {
                    grams
                }
            }
            Self::All(parts) => Ast::And(parts.iter().map(Self::candidates).collect()),
            Self::Any(parts) => Ast::Or(parts.iter().map(Self::candidates).collect()),
        }
    }
    pub(super) fn matches(&self, document: &vault_search::PageDocument) -> bool {
        self.accepts(
            document,
            &FuzzyDocument {
                normalized_url: &document.text[TextField::NormalizedUrl as usize],
                normalized_title: &document.text[TextField::NormalizedTitle as usize],
                normalized_search_terms: &document.text[TextField::NormalizedSearchTerms as usize],
                compact_text: &normalized_tokens(&document.compact_text),
            },
        )
    }
    fn accepts(&self, document: &vault_search::PageDocument, text: &FuzzyDocument<'_>) -> bool {
        match self {
            Self::Exact(ast) => matches_document(ast, document),
            Self::Near(target) => target.is_near(text),
            Self::All(parts) => parts.iter().all(|part| part.accepts(document, text)),
            Self::Any(parts) => parts.iter().any(|part| part.accepts(document, text)),
        }
    }
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
        if token.quote_start == Some(0) {
            pieces.push(Piece::Text { value: token.value, quoted: true });
            continue;
        }
        if token.quote_start.is_none() && token.value.eq_ignore_ascii_case("OR") {
            pieces.push(Piece::Or);
            continue;
        }
        let (negative, value) = token
            .value
            .strip_prefix('-')
            .filter(|value| !value.is_empty())
            .map_or((false, token.value.as_str()), |value| (true, value));
        // In -"site:x", the minus is syntax but the colon is quoted text.
        let (operator, operand) = if negative && token.quote_start == Some(1) {
            (None, value)
        } else {
            split_operator(value)
        };
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
        pieces.push(Piece::Text { value: value.into(), quoted: token.quote_start.is_some() });
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
    // Words outside any OR are searched together, as one run, so compact recall still
    // finds them written as one word; a phrase among them also has to match as written.
    let mut run = Vec::new();
    for &index in &lone {
        let (value, quoted) = text(index);
        run.push((value, quoted));
        if quoted {
            clauses.push(phrase(value, None));
        }
    }
    let mut exact = clauses.clone();
    let mut typo = Vec::new();
    if !run.is_empty() {
        let joined = run.iter().map(|(value, _)| *value).collect::<Vec<_>>().join(" ");
        exact.push(Ast::Or(vec![words(&joined)]));
        typo.push(FuzzyQuery::run(&run));
    }
    for chain in alternatives {
        let chain = chain.into_iter().map(text).collect::<Vec<_>>();
        exact.push(Ast::Or(
            chain.iter().map(|(value, quoted)| alternative(value, *quoted)).collect(),
        ));
        typo.push(FuzzyQuery::Any(
            chain.iter().map(|(value, quoted)| FuzzyQuery::word(value, *quoted)).collect(),
        ));
    }
    // Facets are global in the existing syntax, including across OR branches.
    // Positive tags can preselect posting candidates; canonical SQL checks exact labels.
    let tagged = |ast: Ast| {
        if filters.required_tags.is_empty() {
            return ast;
        }
        Ast::And(
            std::iter::once(ast)
                .chain(
                    filters.required_tags.iter().map(|tag| field_words(tag, Some(TextField::Tags))),
                )
                .collect(),
        )
    };
    // Fields and exclusions are constraints on every result, whatever OR joins.
    let exact = tagged(Ast::And(exact));
    let typo = FuzzyQuery::All(typo);
    let candidates = tagged(Ast::And(clauses.into_iter().chain([typo.candidates()]).collect()));
    let typo =
        typo.has_target().then(|| FuzzyQuery::Any(vec![FuzzyQuery::Exact(exact.clone()), typo]));
    let ast = if fuzzy && typo.is_some() { Ast::Or(vec![candidates, exact]) } else { exact };
    Query { ast, filters, starred, fuzzy: typo }
}
/// One side of an OR. A quoted one has to match as written.
fn alternative(value: &str, quoted: bool) -> Ast {
    if quoted { Ast::And(vec![words(value), phrase(value, None)]) } else { words(value) }
}
/// Punctuation alone (`&`) is not a word: the exact search drops it from the run as well.
fn has_text(value: &str) -> bool {
    query_variants(value)
        .iter()
        .any(|variant| normalized_tokens(variant).chars().any(char::is_alphanumeric))
}
fn words(raw: &str) -> Ast {
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
