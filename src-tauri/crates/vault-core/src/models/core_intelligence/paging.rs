//! Responsibilities: expose complete intelligence pages and their continuation.
//! Not responsible for: SQL ordering or cursor validation.
//! Failure modes: a page size is mistaken for total coverage or continuation is omitted.
use serde::{Deserialize, Serialize};
/// A bounded response whose cursor reaches every remaining record in the same scope.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct IntelligencePage<T> {
    pub items: Vec<T>,
    pub next_cursor: Option<String>,
}
impl<T> std::ops::Deref for IntelligencePage<T> {
    type Target = [T];
    fn deref(&self) -> &[T] {
        &self.items
    }
}
