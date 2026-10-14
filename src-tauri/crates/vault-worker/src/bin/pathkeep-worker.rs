//! Responsibilities: client CLI output and process exit codes.
//! Not responsible for: opening databases or executing worker business logic.
//! Failure modes: stdout lock deadlocks MCP; errors return success. Tests: owner_client.
//!
//! This binary deliberately depends on `vault-worker`, not the Tauri desktop
//! shell. Native schedulers can therefore execute it directly and observe its
//! real process exit status while the database owner stays in the background.

#[cfg(not(test))]
fn main() {
    use std::io::Write;

    let arguments = std::env::args().skip(1).collect::<Vec<_>>();
    let result = vault_worker::run_worker_cli(&arguments).and_then(|payload| {
        let stdout = std::io::stdout();
        let mut writer = stdout.lock();
        writeln!(writer, "{payload}")?;
        Ok(())
    });
    if let Err(error) = result {
        eprintln!("{error:?}");
        std::process::exit(1);
    }
}
