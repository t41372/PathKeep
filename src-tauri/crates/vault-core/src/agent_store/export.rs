//! Responsibilities: render one Ask conversation as a Markdown file and suggest its name.
//! Not responsible for: where the file goes (the desktop asks with the system's save dialog,
//! never a path the interface names), writing it, the interface's words (it sends them), or
//! other export formats.
//! Failure modes: a page title from the web turns into a link or markup of its own, a link
//! with a script scheme, a suggested name with path separators or characters a file system
//! refuses.
use crate::{
    Vault,
    models::{
        AgentCitation, AgentMessage, ConversationExportLabels, ConversationMarkdown,
        ExportAgentConversationRequest,
    },
};
use anyhow::Result;
use std::fmt::Write as _;

/// The conversation's questions, answers and the pages each answer used, as the person saw
/// them; the thinking and searches behind an answer stay out. `None` when it is not saved.
pub fn conversation_markdown(
    vault: &Vault,
    request: &ExportAgentConversationRequest,
) -> Result<Option<ConversationMarkdown>> {
    let Some(conversation) = super::load_conversation(vault, &request.id)? else {
        return Ok(None);
    };
    let title = Some(conversation.summary.title.trim())
        .filter(|title| !title.is_empty())
        .unwrap_or(request.labels.untitled.trim());
    let markdown = render(title, &conversation.messages, &request.labels);
    let day = chrono::Local::now().format("%Y-%m-%d");
    Ok(Some(ConversationMarkdown {
        file_name: format!("{} {day}.md", file_name_part(title)),
        markdown,
    }))
}

fn render(title: &str, messages: &[AgentMessage], labels: &ConversationExportLabels) -> String {
    let mut out = format!("# {}\n", inline(title));
    for message in messages {
        let content = message.content.trim();
        if content.is_empty() {
            continue;
        }
        let who = if message.role == "user" { &labels.you } else { &labels.assistant };
        let _ = write!(out, "\n## {}\n\n{content}\n", inline(who));
        if !message.citations.is_empty() {
            let _ = writeln!(out, "\n{}:\n", inline(&labels.pages_used));
            for (n, citation) in message.citations.iter().enumerate() {
                let _ = writeln!(out, "{}. {}", n + 1, page(citation));
            }
        }
    }
    out
}

/// A page the answer used: a link when its address is a web address, its visit time after.
fn page(citation: &AgentCitation) -> String {
    let title =
        inline(citation.title.as_deref().filter(|t| !t.trim().is_empty()).unwrap_or(&citation.url));
    let link = if is_web(&citation.url) {
        format!("[{title}](<{}>)", citation.url.replace(['<', '>', '\n', '\r'], ""))
    } else {
        format!("{title} ({})", inline(&citation.url))
    };
    match chrono::DateTime::parse_from_rfc3339(&citation.visited_at) {
        Ok(at) => format!("{link} · {}", at.with_timezone(&chrono::Local).format("%Y-%m-%d %H:%M")),
        Err(_) => link,
    }
}

fn is_web(url: &str) -> bool {
    let url = url.trim_start().to_ascii_lowercase();
    url.starts_with("https://") || url.starts_with("http://")
}

/// Text from a web page or the model on one line, with Markdown's own characters escaped so
/// it reads as the text it is and cannot make a link, image or heading of its own.
fn inline(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    for ch in text.split_whitespace().collect::<Vec<_>>().join(" ").chars() {
        if r"\`*_[]<>~".contains(ch) {
            out.push('\\');
        }
        out.push(ch);
    }
    out
}

/// The title as part of a file name: no path separators or characters a file system refuses,
/// at most 60 characters.
fn file_name_part(title: &str) -> String {
    let cleaned: String = title
        .chars()
        .map(|ch| if ch.is_control() || r#"/\:*?"<>|"#.contains(ch) { ' ' } else { ch })
        .collect();
    let words = cleaned.split_whitespace().collect::<Vec<_>>().join(" ");
    let short: String = words.chars().take(60).collect();
    let short = short.trim_matches(|ch: char| ch == '.' || ch.is_whitespace()).to_owned();
    if short.is_empty() { "Ask".to_owned() } else { short }
}
