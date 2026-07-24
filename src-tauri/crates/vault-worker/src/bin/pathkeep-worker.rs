//! Dedicated command-line entrypoint for background PathKeep work.
//!
//! This binary deliberately depends on `vault-worker`, not the Tauri desktop
//! shell. Native schedulers can therefore execute it directly and observe its
//! real process exit status without activating a GUI application.

use anyhow::Result;
use std::io::Write;

fn run_with_arguments<W: Write>(arguments: &[String], writer: &mut W) -> Result<()> {
    write_worker_payload(vault_worker::run_worker_cli(arguments), writer)
}

fn write_worker_payload<W: Write>(payload: Result<String>, writer: &mut W) -> Result<()> {
    let payload = payload?;
    writeln!(writer, "{payload}")?;
    Ok(())
}

fn entrypoint_exit_code(result: Result<()>) -> i32 {
    match result {
        Ok(()) => 0,
        Err(error) => {
            eprintln!("{error:?}");
            1
        }
    }
}

#[cfg(not(test))]
fn main() {
    let arguments = std::env::args().skip(1).collect::<Vec<_>>();
    let stdout = std::io::stdout();
    let mut writer = stdout.lock();
    let exit_code = entrypoint_exit_code(run_with_arguments(&arguments, &mut writer));
    if exit_code != 0 {
        std::process::exit(exit_code);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use anyhow::anyhow;

    #[test]
    fn unknown_commands_fail_without_writing_a_success_payload() {
        let mut output = Vec::new();
        let error = run_with_arguments(&["not-a-command".to_string()], &mut output)
            .expect_err("unknown command must fail");

        assert!(error.to_string().contains("unknown worker command"));
        assert!(output.is_empty());
    }

    #[test]
    fn successful_worker_payload_is_written_with_a_record_boundary() {
        let mut output = Vec::new();

        write_worker_payload(Ok("{\"ok\":true}".to_string()), &mut output).expect("write payload");

        assert_eq!(output, b"{\"ok\":true}\n");
    }

    #[test]
    fn output_failures_are_propagated() {
        struct FailingWriter;

        impl Write for FailingWriter {
            fn write(&mut self, _buffer: &[u8]) -> std::io::Result<usize> {
                Err(std::io::Error::other("output unavailable"))
            }

            fn flush(&mut self) -> std::io::Result<()> {
                Ok(())
            }
        }

        let mut writer = FailingWriter;
        let error = write_worker_payload(Ok("payload".to_string()), &mut writer)
            .expect_err("writer failure");

        assert!(error.to_string().contains("output unavailable"));
        writer.flush().expect("flush");
    }

    #[test]
    fn entrypoint_exit_code_preserves_success_and_failure() {
        assert_eq!(entrypoint_exit_code(Ok(())), 0);
        assert_eq!(entrypoint_exit_code(Err(anyhow!("boom"))), 1);
    }
}
