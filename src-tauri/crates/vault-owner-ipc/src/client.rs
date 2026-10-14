//! Responsibilities: connect, optionally start the app, and consume one ordered reply.
//! Not responsible for: opening databases, credentials, or retrying submitted work.
//! Failure modes: startup waits forever; a failed connection replays mutations;
//! progress arrives out of order; incompatible server. Tests: transport.rs.
use crate::{
    ErrorCode, Request, RpcError,
    protocol::{self, Body, Event, VERSION},
    transport,
};
use serde_json::Value;
use std::{
    path::{Path, PathBuf},
    time::Duration,
};

#[derive(Debug, Clone)]
pub struct Client {
    root: PathBuf,
}
impl Client {
    /// The root identifies the owner's endpoint; this constructor performs no I/O.
    pub fn new(root: impl AsRef<Path>) -> Self {
        Self { root: root.as_ref().into() }
    }

    /// Connects without starting an app, useful for readiness and integration tests.
    pub async fn request(
        &self,
        request: Request,
        progress: impl FnMut(Value),
    ) -> Result<Value, RpcError> {
        let stream = transport::connect(&self.root).await?;
        exchange(stream, request, progress).await
    }

    /// Uses the production background launcher with a bounded readiness wait.
    pub async fn request_or_start(
        &self,
        request: Request,
        progress: impl FnMut(Value),
    ) -> Result<Value, RpcError> {
        self.request_with_start(request, progress, Duration::from_secs(30), || {
            crate::launch_background(&self.root)
        })
        .await
    }

    /// Starts only on a missing/refused endpoint. Once a request is sent it is never replayed.
    pub async fn request_with_start(
        &self,
        request: Request,
        progress: impl FnMut(Value),
        wait: Duration,
        start: impl FnOnce() -> Result<(), RpcError>,
    ) -> Result<Value, RpcError> {
        let stream = match transport::connect(&self.root).await {
            Ok(stream) => stream,
            Err(error) if transport::not_running(&error) => {
                start()?;
                tokio::time::timeout(wait, async {
                    loop {
                        match transport::connect(&self.root).await {
                            Ok(stream) => return Ok(stream),
                            Err(error) if transport::not_running(&error) => tokio::time::sleep(Duration::from_millis(50)).await,
                            Err(error) => return Err(RpcError::from(error)),
                        }
                    }
                }).await.map_err(|_| RpcError::new(ErrorCode::Unavailable,
                    format!("PathKeep did not make its owner endpoint ready within {} seconds; start PathKeep and check its startup log", wait.as_secs())))??
            }
            Err(error) => return Err(error.into()),
        };
        exchange(stream, request, progress).await
    }
}

async fn exchange(
    mut stream: impl tokio::io::AsyncRead + tokio::io::AsyncWrite + Unpin,
    request: Request,
    mut progress: impl FnMut(Value),
) -> Result<Value, RpcError> {
    protocol::write(&mut stream, &request).await?;
    for sequence in 0u64.. {
        let event: Event = protocol::read(&mut stream).await?;
        if event.version != VERSION {
            return Err(RpcError::new(
                ErrorCode::VersionMismatch,
                "incompatible PathKeep owner protocol version",
            ));
        }
        if event.sequence != sequence {
            return Err(RpcError::new(
                ErrorCode::Protocol,
                "owner IPC progress sequence is out of order",
            ));
        }
        match event.body {
            Body::Progress { value } => progress(value),
            Body::Result { value } => return Ok(value),
            Body::Error { error } => return Err(error),
        }
    }
    unreachable!()
}
