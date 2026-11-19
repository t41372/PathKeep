//! Hint acknowledgements are independent device settings. Saving one preserves
//! the capture token of an enable dialog already open in another window.
use super::{admitted_config, backups::Events};
use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;
use vault_core::{AgentHint, CapturePolicy, ControlGate};

/// Dismiss only a known device hint.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DismissHint {
    pub hint: AgentHint,
}
/// The same acknowledgement is safe to replay after a lost reply.
#[derive(Serialize)]
pub struct HintAcknowledgement {
    hint: AgentHint,
    dismissed: bool,
}

pub(super) fn settings_token(policy: &CapturePolicy, hints: &BTreeSet<AgentHint>) -> String {
    let token = policy.token();
    if hints.is_empty() {
        return token;
    }
    vault_core::utils::sha256_hex(
        &serde_json::to_vec(&(token, hints)).expect("device settings are JSON"),
    )
}

/// Store one device acknowledgement without changing capture consent.
pub fn dismiss_hint(request: DismissHint, emit: Events) -> Result<HintAcknowledgement> {
    let paths = vault_core::project_paths()?;
    let gate = ControlGate::acquire(&paths);
    let mut config = admitted_config(&paths)?;
    if config.dismissed_agent_hints.insert(request.hint) {
        vault_core::save_config(&gate, &config)?;
        drop(gate);
        admitted_config(&paths)?;
        emit(
            "pathkeep://agent-settings-changed",
            serde_json::json!({
                "settingsToken":settings_token(&config.capture_policy,&config.dismissed_agent_hints),
                "domains":["hints"]
            }),
        );
    }
    Ok(HintAcknowledgement { hint: request.hint, dismissed: true })
}
