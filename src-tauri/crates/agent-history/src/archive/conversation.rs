//! Responsibilities: bind a source publication to its own content-derived conversation key.
//! Not responsible for: reading live files, resolving foreign identities or user state.
use super::Archive;
use crate::{
    Result, SessionMetadata,
    adapters::{adapter, common, identity::ConversationKey},
};
use sha2::{Digest, Sha256};
use std::io::{BufRead, BufReader, Read};

impl Archive {
    pub(super) fn conversation_key(
        &self,
        source_id: &str,
        revision: i64,
        metadata: &SessionMetadata,
    ) -> Result<ConversationKey> {
        let source = self.source(source_id)?;
        let decoded =
            common::decoded_reader(Box::new(self.raw_reader(revision)?), &source.relative_path);
        let first = decoded.and_then(|reader| first_record(reader));
        // Low-level producers may publish their own interpretation of an opaque
        // native transport. Identity must not discard otherwise retained evidence.
        // The normal capture path never publishes a failed decoder's interpretation.
        let (digest, first) = match first {
            Ok(first) => first,
            Err(_) => first_record(Box::new(self.raw_reader(revision)?))?,
        };
        let mut key = adapter(source.harness.parse()?).conversation_key(
            metadata,
            source_id,
            &source.relative_path,
            &digest,
            &first,
        );
        key.first_record_digest = digest;
        Ok(key)
    }
}

fn first_record(reader: Box<dyn Read + '_>) -> Result<(String, serde_json::Value)> {
    let mut reader = BufReader::new(reader);
    let mut digest = Sha256::new();
    let mut prefix = Vec::new();
    let mut oversized = false;
    loop {
        let buffer = reader.fill_buf()?;
        if buffer.is_empty() {
            break;
        }
        let n = buffer.iter().position(|b| *b == b'\n').map_or(buffer.len(), |i| i + 1);
        let end = buffer[n - 1] == b'\n';
        digest.update(&buffer[..n]);
        if !oversized && prefix.len() + n <= 16 * 1024 * 1024 {
            prefix.extend_from_slice(&buffer[..n]);
        } else {
            oversized = true;
            prefix.clear();
        }
        reader.consume(n);
        if end {
            break;
        }
    }
    Ok((
        hex::encode(digest.finalize()),
        serde_json::from_slice(&prefix).unwrap_or(serde_json::Value::Null),
    ))
}
