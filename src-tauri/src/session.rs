//! Responsibilities: expose the shared owner session to desktop commands.
//! Not responsible for: storing keys or duplicating worker session resolution.
//! Failure modes: facade diverges from owner key resolution. Tests: worker session tests.
pub(crate) use vault_worker::VaultSession as SessionState;
pub(crate) fn update_session_key(state: &SessionState, key: Option<String>) -> Result<(), String> {
    state.update_password(key)
}
pub(crate) fn session_key(state: &SessionState) -> Result<Option<String>, String> {
    state.get_key()
}
