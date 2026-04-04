fn main() {
    if let Err(error) = browser_history_backup_desktop::entrypoint() {
        eprintln!("{error:?}");
        std::process::exit(1);
    }
}
