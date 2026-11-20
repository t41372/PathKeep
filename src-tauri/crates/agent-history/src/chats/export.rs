//! Conversation exports follow a published reader line. Original files come from retained
//! revisions, including child conversations. Destination choice and atomic install are the caller's.
use super::{
    WindowQuery,
    reader::{Anchor, View},
};
use crate::{
    Archive, Error, Result,
    export::ExportFormat,
    read_model::{JobControl, PhysicalOrdinal, ReaderPublication, RecordContext},
};
use serde::Deserialize;
use serde_json::{Value, json};
use std::{
    collections::HashSet,
    io::{Read, Write},
    path::Path,
};
use vault_store::{Db, params};

#[derive(Clone, Copy, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum ConversationFormat {
    Markdown,
    Html,
    Jsonl,
    Originals,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ConversationExport {
    pub locale: Option<super::export_html::ExportLocale>,
    pub id: String,
    pub format: ConversationFormat,
    pub publication: Option<String>,
    pub branch: Option<String>,
    pub leaf: Option<u64>,
}
impl ConversationFormat {
    pub fn extension(self) -> &'static str {
        match self {
            Self::Markdown => "md",
            Self::Html => "html",
            Self::Jsonl => "jsonl",
            Self::Originals => "tar",
        }
    }
}
impl Archive {
    pub fn export_conversation(
        &self,
        q: &ConversationExport,
        out: &mut dyn Write,
        collapse_tools: bool,
        show_thinking: bool,
    ) -> Result<()> {
        self.export_conversation_with_job(
            q,
            out,
            collapse_tools,
            show_thinking,
            &JobControl::default(),
        )
    }
    /// The window can stop an archive read between writes, including native frame replay.
    pub fn export_conversation_with_job(
        &self,
        q: &ConversationExport,
        out: &mut dyn Write,
        collapse_tools: bool,
        show_thinking: bool,
        job: &JobControl,
    ) -> Result<()> {
        job.cancel.check()?;
        let h = self.screen_head(&q.id, q.publication.as_deref())?;
        match q.format {
            ConversationFormat::Originals => self.export_originals(&h, out, job)?,
            ConversationFormat::Jsonl => {
                self.write_normalized(&q.id, ExportFormat::Jsonl, out)?;
            }
            format => {
                let header = self.chat_header(&q.id)?;
                let title = header["summary"]["title"].as_str().unwrap_or("");
                let html = format == ConversationFormat::Html;
                let locale = q.locale.unwrap_or_default();
                let display = self.insight_preferences()?;
                if html {
                    writeln!(
                        out,
                        "<!doctype html><meta charset=utf-8><meta http-equiv=\"Content-Security-Policy\" content=\"default-src 'none'; style-src 'unsafe-inline'; img-src data:\"><title>{0}</title><style>body{{max-width:54rem;margin:3rem auto;font:1rem/1.5 system-ui}}pre{{white-space:pre-wrap;overflow-wrap:anywhere;font:inherit}}details{{margin:1em 0}}</style><h1>{0}</h1>",
                        escape(title)
                    )?;
                } else {
                    writeln!(out, "# {title}\n")?;
                }
                let mut query = WindowQuery {
                    id: q.id.clone(),
                    publication: Some(h.publication.0.clone()),
                    branch: q.branch.clone(),
                    leaf: q.leaf,
                    view: View::Visible,
                    anchor: Anchor::Start,
                    before: None,
                    limit: Some(32),
                    request_id: None,
                    show_cost: Some(display.show_cost),
                    estimate_cost: Some(display.estimate_cost),
                };
                loop {
                    let page = self.chat_window(&query, job)?;
                    for row in page["rows"].as_array().expect("reader window rows") {
                        let role = row["role"].as_str().or(row["kind"].as_str()).unwrap_or("");
                        if html {
                            writeln!(out, "<section><h2>{}</h2>", escape(locale.label(role)))?;
                        } else {
                            writeln!(out, "## {}\n", locale.label(role))?;
                        }
                        let full;
                        let parts = if row["truncated"] == true {
                            full = self.full_export_parts(&h, row)?;
                            &full
                        } else {
                            &row["parts"]
                        };
                        for p in parts.as_array().into_iter().flatten() {
                            let kind = p["kind"].as_str().unwrap_or("text");
                            let text = p["text"].as_str().unwrap_or("");
                            if kind == "thinking" && !show_thinking {
                                continue;
                            }
                            if matches!(kind, "callInput" | "callResult" | "thinking") {
                                let label = if kind == "callInput" {
                                    text.lines().next().unwrap_or(kind)
                                } else {
                                    locale.label(kind)
                                };
                                if html {
                                    let open = if collapse_tools { "" } else { " open" };
                                    writeln!(
                                        out,
                                        "<details{open}><summary>{}</summary><pre>{}</pre></details>",
                                        escape(label),
                                        escape(text)
                                    )?;
                                } else if kind == "thinking" {
                                    writeln!(out, "{text}\n")?;
                                } else {
                                    writeln!(out, "- {label}")?;
                                }
                            } else if html {
                                if role == "assistant" {
                                    super::export_html::markdown(out, text)?;
                                } else {
                                    writeln!(out, "<pre>{}</pre>", escape(text))?;
                                }
                            } else {
                                writeln!(out, "{text}\n")?;
                            }
                        }
                        for chip in row["attachments"].as_array().into_iter().flatten() {
                            let label = chip["name"]
                                .as_str()
                                .or(chip["kind"].as_str())
                                .unwrap_or("attachment");
                            if html {
                                self.export_chip(&h, row, chip, out, job)?;
                            } else {
                                writeln!(out, "[{label}]")?;
                            }
                        }
                        if html {
                            locale.annotations(row, out)?;
                            writeln!(out, "</section>")?;
                        }
                    }
                    let Some(rank) = page["next"].as_u64() else { break };
                    query.anchor = Anchor::Rank { rank };
                }
            }
        }
        // Leave the staged output cancellable after progress reaches the window, even for a
        // single short conversation. Each export pays the test delay only once.
        #[cfg(debug_assertions)]
        crate::debug_delay::wait(
            crate::debug_delay::configured("PATHKEEP_TEST_AGENT_EXPORT_DELAY_US"),
            &job.cancel,
        )?;
        self.screen_head(&q.id, Some(&h.publication.0))?;
        Ok(())
    }

    fn full_export_parts(&self, h: &ReaderPublication, row: &Value) -> Result<Value> {
        let source = row["sourceId"].as_str().unwrap_or_default();
        let ordinal = row["ordinal"]
            .as_u64()
            .ok_or_else(|| Error::Integrity("missing row ordinal".into()))?;
        let (version, _) = self.member_version(h, source)?;
        let (harness,format,payload)=self.store.read(Db::AgentHistoryReadModel,move|s|s.query_one(
            "SELECT m.harness,v.format_version,e.payload FROM ah_reader_version v JOIN ah_sessions m ON m.id=v.source JOIN ah_events e ON e.session_id=v.source AND e.projection_id=v.attempt WHERE v.token=?1 AND e.ordinal=?2",
            params![version,ordinal as i64],|r|Ok((r.get::<String>(0)?,r.get::<Option<String>>(1)?,r.get::<String>(2)?))))?;
        let context = if harness == "deepseek" {
            RecordContext::Deepseek { version: format.and_then(|s| s.parse().ok()) }
        } else {
            RecordContext::new(harness.parse()?)
        };
        let record =
            context.interpret(PhysicalOrdinal(ordinal), serde_json::from_str(&payload)?)?;
        Ok(json!(record.parts.into_iter().map(|p|{
            use crate::read_model::PartKind::*;
            json!({"kind":match p.kind { Text=>"text",Thinking=>"thinking",CallInput=>"callInput",CallResult=>"callResult"},"text":p.text})
        }).collect::<Vec<_>>()))
    }

    fn export_originals(
        &self,
        head: &ReaderPublication,
        out: &mut dyn Write,
        job: &JobControl,
    ) -> Result<()> {
        let mut tar = tar::Builder::new(out);
        let mut pending = vec![head.clone()];
        let mut seen = HashSet::new();
        while let Some(h) = pending.pop() {
            job.cancel.check()?;
            if !seen.insert(h.conversation_id.clone()) {
                continue;
            }
            let mut after = None;
            loop {
                let members = self.reader_sources(&h, after.as_deref(), 64)?;
                if members.is_empty() {
                    break;
                }
                after = members.last().map(|m| m.source_id.clone());
                for m in members {
                    let mut header = tar::Header::new_gnu();
                    header.set_size(m.bytes);
                    header.set_mode(0o600);
                    let mut reader = self.raw_reader(m.revision)?.with_scan_control(job);
                    use sha2::{Digest, Sha256};
                    let directory = hex::encode(Sha256::digest(m.root.as_bytes()));
                    let path = Path::new(&m.harness).join(directory).join(&m.relative_path);
                    tar.append_data(&mut header, path, &mut reader)?;
                    if reader.read(&mut [0])? != 0 {
                        return Err(Error::Integrity(
                            "original file exceeds its published size".into(),
                        ));
                    }
                }
            }
            let mut after = None;
            loop {
                let children = self.reader_children(&h, after.as_deref(), 64)?;
                if children.is_empty() {
                    break;
                }
                after = children.last().cloned();
                for id in children {
                    pending.push(self.reader_header(&id)?);
                }
            }
        }
        tar.finish()?;
        Ok(())
    }
}

pub(super) fn escape(text: &str) -> String {
    text.replace('&', "&amp;").replace('<', "&lt;").replace('>', "&gt;").replace('"', "&quot;")
}
