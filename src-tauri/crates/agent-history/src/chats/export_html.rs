//! Self-contained reader HTML. Only generated markup is active; transcript markup is escaped.
use super::{AttachmentQuery, export::escape};
use crate::{
    Archive, Result,
    read_model::{JobControl, ReaderPublication},
};
use base64::{Engine, engine::general_purpose::STANDARD};
use pulldown_cmark::{Event, Parser, Tag, TagEnd};
use serde_json::Value;
use std::io::Write;

pub(super) fn markdown(out: &mut dyn Write, text: &str) -> std::io::Result<()> {
    for event in Parser::new(text) {
        let tag = match event {
            Event::Start(Tag::Paragraph) => "<p>",
            Event::End(TagEnd::Paragraph) => "</p>",
            Event::Start(Tag::Heading { .. }) => "<h3>",
            Event::End(TagEnd::Heading(_)) => "</h3>",
            Event::Start(Tag::BlockQuote(_)) => "<blockquote>",
            Event::End(TagEnd::BlockQuote(_)) => "</blockquote>",
            Event::Start(Tag::CodeBlock(_)) => "<pre><code>",
            Event::End(TagEnd::CodeBlock) => "</code></pre>",
            Event::Start(Tag::List(None)) => "<ul>",
            Event::Start(Tag::List(Some(_))) => "<ol>",
            Event::End(TagEnd::List(true)) => "</ol>",
            Event::End(TagEnd::List(false)) => "</ul>",
            Event::Start(Tag::Item) => "<li>",
            Event::End(TagEnd::Item) => "</li>",
            Event::Start(Tag::Strong) => "<strong>",
            Event::End(TagEnd::Strong) => "</strong>",
            Event::Start(Tag::Emphasis) => "<em>",
            Event::End(TagEnd::Emphasis) => "</em>",
            Event::Start(Tag::Link { dest_url, .. }) => {
                if let Ok(url) = url::Url::parse(&dest_url)
                    && matches!(url.scheme(), "https" | "http")
                {
                    write!(out, "<a href=\"{}\" rel=\"noreferrer\">", escape(url.as_str()))?;
                } else {
                    out.write_all(b"<a>")?;
                }
                continue;
            }
            Event::End(TagEnd::Link) => "</a>",
            Event::SoftBreak => "\n",
            Event::HardBreak => "<br>",
            Event::Rule => "<hr>",
            Event::Code(code) => {
                write!(out, "<code>{}</code>", escape(&code))?;
                continue;
            }
            Event::Text(text) | Event::Html(text) | Event::InlineHtml(text) => {
                out.write_all(escape(&text).as_bytes())?;
                continue;
            }
            _ => continue,
        };
        out.write_all(tag.as_bytes())?;
    }
    Ok(())
}

impl Archive {
    pub(super) fn export_chip(
        &self,
        h: &ReaderPublication,
        row: &Value,
        chip: &Value,
        out: &mut dyn Write,
        job: &JobControl,
    ) -> Result<()> {
        let label = escape(chip["name"].as_str().or(chip["kind"].as_str()).unwrap_or("attachment"));
        if chip["availability"] == "missing" {
            writeln!(out, "<p>{label}</p>")?;
            return Ok(());
        }
        let mime = chip["mimeType"].as_str().unwrap_or("");
        let image = matches!(mime, "image/png" | "image/jpeg" | "image/gif" | "image/webp");
        if image {
            write!(out, "<img alt=\"{label}\" src=\"data:{mime};base64,")?;
        } else {
            write!(out, "<a download=\"{label}\" href=\"data:application/octet-stream;base64,")?;
        }
        let mut q = AttachmentQuery {
            request_id: None,
            id: h.conversation_id.clone(),
            publication: h.publication.0.clone(),
            source_id: chip["sourceId"]
                .as_str()
                .or(row["sourceId"].as_str())
                .unwrap_or_default()
                .into(),
            ordinal: chip["ordinal"].as_u64().or(row["ordinal"].as_u64()).unwrap_or_default(),
            key: chip["key"].as_str().unwrap_or_default().into(),
            offset: 0,
            limit: 48 * 1024,
        };
        loop {
            let mut page = self.chat_attachment(&q, job)?;
            let bytes: Vec<u8> = serde_json::from_value(page["bytes"].take())?;
            out.write_all(STANDARD.encode(bytes).as_bytes())?;
            let Some(next) = page["next"].as_u64() else { break };
            q.offset = next;
        }
        if image {
            writeln!(out, "\">")?;
        } else {
            writeln!(out, "\">{label}</a>")?;
        }
        Ok(())
    }
}

/// The window may pass its resolved language for a portable, offline reader.
#[derive(Clone, Copy, Default, serde::Deserialize)]
pub enum ExportLocale {
    #[default]
    #[serde(rename = "en")]
    English,
    #[serde(rename = "zh-CN")]
    Simplified,
    #[serde(rename = "zh-TW")]
    Traditional,
}
impl ExportLocale {
    fn words<'a>(self, en: &'a str, cn: &'a str, tw: &'a str) -> &'a str {
        match self {
            Self::English => en,
            Self::Simplified => cn,
            Self::Traditional => tw,
        }
    }
    pub(super) fn label(self, kind: &str) -> &str {
        match kind {
            "user" => self.words("You", "你", "你"),
            "assistant" => self.words("Assistant", "助理", "助理"),
            "callResult" => self.words("Tool result", "工具结果", "工具結果"),
            "thinking" => self.words("Thinking", "思考过程", "思考過程"),
            "attachment" => self.words("Attachment", "附件", "附件"),
            "compaction" => self.words("Context compacted", "上下文已压缩", "上下文已壓縮"),
            "context_edit" => self.words("Context edited", "上下文已编辑", "上下文已編輯"),
            "unresolved_parent" => {
                self.words("Earlier branch unresolved", "无法确认先前分支", "無法確認先前分支")
            }
            "apiError" => self.words("API error", "API 错误", "API 錯誤"),
            "retry" => self.words("Retry", "重试", "重試"),
            "interrupted" => self.words("Interrupted", "已中断", "已中斷"),
            _ => kind,
        }
    }
    pub(super) fn annotations(self, row: &Value, out: &mut dyn Write) -> std::io::Result<()> {
        if let Some(kind) = row["boundary"]["kind"].as_str() {
            write!(out, "<aside><strong>{}</strong>", escape(self.label(kind)))?;
            if let Some(n) = row["boundary"]["tokens_before"].as_u64() {
                write!(out, " · {n} tokens")?;
            }
            writeln!(out, "</aside>")?;
        }
        for notice in row["notices"].as_array().into_iter().flatten() {
            if let Some(code) = notice["code"].as_str() {
                writeln!(out, "<aside>{}</aside>", escape(self.label(code)))?;
            }
        }
        let footer = &row["footer"];
        if footer.is_null() {
            return Ok(());
        }
        let facts = &footer["facts"];
        let mut pieces = Vec::new();
        if let Some(turn) = row["turnNumber"].as_u64() {
            pieces.push(format!("{} {turn}", self.words("Turn", "轮次", "輪次")));
        }
        if let Some(model) = footer["model"].as_str() {
            pieces.push(escape(model));
        }
        if let Some(ms) = facts["agentMs"].as_u64() {
            pieces.push(format!("{ms} ms"));
        }
        if let Some(tokens) = facts["tokens"].as_object() {
            let total: u64 = ["input", "output", "cacheRead", "cacheWrite"]
                .iter()
                .filter_map(|k| tokens.get(*k)?.as_u64())
                .sum();
            pieces.push(format!("{total} tokens"));
        } else {
            pieces.push(self.words("Usage unknown", "用量未知", "用量未知").into());
        }
        if let Some(cost) = facts["recordedUsd"].as_f64() {
            pieces.push(format!("USD {cost}"));
        } else if let Some(cost) = facts["estimatedUsd"].as_f64() {
            pieces.push(format!("{} USD {cost}", self.words("Estimated", "估算", "估算")));
        }
        if footer["endReason"] == "interrupted" {
            pieces.push(self.label("interrupted").into());
        }
        writeln!(out, "<footer>{}</footer>", pieces.join(" · "))
    }
}
