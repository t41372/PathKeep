//! Responsibilities: let the archive hold visits whose browser row ids were used before.
//! Not responsible for: matching a capture's rows to archived visits (`archive::ingest::identity`)
//! or the readers that follow a visit to the one that opened it.
//!
//! A browser's row id does not identify a visit: Chrome numbers its visits from 1 again when it
//! clears all history or rebuilds a damaged file, and Firefox gives a deleted row's id to its next
//! visit. Version 1 made (profile, row id) unique for visits, URLs and downloads, so the second
//! visit with an id was dropped and the backup still said it succeeded. From version 2 a native
//! visit is its page's address and the browser's own time, kept without loss
//! (`visits.source_visit_time`), and the visit that opened it is named by archive id
//! (`visits.parent_visit_id`).
//!
//! One statement per version: each commits on its own, so building one index never holds the
//! pages of another. Building several in one transaction went past 8 GiB on a 14.4M-visit archive.
//! Visits archived by version 1 keep `source_visit_time` empty until a backup reads their browser
//! row again; version 15 gives them `parent_visit_id` from the row id they name.
use vault_store::{BackfillProgress, Migration, Session, params};

/// Visits per backfill transaction.
const OPENER_BATCH: i64 = 50_000;

pub(super) fn archive() -> Vec<Migration> {
    vec![
        Migration::sql(2, "ALTER TABLE visits ADD COLUMN source_visit_time TEXT"),
        Migration::sql(3, "ALTER TABLE visits ADD COLUMN parent_visit_id INTEGER"),
        // A row id stays the identity of a visit without a browser time: version 1's, until a
        // backup reads its browser row again, and Takeout's synthetic ids. A visit with one may
        // share its row id with an older visit.
        Migration::sql(4, "DROP INDEX idx_visits_profile_source_visit_id"),
        Migration::sql(
            5,
            "CREATE UNIQUE INDEX idx_visits_profile_source_visit_id ON visits(source_profile_id,source_visit_id)
               WHERE source_visit_time IS NULL",
        ),
        // URLs are found by their address; nothing looks one up by its browser row id.
        Migration::sql(6, "DROP INDEX idx_urls_profile_source_url_id"),
        // Chrome numbers a download max+1, so a deleted newest download's id comes back. A
        // download is its guid, or, without one, where it was saved and when it started.
        Migration::sql(7, "DROP INDEX idx_downloads_profile_source_download_id"),
        Migration::sql(
            8,
            "CREATE INDEX idx_downloads_profile_guid ON downloads(source_profile_id,guid)",
        ),
        Migration::sql(
            9,
            "CREATE INDEX idx_downloads_profile_path_time ON downloads(source_profile_id,target_path,start_time_ms)",
        ),
        // The visit row a backup stopped at, as it was then: the next backup checks the row with
        // that id is still the same visit before it reads only the rows after it.
        Migration::sql(10, "ALTER TABLE profile_watermarks ADD COLUMN last_visit_key TEXT"),
        Migration::sql(11, "ALTER TABLE profile_watermarks ADD COLUMN last_visit_url TEXT"),
        Migration::sql(12, "ALTER TABLE profile_watermarks ADD COLUMN last_full_scan_at TEXT"),
        // Set while a read of every row is under way: a backup stopped part way carries on from
        // its cursor instead of starting over.
        Migration::sql(13, "ALTER TABLE profile_watermarks ADD COLUMN full_scan_started_at TEXT"),
        // The newest visit time any backup read: a row at or below the cursor newer than it is a
        // new visit under an old id.
        Migration::sql(
            14,
            "ALTER TABLE profile_watermarks ADD COLUMN newest_visit_ms INTEGER NOT NULL DEFAULT 0",
        ),
        legacy_openers(),
        // The newest run version 1 wrote: evidence it captured is looked up by browser row id for
        // version 1 visits, and evidence from later runs may be another visit's under the same id.
        Migration::rust(16, |s| {
            s.execute(
                "CREATE TABLE identity_upgrade(id INTEGER PRIMARY KEY CHECK(id=1),
                   last_legacy_run INTEGER NOT NULL)",
                (),
            )?;
            s.execute(
                "INSERT INTO identity_upgrade VALUES(1,(SELECT COALESCE(MAX(id),0) FROM runs))",
                (),
            )?;
            Ok(())
        }),
    ]
}

/// Brings the upgrade's version 1 records up to facts the converter copied in after the new
/// archive's migrations ran while it was empty.
pub(crate) fn adopt_copied_runs(s: &Session<'_>) -> vault_store::Result<()> {
    s.execute(
        "UPDATE identity_upgrade SET last_legacy_run=(SELECT COALESCE(MAX(id),0) FROM runs) WHERE id=1",
        (),
    )?;
    Ok(())
}

/// Version 1 named the visit that opened a visit by the browser row id it carried
/// (`from_visit`), unique among a profile's visits then. Before any backup gives a version 1
/// visit its browser time, each such link becomes an archive id, so it survives either visit
/// being matched to its browser row again or neither: navigation then follows
/// `parent_visit_id` alone. A link to a row the archive never had stays empty, as version 1's
/// lookup found nothing either.
fn legacy_openers() -> Migration {
    let observer = crate::vault::upgrade::observer();
    Migration::rust(15, |s| {
        s.execute(
            "CREATE TABLE legacy_opener_backfill(id INTEGER PRIMARY KEY CHECK(id=1),
               cursor INTEGER NOT NULL,processed INTEGER NOT NULL,total INTEGER NOT NULL,
               done INTEGER NOT NULL)",
            (),
        )?;
        restart_legacy_openers(s)
    })
    .with_backfill(link_legacy_openers)
    .on_backfill_progress(move |progress| {
        if let Some(observer) = &observer {
            observer(crate::vault::upgrade::archive_event(progress));
        }
    })
}

/// Starts the backfill from the first visit: at migration, and when the converter has copied
/// version 1 visits into a new archive whose migrations ran while it was empty.
pub(crate) fn restart_legacy_openers(s: &Session<'_>) -> vault_store::Result<()> {
    s.execute(
        "INSERT INTO legacy_opener_backfill(id,cursor,processed,total,done)
           VALUES(1,0,0,(SELECT COUNT(*) FROM visits),0)
         ON CONFLICT(id) DO UPDATE SET cursor=0,processed=0,total=excluded.total,done=0",
        (),
    )?;
    Ok(())
}

/// One bounded transaction of the backfill; `None` once it is done. The cursor commits with the
/// links, so an interrupted upgrade carries on where it stopped.
pub(crate) fn link_legacy_openers(
    s: &Session<'_>,
) -> vault_store::Result<Option<BackfillProgress>> {
    let (cursor, processed, total, done) = s.query_one(
        "SELECT cursor,processed,total,done FROM legacy_opener_backfill WHERE id=1",
        (),
        |r| Ok((r.get::<i64>(0)?, r.get::<i64>(1)?, r.get::<i64>(2)?, r.get::<i64>(3)? != 0)),
    )?;
    if done {
        return Ok(None);
    }
    let rows = s.query(
        "SELECT id,source_profile_id,from_visit,parent_visit_id IS NULL AND source_visit_time IS NULL
         FROM visits WHERE id>?1 ORDER BY id LIMIT ?2",
        params![cursor, OPENER_BATCH],
        |r| Ok((r.get::<i64>(0)?, r.get::<i64>(1)?, r.get::<Option<i64>>(2)?, r.get::<i64>(3)? != 0)),
    )?;
    let Some(last) = rows.last().map(|row| row.0) else {
        s.execute("UPDATE legacy_opener_backfill SET done=1 WHERE id=1", ())?;
        return Ok(Some(BackfillProgress {
            processed: processed as u64,
            total: total.max(processed) as u64,
            table: Some("visits".into()),
            complete: true,
        }));
    };
    let mut find = s.prepare(
        "SELECT id FROM visits INDEXED BY idx_visits_profile_source_visit_id
         WHERE source_profile_id=?1 AND source_visit_id=CAST(?2 AS TEXT) AND source_visit_time IS NULL
         LIMIT 1",
    )?;
    let mut link = s.prepare("UPDATE visits SET parent_visit_id=?2 WHERE id=?1")?;
    for (id, profile, opener, open) in &rows {
        let Some(opener) = opener.filter(|opener| *opener > 0 && *open) else { continue };
        let parent = find
            .query_map(params![*profile, opener], |r| Ok(r.get::<i64>(0)?))?
            .next()
            .transpose()?;
        if let Some(parent) = parent.filter(|parent| parent != id) {
            link.execute(params![*id, parent])?;
        }
    }
    let processed = processed + rows.len() as i64;
    s.execute(
        "UPDATE legacy_opener_backfill SET cursor=?1,processed=?2 WHERE id=1",
        params![last, processed],
    )?;
    Ok(Some(BackfillProgress {
        processed: processed as u64,
        total: total.max(processed) as u64,
        table: Some("visits".into()),
        complete: false,
    }))
}
