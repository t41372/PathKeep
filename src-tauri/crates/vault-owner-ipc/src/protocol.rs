//! Responsibilities: versioned, bounded, length-prefixed JSON and typed failures.
//! Not responsible for: transport, authentication or interpreting operation parameters.
//! Failure modes: malformed/truncated/oversized frames; incompatible versions;
//! missing or reordered final replies. Tests: transport.rs.
use serde::{Deserialize, Serialize, de::DeserializeOwned};
use serde_json::Value;
use tokio::io::{AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt};

pub const VERSION: u32 = 1;
/// A wire bound, independent of archive size. Bulk archive export is not an IPC method.
pub const MAX_FRAME_BYTES: usize = 8 * 1024 * 1024;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Request {
    pub version: u32,
    pub method: String,
    pub params: Value,
}
impl Request {
    /// Keeps clients on the current protocol without repeating envelope details.
    pub fn new(method: impl Into<String>, params: Value) -> Self {
        Self { version: VERSION, method: method.into(), params }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ErrorCode {
    VersionMismatch,
    InvalidRequest,
    UnknownMethod,
    OperationFailed,
    Unavailable,
    Protocol,
}

#[derive(Debug, Clone, Serialize, Deserialize, thiserror::Error)]
#[error("{message}")]
pub struct RpcError {
    pub code: ErrorCode,
    pub message: String,
}
impl RpcError {
    pub fn new(code: ErrorCode, message: impl Into<String>) -> Self {
        Self { code, message: message.into() }
    }
}
impl From<std::io::Error> for RpcError {
    fn from(error: std::io::Error) -> Self {
        Self::new(ErrorCode::Unavailable, error.to_string())
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Event {
    pub version: u32,
    pub sequence: u64,
    #[serde(flatten)]
    pub body: Body,
}
#[derive(Debug, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum Body {
    Progress { value: Value },
    Result { value: Value },
    Error { error: RpcError },
}

/// Rejects a frame's length before allocating its payload.
pub async fn read<R: AsyncRead + Unpin, T: DeserializeOwned>(
    reader: &mut R,
) -> Result<T, RpcError> {
    let length = reader.read_u32().await? as usize;
    if length == 0 || length > MAX_FRAME_BYTES {
        return Err(RpcError::new(ErrorCode::Protocol, "invalid owner IPC frame length"));
    }
    let mut bytes = vec![0; length];
    reader.read_exact(&mut bytes).await?;
    serde_json::from_slice(&bytes)
        .map_err(|e| RpcError::new(ErrorCode::InvalidRequest, e.to_string()))
}

/// Writes one complete frame; a connection has exactly one writer.
pub async fn write<W: AsyncWrite + Unpin, T: Serialize>(
    writer: &mut W,
    value: &T,
) -> Result<(), RpcError> {
    let bytes =
        serde_json::to_vec(value).map_err(|e| RpcError::new(ErrorCode::Protocol, e.to_string()))?;
    if bytes.len() > MAX_FRAME_BYTES {
        return Err(RpcError::new(ErrorCode::Protocol, "owner IPC reply exceeds frame limit"));
    }
    writer.write_u32(bytes.len() as u32).await?;
    writer.write_all(&bytes).await?;
    writer.flush().await?;
    Ok(())
}
