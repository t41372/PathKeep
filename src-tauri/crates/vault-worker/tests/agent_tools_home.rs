//! Exercise the worker's unfiltered discovery and preview with conflicting native
//! tool settings. Neither path may open or enumerate a decoy directory.
use agent_history::capture::{default_sources, known_sources};
use serde_json::{from_value, json};
use std::{
    fs,
    path::{Path, PathBuf},
    process::Command,
    time::{Duration, SystemTime},
};
use vault_worker::{VaultSession, data_kinds::preview};

const CHILD: &str = "PATHKEEP_DISCOVERY_TEST_ROOT";
const OVERRIDE: &str = "CHB_AGENT_TOOLS_HOME";
const ROOTS: [(&str, &str); 5] = [
    (".claude", "projects/project/main.jsonl"),
    (".codex", "sessions/rollout-main.jsonl"),
    (".pi/agent", "sessions/main.jsonl"),
    (".dsh", "sessions/project/session.v1.jsonl"),
    ("Claude", "claude-code-sessions/account/session.json"),
];

#[test]
fn one_override_isolates_presence_preview_and_capture_defaults() {
    if let Some(root) = std::env::var_os(CHILD) {
        if std::env::var_os(OVERRIDE).unwrap().is_empty() {
            assert!(known_sources().is_empty());
            assert!(default_sources().is_empty());
        } else {
            check_worker(&PathBuf::from(root));
        }
        return;
    }
    let temp = tempfile::tempdir().unwrap();
    let isolated = temp.path().join("tool fixtures");
    for (root, file) in ROOTS {
        let path = isolated.join(root).join(file);
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(path, b"fixture").unwrap();
    }

    let decoy = temp.path().join("ordinary home");
    let mut child = Command::new(std::env::current_exe().unwrap());
    child
        .args(["--exact", "one_override_isolates_presence_preview_and_capture_defaults"])
        .env(CHILD, &isolated)
        .env(OVERRIDE, &isolated)
        .env("CHB_PROJECT_ROOT", temp.path().join("archive"))
        .env("HOME", &decoy)
        .env("USERPROFILE", &decoy)
        .env("XDG_CONFIG_HOME", &decoy)
        .env("XDG_DATA_HOME", &decoy);

    let mut watched = Vec::new();
    for variable in [
        "CLAUDE_CONFIG_DIR",
        "CODEX_HOME",
        "CODEX_SQLITE_HOME",
        "PI_CODING_AGENT_DIR",
        "PI_CODING_AGENT_SESSION_DIR",
        "DSH_HOME",
    ] {
        let dir = decoy.join(variable);
        child.env(variable, &dir);
        watched.push(dir);
    }
    // Cover the ordinary Claude Desktop locations too, including macOS where
    // changing the native tool variables doesn't redirect application data.
    watched.extend([decoy.join("Claude"), decoy.join("Library/Application Support/Claude")]);
    let old = SystemTime::UNIX_EPOCH + Duration::from_secs(100);
    for dir in watched.clone() {
        fs::create_dir_all(&dir).unwrap();
        let file = dir.join("sentinel.jsonl");
        fs::write(&file, b"outside").unwrap();
        fs::OpenOptions::new()
            .write(true)
            .open(&file)
            .unwrap()
            .set_times(fs::FileTimes::new().set_accessed(old))
            .unwrap();
        #[cfg(unix)]
        fs::File::open(&dir).unwrap().set_times(fs::FileTimes::new().set_accessed(old)).unwrap();
        watched.push(file);
    }
    #[cfg(target_os = "linux")]
    let opens = watch_opens(&watched);
    let status = child.status().unwrap();
    let empty_status = child.env(OVERRIDE, "").status().unwrap();
    for path in watched {
        let meta = fs::metadata(&path).unwrap();
        if cfg!(unix) || meta.is_file() {
            assert_eq!(meta.accessed().unwrap(), old, "accessed {}", path.display());
        }
    }
    #[cfg(target_os = "linux")]
    {
        use std::io::Read;
        let error = (&opens).read(&mut [0; 4096]).expect_err("opened a decoy");
        assert_eq!(error.kind(), std::io::ErrorKind::WouldBlock);
    }
    assert!(status.success() && empty_status.success());
}

fn check_worker(home: &Path) {
    let expected: Vec<_> = ROOTS.iter().map(|(name, _)| home.join(name)).collect();
    let paths = |roots: Vec<agent_history::SourceRoot>| {
        roots.into_iter().map(|r| r.root).collect::<Vec<_>>()
    };
    fs::rename(home.join(".dsh"), home.join("removed-tool")).unwrap();
    assert_eq!(paths(known_sources()), expected);
    assert_eq!(
        paths(default_sources()),
        expected.clone().into_iter().filter(|p| !p.ends_with(".dsh")).collect::<Vec<_>>()
    );
    fs::rename(home.join("removed-tool"), home.join(".dsh")).unwrap();
    let read = preview::detect(from_value(json!({"kinds":["agent-history"]})).unwrap()).unwrap();
    let rows = read["sources"].as_array().unwrap();
    assert_eq!(rows.len(), expected.len());
    for (row, path) in rows.iter().zip(&expected) {
        assert_eq!(row["path"], json!(path));
        assert_eq!(row["found"], true);
    }
    let mut frames =
        preview::start(&VaultSession::default(), from_value(json!({"purpose":"enable"})).unwrap())
            .unwrap();
    let report =
        tokio::runtime::Builder::new_current_thread().enable_all().build().unwrap().block_on(
            async {
                while let Some(frame) = frames.next_frame().await {
                    if frame["type"] == "finished" {
                        return frame["payload"].clone();
                    }
                }
                panic!("preview closed without inventory");
            },
        );
    let rows = report["rows"].as_array().unwrap();
    assert_eq!(rows.len(), expected.len());
    for (row, path) in rows.iter().zip(&expected) {
        assert_eq!(row["path"], json!(path));
        assert_eq!(row["counts"]["files"], "1");
        assert_eq!(row["counts"]["sourceBytes"], "7");
    }
    assert_eq!(paths(default_sources()), expected);
}

#[cfg(target_os = "linux")]
fn watch_opens(paths: &[PathBuf]) -> fs::File {
    use std::{
        ffi::CString,
        os::{
            fd::{AsRawFd, FromRawFd},
            unix::ffi::OsStrExt,
        },
    };
    let fd = unsafe { libc::inotify_init1(libc::IN_NONBLOCK | libc::IN_CLOEXEC) };
    assert!(fd >= 0);
    let file = unsafe { fs::File::from_raw_fd(fd) };
    for path in paths {
        let path = CString::new(path.as_os_str().as_bytes()).unwrap();
        assert!(
            unsafe { libc::inotify_add_watch(file.as_raw_fd(), path.as_ptr(), libc::IN_OPEN) } >= 0
        );
    }
    file
}
