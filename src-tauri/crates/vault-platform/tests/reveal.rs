//! Exercise the exact FileManager1 message on a private bus, without opening a desktop window.
#![cfg(target_os = "linux")]

#[test]
fn select_the_exact_source_in_a_private_file_manager() {
    use dbus::{
        blocking::Connection,
        channel::{MatchingReceiver, Sender},
        message::MatchRule,
    };
    use std::{process::Command, sync::mpsc, time::Duration};

    if std::env::var_os("PATHKEEP_REVEAL_TEST_BUS").is_none() {
        let child = Command::new("dbus-run-session")
            .arg("--")
            .arg(std::env::current_exe().unwrap())
            .args(["--exact", "select_the_exact_source_in_a_private_file_manager", "--nocapture"])
            .env("PATHKEEP_REVEAL_TEST_BUS", "1")
            .status();
        if matches!(&child,Err(e) if e.kind()==std::io::ErrorKind::NotFound) {
            eprintln!("Private bus unavailable on this host");
            return;
        }
        assert!(child.unwrap().success());
        return;
    }
    let temp = tempfile::tempdir().unwrap();
    let file = temp.path().join("會話 #1,2.jsonl");
    std::fs::write(&file, b"{}\n").unwrap();
    let (ready, started) = mpsc::channel();
    let (sent, received) = mpsc::channel();
    let service = std::thread::spawn(move || {
        let bus = Connection::new_session().unwrap();
        bus.request_name("org.freedesktop.FileManager1", false, false, true).unwrap();
        let rule = MatchRule::new_method_call()
            .with_interface("org.freedesktop.FileManager1")
            .with_member("ShowItems");
        bus.start_receive(
            rule,
            Box::new(move |msg, conn| {
                let (uris, startup): (Vec<String>, String) = msg.read2().unwrap();
                assert!(startup.is_empty());
                assert_eq!(msg.path().unwrap().to_string(), "/org/freedesktop/FileManager1");
                conn.send(msg.method_return()).unwrap();
                sent.send(uris).unwrap();
                false
            }),
        );
        ready.send(()).unwrap();
        // The bus may first deliver NameAcquired for the newly registered service.
        let deadline = std::time::Instant::now() + Duration::from_secs(5);
        loop {
            bus.process(Duration::from_millis(100)).unwrap();
            if let Ok(uris) = received.try_recv() {
                return uris;
            }
            assert!(std::time::Instant::now() < deadline);
        }
    });
    started.recv().unwrap();
    assert_eq!(
        vault_platform::reveal_file(file.display().to_string()).unwrap(),
        file.display().to_string()
    );
    let uris = service.join().unwrap();
    assert_eq!(uris.len(), 1);
    assert!(uris[0].contains("%23"));
    assert_eq!(url::Url::parse(&uris[0]).unwrap().to_file_path().unwrap(), file);
}

#[test]
fn unavailable_file_manager_service_opens_the_parent_folder() {
    use std::os::unix::fs::PermissionsExt;
    use std::process::Command;
    const FILE: &str = "PATHKEEP_REVEAL_FALLBACK_TEST_FILE";
    if let Some(file) = std::env::var_os(FILE) {
        let file = std::path::PathBuf::from(file);
        let target = vault_platform::reveal_file(file.to_str().unwrap().into()).unwrap();
        assert_eq!(Some(std::path::Path::new(&target)), file.parent());
        return;
    }
    let temp = tempfile::tempdir().unwrap();
    let file = temp.path().join("native transcript.jsonl");
    let log = temp.path().join("opened");
    std::fs::write(&file, b"{}").unwrap();
    let opener = temp.path().join("xdg-open");
    std::fs::write(&opener, b"#!/bin/sh\nprintf '%s' \"$1\" > \"$PATHKEEP_REVEAL_TEST_LOG\"\n")
        .unwrap();
    std::fs::set_permissions(&opener, std::fs::Permissions::from_mode(0o700)).unwrap();
    let output = Command::new(std::env::current_exe().unwrap())
        .args(["--exact", "unavailable_file_manager_service_opens_the_parent_folder"])
        .env(FILE, &file)
        .env("PATH", temp.path())
        .env("PATHKEEP_REVEAL_TEST_LOG", &log)
        .env(
            "DBUS_SESSION_BUS_ADDRESS",
            format!("unix:path={}", temp.path().join("absent").display()),
        )
        .output()
        .unwrap();
    assert!(output.status.success(), "{output:?}");
    assert_eq!(std::fs::read_to_string(log).unwrap(), temp.path().to_str().unwrap());
}
