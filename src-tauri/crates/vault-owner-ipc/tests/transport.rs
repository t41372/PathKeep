//! Responsibilities: exercise framing, local access and streaming over real endpoints.
//! Not responsible for: business operations or app lifecycle (desktop process tests).
//! Failure modes: stale endpoints prevent restart; live endpoints get replaced; another
//! user can connect; mismatched versions execute work; progress is reordered/lost;
//! malformed/truncated/oversized frames allocate without bounds or hang a request.
use serde_json::json;
use std::sync::{
    Arc,
    atomic::{AtomicUsize, Ordering},
};
use vault_owner_ipc::{
    Client, ErrorCode, Request, Server,
    protocol::{self, Event},
};

#[test]
fn progress_is_ordered_before_final_reply_and_existing_owner_is_not_replaced() {
    let root = tempfile::tempdir().unwrap();
    let count = Arc::new(AtomicUsize::new(0));
    let calls = count.clone();
    let _server = Server::start(root.path(), move |_, progress| {
        calls.fetch_add(1, Ordering::SeqCst);
        for index in 0..40 {
            progress.send(json!({"index": index}));
        }
        Ok(json!({"done": true}))
    })
    .unwrap();
    assert!(Server::start(root.path(), |_, _| Ok(json!(null))).is_err());
    let rt = tokio::runtime::Runtime::new().unwrap();
    let mut events = Vec::new();
    let reply = rt
        .block_on(
            Client::new(root.path()).request(Request::new("ping", json!({})), |v| events.push(v)),
        )
        .unwrap();
    assert_eq!(reply, json!({"done":true}));
    assert_eq!(events, (0..40).map(|index| json!({"index":index})).collect::<Vec<_>>());
    assert_eq!(count.load(Ordering::SeqCst), 1);
}

#[test]
fn protocol_mismatch_returns_typed_error_without_dispatch() {
    let root = tempfile::tempdir().unwrap();
    let _server = Server::start(root.path(), |_, _| panic!("must not dispatch")).unwrap();
    let rt = tokio::runtime::Runtime::new().unwrap();
    let mut request = Request::new("ping", json!({}));
    request.version += 1;
    let error = rt.block_on(Client::new(root.path()).request(request, |_| {})).unwrap_err();
    assert_eq!(error.code, ErrorCode::VersionMismatch);
    assert!(error.message.contains("version"));
}

#[cfg(unix)]
#[test]
fn stale_socket_is_recovered_and_directory_and_socket_are_user_only() {
    use std::os::unix::{fs::PermissionsExt, net::UnixListener};
    let root = tempfile::tempdir().unwrap();
    std::fs::create_dir(root.path().join("ipc")).unwrap();
    let path = root.path().join("ipc/owner.sock");
    drop(UnixListener::bind(&path).unwrap());
    let server = Server::start(root.path(), |_, _| Ok(json!(true))).unwrap();
    assert_eq!(std::fs::metadata(&path).unwrap().permissions().mode() & 0o777, 0o600);
    assert_eq!(
        std::fs::metadata(path.parent().unwrap()).unwrap().permissions().mode() & 0o777,
        0o700
    );
    drop(server);
    assert!(!path.exists());
}

#[tokio::test]
async fn malformed_truncated_and_oversized_frames_fail() {
    use tokio::io::AsyncWriteExt;
    for bytes in
        [vec![0, 0, 0, 2, b'{', b'}'], vec![0, 0, 0, 5, b'{'], u32::MAX.to_be_bytes().to_vec()]
    {
        let (mut tx, mut rx) = tokio::io::duplex(64);
        tx.write_all(&bytes).await.unwrap();
        drop(tx);
        assert!(protocol::read::<_, Event>(&mut rx).await.is_err());
    }
}

#[test]
fn oversized_reply_is_typed_error_and_owner_survives() {
    let root = tempfile::tempdir().unwrap();
    let _server = Server::start(root.path(), |request, _| {
        if request.method == "large" {
            Ok(json!("x".repeat(protocol::MAX_FRAME_BYTES)))
        } else {
            Ok(json!(true))
        }
    })
    .unwrap();
    let rt = tokio::runtime::Runtime::new().unwrap();
    let client = Client::new(root.path());
    let error = rt.block_on(client.request(Request::new("large", json!({})), |_| {})).unwrap_err();
    assert_eq!(error.code, ErrorCode::Protocol);
    assert_eq!(
        rt.block_on(client.request(Request::new("ping", json!({})), |_| {})).unwrap(),
        json!(true)
    );
}

#[cfg(unix)]
#[test]
fn symlinked_socket_directory_is_rejected() {
    let root = tempfile::tempdir().unwrap();
    let elsewhere = tempfile::tempdir().unwrap();
    std::os::unix::fs::symlink(elsewhere.path(), root.path().join("ipc")).unwrap();
    assert!(Server::start(root.path(), |_, _| Ok(json!(true))).is_err());
    assert!(!elsewhere.path().join("owner.sock").exists());
}
