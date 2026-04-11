//! In-memory desktop session state.
//!
//! This module intentionally stores only ephemeral values that should disappear
//! when the desktop process exits. Today that is just the current database key
//! cached for the unlocked session. Persisted secrets still belong in native
//! keyrings or the archive itself.

use std::sync::{Arc, Mutex};

/// Shared in-process state that Tauri commands can borrow safely.
#[derive(Clone, Default)]
pub(crate) struct SessionState {
    database_key: Arc<Mutex<Option<String>>>,
}

impl SessionState {
    /// Returns the current session-scoped database key, if the UI has one.
    pub(crate) fn get_key(&self) -> Option<String> {
        self.database_key.lock().ok().and_then(|guard| guard.clone())
    }
}

/// Replaces the transient database key cached for this desktop session.
pub(crate) fn update_session_key(
    state: &SessionState,
    database_key: Option<String>,
) -> Result<(), String> {
    *state.database_key.lock().map_err(|error| error.to_string())? = database_key;
    Ok(())
}

/// Convenience accessor used by the worker bridge and tests.
pub(crate) fn session_key(state: &SessionState) -> Option<String> {
    state.get_key()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn session_helpers_round_trip_database_key() {
        let session = SessionState::default();
        assert_eq!(session_key(&session), None);

        update_session_key(&session, Some("abc".to_string())).expect("set key");
        assert_eq!(session_key(&session), Some("abc".to_string()));

        update_session_key(&session, None).expect("clear key");
        assert_eq!(session_key(&session), None);
    }
}
