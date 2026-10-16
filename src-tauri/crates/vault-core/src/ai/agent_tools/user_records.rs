//! Responsibilities: expose complete starred-page and annotation pages to the agent.
//! Not responsible for: argument repair, search, intelligence rebuilding or mutations.
//! Why: typed preflight rejects unknown fields before a tool call is replayed.
//! Failure modes: cursors disappear, note previews lose content or long output splits UTF-8.
use super::*;

// ---------------------------------------------------------------------------
// Tool 2: list_stars
// ---------------------------------------------------------------------------

/// JSON arguments for `list_stars`.
// An argument the tool does not know is refused, not dropped: local models through LM Studio
// sometimes garble a name (`end_date2026-10-09`), and dropping it searched the wrong dates.
#[derive(Debug, Default, Deserialize)]
#[serde(deny_unknown_fields)]
struct ListStarsArgs {
    sort: Option<String>,
    limit: Option<usize>,
    cursor: Option<String>,
    kind: Option<crate::StarEntityKind>,
}

/// Lists the user's starred (favorited) pages (W-AI-9 WPB-2).
pub struct ListStarsTool;

const LIST_STARS_DESCRIPTION: &str = "List the user's starred (favorited) pages. Returns entity keys, domains, titles, visit counts, and when each was starred. Use this to answer questions about the user's favorites or bookmarks.";

impl AgentTool for ListStarsTool {
    fn validate_arguments(&self, args: &Value) -> Result<()> {
        super::super::tool_arguments::validate_optional_field::<usize>(args, "limit")?;
        serde_json::from_value::<ListStarsArgs>(args.clone())?;
        Ok(())
    }

    fn name(&self) -> &str {
        "list_stars"
    }

    fn definition(&self) -> LlmToolDef {
        LlmToolDef {
            name: "list_stars".to_string(),
            description: LIST_STARS_DESCRIPTION.to_string(),
            parameters: json!({
                "type": "object",
                "properties": {
                    "cursor": {"type":["string","null"],"description":"nextCursor from the previous page"},
                    "kind": {"type":["string","null"],"enum":["url","domain",null]},
                    "sort": {
                        "type": "string",
                        "enum": ["recent", "most_revisited"],
                        "description": "Sort order."
                    },
                    "limit": {
                        "type": "integer",
                        "description": "Page size (default 50, max 500)."
                    }
                }
            }),
        }
    }

    async fn call(&self, args: Value, context: &AgentToolContext) -> Result<ToolOutcome> {
        let parsed: ListStarsArgs = serde_json::from_value(args)
            .map_err(|error| anyhow::anyhow!("invalid list_stars arguments: {error}"))?;
        let sort = match parsed.sort.as_deref() {
            Some("most_revisited") => StarSort::MostRevisited,
            _ => StarSort::RecentlyStarred,
        };
        let limit = parsed.limit.unwrap_or(50).clamp(1, 500);
        let page = crate::stars::list_stars(
            &context.vault,
            parsed.kind,
            sort,
            Some(limit),
            parsed.cursor,
        )?;
        let items = &page.items;
        if items.is_empty() {
            return Ok(ToolOutcome {
                model_text: format!(
                    "list_stars: no starred pages found.\n{}",
                    serde_json::to_string(&page)?
                ),
                ..ToolOutcome::default()
            });
        }
        let mut text = format!(
            "list_stars: {} starred item(s). {} nextCursor: {}",
            items.len(),
            if page.next_cursor.is_some() {
                "More stars exist; continue with this cursor."
            } else {
                "End of list."
            },
            serde_json::to_string(&page.next_cursor)?
        );
        for item in items {
            text.push_str(&format!(
                "\n- {key} | {domain} — {title} ({visits}x, starred {at})",
                key = item.entity_key,
                domain = item.domain,
                title = if item.title.is_empty() { "(untitled)" } else { &item.title },
                visits = item.visit_count,
                at = item.starred_at,
            ));
        }
        if page.next_cursor.is_some() {
            text.push_str("\nMore stars exist; pass nextCursor to continue.");
        }
        text.push_str(&format!("\n{}", serde_json::to_string(&page)?));
        Ok(ToolOutcome { model_text: text, ..ToolOutcome::default() })
    }
}

// ---------------------------------------------------------------------------
// Tool 3: list_annotations
// ---------------------------------------------------------------------------

/// JSON arguments for `list_annotations`.
// An argument the tool does not know is refused, not dropped: local models through LM Studio
// sometimes garble a name (`end_date2026-10-09`), and dropping it searched the wrong dates.
#[derive(Debug, Default, Deserialize)]
#[serde(deny_unknown_fields)]
struct ListAnnotationsArgs {
    output_cursor: Option<String>,
    query: Option<String>,
    limit: Option<usize>,
    cursor: Option<String>,
}

/// Lists or searches the user's per-URL annotations (W-AI-9 WPB-3).
pub struct ListAnnotationsTool;

const LIST_ANNOTATIONS_DESCRIPTION: &str = "List or search the user's annotations (notes and tags on URLs). Use this to find pages the user has tagged or noted.";

impl AgentTool for ListAnnotationsTool {
    fn validate_arguments(&self, args: &Value) -> Result<()> {
        super::super::tool_arguments::validate_optional_field::<usize>(args, "limit")?;
        serde_json::from_value::<ListAnnotationsArgs>(args.clone())?;
        Ok(())
    }

    fn name(&self) -> &str {
        "list_annotations"
    }

    fn definition(&self) -> LlmToolDef {
        LlmToolDef {
            name: "list_annotations".to_string(),
            description: LIST_ANNOTATIONS_DESCRIPTION.to_string(),
            parameters: json!({
                "type": "object",
                "properties": {
                    "output_cursor":{"type":"string","description":"If a response has nextOutputCursor, pass it here with the same row cursor and filters until the full text is consumed."},
                    "query": {
                        "type": "string",
                        "description": "Optional text to search within notes."
                    },
                    "limit": {
                        "type": "integer",
                        "description": "Page size (default 50, max 500)."
                    },
                    "cursor": {"type":"string","description":"The next cursor returned by the previous page."}
                }
            }),
        }
    }

    async fn call(&self, args: Value, context: &AgentToolContext) -> Result<ToolOutcome> {
        let parsed: ListAnnotationsArgs = serde_json::from_value(args)
            .map_err(|error| anyhow::anyhow!("invalid list_annotations arguments: {error}"))?;
        let limit = Some(parsed.limit.unwrap_or(50).clamp(1, 500));
        let query_str = parsed.query.as_deref().unwrap_or("").trim();
        let items = if query_str.is_empty() {
            crate::annotations::list_annotations(&context.vault, limit, parsed.cursor)?
        } else {
            crate::annotations::search_annotations(&context.vault, query_str, limit, parsed.cursor)?
        };
        let next_cursor = items.next_cursor;
        let items = items.items;
        if items.is_empty() {
            let qualifier = if query_str.is_empty() {
                String::new()
            } else {
                format!(" matching \"{query_str}\"")
            };
            return Ok(ToolOutcome {
                model_text: format!("list_annotations: no annotations found{qualifier}."),
                ..ToolOutcome::default()
            });
        }
        let mut text = format!("list_annotations: {} annotation(s).", items.len());
        if let Some(cursor) = next_cursor {
            text.push_str(&format!("\nNext cursor: {cursor}"));
        }
        for item in &items {
            let tags_str = if item.tags.is_empty() {
                String::new()
            } else {
                format!(" [{}]", item.tags.join(", "))
            };
            text.push_str(&format!(
                "\n- {url}{tags} — {notes} (updated {at})",
                url = item.url,
                tags = tags_str,
                notes = item.notes,
                at = item.updated_at,
            ));
        }
        Ok(ToolOutcome {
            model_text: crate::ai::report_pages::output(&text, parsed.output_cursor.as_deref())?,
            ..ToolOutcome::default()
        })
    }
}
