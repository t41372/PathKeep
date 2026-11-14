//! Responsibilities: describe one durable index phase with an explicit unit and known/unknown total.
//! Not responsible for: candidate selection, persistence or estimating time across phases.
use serde::{Deserialize, Serialize};

/// A completed page scan does not imply that search vectors or publication have finished.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum IndexBuildPhase {
    #[default]
    Planning,
    Sources,
    Visits,
    Searches,
    Projection,
    Publishing,
}

/// Both index screens consume this same measurement; visit outcomes are separate from scan progress.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct IndexBuildProgress {
    pub phase: IndexBuildPhase,
    pub done: u64,
    pub total: Option<u64>,
    pub new_visits: u64,
    pub updated_visits: u64,
}
