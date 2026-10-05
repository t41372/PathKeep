//! Responsibilities: find the installed app beside a client and launch it invisibly.
//! Not responsible for: waiting for readiness or executing shell command strings.
//! Failure modes: sidecar launches itself recursively; spaces split paths; child keeps
//! MCP stdout open; macOS opens a visible window. Tests: below and owner_process.rs.
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn desktop_launch_uses_same_executable_and_background_argument() {
        let spec = launch_spec(Path::new("/some directory/pathkeep-desktop")).unwrap();
        assert_eq!(spec.program, Path::new("/some directory/pathkeep-desktop"));
        assert_eq!(spec.args, ["--background"]);
    }
    #[test]
    fn sidecar_without_app_reports_installation_error() {
        let root = tempfile::tempdir().unwrap();
        let name = if cfg!(windows) { "pathkeep-worker.exe" } else { "pathkeep-worker" };
        assert!(launch_spec(&root.path().join(name)).unwrap_err().message.contains("beside"));
    }
    #[cfg(target_os = "macos")]
    #[test]
    fn launch_services_keeps_the_client_data_root_and_sandbox() {
        let spec =
            launch_spec(Path::new("/Applications/PathKeep.app/Contents/MacOS/pathkeep-worker"))
                .unwrap();
        let command = launch_command(
            &spec,
            Path::new("/tmp/archive with spaces"),
            [("PATHKEEP_PLATFORM_TEST_SANDBOX_DIR".into(), "/tmp/sandbox".into())],
        );
        let args = command.get_args().collect::<Vec<_>>();
        assert!(args.contains(&std::ffi::OsStr::new("CHB_PROJECT_ROOT=/tmp/archive with spaces")));
        assert!(
            args.contains(&std::ffi::OsStr::new("PATHKEEP_PLATFORM_TEST_SANDBOX_DIR=/tmp/sandbox"))
        );
        assert_eq!(args.last(), Some(&std::ffi::OsStr::new("--background")));
    }
    #[cfg(target_os = "macos")]
    #[test]
    fn bundled_clients_use_launch_services_without_frontmost_window() {
        let spec =
            launch_spec(Path::new("/Applications/Path Keep.app/Contents/MacOS/pathkeep-worker"))
                .unwrap();
        assert_eq!(spec.program, Path::new("/usr/bin/open"));
        assert_eq!(
            spec.args,
            ["-n", "-g", "-a", "/Applications/Path Keep.app", "--args", "--background"]
        );
    }
}
use crate::{ErrorCode, RpcError};
use std::{
    ffi::OsString,
    path::{Path, PathBuf},
    process::{Command, Stdio},
};

#[derive(Debug)]
struct LaunchSpec {
    program: PathBuf,
    args: Vec<OsString>,
}
fn launch_spec(executable: &Path) -> Result<LaunchSpec, RpcError> {
    #[cfg(target_os = "macos")]
    if let Some(bundle) = executable.ancestors().find(|p| p.extension().is_some_and(|e| e == "app"))
    {
        return Ok(LaunchSpec {
            program: "/usr/bin/open".into(),
            args: vec![
                "-n".into(),
                "-g".into(),
                "-a".into(),
                bundle.as_os_str().into(),
                "--args".into(),
                "--background".into(),
            ],
        });
    }
    let stem = executable.file_stem().and_then(|v| v.to_str()).unwrap_or_default();
    let program = if stem.starts_with("pathkeep-worker") {
        let directory = executable.parent().ok_or_else(|| {
            RpcError::new(ErrorCode::Unavailable, "worker executable has no parent directory")
        })?;
        ["pathkeep-desktop", "PathKeep", "pathkeep"].iter().map(|name| {
            let mut path = directory.join(name);
            if cfg!(windows) { path.set_extension("exe"); }
            path
        }).find(|p| p.is_file()).ok_or_else(|| RpcError::new(ErrorCode::Unavailable, "PathKeep app was not found beside pathkeep-worker; install the app and its sidecar together"))?
    } else {
        executable.to_owned()
    };
    Ok(LaunchSpec { program, args: vec!["--background".into()] })
}

fn launch_command(
    spec: &LaunchSpec,
    root: &Path,
    overrides: impl IntoIterator<Item = (OsString, OsString)>,
) -> Command {
    let mut command = Command::new(&spec.program);
    command.env("CHB_PROJECT_ROOT", root);
    #[cfg(target_os = "macos")]
    if spec.program == Path::new("/usr/bin/open") {
        let (launcher_args, app_args) = spec.args.split_at(spec.args.len() - 2);
        command.args(launcher_args);
        let variables =
            std::iter::once((OsString::from("CHB_PROJECT_ROOT"), root.as_os_str().to_owned()))
                .chain(overrides);
        for (mut name, value) in variables {
            name.push("=");
            name.push(value);
            command.arg("--env").arg(name);
        }
        command.arg("--stderr").arg(root.join("logs/owner-startup.log"));
        command.arg("--stdout").arg("/dev/null");
        command.args(app_args);
        return command;
    }
    command.args(&spec.args).envs(overrides);
    command
}

/// Spawns the app with detached standard streams; readiness is checked by Client.
pub fn launch_background(root: &Path) -> Result<(), RpcError> {
    let spec = launch_spec(&std::env::current_exe()?)?;
    // Launch Services does not inherit the client's environment. Carry only the
    // established fixture/platform overrides, as well as the explicit data root.
    let overrides = [
        "CHB_CHROME_USER_DATA_DIR",
        "CHB_FIREFOX_PROFILES_DIR",
        "CHB_SAFARI_ROOT",
        "CHB_TEST_KEYRING_DIR",
        "PATHKEEP_PLATFORM_TEST_KEYRING_DIR",
        "PATHKEEP_PLATFORM_TEST_SANDBOX_DIR",
    ]
    .into_iter()
    .filter_map(|name| std::env::var_os(name).map(|value| (name.into(), value)));
    let mut command = launch_command(&spec, root, overrides);
    let logs = root.join("logs");
    std::fs::create_dir_all(&logs)?;
    let mut stderr = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(logs.join("owner-startup.log"))?;
    command.stdin(Stdio::null()).stdout(Stdio::null()).stderr(stderr.try_clone()?);
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        command.creation_flags(0x00000008 | 0x00000200); // DETACHED_PROCESS | CREATE_NEW_PROCESS_GROUP
    }
    let mut child = command.spawn().map_err(|e| {
        RpcError::new(
            ErrorCode::Unavailable,
            format!("could not start PathKeep at {}: {e}", spec.program.display()),
        )
    })?;
    use std::io::Write;
    writeln!(stderr, "PathKeep background child pid={}", child.id())?;
    // Reap direct children without blocking the CLI on the app's lifetime.
    std::thread::spawn(move || {
        let _ = child.wait();
    });
    Ok(())
}
