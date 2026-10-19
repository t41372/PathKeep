//! Responsibilities: adapt existing business regression tests to real owner transport.
//! Not responsible for: substituting database operations or app lifecycle tests.
//! Failure modes: tests accidentally start a desktop app or bypass IPC. Each helper
//! binds a private endpoint and executes the production client/dispatcher.
use vault_owner_ipc::Server;

pub(crate) fn serve() -> Server {
    let root = vault_core::project_paths().unwrap().app_root;
    std::fs::create_dir_all(&root).unwrap();
    let session = crate::VaultSession::default();
    session.begin_startup();
    let _ = session.finish_startup();
    Server::start(&root, move |request, progress| {
        super::dispatch(request, progress, session.get_key(), || {
            session.background_vault().map_err(anyhow::Error::msg)
        })
    })
    .unwrap()
}
pub(crate) fn run_worker_cli(arguments: &[String]) -> anyhow::Result<String> {
    // Unknown commands have no I/O contract and must not require a test environment.
    if arguments.first().is_none_or(|a| {
        !["backup", "doctor", "ai-index", "ai-queue", "ping", "mcp-server"].contains(&a.as_str())
    }) {
        return crate::run_worker_cli(arguments);
    }
    let _server = serve();
    crate::run_worker_cli(arguments)
}
