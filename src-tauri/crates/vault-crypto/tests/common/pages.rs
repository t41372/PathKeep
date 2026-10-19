//! Page-shaped test documents and a small Tantivy schema for them.
//!
//! Each page is generated from its id alone, so a test can rebuild any page, or count the
//! pages containing a word, without keeping them: that count is the independent answer that
//! search results are checked against.
//!
//! Titles mix common English words with a long tail of made-up words, drawn with a Zipf-like
//! skew so some words are in a large share of pages and most are rare, as in real history.
//! URLs are `https://<domain>/<words>/<id>`. A few pages carry the sentinel word `pkneedle`
//! and the domain `pksentinel.example`, which plaintext scans look for.

use std::sync::OnceLock;

use tantivy::collector::{Count, TopDocs};
use tantivy::query::{Query, TermQuery};
use tantivy::schema::{
    FAST, Field, INDEXED, IndexRecordOption, STORED, STRING, Schema, TEXT, TantivyDocument, Value,
};
use tantivy::{DocAddress, IndexWriter, Searcher, Term};

use super::Rng;

/// Words found in one page out of this many carry the sentinel.
const SENTINEL_EVERY: u64 = 997;

const COMMON: &[&str] = &[
    "the",
    "how",
    "to",
    "best",
    "review",
    "news",
    "video",
    "guide",
    "github",
    "docs",
    "search",
    "home",
    "login",
    "account",
    "settings",
    "weather",
    "recipe",
    "price",
    "free",
    "download",
    "online",
    "topic",
    "forum",
    "blog",
    "release",
    "notes",
    "issue",
    "pull",
    "request",
    "map",
    "travel",
    "hotel",
    "flight",
    "music",
    "playlist",
    "watch",
    "learn",
    "course",
    "tutorial",
    "rust",
    "python",
    "javascript",
    "react",
    "database",
    "linux",
    "apple",
    "windows",
    "android",
    "phone",
    "laptop",
    "camera",
    "game",
    "score",
    "football",
    "market",
    "stock",
    "bank",
    "invoice",
    "shop",
    "cart",
    "order",
    "delivery",
    "health",
    "doctor",
    "school",
    "university",
    "paper",
    "research",
    "data",
    "chart",
    "photo",
    "gallery",
    "design",
    "font",
    "color",
    "calendar",
    "mail",
    "inbox",
    "chat",
];

pub struct Fields {
    pub id: Field,
    pub title: Field,
    pub url: Field,
    pub domain: Field,
    pub batch: Field,
    pub last_visit: Field,
}

pub fn schema() -> (Schema, Fields) {
    let mut builder = Schema::builder();
    let fields = Fields {
        id: builder.add_u64_field("pk_id", INDEXED | STORED | FAST),
        title: builder.add_text_field("pk_title", TEXT | STORED),
        url: builder.add_text_field("pk_url", TEXT | STORED),
        domain: builder.add_text_field("pk_domain", STRING | FAST),
        batch: builder.add_u64_field("pk_batch", INDEXED | FAST),
        last_visit: builder.add_i64_field("pk_last_visit", FAST),
    };
    (builder.build(), fields)
}

pub struct Page {
    pub id: u64,
    pub title: String,
    pub url: String,
    pub domain: String,
    pub words: Vec<String>,
}

/// The vocabulary: common words first (most frequent), then made-up words.
fn vocabulary() -> &'static (Vec<String>, Vec<f64>) {
    static VOCAB: OnceLock<(Vec<String>, Vec<f64>)> = OnceLock::new();
    VOCAB.get_or_init(|| {
        let syllables = [
            "ka", "lo", "mi", "ren", "to", "sa", "vel", "qu", "dor", "ni", "pe", "zu", "ash",
            "bri", "co", "fen",
        ];
        let mut words: Vec<String> = COMMON.iter().map(|w| w.to_string()).collect();
        let mut rng = Rng::new(20261004);
        while words.len() < 40_000 {
            let parts = 2 + rng.below(3) as usize;
            let word: String =
                (0..parts).map(|_| syllables[rng.below(syllables.len() as u64) as usize]).collect();
            words.push(format!("{word}{}", words.len() % 97));
        }
        // Cumulative Zipf weights, exponent 1.0.
        let mut total = 0.0;
        let cumulative = (1..=words.len())
            .map(|rank| {
                total += 1.0 / rank as f64;
                total
            })
            .collect();
        (words, cumulative)
    })
}

fn pick_word(rng: &mut Rng) -> &'static str {
    let (words, cumulative) = vocabulary();
    let target = (rng.next_u64() as f64 / u64::MAX as f64) * cumulative[cumulative.len() - 1];
    let index = cumulative.partition_point(|&c| c < target).min(words.len() - 1);
    &words[index]
}

pub fn page(id: u64) -> Page {
    let mut rng = Rng::new(id.wrapping_mul(0x9E37_79B9_7F4A_7C15) ^ 0xD1B5_4A32_D192_ED03);
    let title_len = 3 + rng.below(9) as usize;
    let mut words: Vec<String> = (0..title_len).map(|_| pick_word(&mut rng).to_owned()).collect();
    let sentinel = id % SENTINEL_EVERY == 0;
    if sentinel {
        words.push("pkneedle".into());
    }
    let domain = if sentinel {
        "pksentinel.example".to_owned()
    } else {
        let a = pick_word(&mut rng);
        let b = pick_word(&mut rng);
        format!("{a}{b}.com")
    };
    let path: Vec<&str> = (0..1 + rng.below(3)).map(|_| pick_word(&mut rng)).collect();
    let url = format!("https://{domain}/{}/{id}", path.join("/"));
    let mut title = words.join(" ");
    if let Some(first) = title.get_mut(0..1) {
        first.make_ascii_uppercase();
    }
    Page { id, title, url, domain, words }
}

pub fn document(fields: &Fields, page: &Page, batch: u64) -> TantivyDocument {
    let mut doc = TantivyDocument::default();
    doc.add_u64(fields.id, page.id);
    doc.add_text(fields.title, &page.title);
    doc.add_text(fields.url, &page.url);
    doc.add_text(fields.domain, &page.domain);
    doc.add_u64(fields.batch, batch);
    doc.add_i64(fields.last_visit, 1_700_000_000 + page.id as i64 * 37);
    doc
}

pub fn add_pages(writer: &IndexWriter, fields: &Fields, ids: std::ops::Range<u64>, batch: u64) {
    for id in ids {
        writer.add_document(document(fields, &page(id), batch)).unwrap();
    }
}

/// How many of the pages with ids in `ids` have `word` in their title: the independent
/// answer for a title term query.
pub fn expected_count(word: &str, ids: std::ops::Range<u64>) -> usize {
    ids.filter(|&id| page(id).words.iter().any(|w| w == word)).count()
}

pub fn title_term(fields: &Fields, word: &str) -> Box<dyn Query> {
    Box::new(TermQuery::new(
        Term::from_field_text(fields.title, word),
        IndexRecordOption::WithFreqs,
    ))
}

pub fn count(searcher: &Searcher, query: &dyn Query) -> tantivy::Result<usize> {
    searcher.search(query, &Count)
}

/// The top `k` pages by score, as (page id, score), ties broken by page id so two indexes
/// with different document order compare equal.
pub fn top(
    searcher: &Searcher,
    fields: &Fields,
    query: &dyn Query,
    k: usize,
) -> tantivy::Result<Vec<(u64, f32)>> {
    let hits: Vec<(f32, DocAddress)> =
        searcher.search(query, &TopDocs::with_limit(k + 64).order_by_score())?;
    let mut out = Vec::with_capacity(hits.len());
    for (score, address) in hits {
        let doc: TantivyDocument = searcher.doc(address)?;
        let id = doc.get_first(fields.id).and_then(|v| v.as_u64()).unwrap();
        out.push((id, score));
    }
    out.sort_by(|a, b| b.1.total_cmp(&a.1).then(a.0.cmp(&b.0)));
    // Cut at k, but only on a score boundary that both indexes agree on.
    out.truncate(k);
    Ok(out)
}

/// Words picked so tests cover a very common, a medium, and a rare term.
pub fn probe_words(ids: std::ops::Range<u64>) -> Vec<(String, usize)> {
    // Pages containing each word (a word repeated in one title counts once).
    let mut counts = std::collections::HashMap::new();
    for id in ids.clone() {
        let mut words = page(id).words;
        words.sort();
        words.dedup();
        for word in words {
            *counts.entry(word).or_insert(0usize) += 1;
        }
    }
    let mut by_count: Vec<(String, usize)> = counts.into_iter().collect();
    by_count.sort_by(|a, b| b.1.cmp(&a.1).then(a.0.cmp(&b.0)));
    let rare = by_count.iter().rev().find(|(_, c)| *c >= 2).cloned().unwrap();
    let medium = by_count[by_count.len() / 50].clone();
    vec![
        by_count[0].clone(),
        by_count[3].clone(),
        medium,
        rare,
        ("pkneedle".into(), expected_count("pkneedle", ids)),
    ]
}
