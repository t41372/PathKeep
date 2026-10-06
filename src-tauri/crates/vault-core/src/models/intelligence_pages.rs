//! Responsibilities: carry cursor requests for complete queue history.
//! Not responsible for: queue transitions or database ownership.
//! Failure modes: the status preview becomes a lifetime cap or timestamp ties lose jobs.
use super::IntelligenceJobCursor;
use serde::{Deserialize, Serialize};
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ListAiJobsRequest {
    pub cursor: Option<i64>,
    pub limit: Option<u32>,
}
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ListIntelligenceJobsRequest {
    pub cursor: Option<IntelligenceJobCursor>,
    pub limit: Option<u32>,
}
