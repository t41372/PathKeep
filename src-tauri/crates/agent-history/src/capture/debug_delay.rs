//! Debug-only pacing lets desktop tests observe and cancel real source work.
use crate::{Error, Result};
use std::time::Duration;
use vault_store::CancelToken;

pub(super) fn configured(name: &str) -> Duration {
    if cfg!(debug_assertions) {
        let micros = std::env::var(name).ok().and_then(|s| s.parse::<u64>().ok()).unwrap_or(0);
        Duration::from_micros(micros)
    } else {
        Duration::ZERO
    }
}

pub(super) fn wait(mut remaining: Duration, cancel: &CancelToken) -> Result<()> {
    while !remaining.is_zero() {
        cancel.check().map_err(|_| Error::Cancelled)?;
        let slice = remaining.min(Duration::from_millis(10));
        std::thread::sleep(slice);
        remaining -= slice;
    }
    Ok(())
}
