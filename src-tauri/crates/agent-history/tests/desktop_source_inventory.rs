//! Failure modes: sampling hides later files, a generation becomes a conversation,
//! crawling settings or .git leaks into totals, and cancellation waits for a full walk.
#![allow(dead_code)]
mod support;
use agent_history::{Harness, SourceRoot, capture::inventory_root};
use std::{
    fs,
    time::{Duration, SystemTime},
};
use support::{fixtures::Fixture, native};
use vault_store::CancelToken;

#[test]
fn native_metadata_only_and_cancellable() {
    let temp = tempfile::tempdir().unwrap();
    let fixture = Fixture::four_harnesses(temp.path());
    let claude = &fixture.roots[0].1;
    for id in 0..270 {
        native::claude_main(claude, &format!("sample-{id}"), &[]);
    }
    native::claude_subagent(claude, "sample-0", "agent-child.jsonl", "child", "/", "child");
    native::claude_upload(claude, "sample-0", "image.png", b"image");
    native::deepseek_generations(&fixture.roots[3].1, "more", "/", "saved");
    for name in ["settings.json", "projects/.git/config", "projects/.git/HEAD"] {
        support::write(&claude.join(name), b"forbidden");
    }
    support::write(&fixture.roots[1].1.join("sessions/2026/future.format"), b"unknown");
    let files: Vec<_> = walkdir::WalkDir::new(&fixture.source_tree)
        .into_iter()
        .map(Result::unwrap)
        .filter(|entry| entry.file_type().is_file())
        .map(walkdir::DirEntry::into_path)
        .collect();
    let old = SystemTime::UNIX_EPOCH + Duration::from_secs(100);
    for path in &files {
        fs::OpenOptions::new()
            .write(true)
            .open(path)
            .unwrap()
            .set_times(fs::FileTimes::new().set_accessed(old))
            .unwrap();
    }
    let roots: Vec<_> = fixture
        .roots
        .iter()
        .map(|(name, root)| SourceRoot { harness: name.parse().unwrap(), root: root.clone() })
        .collect();
    let rows: Vec<_> = roots
        .iter()
        .map(|root| inventory_root(root, &CancelToken::new(), |_| {}).unwrap())
        .collect();
    assert_eq!(rows[0].counts.files, 273);
    assert_eq!(rows[0].counts.child_files, 1);
    assert_eq!(rows[0].counts.ancillary_files, 1);
    assert_eq!(rows[1].counts.unknown_files, 1);
    assert!(rows[3].counts.compressed_bytes > 0);
    assert!(rows.iter().all(|row| row.readable
        && row.counts.folders > 0
        && row.earliest_at.is_some()
        && row.latest_at.is_some()));
    for path in files {
        assert_eq!(path.metadata().unwrap().accessed().unwrap(), old, "read {}", path.display());
    }
    let token = CancelToken::new();
    assert!(matches!(
        inventory_root(&roots[0], &token, |done| if done >= 8 {
            token.cancel()
        }),
        Err(agent_history::Error::Cancelled)
    ));
    let missing = SourceRoot { harness: Harness::Pi, root: temp.path().join("absent") };
    assert_eq!(
        inventory_root(&missing, &CancelToken::new(), |_| {}).unwrap().reason,
        Some("root-missing")
    );
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let path = temp.path().join("denied");
        fs::create_dir(&path).unwrap();
        fs::set_permissions(&path, fs::Permissions::from_mode(0o000)).unwrap();
        let row = inventory_root(
            &SourceRoot { harness: Harness::Pi, root: path.clone() },
            &CancelToken::new(),
            |_| {},
        )
        .unwrap();
        fs::set_permissions(path, fs::Permissions::from_mode(0o700)).unwrap();
        assert_eq!(row.reason, Some("root-unreadable"));
    }
}

#[cfg(target_os = "linux")]
#[test]
fn no_source_file_or_git_directory_is_opened() {
    // IN_OPEN fires for an open followed immediately by close, including empty
    // files for which comparing access times alone would miss a regression.
    use std::{
        ffi::CString,
        os::{
            fd::{AsRawFd, FromRawFd, OwnedFd},
            unix::ffi::OsStrExt,
        },
    };
    let home = tempfile::tempdir().unwrap();
    let fixture = Fixture::four_harnesses(home.path());
    let claude = &fixture.roots[0].1;
    support::write(&claude.join("projects/.git/config"), b"");
    support::write(&claude.join("settings.json"), b"");
    let fd = unsafe { libc::inotify_init1(libc::IN_NONBLOCK | libc::IN_CLOEXEC) };
    assert!(fd >= 0);
    let fd = unsafe { OwnedFd::from_raw_fd(fd) };
    for entry in walkdir::WalkDir::new(home.path()) {
        let entry = entry.unwrap();
        if entry.file_type().is_file() || entry.file_name() == ".git" {
            let path = CString::new(entry.path().as_os_str().as_bytes()).unwrap();
            assert!(
                unsafe { libc::inotify_add_watch(fd.as_raw_fd(), path.as_ptr(), libc::IN_OPEN) }
                    >= 0
            );
        }
    }
    for (tool, root) in fixture.roots {
        inventory_root(
            &SourceRoot { harness: tool.parse().unwrap(), root },
            &Default::default(),
            |_| {},
        )
        .unwrap();
    }
    let mut buf = [0u8; 4096];
    let read = unsafe { libc::read(fd.as_raw_fd(), buf.as_mut_ptr().cast(), buf.len()) };
    assert_eq!(read, -1, "preview opened a watched file or .git directory");
    assert_eq!(std::io::Error::last_os_error().kind(), std::io::ErrorKind::WouldBlock);
}
