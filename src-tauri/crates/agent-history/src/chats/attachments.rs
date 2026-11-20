//! Read an attachment by its issued conversation record and binding. Display
//! paths are labels; expansion uses only the archive.
use crate::{Archive, Error, Result, read_model::JobControl};
use base64::{Engine, engine::general_purpose::STANDARD};
use serde::Deserialize;
use serde_json::{Value, json};
use std::io::Read;
use vault_store::{Db, OptionalRow, params};

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct AttachmentQuery {
    pub request_id: Option<String>,
    pub id: String,
    pub publication: String,
    pub source_id: String,
    pub ordinal: u64,
    pub key: String,
    pub offset: u64,
    pub limit: usize,
}

pub(super) struct Image<'a> {
    key: String,
    data: &'a str,
    mime: Option<&'a str>,
}
impl Image<'_> {
    fn size(&self) -> usize {
        (self.data.len() / 4) * 3
            - self
                .data
                .bytes()
                .rev()
                .take_while(|b| *b == b'=')
                .take(2)
                .count()
                .min(self.data.len() / 4 * 3)
    }
    pub fn descriptor(&self) -> Value {
        json!({"kind":"attachment","availability":"inline","key":self.key,"mimeType":self.mime,"bytes":self.size()})
    }
}
pub(super) fn inline(v: &Value) -> Vec<Image<'_>> {
    let mut images = vec![];
    for pointer in [
        "/message/content",
        "/payload/content",
        "/data/message/content",
        "/data/content",
        "/content",
    ] {
        if let Some(blocks) = v.pointer(pointer).and_then(Value::as_array) {
            for b in blocks {
                if !matches!(b["type"].as_str(), Some("image" | "input_image")) {
                    continue;
                }
                let Some(data) =
                    b["data"].as_str().or(b.pointer("/source/data").and_then(Value::as_str))
                else {
                    continue;
                };
                images.push(Image {
                    key: format!("inline-{}", images.len()),
                    data,
                    mime: b["mimeType"]
                        .as_str()
                        .or(b.pointer("/source/media_type").and_then(Value::as_str)),
                });
            }
        }
    }
    images
}
impl Archive {
    pub fn chat_attachment(&self, q: &AttachmentQuery, job: &JobControl) -> Result<Value> {
        if !(1..=65_536).contains(&q.limit) {
            return Err(Error::InvalidInput("attachment limit must be 1..65536 bytes".into()));
        }
        let _guard = self.jobs.enter_window(&job.cancel)?;
        let h = self.screen_head(&q.id, Some(&q.publication))?;
        let (version, end) = self.member_version(&h, &q.source_id)?;
        if q.ordinal >= end {
            return Err(Error::NotFound("attachment record".into()));
        }
        let (attempt, revision) = self.store.read(Db::AgentHistoryReadModel, move |s| {
            s.query_one(
                "SELECT attempt,revision FROM ah_reader_version WHERE token=?1",
                [version],
                |r| Ok((r.get::<i64>(0)?, r.get::<i64>(1)?)),
            )
        })?;
        let ordinal = q.ordinal as i64;
        let key = q.key.clone();
        let (total, bytes) = if key.starts_with("inline-") {
            let source = q.source_id.clone();
            let payload=self.store.read(Db::AgentHistoryReadModel,move|s|s.query_one("SELECT payload FROM ah_events WHERE session_id=?1 AND projection_id=?2 AND ordinal=?3",params![source,attempt,ordinal],|r|Ok(r.get::<String>(0)?)).optional())?.ok_or_else(||Error::NotFound("inline record".into()))?;
            let parsed = serde_json::from_str(&payload)?;
            let image = inline(&parsed)
                .into_iter()
                .find(|img| img.key == key)
                .ok_or_else(|| Error::NotFound("inline image".into()))?;
            let size = image.size();
            let start = q.offset.min(size as u64) as usize;
            let end = start.saturating_add(q.limit).min(size);
            let decoded = STANDARD
                .decode(&image.data.as_bytes()[start / 3 * 4..end.div_ceil(3) * 4])
                .map_err(|_| Error::InvalidInput("invalid image encoding".into()))?;
            let skip = start % 3;
            (size as u64, decoded.into_iter().skip(skip).take(end - start).collect::<Vec<_>>())
        } else {
            let check = key.clone();
            let known=self.store.read(Db::AgentHistoryReadModel,move|s|s.query_one("SELECT EXISTS(SELECT 1 FROM ah_event_references WHERE projection_id=?1 AND ordinal=?2 AND reference_key=?3)",params![attempt,ordinal,check],|r|Ok(r.get::<i64>(0)?!=0)))?;
            if !known {
                return Err(Error::NotFound("record attachment".into()));
            }
            let target=self.store.read(Db::AgentHistoryEvidence,move|s|s.query_one("SELECT target_revision FROM ah_reference_bindings WHERE parent_revision=?1 AND reference_key=?2 ORDER BY id DESC LIMIT 1",params![revision,key],|r|Ok(r.get::<i64>(0)?)).optional())?.ok_or_else(||Error::NotFound("retained attachment".into()))?;
            let info = self.revision(target)?;
            if !info.bytes_retained {
                return Err(Error::NotFound("attachment bytes expired".into()));
            }
            let mut bytes = vec![];
            if q.offset < info.byte_len {
                self.raw_window(&info, q.offset, (q.limit as u64).min(info.byte_len - q.offset))
                    .read_to_end(&mut bytes)?;
            }
            (info.byte_len, bytes)
        };
        let end = q.offset.saturating_add(bytes.len() as u64);
        Ok(json!({"bytes":bytes,"totalBytes":total,"next":(end<total).then_some(end)}))
    }
}
