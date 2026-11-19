//! Interactive reads and guarded changes to device capture policy. This module
//! does not start a backup or grant access to an unreviewed source directory.
pub mod preview;
use crate::VaultSession;
use anyhow::Result;
use serde::{Deserialize, Serialize};
use vault_core::{
    AgentTool, AppConfig, CapturePolicy, ControlGate, DataKind, KindState, ProjectPaths,
};
use vault_store::Db;

/// Require an explicit empty request on both transports.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Empty {}
/// Carry the policy fingerprint the Manage Kinds control displayed.
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SetState {
    pub kind: DataKind,
    pub state: KindState,
    pub expected_policy_token: String,
}
/// Apply only the switches the user changed in this edit.
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SourcesPatch {
    pub expected_token: String,
    pub read_git: Option<bool>,
    #[serde(default)]
    pub tool_changes: std::collections::BTreeMap<AgentTool, bool>,
}

/// Stable domain causes shared by command replies and terminal preview frames.
#[derive(Debug, Clone, Copy)]
pub enum Refusal {
    LastActive,
    EnablePreview,
    PolicyChanged,
    Locked,
    ScopeDenied,
    EmptySelection,
    Destination,
    Cancelled,
    Busy,
    Replaced,
    RootChanged,
    Expired,
    NotInitialized,
}
impl Refusal {
    pub fn code(self) -> &'static str {
        match self {
            Self::LastActive => "last-active-kind",
            Self::EnablePreview => "enable-preview-required",
            Self::PolicyChanged => "policy-changed",
            Self::Locked => "lock-required",
            Self::ScopeDenied => "scope-denied",
            Self::EmptySelection => "source-selection-empty",
            Self::Destination => "destination-unavailable",
            Self::Cancelled => "cancelled",
            Self::Busy => "archive-busy",
            Self::Replaced => "archive-replaced",
            Self::RootChanged => "root-changed",
            Self::Expired => "preview-expired",
            Self::NotInitialized => "archive-uninitialized",
        }
    }
}
impl std::fmt::Display for Refusal {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.code())
    }
}
impl std::error::Error for Refusal {}

/// Called both at admission and before delivering private metadata.
pub(super) fn admitted_config(paths: &ProjectPaths) -> Result<AppConfig> {
    let config = vault_core::load_config(paths)?;
    if vault_core::app_lock_status(paths, &config)?.locked {
        return Err(Refusal::Locked.into());
    }
    Ok(config)
}

/// Windows refetch these kinds after the guarded write succeeds.
#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PolicyChange {
    pub policy_token: String,
    pub changed_kinds: Vec<DataKind>,
}
pub const POLICY_EVENT: &str = "pathkeep://data-kinds-changed";

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
    count_unit: &'static str,
    coverage: Coverage,
    #[serde(skip_serializing_if = "Option::is_none")]
    first_backup: Option<String>,
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
        let (count,first_backup)=match &vault {
            _ if unavailable=>(None,None),
            None=>(Some(0),agent.then(||"not-started".into())),
            Some(v) if agent=>v.store().read(Db::AgentHistoryEvidence,|s|{
                let count=s.query_one("SELECT count(*) FROM ah_sources WHERE latest_revision_id IS NOT NULL",(),|r|Ok(r.get::<i64>(0)?))?;
                let first=s.query_one("SELECT COALESCE((SELECT status FROM ah_backup_runs ORDER BY id LIMIT 1),'not-started')",(),|r|Ok(r.get::<String>(0)?))?;
                Ok((Some(count),Some(first)))
            })?,
            Some(v)=>(Some(v.store().read(Db::Archive,|s|s.query_one("SELECT COALESCE((SELECT value FROM archive_totals WHERE name='visits'),0)",(),|r|Ok(r.get::<i64>(0)?)))?),None)
        };
        kinds.push(KindRow {
            kind,
            order: order as u8,
            state: config.capture_policy.state(kind),
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
            first_backup,
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

/// Source controls share the same policy and comparison token as Manage Kinds.
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Settings {
    pub policy: CapturePolicy,
    pub policy_token: String,
}

/// Capture choices remain accessible to the unlocked manager while the kind is off.
pub fn settings() -> Result<Settings> {
    let policy = admitted_config(&vault_core::project_paths()?)?.capture_policy;
    Ok(Settings { policy_token: policy.token(), policy })
}

fn change(
    kind: DataKind,
    expected: &str,
    mutate: impl FnOnce(&mut CapturePolicy) -> Result<()>,
    emit: impl FnOnce(PolicyChange),
) -> Result<Settings> {
    let paths = vault_core::project_paths()?;
    let gate = ControlGate::acquire(&paths);
    let mut config = admitted_config(&paths)?;
    let before = config.capture_policy.token();
    if before != expected {
        return Err(Refusal::PolicyChanged.into());
    }
    mutate(&mut config.capture_policy)?;
    config.capture_policy.validate().map_err(|_| Refusal::LastActive)?;
    let token = config.capture_policy.token();
    if token != before {
        if !config.initialized {
            return Err(Refusal::NotInitialized.into());
        }
        vault_core::save_config(&gate, &config)?;
        preview::revoke(&paths.app_root);
        emit(PolicyChange { policy_token: token.clone(), changed_kinds: vec![kind] });
    }
    Ok(Settings { policy: config.capture_policy, policy_token: token })
}

/// The interactive transition cannot stand in for first-save consent.
pub fn set_state(
    session: &VaultSession,
    req: SetState,
    emit: impl FnOnce(PolicyChange),
) -> Result<Transition> {
    change(
        req.kind,
        &req.expected_policy_token,
        |policy| {
            let slot = match req.kind {
                DataKind::Browsing => &mut policy.browsing,
                DataKind::AgentHistory => &mut policy.agent_history,
            };
            if req.kind == DataKind::AgentHistory
                && *slot == KindState::Off
                && req.state != KindState::Off
            {
                return Err(Refusal::EnablePreview.into());
            }
            *slot = req.state;
            Ok(())
        },
        emit,
    )?;
    Ok(Transition { snapshot: list(session)?, affected_tasks: vec![] })
}

/// Tool switches narrow future capture. While off they are choices for the
/// enable dialog. A switch alone never grants a directory.
pub fn update_sources(req: SourcesPatch, emit: impl FnOnce(PolicyChange)) -> Result<Settings> {
    change(
        DataKind::AgentHistory,
        &req.expected_token,
        |policy| {
            for (tool, on) in req.tool_changes {
                if on {
                    if policy.agent_history != KindState::Off
                        && !policy.enabled_tools.contains(&tool)
                    {
                        return Err(Refusal::EnablePreview.into());
                    }
                    policy.enabled_tools.insert(tool);
                } else {
                    policy.enabled_tools.remove(&tool);
                }
            }
            if let Some(read_git) = req.read_git {
                policy.read_git = read_git
            }
            Ok(())
        },
        emit,
    )
}

/// Whole-settings callers cannot construct a policy that the transition would refuse.
pub fn validate_config_input(config: &AppConfig) -> Result<()> {
    config.capture_policy.validate().map_err(|_| Refusal::LastActive)?;
    Ok(())
}
