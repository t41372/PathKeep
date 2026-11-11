//! Responsibilities: expose the independent archive library through a JSON CLI.
//! Not responsible for: parsing native transcripts, product UI or cloud access.
#[path = "../cli/mod.rs"]
mod cli;

fn main() -> std::process::ExitCode {
    cli::main_entry()
}
