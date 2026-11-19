//! Identifies visible visit membership for semantic coverage and its admission checks.
//! The archive owns membership changes. This module only reads their committed stamp.
use crate::Vault;
use anyhow::Result;
use vault_store::{Db, Session};

/// A completed scan that cannot certify a single version of the visible visits.
pub(super) const MIXED: i64 = -1;

/// Reads the same membership token at capture and in the publication transaction.
pub(super) fn read(s: &Session<'_>, database: Db) -> Result<i64> {
    let schema = match database {
        Db::Archive => "main",
        Db::Intelligence => "archive",
        _ => anyhow::bail!("visit membership needs the canonical archive"),
    };
    let (count, token) = s.query_one(
        &format!(
            "SELECT value,(SELECT value FROM {schema}.archive_totals
            WHERE name='visitMembershipToken') FROM {schema}.archive_totals WHERE name='visits'"
        ),
        (),
        |r| Ok((r.get::<i64>(0)?, r.get::<Option<i64>>(1)?)),
    )?;
    if let Some(token) = token {
        // Tokens occupy the range below the three reserved / legacy values.
        return (-2_i64)
            .checked_sub(token)
            .filter(|&stamp| stamp < -2)
            .ok_or_else(|| anyhow::anyhow!("invalid visit membership token"));
    }
    // Keep the released watermark until the first membership change. A historical
    // nonzero total can outlive the last visible row.
    if count == 0 {
        return Ok(-2);
    }
    let last = s.query_one(
        &format!(
            "SELECT (SELECT id FROM {schema}.visits WHERE reverted_at IS NULL
            ORDER BY id DESC LIMIT 1)"
        ),
        (),
        |r| Ok(r.get::<Option<i64>>(0)?),
    )?;
    Ok(last.map_or(-2, |id| (((count << 32) ^ id) & i64::MAX).max(1)))
}

pub(super) fn current(vault: &Vault) -> Result<i64> {
    vault.read(Db::Archive, |s| read(s, Db::Archive))
}

pub(super) fn stale(published: i64, current: i64) -> bool {
    published == MIXED || (published != 0 && published != current)
}
