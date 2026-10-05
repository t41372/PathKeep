//! Responsibilities: own the app Vault, separate legacy password and data key,
//! coordinate startup/open/close, and resolve backend keychain unlocks.
//! Not responsible for: UI commands, IPC framing or App Lock authentication.
//! Failure modes: raw keys escape to readers, opens race or fail fast, an unchecked
//! password replaces an envelope, App Lock drops secrets, store I/O holds the mutex,
//! startup loses a typed password, panic strands waiters, or status reads prompt the keychain.
//! Tests: session::tests and owner_requests tests.
mod open_result;
use open_result::OpenResult;
use std::sync::{Arc, Condvar, Mutex};
use vault_core::{AppConfig, ArchiveMode, ProjectPaths, Vault};
use vault_crypto::DataKey;
use zeroize::Zeroizing;

#[derive(Debug, Default, PartialEq)]
enum Transition {
    #[default]
    Idle,
    Opening,
    Closing,
}
#[derive(Default)]
struct Session {
    password: Option<Zeroizing<String>>,
    data_key: Option<Arc<DataKey>>,
    vault: Option<Vault>,
    owner: Option<Arc<vault_store::OwnerLock>>,
    transition: Transition,
    result: Arc<OpenResult>,
    last_error: Option<String>,
}

/// One session is shared by desktop commands and the owner's IPC service.
#[derive(Clone, Default)]
pub struct VaultSession {
    inner: Arc<(Mutex<Session>, Condvar)>,
}
impl VaultSession {
    /// Legacy SQLCipher readers require the exact typed password until area 6.
    /// Call from a blocking thread: a read arriving during startup waits for open.
    pub fn get_key(&self) -> Result<Option<String>, String> {
        let (lock, ready) = &*self.inner;
        let mut state = lock.lock().expect("session state");
        while state.transition == Transition::Opening {
            state = ready.wait(state).expect("session state");
        }
        Ok(state.password.as_ref().map(|value| value.to_string()))
    }

    /// Status reads inspect an existing handle without opening storage or the keychain.
    pub fn current_vault(&self) -> Option<Vault> {
        self.inner.0.lock().expect("session state").vault.clone().filter(|v| v.store().is_open())
    }

    /// Ported writes may proceed without the password. Their optional legacy
    /// follow-up work must report migration unavailability in that case.
    pub fn typed_password(&self) -> Option<String> {
        self.inner.0.lock().expect("session state").password.as_ref().map(|value| value.to_string())
    }

    pub fn reserve_owner(&self, owner: Arc<vault_store::OwnerLock>) {
        self.inner.0.lock().expect("session state").owner = Some(owner);
    }

    /// Mark startup before spawning its blocking job, so immediate UI reads wait.
    pub fn begin_startup(&self) {
        let mut state = self.inner.0.lock().expect("session state");
        state.result = Arc::default();
        state.transition = Transition::Opening;
    }
    pub fn abort_startup(&self, reason: String) {
        let _ = self.finish_open(Err(reason));
    }
    pub fn finish_startup(&self) -> Result<Vault, String> {
        self.complete_open(|| self.resolve_background(true))
    }
    fn resolve_background(&self, keychain: bool) -> Result<Vault, String> {
        let paths = vault_core::project_paths().map_err(|e| e.to_string())?;
        let config = vault_core::load_config(&paths).map_err(|e| e.to_string())?;
        self.resolve_store(paths, config, keychain)
    }
    pub fn vault(&self) -> Result<Vault, String> {
        let paths = vault_core::project_paths().map_err(|e| e.to_string())?;
        let config = vault_core::load_config(&paths).map_err(|e| e.to_string())?;
        vault_core::ensure_app_lock_unlocked(&paths, &config).map_err(|e| e.to_string())?;
        self.load_vault(paths, config)
    }
    pub fn background_vault(&self) -> Result<Vault, String> {
        self.open(|| self.resolve_background(false))
    }
    fn load_vault(&self, paths: ProjectPaths, config: AppConfig) -> Result<Vault, String> {
        self.open(|| self.resolve_store(paths, config, false))
    }
    fn resolve_store(
        &self,
        paths: ProjectPaths,
        config: AppConfig,
        keychain: bool,
    ) -> Result<Vault, String> {
        let (current, password, key) = {
            let state = self.inner.0.lock().expect("session state");
            (state.vault.clone(), state.password.clone(), state.data_key.clone())
        };
        if let Some(vault) = current {
            return Ok(vault.with_config(config));
        }
        if !config.initialized {
            return Err("archive has not been initialized".into());
        }
        if config.archive_mode == ArchiveMode::Plaintext || password.is_some() || key.is_some() {
            return self.open_store(paths, config, password, key);
        }
        if keychain {
            self.open_keychain(paths, config)
        } else {
            Err(self
                .inner
                .0
                .lock()
                .expect("session state")
                .last_error
                .clone()
                .unwrap_or_else(|| "archive-password-required: unlock the archive first".into()))
        }
    }

    fn open(&self, work: impl FnOnce() -> Result<Vault, String>) -> Result<Vault, String> {
        self.open_with(work, false)
    }
    fn open_with(
        &self,
        work: impl FnOnce() -> Result<Vault, String>,
        credentials: bool,
    ) -> Result<Vault, String> {
        loop {
            let mut state = self.inner.0.lock().expect("session state");
            if state.transition == Transition::Closing {
                return Err("archive-busy: archive is closing".into());
            }
            if state.transition == Transition::Opening {
                let attempt = state.result.clone();
                drop(state);
                let result = attempt.wait();
                if !credentials {
                    return result;
                }
                // The supplied password still needs validation and publication, even
                // when an unrelated startup/keychain attempt just finished.
                continue;
            }
            state.result = Arc::default();
            state.transition = Transition::Opening;
            drop(state);
            return self.complete_open(work);
        }
    }
    fn complete_open(&self, work: impl FnOnce() -> Result<Vault, String>) -> Result<Vault, String> {
        let guard = OpeningGuard::new(self);
        guard.finish(work())
    }
    fn finish_open(&self, result: Result<Vault, String>) -> Result<Vault, String> {
        let mut state = self.inner.0.lock().expect("session state");
        state.last_error = result.as_ref().err().cloned();
        state.result.finish(result.clone());
        state.transition = Transition::Idle;
        self.inner.1.notify_all();
        result
    }
    pub fn open_password(
        &self,
        paths: ProjectPaths,
        config: AppConfig,
        password: String,
    ) -> Result<Vault, String> {
        self.open_with(
            || self.open_store(paths, config, Some(Zeroizing::new(password)), None),
            true,
        )
    }
    fn open_store(
        &self,
        paths: ProjectPaths,
        config: AppConfig,
        password: Option<Zeroizing<String>>,
        key: Option<Arc<DataKey>>,
    ) -> Result<Vault, String> {
        let remember = config.remember_database_key_in_keyring;
        self.open_store_with_policy(paths, config, password, key, remember)
    }
    fn open_store_with_policy(
        &self,
        paths: ProjectPaths,
        config: AppConfig,
        password: Option<Zeroizing<String>>,
        mut key: Option<Arc<DataKey>>,
        remember: bool,
    ) -> Result<Vault, String> {
        let (current, owner) = {
            let state = self.inner.0.lock().expect("session state");
            (state.vault.clone(), state.owner.clone())
        };
        let vault = if let Some(current) = current {
            if config.archive_mode == ArchiveMode::Encrypted
                && let Some(password) = &password
            {
                key = Some(Arc::new(
                    Vault::unlock_data_key(&paths, password).map_err(|e| e.to_string())?,
                ));
            }
            current.with_config(config.clone())
        } else if config.archive_mode == ArchiveMode::Plaintext {
            Vault::open_owned(paths, config.clone(), None, owner).map_err(|e| e.to_string())?
        } else if let Some(password) = &password {
            let (vault, data) = Vault::open_password(paths, config.clone(), password, owner)
                .map_err(|e| e.to_string())?;
            key = Some(Arc::new(data));
            vault
        } else {
            if !paths.app_root.join("archive/keys.envelope").is_file() {
                return Err(
                    "envelope-missing: the encrypted archive's key envelope is missing".into()
                );
            }
            let data = key.as_ref().ok_or("database key is required for encrypted archives")?;
            Vault::open_owned(paths, config.clone(), Some(data), owner)
                .map_err(|e| e.to_string())?
        };
        if remember && let Some(key) = &key {
            crate::security::store_data_key_in_keyring(key).map_err(|e| e.to_string())?;
        }
        let mut state = self.inner.0.lock().expect("session state");
        if password.is_some() {
            state.password = password;
        }
        if key.is_some() {
            state.data_key = key;
        }
        state.vault = Some(vault.clone());
        Ok(vault)
    }
    /// Auto-unlock never returns key material, even to the desktop facade.
    pub fn unlock_from_keychain(&self) -> Result<(), String> {
        self.open(|| {
            let paths = vault_core::project_paths().map_err(|e| e.to_string())?;
            let config = vault_core::load_config(&paths).map_err(|e| e.to_string())?;
            if let Some(vault) = self.inner.0.lock().expect("session state").vault.clone() {
                return Ok(vault.with_config(config));
            }
            self.open_keychain(paths, config)
        })?;
        Ok(())
    }
    fn open_keychain(&self, paths: ProjectPaths, config: AppConfig) -> Result<Vault, String> {
        let text = crate::read_database_key_from_keyring()
            .map_err(|e| format!("keychain-unavailable: {e}"))?
            .ok_or("keychain-key-missing: no archive key is stored")?;
        if paths.app_root.join("archive/keys.envelope").exists()
            && let Ok(key) = decode_data_key(&text)
        {
            match self.open_store_with_policy(
                paths.clone(),
                config.clone(),
                None,
                Some(Arc::new(key)),
                false,
            ) {
                Ok(vault) => Ok(vault),
                Err(key_error) => {
                    // Old passwords can themselves be 64 hexadecimal characters.
                    // Authenticate the envelope before treating a failed key as a password.
                    if Vault::unlock_data_key(&paths, &text).is_err() {
                        return Err(key_error);
                    }
                    self.open_store_with_policy(paths, config, Some(text), None, true)
                }
            }
        } else {
            self.open_store_with_policy(paths, config, Some(text), None, true)
        }
    }

    pub fn update_password(&self, password: Option<String>) -> Result<(), String> {
        let Some(password) = password else {
            return self.close_vault();
        };
        let paths = vault_core::project_paths().map_err(|e| e.to_string())?;
        let config = vault_core::load_config(&paths).map_err(|e| e.to_string())?;
        if config.initialized {
            self.open_password(paths, config, password)?;
        } else {
            self.inner.0.lock().expect("session state").password = Some(Zeroizing::new(password));
        }
        Ok(())
    }
    pub fn close_vault(&self) -> Result<(), String> {
        let (lock, ready) = &*self.inner;
        let mut state = lock.lock().expect("session state");
        while state.transition == Transition::Opening {
            state = ready.wait(state).expect("session state");
        }
        if state.transition == Transition::Closing {
            return Err("archive-busy: archive is closing".into());
        }
        state.transition = Transition::Closing;
        state.password = None;
        state.data_key = None;
        state.last_error = None;
        state.result = Arc::default();
        let vault = state.vault.take();
        drop(state);
        let result = vault.map_or(Ok(()), |vault| vault.store().close().map_err(|e| e.to_string()));
        lock.lock().expect("session state").transition = Transition::Idle;
        ready.notify_all();
        result
    }
}
struct OpeningGuard<'a> {
    session: &'a VaultSession,
    finished: bool,
}
impl<'a> OpeningGuard<'a> {
    fn new(session: &'a VaultSession) -> Self {
        Self { session, finished: false }
    }
    fn finish(mut self, result: Result<Vault, String>) -> Result<Vault, String> {
        self.finished = true;
        self.session.finish_open(result)
    }
}
impl Drop for OpeningGuard<'_> {
    fn drop(&mut self) {
        if !self.finished {
            let _ = self
                .session
                .finish_open(Err("archive-open-panicked: storage opening was interrupted".into()));
        }
    }
}

fn decode_data_key(text: &str) -> Result<DataKey, String> {
    if text.len() != 64 || !text.is_ascii() {
        return Err("invalid-keychain-key: invalid data key".into());
    }
    let mut key = Zeroizing::new([0; 32]);
    for (i, byte) in key.iter_mut().enumerate() {
        *byte = u8::from_str_radix(&text[i * 2..i * 2 + 2], 16)
            .map_err(|_| "invalid-keychain-key: invalid data key")?;
    }
    Ok(DataKey::from_bytes(key))
}
pub(crate) fn encode_data_key(key: &DataKey) -> Zeroizing<String> {
    use std::fmt::Write;
    let mut text = Zeroizing::new(String::with_capacity(64));
    for byte in key.expose_bytes() {
        write!(text, "{byte:02x}").unwrap();
    }
    text
}
#[cfg(test)]
mod tests;
