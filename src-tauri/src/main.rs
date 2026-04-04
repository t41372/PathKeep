fn main() {
    if let Err(error) = chrome_history_vault_desktop::entrypoint() {
        eprintln!("{error:?}");
        std::process::exit(1);
    }
}
