//! Manage Kinds snapshots and local retry. Physical size is optional so shell
//! navigation never waits for filesystem accounting of derived indexes.
use super::{ListKinds, PolicyChange, Refusal, admitted_config, backups};
use crate::VaultSession;
use anyhow::Result;
use serde::Serialize;
use vault_core::{DataKind, KindState};
use vault_store::Db;

/// Manage Kinds can distinguish retained evidence, device policy and role failure.
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct KindRow {
    kind: DataKind,
    order: u8,
    state: KindState,
    availability: &'static str,
    has_saved_data: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    archive_count: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    archive_bytes: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    has_backup_source: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    source_review_required: Option<bool>,
    count_unit: &'static str,
    coverage: Coverage,
    #[serde(skip_serializing_if = "Option::is_none")]
    first_backup: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    active_task_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    failure: Option<RoleFailure>,
}
#[derive(Debug, Serialize)]
struct RoleFailure {
    role: &'static str,
    stage: &'static str,
    reason: &'static str,
}
impl From<&vault_store::StoreOpenFailure> for RoleFailure {
    fn from(failure: &vault_store::StoreOpenFailure) -> Self {
        use vault_store::{Error, OpenStage};
        Self {
            role: match failure.role {
                Db::AgentHistoryEvidence => "evidence",
                Db::AgentHistoryReadModel => "read-model",
                _ => "shared",
            },
            stage: match failure.stage {
                OpenStage::Preflight => "preflight",
                OpenStage::Open => "open",
                OpenStage::KeyCheck => "key-check",
                OpenStage::Migrate => "migrate",
                OpenStage::Recover => "recover",
            },
            reason: match &failure.cause {
                Error::MissingStore(_) => "archive-store-missing",
                Error::WrongKey { .. } => "wrong-key",
                Error::Corrupt { .. } => "integrity-failed",
                Error::SchemaTooNew { .. } => "schema-too-new",
                Error::Busy { .. } => "archive-busy",
                _ => "kind-unavailable",
            },
        }
    }
}
#[derive(Debug, Serialize)]
struct Coverage {
    state: &'static str,
    reasons: Vec<&'static str>,
}
/// The shell renders both registered kinds from one policy snapshot.
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct KindList {
    pub kinds: Vec<KindRow>,
    pub policy_token: String,
}
/// Return the snapshot and the affected work to the initiating window.
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Transition {
    #[serde(flatten)]
    pub snapshot: KindList,
    pub affected_tasks: Vec<String>,
}

/// Shell snapshot from committed facts on the current owner. A damaged role returns unknown counts.
pub fn list(session: &VaultSession) -> Result<KindList> {
    list_with(session, ListKinds::default())
}

pub fn list_with(session: &VaultSession, request: ListKinds) -> Result<KindList> {
    let paths = vault_core::project_paths()?;
    let config = admitted_config(&paths)?;
    let vault =
        if config.initialized { Some(session.vault().map_err(anyhow::Error::msg)?) } else { None };
    let mut kinds = Vec::new();
    for (order, kind) in [DataKind::Browsing, DataKind::AgentHistory].into_iter().enumerate() {
        let agent = kind == DataKind::AgentHistory;
        let unavailable = agent
            && vault.as_ref().is_some_and(|v| {
                !v.available_kinds().contains(&vault_layout::DataScope::AgentHistory)
            });
        let failure = if agent {
            vault
                .as_ref()
                .and_then(|v| v.conversation_availability().err())
                .map(|f| RoleFailure::from(f.as_ref()))
        } else {
            None
        };
        let (count, first_backup) = match &vault {
            _ if unavailable => (None, None),
            None => (Some(0), agent.then(|| "not-started".into())),
            Some(vault) if agent => {
                session.conversations.reconcile_terminal(vault)?;
                let files = vault.store().read(Db::AgentHistoryEvidence, |tx| {
                    tx.query_one(
                        "SELECT count(*) FROM ah_sources WHERE latest_revision_id IS NOT NULL",
                        (),
                        |r| Ok(r.get::<i64>(0)?),
                    )
                })?;
                let first = backups::first_backup(vault, &config.capture_policy)?;
                (Some(files), Some(first))
            }
            Some(vault) => {
                let visits =
                    vault.store().read(Db::Archive, |tx| {
                        tx.query_one(
                    "SELECT COALESCE((SELECT value FROM archive_totals WHERE name='visits'),0)",
                    (),|row|Ok(row.get::<i64>(0)?))
                    })?;
                (Some(visits), None)
            }
        };
        let has_backup_source = if agent && !unavailable {
            Some(match &vault {
                Some(v) => backups::has_grant(v, &config.capture_policy)?,
                None => false,
            })
        } else {
            None
        };
        kinds.push(KindRow {
            kind,
            order: order as u8,
            state: match &vault {
                Some(v) if agent && !unavailable => v.agent_visibility(&config.capture_policy)?,
                _ => config.capture_policy.state(kind),
            },
            archive_bytes: match &vault {
                Some(v) if request.include_archive_bytes && !unavailable => Some(
                    if agent {
                        v.conversation_archive_bytes()?
                    } else {
                        vault_core::archive::browsing_archive_bytes(v)?
                    }
                    .to_string(),
                ),
                _ => None,
            },
            source_review_required: (config.initialized
                && config.capture_policy.state(kind) == KindState::On
                && match kind {
                    DataKind::AgentHistory => has_backup_source == Some(false),
                    DataKind::Browsing => config.selected_profile_ids.is_empty(),
                })
            .then_some(true),
            has_backup_source,
            availability: if unavailable {
                "unavailable"
            } else if vault.is_some() {
                "available"
            } else {
                "not-initialized"
            },
            failure,
            archive_count: count.map(|n| n.to_string()),
            count_unit: if agent { "source-files" } else { "visits" },
            has_saved_data: count.map(|n| n > 0),
            coverage: Coverage {
                state: if unavailable { "unavailable" } else { "complete" },
                reasons: if unavailable { vec!["kind-unavailable"] } else { vec![] },
            },
            first_backup: first_backup.map(|s| backups::result_name(&s).to_owned()),
            active_task_id: if agent
                && !unavailable
                && config.capture_policy.agent_history != KindState::Off
            {
                backups::active_id(session)
            } else {
                None
            },
        });
    }
    let current = admitted_config(&paths)?;
    let replaced = vault.as_ref().is_some_and(|original| {
        session
            .current_vault()
            .is_none_or(|now| !std::sync::Arc::ptr_eq(now.store(), original.store()))
    });
    if replaced || current.initialized != config.initialized {
        return Err(Refusal::Replaced.into());
    }
    if current.capture_policy != config.capture_policy {
        return Err(Refusal::PolicyChanged.into());
    }
    Ok(KindList { kinds, policy_token: config.capture_policy.token() })
}

/// Retry the two conversation roles on the current owner. A continuing role
/// failure is part of the snapshot, so Manage Kinds can display the new cause.
pub fn retry_agent_store(
    session: &VaultSession,
    emit: impl FnOnce(PolicyChange),
) -> Result<KindList> {
    let paths = vault_core::project_paths()?;
    if !admitted_config(&paths)?.initialized {
        return Err(Refusal::NotInitialized.into());
    }
    let vault = session.vault().map_err(anyhow::Error::msg)?;
    let before = vault.conversation_availability().is_ok();
    match vault.retry_conversations() {
        Ok(()) if !before => {
            if let Err(error) = vault.agent_history()
                && vault.conversation_availability().is_ok()
            {
                return Err(error.into());
            }
        }
        Ok(()) => {}
        Err(vault_store::Error::Cancelled { .. } | vault_store::Error::Busy { .. }) => {
            return Err(Refusal::Busy.into());
        }
        Err(vault_store::Error::Open(failure))
            if matches!(failure.role, Db::AgentHistoryEvidence | Db::AgentHistoryReadModel) => {}
        Err(vault_store::Error::Closed) => return Err(Refusal::Replaced.into()),
        Err(error) => return Err(error.into()),
    }
    if !before && vault.conversation_availability().is_ok() {
        super::prepare_view(&vault, &admitted_config(&paths)?.capture_policy)?;
    }
    let snapshot = list(session)?;
    if session.current_vault().is_none_or(|v| !std::sync::Arc::ptr_eq(v.store(), vault.store())) {
        return Err(Refusal::Replaced.into());
    }
    if before != vault.conversation_availability().is_ok() {
        emit(PolicyChange {
            policy_token: snapshot.policy_token.clone(),
            changed_kinds: vec![DataKind::AgentHistory],
        });
    }
    Ok(snapshot)
}
