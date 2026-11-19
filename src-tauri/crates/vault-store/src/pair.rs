//! Proves that manual retry found the two files last checkpointed together.
//! Business schemas register the seal table. This module only seals a paused pair
//! and invalidates a role's seal in the transaction that first changes its contents.

use crate::{Db, Error, OpenStage, Result, Session, worker::Workers};

/// A later migration version fences writers which would leave a stale seal after changing data.
pub const SCHEMA: &str =
    "CREATE TABLE ah_pair_seal(id INTEGER PRIMARY KEY CHECK(id=1), ticket TEXT NOT NULL)";

pub(crate) fn ticket(s: &Session<'_>) -> Result<Option<String>> {
    if !s.query_one(
        "SELECT EXISTS(SELECT 1 FROM sqlite_schema WHERE name='ah_pair_seal' AND type='table')",
        (),
        |r| Ok(r.get::<i64>(0)? != 0),
    )? {
        return Ok(None);
    }
    let value: Option<String> =
        s.query_one("SELECT max(ticket) FROM ah_pair_seal", (), |r| Ok(r.get(0)?))?;
    if value.as_ref().is_some_and(|s| s.len() != 64 || !s.bytes().all(|b| b.is_ascii_hexdigit())) {
        return Err(Error::PairUnverified);
    }
    Ok(value)
}

pub(crate) fn changes(s: &Session<'_>) -> Result<(i64, i64)> {
    let rows = s.query_one("SELECT total_changes()", (), |r| Ok(r.get(0)?))?;
    let schema = s.runtime.block_on(async {
        let mut rows = s.conn.query("PRAGMA schema_version", ()).await?;
        Ok::<_, Error>(rows.next().await?.unwrap().get(0)?)
    })?;
    Ok((rows, schema))
}

pub(crate) fn seal(workers: &[(Db, Workers)], paths: &crate::Paths) -> Result<()> {
    let get = |db| workers.iter().find(|(id, _)| *id == db).map(|(_, w)| w);
    let (Some(evidence), Some(model)) =
        (get(Db::AgentHistoryEvidence), get(Db::AgentHistoryReadModel))
    else {
        return Ok(());
    };
    let read = |w: &Workers| w.maintenance(|e| ticket(&e.session(true)));
    let left = read(evidence)?;
    if left.is_some() && left == read(model)? {
        return Ok(());
    }
    // A crash while recording a new pair can leave an unsealed role, but must
    // never leave two different nonempty tickets on a formerly consistent pair.
    for w in [evidence, model] {
        w.maintenance(|e| {
            crate::session::transaction(&e.session(false), |s| {
                s.execute("DELETE FROM ah_pair_seal", ())?;
                Ok(())
            })
        })?;
    }
    let nonce: String = evidence.maintenance(|e| {
        e.session(true).query_one("SELECT lower(hex(randomblob(32)))", (), |r| Ok(r.get(0)?))
    })?;
    for (db, w) in [(Db::AgentHistoryEvidence, evidence), (Db::AgentHistoryReadModel, model)] {
        let nonce = nonce.clone();
        w.maintenance(move |e| crate::session::transaction(&e.session(false), |s| {
            s.execute("INSERT INTO ah_pair_seal VALUES(1,?1) ON CONFLICT(id) DO UPDATE SET ticket=excluded.ticket",[nonce])?;
            Ok(())
        })).map_err(|e| e.at_file(db,OpenStage::Recover,paths.get(db)))?;
        w.inspect_seal.store(true, std::sync::atomic::Ordering::Release);
    }
    Ok(())
}
