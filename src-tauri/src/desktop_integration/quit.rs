//! What happens when someone quits while PathKeep is doing work they are waiting on.
//!
//! Every way to quit that the app gets a say in comes through [`hold_quit`]:
//! - the window's close button while the menu bar icon is off (`window.rs`, at `CloseRequested`,
//!   while the window is still there to ask in; with the icon on, closing only hides it);
//! - everything that calls `AppHandle::exit`: the menu bar icon's Quit (`tray.rs`), ⌘Q
//!   (`app_menu.rs`) and this module's own answers, at `RunEvent::ExitRequested` (`lib.rs`).
//!
//! When `vault_worker::foreground_work` lists something, the quit is held: the window comes
//! forward and gets [`QUIT_REQUEST_EVENT`], and the person answers with [`answer_quit_request`]:
//! wait for it and then quit, quit now, or keep PathKeep open. While the question is open the
//! event follows what is running. If the work ends before they answer, the question says so and
//! they still choose; PathKeep neither quits nor drops the question under them. Someone who
//! chose to wait gets the quit as soon as nothing is running. Quitting again while the question
//! is open brings it back to the front.
//!
//! Queue work that carries on by itself after a restart (index builds, Insights, link previews)
//! is not listed, so it never holds a quit.
//!
//! macOS's Dock menu Quit, logging out and shutting down go through AppKit's `terminate:`, which
//! tao 0.35 gives the app no say in: they end the process at once, as before, so PathKeep never
//! holds up a log out or shutdown. Running work stops there, and what it had written stays
//! intact (files are written beside the target and renamed into place, archive changes are
//! transactions or are put right at the next launch).
//!
//! ## Responsibilities
//! - Decide whether a quit goes ahead now, hold it while work runs, and carry out the answer.
//! - Keep the one open question and tell the window about it.
//!
//! ## Not responsible for
//! - Knowing what is running (`vault_worker::foreground_work`), or the question's words and
//!   buttons (`src/app/shell/quit-prompt.tsx`).
//!
//! ## Threading
//! [`hold_quit`] runs on the main thread inside window and run-loop callbacks, and its answer
//! must be ready before the callback returns, so it reads only memory: this module's state and
//! the vault-worker registry, never the menu state lock (`mod.rs`). Bringing the window forward,
//! sending the event and quitting once the work is done happen on a watcher thread. Events are
//! sent while this module's lock is held, so the window gets them in the order the state changed.
//!
//! Failure modes: a quit path that skips `hold_quit` (work thrown away without asking); a held
//! quit nobody can answer (window hidden or gone); an answer meant for an older question quitting
//! or cancelling a newer one; a quit the person confirmed being held again; a watcher that keeps
//! a closed question on screen. Tests: the E2E spec `quit-during-work.spec.ts`.

use serde::{Deserialize, Serialize};
use std::{sync::Mutex, time::Duration};
use tauri::{AppHandle, Emitter, Manager, Runtime};
use vault_worker::ForegroundWork;

/// Event carrying the open quit question ([`QuitRequest`]), or `null` once there is none.
pub(crate) const QUIT_REQUEST_EVENT: &str = "pathkeep://quit-request";

/// How often the watcher looks at what is running while a quit is held.
const WATCH_INTERVAL: Duration = Duration::from_millis(200);

/// The open question, as the window shows it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct QuitRequest {
    /// Tells this question from a later one, so an answer meant for an old one does nothing.
    id: u64,
    /// What is running, each kind once, oldest first; empty once it has all ended.
    work: Vec<ForegroundWork>,
    /// The person chose to wait: PathKeep quits as soon as `work` is empty.
    waiting: bool,
}

/// The person's answer to a [`QuitRequest`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub(crate) enum QuitAnswer {
    /// Quit once nothing is running.
    Wait,
    /// Quit now; running work stops.
    QuitNow,
    /// Do not quit.
    KeepOpen,
}

#[derive(Default)]
struct Quit {
    /// The open question's id; `None` while no quit is held.
    open: Option<u64>,
    last_id: u64,
    waiting: bool,
    /// The window should come forward and get the question again.
    reveal: bool,
    /// The person said the quit may go ahead; it is not held again.
    confirmed: bool,
    /// A watcher thread is running.
    watching: bool,
}

impl Quit {
    fn request(&self, work: Vec<ForegroundWork>) -> Option<QuitRequest> {
        self.open.map(|id| QuitRequest { id, work, waiting: self.waiting })
    }
}

/// This process's quit state; see the module header.
pub(crate) struct QuitState(Mutex<Quit>);

fn with_quit<R: Runtime, T>(app: &AppHandle<R>, f: impl FnOnce(&mut Quit) -> T) -> T {
    if app.try_state::<QuitState>().is_none() {
        app.manage(QuitState(Mutex::new(Quit::default())));
    }
    let state = app.state::<QuitState>();
    let mut quit = state.0.lock().unwrap_or_else(|poisoned| poisoned.into_inner());
    f(&mut quit)
}

fn send<R: Runtime>(app: &AppHandle<R>, request: Option<&QuitRequest>) {
    if let Err(error) = app.emit(QUIT_REQUEST_EVENT, request) {
        log::warn!("could not send the quit question to the window: {error}");
    }
}

/// Whether to hold this quit; true means the caller stops the window closing or the app exiting.
/// Called on the main thread by every quit path (see the module header).
pub(crate) fn hold_quit<R: Runtime>(app: &AppHandle<R>) -> bool {
    let work = vault_worker::foreground_work();
    let start_watcher = with_quit(app, |quit| {
        if quit.confirmed || work.is_empty() {
            return None;
        }
        if quit.open.is_none() {
            quit.last_id += 1;
            quit.open = Some(quit.last_id);
            quit.waiting = false;
        }
        quit.reveal = true;
        Some(!std::mem::replace(&mut quit.watching, true))
    });
    let Some(start_watcher) = start_watcher else { return false };
    log::info!("quit held while running: {work:?}");
    if start_watcher {
        let watched = app.clone();
        let spawned =
            std::thread::Builder::new().name("pathkeep-quit".into()).spawn(move || watch(&watched));
        if let Err(error) = spawned {
            // Nobody could be asked, so holding the quit would trap the person.
            log::error!("could not watch the held quit, quitting: {error}");
            with_quit(app, |quit| quit.confirmed = true);
            return false;
        }
    }
    true
}

enum Step {
    Stop,
    Quit,
    Wait { reveal: bool },
}

/// Runs while a quit is held: keeps the window's question current and quits once someone who
/// chose to wait has nothing left to wait for.
fn watch<R: Runtime>(app: &AppHandle<R>) {
    let mut sent = None;
    loop {
        let work = vault_worker::foreground_work();
        let step = with_quit(app, |quit| {
            if quit.open.is_none() {
                quit.watching = false;
                return Step::Stop;
            }
            if quit.waiting && work.is_empty() {
                quit.confirmed = true;
                quit.open = None;
                quit.watching = false;
                return Step::Quit;
            }
            let reveal = std::mem::take(&mut quit.reveal);
            let request = quit.request(work);
            if reveal || request != sent {
                send(app, request.as_ref());
                sent = request;
            }
            Step::Wait { reveal }
        });
        match step {
            Step::Stop => return,
            Step::Quit => {
                log::info!("the work PathKeep waited for is done; quitting");
                app.exit(0);
                return;
            }
            Step::Wait { reveal } => {
                if reveal {
                    let handle = app.clone();
                    if let Err(error) =
                        app.run_on_main_thread(move || super::window::show_main_window(&handle))
                    {
                        log::warn!("could not bring the window forward to ask: {error}");
                    }
                }
            }
        }
        std::thread::sleep(WATCH_INTERVAL);
    }
}

/// `get_quit_request`: the open question, for a window that starts listening after it was sent.
pub(crate) fn quit_request<R: Runtime>(app: &AppHandle<R>) -> Option<QuitRequest> {
    let work = vault_worker::foreground_work();
    with_quit(app, |quit| quit.request(work))
}

/// `answer_quit_request`: carries out the answer to question `id` and returns the question as it
/// stands after, `None` once there is none. An answer to a question that is no longer open (the
/// other window answered it, or it was answered before) changes nothing.
pub(crate) fn answer_quit_request<R: Runtime>(
    app: &AppHandle<R>,
    id: u64,
    answer: QuitAnswer,
) -> Option<QuitRequest> {
    let work = vault_worker::foreground_work();
    let quit_now = with_quit(app, |quit| {
        if quit.open != Some(id) {
            return false;
        }
        match answer {
            QuitAnswer::Wait => quit.waiting = true,
            QuitAnswer::QuitNow => {
                quit.confirmed = true;
                quit.open = None;
            }
            QuitAnswer::KeepOpen => {
                quit.open = None;
                quit.waiting = false;
            }
        }
        send(app, quit.request(work.clone()).as_ref());
        answer == QuitAnswer::QuitNow
    });
    if quit_now {
        log::info!("quitting now as asked; stopping: {work:?}");
        app.exit(0);
    }
    quit_request(app)
}
