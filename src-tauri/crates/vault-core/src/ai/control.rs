//! Cooperative cancellation helpers for long-running AI work.
//!
//! ## Responsibilities
//! - normalize safe-boundary cancellation checkpoints for index and assistant runs
//! - poll cooperative stop requests while async provider/network work is in flight
//! - keep cancellation semantics consistent across semantic indexing and assistant answers
//!
//! ## Not responsible for
//! - provider validation or network client setup
//! - semantic indexing, semantic search, or assistant prompt composition
//! - persisted queue or run-ledger state transitions
//!
//! ## Dependencies
//! - `super::AiRunControl` for the shared cancellation trait
//! - Tokio timing primitives already imported by the parent `ai` module
//!
//! ## Performance notes
//! - polling is intentionally coarse (`250ms`) so stop requests are observed without
//!   spinning the executor or materially affecting provider throughput

use super::*;

/// Checks whether the current AI run was asked to stop at this explicit safe boundary.
///
/// Long-running AI work should only stop at points where partial progress is still
/// consistent on disk. This helper keeps those boundaries explicit instead of sprinkling
/// ad-hoc cancellation logic through every async call site.
pub(super) fn checkpoint_ai_run(
    control: Option<&Arc<dyn AiRunControl>>,
    detail: &str,
) -> Result<()> {
    if let Some(control) = control {
        control.checkpoint(detail)?;
    }
    Ok(())
}

/// Awaits one async provider step while periodically re-checking cooperative cancellation.
///
/// This wrapper is needed because provider calls can block for noticeable time. It lets
/// PathKeep stay responsive to stop requests without forcing every embedding or LLM client
/// call to grow its own polling loop.
pub(super) async fn await_with_ai_cancellation<T, F>(
    control: Option<&Arc<dyn AiRunControl>>,
    detail: &str,
    future: F,
) -> Result<T>
where
    F: Future<Output = Result<T>>,
{
    checkpoint_ai_run(control, detail)?;
    if control.is_none() {
        return future.await;
    }

    tokio::pin!(future);
    loop {
        tokio::select! {
            result = &mut future => return result,
            _ = tokio::time::sleep(Duration::from_millis(250)) => {
                checkpoint_ai_run(control, detail)?;
            }
        }
    }
}
