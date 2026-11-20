//! Bound bytes awaiting Tantivy's indexing thread without committing a partial replacement.
use std::sync::{Arc, Condvar, Mutex};
use tantivy::{Document, TantivyDocument};
#[derive(Default)]
pub(super) struct Queue {
    bytes: Mutex<usize>,
    room: Condvar,
}
impl Queue {
    pub fn wrap(self: &Arc<Self>, doc: TantivyDocument, bytes: usize) -> Queued {
        let mut used = self
            .room
            .wait_while(self.bytes.lock().unwrap(), |n| *n != 0 && *n + bytes > 4 * 1024 * 1024)
            .unwrap();
        *used += bytes;
        Queued { doc, lease: Lease { queue: self.clone(), bytes } }
    }
}
struct Lease {
    queue: Arc<Queue>,
    bytes: usize,
}
impl Drop for Lease {
    fn drop(&mut self) {
        *self.queue.bytes.lock().unwrap() -= self.bytes;
        self.queue.room.notify_one();
    }
}
pub(super) struct Queued {
    doc: TantivyDocument,
    lease: Lease,
}
impl Document for Queued {
    type Value<'a> = <TantivyDocument as Document>::Value<'a>;
    type FieldsValuesIter<'a> = <TantivyDocument as Document>::FieldsValuesIter<'a>;
    fn iter_fields_and_values(&self) -> Self::FieldsValuesIter<'_> {
        self.doc.iter_fields_and_values()
    }
    fn into_tantivy_document(self) -> TantivyDocument {
        drop(self.lease);
        self.doc
    }
}
