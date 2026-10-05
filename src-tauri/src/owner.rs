//! Responsibilities: reserve app ownership before startup and serve worker clients.
//! Not responsible for: database implementation, IPC framing or drawing windows.
//! Failure modes: second instance acquires lock; handoff loses arguments or foregrounds
//! background work; endpoint starts before session setup. Tests: owner_process.rs.
use crate::session::SessionState;
use std::{
    path::Path,
    sync::{Arc, Mutex},
};
use tauri::{AppHandle, Manager, Runtime};

#[derive(Clone, Default)]
pub(crate) struct LaunchState(Arc<Mutex<Vec<String>>>);

/// Single-instance callbacks run before the app setup closure in the losing process.
pub(crate) fn handoff<R: Runtime>(app: &AppHandle<R>, arguments: Vec<String>, _cwd: String) {
    let background = arguments.iter().any(|arg| {
        arg == "--background" || arg == crate::desktop_integration::LAUNCHED_AT_LOGIN_ARG
    });
    *app.state::<LaunchState>().0.lock().expect("launch state") = arguments;
    if !background {
        let app = app.clone();
        let handle = app.clone();
        let _ =
            app.run_on_main_thread(move || crate::desktop_integration::show_main_window(&handle));
    }
}

/// Retained in app state for its whole lifetime. Server joins before lock release.
pub(crate) struct OwnerService {
    _server: vault_owner_ipc::Server,
    _lock: Arc<vault_store::OwnerLock>,
}
impl OwnerService {
    pub(crate) fn start(
        root: &Path,
        lock: vault_store::OwnerLock,
        session: SessionState,
        launches: LaunchState,
        background: bool,
        startup_windows: usize,
    ) -> anyhow::Result<Self> {
        let lock = Arc::new(lock);
        let operation_lock = lock.clone();
        let server = vault_owner_ipc::Server::start(root, move |request, progress| {
            let _ownership = &operation_lock;
            let ping = request.method == "ping";
            let mut result =
                vault_worker::owner_requests::dispatch(request, progress, session.get_key())?;
            if ping {
                result["background"] = background.into();
                result["startupWindows"] = startup_windows.into();
                result["lastLaunchArguments"] =
                    serde_json::json!(*launches.0.lock().expect("launch state"));
            }
            Ok(result)
        })?;
        Ok(Self { _server: server, _lock: lock })
    }
}
