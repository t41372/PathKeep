//! Responsibilities: keep user edits canonical and mirror only pending changed rows.
//! Not responsible for: native metadata, query locks or permanent deletion.
use super::Archive;
use crate::{Error, Result};
use serde::{Deserialize, Serialize};
use vault_store::{Db, Priority, params};

/// Parser output cannot overwrite fields owned by the user.
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct ConversationMark {
    pub starred: Option<bool>,
    pub put_away: Option<bool>,
    pub trash: Option<bool>,
    /// None leaves the title unchanged; Some(None) clears the override.
    pub title: Option<Option<String>>,
}

impl Archive {
    /// Publishes one reversible edit without blocking readers or backup publication.
    pub fn mark_conversation(
        &self,
        id: &str,
        mark: &ConversationMark,
        execute: bool,
    ) -> Result<crate::query::SessionView> {
        let _job = self.jobs.enter(None)?;
        let item = self.session(id).map_err(|error| {
            if matches!(error, Error::NotFound(_)) { Error::ConversationNotFound } else { error }
        })?;
        if !execute {
            return Ok(item);
        }
        let state = self
            .state_gate
            .lock()
            .map_err(|_| Error::Integrity("state writer was interrupted".into()))?;
        self.replay_pending_state()?;
        let id = id.to_owned();
        let mark = mark.clone();
        let now = chrono::Utc::now().timestamp_millis();
        self.store.write(Db::AgentHistoryEvidence, Priority::Interactive, move |tx| {
            tx.execute(
                "INSERT OR IGNORE INTO ah_user_state(conversation_id) VALUES(?1)",
                [id.as_str()],
            )?;
            if let Some(value) = mark.starred {
                tx.execute(
                    "UPDATE ah_user_state SET starred=?1 WHERE conversation_id=?2",
                    params![i64::from(value), id.clone()],
                )?;
            }
            if let Some(value) = mark.put_away {
                tx.execute(
                    "UPDATE ah_user_state SET put_away=?1 WHERE conversation_id=?2",
                    params![i64::from(value), id.clone()],
                )?;
            }
            if let Some(value) = mark.trash {
                tx.execute(
                    "UPDATE ah_user_state SET trashed_at=CASE WHEN ?1=1 THEN
                        coalesce(trashed_at, ?2) ELSE NULL END WHERE conversation_id=?3",
                    params![i64::from(value), now, id.clone()],
                )?;
            }
            if let Some(value) = mark.title {
                tx.execute(
                    "UPDATE ah_user_state SET title=?1 WHERE conversation_id=?2",
                    params![value, id.clone()],
                )?;
            }
            tx.execute("UPDATE ah_state_generation SET generation=generation+1 WHERE id=1", ())?;
            tx.execute(
                "INSERT INTO ah_user_state_pending(conversation_id, generation) SELECT ?1,
                    generation FROM ah_state_generation WHERE id=1 ON CONFLICT(conversation_id) DO
                    UPDATE SET generation=excluded.generation",
                [id],
            )?;
            Ok(())
        })?;
        self.replay_pending_state()?;
        drop(state);
        self.session(&item.id)
    }

    pub(crate) fn state_visibility(&self) -> Result<i64> {
        Ok(self.store.read(Db::AgentHistoryReadModel, |tx| {
            tx.query_one("SELECT visibility FROM ah_state_generation WHERE id=1", (), |row| {
                Ok(row.get(0)?)
            })
        })?)
    }

    pub(crate) fn synchronize_user_state(&self) -> Result<()> {
        let _state = self
            .state_gate
            .lock()
            .map_err(|_| Error::Integrity("state writer was interrupted".into()))?;
        self.replay_pending_state()
    }

    fn replay_pending_state(&self) -> Result<()> {
        let target = self.store.read(Db::AgentHistoryEvidence, |tx| {
            tx.query_one("SELECT generation FROM ah_state_generation WHERE id=1", (), |row| {
                Ok(row.get::<i64>(0)?)
            })
        })?;
        let applied = self.store.read(Db::AgentHistoryReadModel, |tx| {
            tx.query_one("SELECT generation FROM ah_state_generation WHERE id=1", (), |row| {
                Ok(row.get::<i64>(0)?)
            })
        })?;
        if applied != target {
            let mut after = String::new();
            loop {
                let cursor = after.clone();
                let rows = self.store.read(Db::AgentHistoryEvidence, move |tx| {
                    tx.query(
                        "SELECT u.conversation_id, u.starred, u.put_away, u.trashed_at, u.title
                            FROM ah_user_state_pending p JOIN ah_user_state u ON
                            u.conversation_id=p.conversation_id WHERE p.conversation_id>?1 AND
                            p.generation<=?2 ORDER BY p.conversation_id LIMIT 128",
                        params![cursor, target],
                        |row| {
                            Ok((
                                row.get::<String>(0)?,
                                row.get::<i64>(1)?,
                                row.get::<i64>(2)?,
                                row.get::<Option<i64>>(3)?,
                                row.get::<Option<String>>(4)?,
                            ))
                        },
                    )
                })?;
                if rows.is_empty() {
                    break;
                }
                after = rows.last().unwrap().0.clone();
                self.store.write(Db::AgentHistoryReadModel, Priority::Interactive, move |tx| {
                    for (id, star, away, trash, title) in rows {
                        tx.execute(
                            "INSERT INTO ah_user_state VALUES(?1, ?2, ?3, ?4, ?5) ON
                                CONFLICT(conversation_id) DO UPDATE SET starred=excluded.starred,
                                put_away=excluded.put_away, trashed_at=excluded.trashed_at,
                                title=excluded.title",
                            params![id, star, away, trash, title],
                        )?;
                    }
                    tx.execute(
                        "UPDATE ah_state_generation SET visibility=visibility+1 WHERE id=1",
                        (),
                    )?;
                    Ok(())
                })?;
            }
            self.store.write(Db::AgentHistoryReadModel, Priority::Interactive, move |tx| {
                tx.execute("UPDATE ah_state_generation SET generation=?1 WHERE id=1", [target])?;
                Ok(())
            })?;
        }
        // A crash before cleanup replays harmlessly. Never acknowledge changes
        // before their hot rows are committed, or skip a previously failed mirror.
        self.store.write(Db::AgentHistoryEvidence, Priority::Interactive, move |tx| {
            tx.execute("DELETE FROM ah_user_state_pending WHERE generation<=?1", [target])?;
            Ok(())
        })?;
        Ok(())
    }
}
