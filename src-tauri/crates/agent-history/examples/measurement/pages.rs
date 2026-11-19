//! Scratch page directory shared by the two seek prototypes. This table lives
//! only in a temporary benchmark database and is unrelated to archive migrations.
use vault_store::{Db, Priority, Store, params};

pub const ENTRIES: usize = 256;

pub fn init(store: &Store) {
    store
        .write(Db::AgentHistoryEvidence, Priority::Batch, |tx| {
            tx.execute(
                "CREATE TABLE pages(first_rank INTEGER PRIMARY KEY, data BLOB NOT NULL)",
                (),
            )?;
            Ok(())
        })
        .unwrap()
}
pub fn insert(store: &Store, pages: Vec<(usize, Vec<u8>)>) {
    store
        .write(Db::AgentHistoryEvidence, Priority::Batch, move |tx| {
            let mut statement = tx.prepare("INSERT INTO pages VALUES(?1,?2)")?;
            for (first, data) in pages {
                statement.execute(params![first as i64, data])?;
            }
            Ok(())
        })
        .unwrap()
}
pub fn covering(store: &Store, start: usize, count: usize) -> Vec<(usize, Vec<u8>)> {
    let first = (start / ENTRIES * ENTRIES) as i64;
    let last = (start + count - 1) as i64;
    store.read(Db::AgentHistoryEvidence,move |tx|tx.query("SELECT first_rank,data FROM pages WHERE first_rank BETWEEN ?1 AND ?2 ORDER BY first_rank",params![first,last],|row|Ok((row.get::<i64>(0)? as usize,row.get(1)?)))).unwrap()
}
