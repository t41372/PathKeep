//! Responsibilities: Define the versioned schema and deterministic token analyzers.
//! Not responsible for: Language normalization or query parsing.
//! Failure modes: Fields lose positions, weights, or multi-valued membership; short/CJK
//! grams disappear; analyzer/schema drift opens silently. Tested in contracts and recovery.

use crate::{PageDocument, TextField};
use tantivy::schema::{
    FAST, Field, INDEXED, IndexRecordOption, STRING, Schema, TextFieldIndexing, TextOptions,
};
use tantivy::tokenizer::NgramTokenizer;
use tantivy::{Index, TantivyDocument};

pub(crate) const SCHEMA_VERSION: u32 = 1;
pub(crate) const ANALYZER_VERSION: u32 = 1;

#[derive(Clone)]
pub(crate) struct Fields {
    pub text: [Field; 10],
    pub grams: Field,
    pub id: Field,
    pub newest: Field,
    pub domain: Field,
    pub profiles: Field,
}
pub(crate) fn schema() -> (Schema, Fields) {
    let mut b = Schema::builder();
    let options = TextOptions::default().set_indexing_options(
        TextFieldIndexing::default()
            .set_tokenizer("normalized")
            .set_index_option(IndexRecordOption::WithFreqsAndPositions),
    );
    let text = TextField::NAMES.map(|name| b.add_text_field(name, options.clone()));
    let grams = b.add_text_field(
        "trigrams",
        TextOptions::default().set_indexing_options(
            TextFieldIndexing::default()
                .set_tokenizer("grams")
                .set_index_option(IndexRecordOption::WithFreqs),
        ),
    );
    let fields = Fields {
        text,
        grams,
        id: b.add_u64_field("page_id", INDEXED | FAST),
        newest: b.add_i64_field("newest_visit_ms", FAST),
        domain: b.add_text_field("domain", STRING | FAST),
        profiles: b.add_u64_field("profile_ids", INDEXED | FAST),
    };
    (b.build(), fields)
}
pub(crate) fn register(index: &Index) {
    index.tokenizers().register("normalized", crate::analyzer::PrefixTokenizer::default());
    index
        .tokenizers()
        .register("grams", NgramTokenizer::new(3, 3, false).expect("fixed valid ngram widths"));
}
impl Fields {
    pub fn document(&self, page: PageDocument) -> TantivyDocument {
        let mut doc = TantivyDocument::default();
        doc.add_u64(self.id, page.page_id);
        doc.add_i64(self.newest, page.newest_visit_ms);
        doc.add_text(self.domain, page.domain);
        for profile in page.profile_ids {
            doc.add_u64(self.profiles, profile);
        }
        for (field, text) in self.text.into_iter().zip(page.text) {
            doc.add_text(field, text);
        }
        doc.add_text(self.grams, page.compact_text);
        doc
    }
}
