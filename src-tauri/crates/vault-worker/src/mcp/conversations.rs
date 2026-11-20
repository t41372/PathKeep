//! Read-only, consent-gated MCP calls. Audit records identify the request and number of
//! returned rows. Transcripts and provider keys stay out of the audit ledger.
use super::record_mcp_query_run;
use anyhow::Result;
use rmcp::{schemars, schemars::JsonSchema};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use vault_core::{
    AiCapability, Vault,
    ai::conversations::{PassageSearch, consent, search},
};

#[derive(Deserialize, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct Search {
    query: String,
    limit: Option<usize>,
    offset: Option<usize>,
}
#[derive(Deserialize, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct Record {
    source_id: String,
    ordinal: u64,
    chunk: Option<usize>,
}
#[derive(Deserialize, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct Read {
    record: Option<Record>,
    id: String,
    start: Option<u64>,
    limit: Option<usize>,
    publication: Option<String>,
}
pub(crate) fn search_agent(v: &Vault, q: Search) -> Result<Value> {
    let query = q.query;
    let page = search(
        v,
        AiCapability::Mcp,
        PassageSearch { query: query.clone(), limit: q.limit, offset: q.offset },
    )?;
    record_mcp_query_run(
        v,
        "search-conversations",
        &[],
        &[],
        json!({"query":query,"offset":q.offset.unwrap_or(0),"returned":page.items.len(),"total":page.total}),
    )?;
    consent(v, AiCapability::Mcp)?;
    Ok(serde_json::to_value(page)?)
}
pub(crate) fn read_agent(v: &Vault, q: Read) -> Result<Value> {
    consent(v, AiCapability::Mcp)?;
    let archive = v.agent_history()?;
    archive.ensure_ai_visible(&q.id)?;
    if let Some(record) = q.record {
        let result = archive.ai_record_chunks(
            &q.id,
            q.publication.as_deref(),
            &record.source_id,
            record.ordinal,
            record.chunk.unwrap_or(0),
            q.limit.unwrap_or(12).clamp(1, 50),
        )?;
        record_mcp_query_run(
            v,
            "read-conversation",
            &[],
            &[],
            json!({"conversationId":q.id,"sourceId":record.source_id,"ordinal":record.ordinal,"chunk":record.chunk.unwrap_or(0),"returned":result["chunks"].as_array().map_or(0,Vec::len)}),
        )?;
        archive.ensure_ai_visible(&q.id)?;
        consent(v, AiCapability::Mcp)?;
        return Ok(result);
    }
    let query = serde_json::from_value(json!({"id":q.id,"publication":q.publication,
        "anchor":{"kind":"rank","rank":q.start.unwrap_or(0)},"limit":q.limit.unwrap_or(12).clamp(1,50)}))?;
    let window = archive.chat_window(&query, &Default::default())?;
    let rows=window["rows"].as_array().unwrap().iter().map(|r|json!({"sourceId":r["sourceId"],"ordinal":r["ordinal"],"role":r["role"],"parts":r["parts"],"truncated":r["truncated"]})).collect::<Vec<_>>();
    record_mcp_query_run(
        v,
        "read-conversation",
        &[],
        &[],
        json!({"conversationId":q.id,"first":window["first"],"returned":rows.len()}),
    )?;
    archive.ensure_ai_visible(&q.id)?;
    consent(v, AiCapability::Mcp)?;
    Ok(
        json!({"id":q.id,"publication":window["publication"],"first":window["first"],"next":window["next"],"total":window["total"],"rows":rows}),
    )
}
