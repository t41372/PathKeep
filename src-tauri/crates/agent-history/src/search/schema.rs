//! Responsibilities: define the agent-event search schema and analyzer identity.
//! Not responsible for: source identity, SQL storage, coverage or query assembly.

use sha2::{Digest, Sha256};
use tantivy::schema::{
    Field, INDEXED, IndexRecordOption, STORED, STRING, Schema, TextFieldIndexing, TextOptions,
};
use tantivy::{Index, TantivyDocument};

pub(super) const VERSION: u32 = 3;

pub(super) struct Fields {
    pub event_id: Field,
    pub projection_id: Field,
    pub session_id: Field,
    pub harness: Field,
    pub project: Field,
    pub text: Field,
    pub readable: [Field; 4],
    pub title: Field,
    pub card_id: Field,
    pub ordinal: Field,
}

pub(super) fn schema() -> (Schema, Fields) {
    let mut builder = Schema::builder();
    let text_options = TextOptions::default().set_indexing_options(
        TextFieldIndexing::default()
            .set_tokenizer("agent-history-v1")
            .set_index_option(IndexRecordOption::WithFreqsAndPositions),
    );
    let fields = Fields {
        event_id: builder.add_u64_field("event_id", INDEXED | STORED),
        projection_id: builder.add_u64_field("projection_id", STORED | tantivy::schema::FAST),
        session_id: builder.add_text_field("session_id", STRING),
        harness: builder.add_text_field("harness", STRING),
        project: builder.add_text_field("project", STRING),
        ordinal: builder.add_u64_field("ordinal", tantivy::schema::FAST),
        title: builder.add_text_field("title", text_options.clone()),
        card_id: builder.add_text_field("card_id", STRING | STORED),
        readable: ["user", "agent", "tool", "thinking"]
            .map(|name| builder.add_text_field(name, text_options.clone())),
        text: builder.add_text_field("text", text_options),
    };
    (builder.build(), fields)
}

pub(super) fn register(index: &Index) {
    index.tokenizers().register("agent-history-v1", super::tokenizer::HistoryTokenizer);
}

pub(super) struct EventDocument {
    pub event_id: i64,
    pub projection_id: i64,
    pub ordinal: i64,
    pub session_id: String,
    pub harness: String,
    pub project: String,
    pub text: String,
    pub native_id: Option<String>,
    pub parts: Vec<super::expression::Part>,
}

#[derive(Clone)]
pub(super) struct EventSource {
    pub format: Option<i64>,
    pub publication: Option<String>,
    pub session_id: String,
    pub projection_id: i64,
    pub harness: String,
    pub project: String,
}

impl EventDocument {
    pub fn byte_len(&self) -> usize {
        self.session_id.len()
            + self.harness.len()
            + self.project.len()
            + self.text.len()
            + self.parts.iter().map(|p| p.text.len()).sum::<usize>()
    }
}

/// Exact identifiers can exceed Tantivy's term-length ceiling, especially native
/// Windows paths encoded as UTF-8. Hash every exact value consistently rather than
/// allowing the engine to silently omit large `STRING` terms.
pub(super) fn exact_term(value: &str) -> String {
    hex::encode(Sha256::digest(value.as_bytes()))
}

impl Fields {
    pub fn document(&self, event: EventDocument) -> TantivyDocument {
        let mut document = TantivyDocument::default();
        document.add_u64(self.ordinal, event.ordinal as u64);
        document.add_u64(self.event_id, event.event_id as u64);
        document.add_u64(self.projection_id, event.projection_id as u64);
        document.add_text(self.session_id, exact_term(&event.session_id));
        document.add_text(self.harness, exact_term(&event.harness));
        document.add_text(self.project, exact_term(&event.project));
        for part in event.parts {
            document.add_text(self.readable[part.role as usize], part.text);
        }
        document.add_text(self.text, event.text);
        document
    }
}
