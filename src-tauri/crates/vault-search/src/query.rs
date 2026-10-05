//! Responsibilities: Compile a typed AST into weighted Tantivy queries.
//! Not responsible for: Parsing text or changing normalized tokens.
//! Failure modes: Single-term prefixes fail or truncate expansion; punctuation becomes
//! syntax; phrases cross fields; exclusions and empty AND/OR change meaning; typo
//! thresholds cap recall. Tested in ast.

use crate::{Error, Result, TextField, schema::Fields};
use tantivy::Term;
use tantivy::query::{
    AllQuery, BooleanQuery, BoostQuery, ConstScoreQuery, EmptyQuery, Occur, PhraseQuery, Query,
    TermQuery,
};
use tantivy::schema::{Field, IndexRecordOption};

/// Typed normalized expressions. Empty AND is all pages; empty OR/phrase is none.
/// Prefix tokens are literal, never regex or query-parser syntax. Trigrams use a
/// caller-selected number of distinct matching Unicode grams, with no candidate cap.
#[derive(Clone, Debug)]
pub enum Ast {
    Term { token: String, field: Option<TextField> },
    Prefix { token: String, field: Option<TextField> },
    Phrase { tokens: Vec<PhraseTerm>, field: Option<TextField> },
    Not(Box<Ast>),
    And(Vec<Ast>),
    Or(Vec<Ast>),
    Trigrams { text: String, minimum_match: usize },
}
/// A phrase position can require an exact token or a prefix, without string syntax.
#[derive(Clone, Debug)]
pub enum PhraseTerm {
    Exact(String),
    Prefix(String),
}
impl From<String> for PhraseTerm {
    fn from(token: String) -> Self {
        Self::Exact(token)
    }
}
impl From<&str> for PhraseTerm {
    fn from(token: &str) -> Self {
        Self::Exact(token.into())
    }
}
impl PhraseTerm {
    fn indexed(&self) -> String {
        match self {
            Self::Exact(token) => format!("={token}"),
            Self::Prefix(token) => format!("^{token}"),
        }
    }
}
impl Ast {
    pub(crate) fn compile(&self, fields: &Fields) -> Result<Box<dyn Query>> {
        match self {
            Self::Term { token, field } => {
                across(fields, *field, |f| Ok(term(f, &format!("={token}"))))
            }
            Self::Prefix { token, field } => {
                across(fields, *field, |f| Ok(term(f, &format!("^{token}"))))
            }
            Self::Phrase { tokens, field } => across(fields, *field, |f| {
                Ok(match tokens.as_slice() {
                    [] => Box::new(EmptyQuery),
                    [token] => term(f, &token.indexed()),
                    _ => Box::new(PhraseQuery::new(
                        tokens.iter().map(|s| Term::from_field_text(f, &s.indexed())).collect(),
                    )),
                })
            }),
            Self::Not(child) => Ok(Box::new(BooleanQuery::new(vec![
                (Occur::Must, Box::new(ConstScoreQuery::new(Box::new(AllQuery), 0.0))),
                (Occur::MustNot, child.compile(fields)?),
            ]))),
            Self::And(children) if children.is_empty() => Ok(Box::new(AllQuery)),
            Self::And(children) => {
                Ok(Box::new(BooleanQuery::intersection(compile_all(children, fields)?)))
            }
            Self::Or(children) => Ok(Box::new(BooleanQuery::union(compile_all(children, fields)?))),
            Self::Trigrams { text, minimum_match } => {
                if *minimum_match == 0 {
                    return Err(Error::Input("trigram minimum_match must be positive".into()));
                }
                let chars: Vec<_> = text.chars().collect();
                let mut grams: Vec<String> = chars.windows(3).map(|w| w.iter().collect()).collect();
                grams.sort_unstable();
                grams.dedup();
                if *minimum_match > grams.len() {
                    return Ok(Box::new(EmptyQuery));
                }
                Ok(Box::new(BooleanQuery::union_with_minimum_required_clauses(
                    grams.iter().map(|g| term(fields.grams, g)).collect(),
                    *minimum_match,
                )))
            }
        }
    }
}
fn term(field: Field, token: &str) -> Box<dyn Query> {
    Box::new(TermQuery::new(Term::from_field_text(field, token), IndexRecordOption::WithFreqs))
}
fn compile_all(children: &[Ast], fields: &Fields) -> Result<Vec<Box<dyn Query>>> {
    children.iter().map(|child| child.compile(fields)).collect()
}
fn across(
    fields: &Fields,
    selected: Option<TextField>,
    mut build: impl FnMut(Field) -> Result<Box<dyn Query>>,
) -> Result<Box<dyn Query>> {
    let mut queries = vec![];
    for field in TextField::ALL {
        if selected.is_none_or(|f| f == field) {
            queries.push(Box::new(BoostQuery::new(
                build(fields.text[field as usize])?,
                TextField::WEIGHTS[field as usize],
            )) as Box<dyn Query>);
        }
    }
    Ok(Box::new(BooleanQuery::union(queries)))
}
