//! Thin binary entrypoint for the desktop crate.
//!
//! `src/lib.rs` owns the real application logic. This file only translates the
//! library result into an OS exit code.

#[cfg(not(test))]
/// Runs the desktop entrypoint and exits non-zero on startup failure.
fn main() {
    if let Err(error) = pathkeep_desktop::entrypoint() {
        eprintln!("{error:?}");
        std::process::exit(1);
    }
}
