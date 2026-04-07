fn entrypoint_exit_code(result: anyhow::Result<()>) -> i32 {
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
    let exit_code = entrypoint_exit_code(pathkeep_desktop::entrypoint());
    if exit_code != 0 {
        std::process::exit(exit_code);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use anyhow::anyhow;

    #[test]
    fn entrypoint_exit_code_reports_success_and_failure() {
        assert_eq!(entrypoint_exit_code(Ok(())), 0);
        assert_eq!(entrypoint_exit_code(Err(anyhow!("boom"))), 1);
    }
}
