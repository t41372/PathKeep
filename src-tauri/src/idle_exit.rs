//! Responsibilities: end a background launch nobody is using once its work is done.
//! A scheduled backup, the CLI or MCP start PathKeep with `--background`; without
//! this, that hidden process stayed resident until logout.
//! Not responsible for: starting the owner again (clients do that on demand), or
//! deciding what counts as queue work (`vault_worker::background_work_running`).
//! Failure modes: exiting during a request, a queue drain or index catch-up; exiting
//! once a window was opened or while the menu bar icon is wanted; never exiting.
//! Tests: below for the decision; owner_process for a real background launch.
#![cfg_attr(test, allow(dead_code))]
use crate::session::SessionState;
use std::{
    sync::{
        Arc, Mutex,
        atomic::{AtomicUsize, Ordering},
    },
    time::{Duration, Instant},
};
use tauri::{AppHandle, Manager, Runtime};

/// How long a background launch with nothing left to do stays before exiting.
const IDLE: Duration = Duration::from_secs(120);
const TICK: Duration = Duration::from_secs(5);

/// Requests on the owner endpoint: backups, imports, maintenance and MCP calls.
#[derive(Default)]
pub(crate) struct Activity {
    in_flight: AtomicUsize,
    finished: Mutex<Option<Instant>>,
}
impl Activity {
    /// Held for the duration of one request.
    pub(crate) fn begin(self: &Arc<Self>) -> Request {
        self.in_flight.fetch_add(1, Ordering::SeqCst);
        Request(self.clone())
    }
}
pub(crate) struct Request(Arc<Activity>);
impl Drop for Request {
    fn drop(&mut self) {
        *self.0.finished.lock().unwrap() = Some(Instant::now());
        self.0.in_flight.fetch_sub(1, Ordering::SeqCst);
    }
}

/// What the watcher sees on one tick.
#[derive(Debug, Clone, Copy)]
struct Moment {
    window_opened: bool,
    menu_bar_icon: bool,
    working: bool,
}

#[derive(Debug, PartialEq, Eq)]
enum Next {
    /// A window was opened: this is now an ordinary launch, never exited here.
    StopWatching,
    /// Something is going on, or the icon is wanted: the idle period starts over.
    Restart,
    Wait,
    Exit,
}

fn decide(moment: Moment, quiet_for: Duration, idle: Duration) -> Next {
    if moment.window_opened {
        Next::StopWatching
    } else if moment.menu_bar_icon || moment.working {
        Next::Restart
    } else if quiet_for >= idle {
        Next::Exit
    } else {
        Next::Wait
    }
}

fn idle_period() -> Duration {
    // Debug builds only: lets owner_process prove the exit without waiting minutes.
    #[cfg(debug_assertions)]
    if let Some(seconds) =
        std::env::var("PATHKEEP_DEBUG_IDLE_EXIT_SECS").ok().and_then(|value| value.parse().ok())
    {
        return Duration::from_secs(seconds);
    }
    IDLE
}

fn indexing(session: &SessionState) -> bool {
    use vault_core::search::IndexerPhase;
    session
        .current_vault()
        .and_then(|vault| vault_core::search::indexer_state(&vault).ok())
        .is_some_and(|state| {
            matches!(
                state.phase,
                IndexerPhase::Building | IndexerPhase::Updating | IndexerPhase::Rebuilding
            )
        })
}

/// Watches a `--background` launch and exits it once it has been idle long enough.
pub(crate) fn watch<R: Runtime>(app: AppHandle<R>, session: SessionState, activity: Arc<Activity>) {
    let idle = idle_period();
    let spawned = std::thread::Builder::new().name("pathkeep-idle-exit".into()).spawn(move || {
        let tick = TICK.min(idle.max(Duration::from_secs(1)));
        let mut quiet_since = Instant::now();
        loop {
            std::thread::sleep(tick);
            let menu_bar_icon = vault_core::project_paths()
                .and_then(|paths| vault_core::load_config(&paths))
                .map(|config| config.menu_bar_icon)
                .unwrap_or(false);
            let moment = Moment {
                window_opened: !app.webview_windows().is_empty(),
                menu_bar_icon,
                working: activity.in_flight.load(Ordering::SeqCst) > 0
                    || vault_worker::background_work_running()
                    || indexing(&session),
            };
            if let Some(finished) = *activity.finished.lock().unwrap() {
                quiet_since = quiet_since.max(finished);
            }
            match decide(moment, quiet_since.elapsed(), idle) {
                Next::StopWatching => return,
                Next::Restart => quiet_since = Instant::now(),
                Next::Wait => {}
                Next::Exit => {
                    log::info!("background launch idle for {idle:?}; exiting");
                    if let Err(error) = session.close_vault() {
                        log::warn!("closing the archive before an idle exit: {error}");
                    }
                    app.exit(0);
                    return;
                }
            }
        }
    });
    if let Err(error) = spawned {
        log::warn!("could not watch the background launch for idleness: {error}");
    }
}
