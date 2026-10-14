//! Responsibilities: one request per connection, bounded progress and blocking dispatch.
//! Not responsible for: acquiring the archive owner lock or business authorization.
//! The app must hold its owner lock before starting this service and until it stops.
//! Failure modes: stalled readers grow memory; panic loses final reply; disconnected
//! clients abort committed work; stop releases ownership during work. Tests: transport.rs.
use crate::{
    ErrorCode, Request, RpcError,
    protocol::{self, Body, Event, VERSION},
    transport,
};
use serde_json::Value;
use std::{
    path::Path,
    sync::{Arc, mpsc},
    thread,
};
use tokio::{
    io::{AsyncRead, AsyncWrite},
    sync::{Semaphore, mpsc as channel, oneshot},
};

type Handler = dyn Fn(Request, Progress) -> Result<Value, RpcError> + Send + Sync + 'static;

/// Bounded delivery applies backpressure; a disconnected client does not cancel a backup.
#[derive(Clone)]
pub struct Progress(channel::Sender<Value>);
impl Progress {
    pub fn send(&self, value: Value) {
        let _ = self.0.blocking_send(value);
    }
}

pub struct Server {
    stop: Option<oneshot::Sender<()>>,
    thread: Option<thread::JoinHandle<()>>,
}
impl Server {
    /// Returns only after binding succeeds, so the app can report startup failures.
    pub fn start(
        root: &Path,
        handler: impl Fn(Request, Progress) -> Result<Value, RpcError> + Send + Sync + 'static,
    ) -> Result<Self, RpcError> {
        let root = root.to_owned();
        let (ready_tx, ready_rx) = mpsc::sync_channel(1);
        let (stop, stopped) = oneshot::channel();
        let thread = thread::Builder::new().name("owner-ipc".into()).spawn(move || {
            let runtime = match tokio::runtime::Builder::new_multi_thread()
                .worker_threads(2)
                .enable_all()
                .build()
            {
                Ok(runtime) => runtime,
                Err(error) => {
                    let _ = ready_tx.send(Err(error.into()));
                    return;
                }
            };
            runtime.block_on(async move {
                let mut listener = match transport::Listener::bind(&root).await {
                    Ok(listener) => listener,
                    Err(error) => {
                        let _ = ready_tx.send(Err(error.into()));
                        return;
                    }
                };
                let handler: Arc<Handler> = Arc::new(handler);
                let slots = Arc::new(Semaphore::new(16));
                let _ = ready_tx.send(Ok(()));
                tokio::pin!(stopped);
                loop {
                    tokio::select! {
                        _ = &mut stopped => break,
                        accepted = listener.accept() => {
                            let Ok(stream) = accepted else { break };
                            let Ok(slot) = slots.clone().try_acquire_owned() else { continue };
                            let handler = handler.clone();
                            tokio::spawn(async move {
                                let _slot = slot;
                                let _ = serve(stream, handler).await;
                            });
                        }
                    }
                }
            });
            // Runtime drop waits for blocking handlers. The app's lock must outlive this join.
        })?;
        match ready_rx.recv().map_err(|_| {
            RpcError::new(ErrorCode::Unavailable, "owner IPC startup thread stopped")
        })? {
            Ok(()) => Ok(Self { stop: Some(stop), thread: Some(thread) }),
            Err(error) => {
                let _ = thread.join();
                Err(error)
            }
        }
    }
}
impl Drop for Server {
    fn drop(&mut self) {
        if let Some(stop) = self.stop.take() {
            let _ = stop.send(());
        }
        if let Some(thread) = self.thread.take() {
            let _ = thread.join();
        }
    }
}

async fn serve(
    mut stream: impl AsyncRead + AsyncWrite + Unpin,
    handler: Arc<Handler>,
) -> Result<(), RpcError> {
    let request = tokio::time::timeout(
        std::time::Duration::from_secs(5),
        protocol::read::<_, Request>(&mut stream),
    )
    .await
    .map_err(|_| RpcError::new(ErrorCode::Protocol, "owner request frame timed out"))?;
    let request = match request {
        Ok(request) if request.version == VERSION => request,
        other => {
            let error = match other {
                Ok(request) => RpcError::new(
                    ErrorCode::VersionMismatch,
                    format!(
                        "unsupported owner protocol version {}; expected {VERSION}",
                        request.version
                    ),
                ),
                Err(error) => error,
            };
            return protocol::write(
                &mut stream,
                &Event { version: VERSION, sequence: 0, body: Body::Error { error } },
            )
            .await;
        }
    };
    let (tx, mut rx) = channel::channel(16);
    let job = tokio::task::spawn_blocking(move || handler(request, Progress(tx)));
    let mut sequence = 0;
    while let Some(value) = rx.recv().await {
        send(&mut stream, sequence, Body::Progress { value }).await?;
        sequence += 1;
    }
    let result = job.await.unwrap_or_else(|_| {
        Err(RpcError::new(ErrorCode::OperationFailed, "owner operation panicked"))
    });
    let body = match result {
        Ok(value) => Body::Result { value },
        Err(error) => Body::Error { error },
    };
    send(&mut stream, sequence, body).await
}
async fn send(
    stream: &mut (impl AsyncWrite + Unpin),
    sequence: u64,
    body: Body,
) -> Result<(), RpcError> {
    let event = Event { version: VERSION, sequence, body };
    tokio::time::timeout(std::time::Duration::from_secs(30), async {
        match protocol::write(stream, &event).await {
            Err(error) if error.code == ErrorCode::Protocol => {
                // Encoding failed before any bytes were sent. Finish this connection with
                // a small typed error, even when the oversized value was progress.
                protocol::write(
                    stream,
                    &Event {
                        version: VERSION,
                        sequence,
                        body: Body::Error { error: error.clone() },
                    },
                )
                .await?;
                Err(error)
            }
            result => result,
        }
    })
    .await
    .map_err(|_| RpcError::new(ErrorCode::Unavailable, "owner client stopped reading"))?
}
