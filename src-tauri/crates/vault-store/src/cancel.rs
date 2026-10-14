//! Responsibilities: provide cooperative cancellation and bounded keyset-page walks.
//! Not responsible for: interrupting running statements or holding a whole-walk snapshot.
//! Failure modes: cancellation is missed between pages; walking buffers every row;
//! long walks pin WAL snapshots; invalid batch sizes cannot progress. Tests: cancellation.
use crate::{Db, Error, Result, Session, Store};
use std::{
    num::NonZeroUsize,
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
};

/// Cancellation does not wait behind queued SQL and is safe to signal from any thread.
#[derive(Clone, Default)]
pub struct CancelToken(Arc<AtomicBool>);
impl CancelToken {
    pub fn new() -> Self {
        Self::default()
    }
    pub fn cancel(&self) {
        self.0.store(true, Ordering::Release)
    }
    pub fn check(&self) -> Result<()> {
        if self.0.load(Ordering::Acquire) { Err(Error::cancelled()) } else { Ok(()) }
    }
}
impl Store {
    /// Walk keyset pages in independent short read transactions. `fetch` must obey
    /// `limit`; `consume` runs on the calling thread after that transaction ends.
    /// It returns the next cursor. Use an immutable upper watermark when a stable
    /// export range is required; there is deliberately no whole-walk snapshot.
    pub fn walk<C, T, F, V>(
        &self,
        db: Db,
        cancel: &CancelToken,
        batch: NonZeroUsize,
        mut cursor: C,
        fetch: F,
        mut consume: V,
    ) -> Result<()>
    where
        C: Send + 'static,
        T: Send + 'static,
        F: Fn(&Session<'_>, C, usize) -> Result<Vec<T>> + Send + Sync + 'static,
        V: FnMut(&[T]) -> Result<C>,
    {
        let fetch = Arc::new(fetch);
        loop {
            cancel.check()?;
            let fetch = fetch.clone();
            let token = cancel.clone();
            let limit = batch.get();
            let page = self.read(db, move |conn| {
                token.check()?;
                let page = fetch(conn, cursor, limit)?;
                if page.len() > limit {
                    return Err(Error::InvalidConfig(
                        "walk query exceeded its row batch limit".into(),
                    ));
                }
                Ok(page)
            })?;
            cancel.check()?;
            if page.is_empty() {
                return Ok(());
            }
            let finished = page.len() < limit;
            cursor = consume(&page)?;
            cancel.check()?;
            if finished {
                return Ok(());
            }
        }
    }
}
