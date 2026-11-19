//! Exercise the compiled-out origin and the production downloader/loader with pinned local files.
use std::{path::Path, process::Command};

#[test]
fn release_selector_has_no_local_model_source() {
    let root = tempfile::tempdir().unwrap();
    let exe = root.path().join(format!("probe{}", std::env::consts::EXE_SUFFIX));
    let source =
        Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/builtin-model/release_probe.rs");
    let compile = Command::new(std::env::var_os("RUSTC").unwrap_or_else(|| "rustc".into()))
        .args(["--edition=2024", "-Cdebug-assertions=no"])
        .arg(source)
        .arg("-o")
        .arg(&exe)
        .status()
        .unwrap();
    assert!(compile.success());
    assert!(
        Command::new(exe)
            .env("PATHKEEP_TEST_MODEL_SOURCE", "http://127.0.0.1:1/v2")
            .status()
            .unwrap()
            .success()
    );
}

#[cfg(debug_assertions)]
#[tokio::test]
async fn real_http_fixture_verifies_replaces_and_embeds() {
    use std::io::{BufRead, BufReader, Write};
    use std::net::TcpListener;
    use vault_core::{EmbeddingProvider, EmbeddingRole, StaticEmbeddingProvider};
    // Run with a private process environment even when the Rust test harness uses parallel threads.
    const CHILD: &str = "PATHKEEP_MODEL_SOURCE_TEST_CHILD";
    if std::env::var_os(CHILD).is_none() {
        let status = Command::new(std::env::current_exe().unwrap())
            .args(["--exact", "real_http_fixture_verifies_replaces_and_embeds"])
            .env(CHILD, "1")
            .status()
            .unwrap();
        assert!(status.success());
        return;
    }

    let root = tempfile::tempdir().unwrap();
    let paths = vault_core::project_paths_with_root(root.path());
    let mut config = vault_core::AppConfig::default();
    config.ai.enabled = true;
    config.ai.semantic_index_enabled = true;
    vault_core::save_config(&vault_core::ControlGate::acquire(&paths), &config).unwrap();
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let addr = listener.local_addr().unwrap();
    let (release, resume) = std::sync::mpsc::channel();
    let server = std::thread::spawn(move || {
        let mut held_first_weights = false;
        let mut gets = Vec::new();
        for stream in listener.incoming() {
            let mut stream = stream.unwrap();
            stream.set_read_timeout(Some(std::time::Duration::from_secs(10))).unwrap();
            let mut reader = BufReader::new(&stream);
            let mut line = String::new();
            reader.read_line(&mut line).unwrap();
            for header in reader.lines() {
                if header.unwrap().is_empty() {
                    break;
                }
            }
            let path = line.split_whitespace().nth(1).unwrap();
            if path == "/stop" {
                break;
            }
            let (version, file) = path.trim_start_matches('/').split_once('/').unwrap();
            assert!(matches!(version, "v1" | "v2"));
            assert!(matches!(file, "config.json" | "tokenizer.json" | "model.safetensors"));
            let fixture = Path::new(env!("CARGO_MANIFEST_DIR"))
                .join("tests/fixtures/builtin-model")
                .join(if file == "model.safetensors" { version } else { "common" })
                .join(file);
            let bytes = std::fs::read(fixture).unwrap();
            write!(
                stream,
                "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
                bytes.len()
            )
            .unwrap();
            if !held_first_weights && file == "model.safetensors" {
                held_first_weights = true;
                // Send one genuine progress stride; hold the last chunk until the replacement
                // has received its ID and reported that it is waiting for this response.
                let split = 1024 * 1024;
                stream.write_all(&bytes[..split]).unwrap();
                resume.recv_timeout(std::time::Duration::from_secs(15)).unwrap();
                let _ = stream.write_all(&bytes[split..]);
            } else {
                stream.write_all(&bytes).unwrap();
            }
            gets.push(path.to_string());
        }
        gets
    });
    unsafe {
        std::env::set_var("CHB_PROJECT_ROOT", root.path());
        std::env::set_var("PATHKEEP_TEST_MODEL_SOURCE", format!("http://{addr}/v1"));
    }
    let (tx, cancelled_events) = std::sync::mpsc::channel();
    let (resume_reader, reader_held) = std::sync::mpsc::channel();
    let first = vault_core::ai::download_static_embedding_model(move |event| {
        let hold = matches!(&event.update, vault_core::ModelDownloadUpdate::FileProgress { file, downloaded_bytes, total_bytes } if file == "model.safetensors" && downloaded_bytes < total_bytes);
        tx.send(event).unwrap();
        if hold { reader_held.recv_timeout(std::time::Duration::from_secs(15)).unwrap(); }
    }).unwrap();
    loop {
        let update =
            cancelled_events.recv_timeout(std::time::Duration::from_secs(10)).unwrap().update;
        if matches!(update, vault_core::ModelDownloadUpdate::FileProgress { ref file, downloaded_bytes, total_bytes } if file == "model.safetensors" && downloaded_bytes < total_bytes)
        {
            break;
        }
        assert!(!matches!(
            update,
            vault_core::ModelDownloadUpdate::Error { .. } | vault_core::ModelDownloadUpdate::Done
        ));
    }
    assert!(vault_core::ai::cancel_model_download(&first.transfer_id));
    let mut engines = Vec::new();
    for version in ["v1", "v2"] {
        unsafe {
            std::env::set_var("PATHKEEP_TEST_MODEL_SOURCE", format!("http://{addr}/{version}"));
        }
        let (tx, rx) = std::sync::mpsc::channel();
        let transfer =
            vault_core::ai::download_static_embedding_model(move |event| tx.send(event).unwrap())
                .unwrap();
        if version == "v1" {
            let wait = rx.recv_timeout(std::time::Duration::from_secs(5)).unwrap();
            assert_eq!(wait.transfer_id, transfer.transfer_id);
            assert_eq!(
                wait.update,
                vault_core::ModelDownloadUpdate::Waiting {
                    reason: vault_core::ModelDownloadWait::PreviousTransfer
                }
            );
            let repository =
                vault_core::model_dir_for_repo(&paths, vault_core::DEFAULT_STATIC_MODEL_REPO);
            assert_eq!(
                std::fs::read_dir(repository).unwrap().count(),
                1,
                "only the predecessor owns a candidate"
            );
            release.send(()).unwrap();
            resume_reader.send(()).unwrap();
            assert_eq!(
                cancelled_events.iter().last().unwrap().update,
                vault_core::ModelDownloadUpdate::Cancelled
            );
        }
        loop {
            let event = rx.recv_timeout(std::time::Duration::from_secs(10)).unwrap();
            assert_eq!(event.transfer_id, transfer.transfer_id);
            match event.update {
                vault_core::ModelDownloadUpdate::Done => break,
                vault_core::ModelDownloadUpdate::Error { message } => panic!("{message}"),
                _ => (),
            }
        }
        engines.push(StaticEmbeddingProvider::load_default(&paths).unwrap());
    }
    std::net::TcpStream::connect(addr)
        .unwrap()
        .write_all(b"GET /stop HTTP/1.1\r\nHost: localhost\r\n\r\n")
        .unwrap();
    assert_eq!(
        server.join().unwrap(),
        [
            "/v1/config.json",
            "/v1/tokenizer.json",
            "/v1/model.safetensors",
            "/v1/config.json",
            "/v1/tokenizer.json",
            "/v1/model.safetensors",
            "/v2/model.safetensors"
        ]
    );
    let texts = ["alpha beta".to_string()];
    let old = engines[0].embed(&texts, EmbeddingRole::Query).await.unwrap();
    let new = engines[1].embed(&texts, EmbeddingRole::Query).await.unwrap();
    assert_eq!(old[0].len(), 256);
    assert_ne!(old, new);
}
