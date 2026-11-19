//! Responsibilities: persist interpretation evidence and honest backup-run outcomes.
//! Not responsible for: generating diagnostics, source capture or retry policy.
//! Failure modes: coverage disappears; terminal success is overwritten; restart
//! leaves runs active; unbounded reports exhaust memory or partially commit.
use super::Archive;
use crate::{Error, Result, model::ParseReport};
use serde::{Serialize, Serializer};
use serde_json::Value;
use std::io::{self, Write};
use vault_store::{Db, OptionalRow, Priority, params};

const REPORT_BYTES: usize = 16 * 1024 * 1024;
const RECOVERY_BATCH: i64 = 128;

/// Persistent outcomes distinguish abandoned work from a completed partial backup.
/// The optional report contains only facts the finishing operation committed.
#[derive(Clone, Debug, Serialize)]
pub struct BackupRun {
    #[serde(serialize_with = "serialize_id")]
    pub id: i64,
    pub status: String,
    pub started_at: i64,
    pub finished_at: Option<i64>,
    pub report: Option<Value>,
}

fn serialize_id<S: Serializer>(value: &i64, serializer: S) -> std::result::Result<S::Ok, S::Error> {
    serializer.serialize_str(&value.to_string())
}

impl Archive {
    /// Keeps parser upgrades independent even when they interpret identical raw bytes.
    pub fn save_versioned_parse_report(
        &self,
        revision: i64,
        parser_version: &str,
        report: &ParseReport,
    ) -> Result<()> {
        self.revision(revision)?;
        validate_parser_version(parser_version)?;
        let parser_version = parser_version.to_owned();
        let json = encode(report)?;
        self.store.write(Db::AgentHistoryEvidence, Priority::Batch, move |tx| {
            tx.execute(
                "INSERT INTO ah_parse_reports(revision_id, parser_version, report_json)
                    VALUES(?1, ?2, ?3) ON CONFLICT(revision_id, parser_version) DO UPDATE SET
                    report_json=excluded.report_json",
                params![revision, parser_version, json],
            )?;
            Ok(())
        })?;
        Ok(())
    }

    /// Absence requests a new parse for this interpretation version, independently
    /// of reports retained for an older parser or resource-limit configuration.
    pub fn load_versioned_parse_report(
        &self,
        revision: i64,
        parser_version: &str,
    ) -> Result<Option<ParseReport>> {
        validate_parser_version(parser_version)?;
        let parser_version = parser_version.to_owned();
        let json = self.store.read(Db::AgentHistoryEvidence, move |tx| {
            tx.query_one(
                "SELECT report_json FROM ah_parse_reports WHERE revision_id=?1 AND
                    parser_version=?2",
                params![revision, parser_version],
                |row| Ok(row.get::<String>(0)?),
            )
            .optional()
        })?;
        json.map(|json| decode(&json)).transpose()
    }

    /// Creates the durable running record before a backup begins changing data.
    /// A subsequent owner marks an unfinished record interrupted during open.
    pub fn start_backup_run(&self) -> Result<i64> {
        let now = chrono::Utc::now().timestamp_millis();
        self.store
            .write(Db::AgentHistoryEvidence, Priority::Batch, move |tx| {
                tx.insert(
                    "INSERT INTO ah_backup_runs(status, started_at,public_id) VALUES('running', ?1,lower(hex(randomblob(16))))",
                    [now],
                )?
                .ok_or_else(|| operation("backup run insertion did not produce an identity"))
            })
            .map_err(Into::into)
    }

    /// Ends only a running attempt. A late cleanup/response error cannot rewrite
    /// a previously committed success or partial result as failure.
    pub fn end_backup_run(&self, id: i64, status: &str, report: &Value) -> Result<()> {
        if !["complete", "partial", "failed", "interrupted"].contains(&status) {
            return Err(Error::InvalidInput("backup run requires a terminal status".into()));
        }
        let report = encode(report)?;
        let status = status.to_owned();
        let now = chrono::Utc::now().timestamp_millis();
        self.store.write(Db::AgentHistoryEvidence, Priority::Batch, move |tx| {
            let succeeded=status=="complete";
            if matches!(status.as_str(),"complete"|"partial"){
                // Claim the alert in the terminal transaction. A crash before
                // publication cannot consume a tool/version's single notice.
                tx.execute("INSERT OR IGNORE INTO ah_format_notices(tool,version,run_id)
                    SELECT DISTINCT tool,version,run_id FROM ah_run_note_counts WHERE run_id=?1 AND version<>''
                    AND EXISTS(SELECT 1 FROM ah_desktop_backups WHERE run_id=?1)",[id])?;
            }
            let changed = tx.execute(
                "UPDATE ah_backup_runs SET status=?1, finished_at=?2, report_json=?3 WHERE
                    id=?4 AND status='running'",
                params![status, now, report, id],
            )?;
            if changed != 1 {
                return Err(operation("backup run is absent or already terminal"));
            }
            if succeeded {
                tx.execute("WITH RECURSIVE parents(task_id) AS (
                    SELECT retry_of FROM ah_desktop_backups WHERE run_id=?1 AND retry_of IS NOT NULL
                    UNION SELECT d.retry_of FROM ah_desktop_backups d JOIN parents p ON p.task_id=d.task_id WHERE d.retry_of IS NOT NULL)
                    UPDATE ah_desktop_backups SET acknowledged=1 WHERE task_id IN (SELECT task_id FROM parents)",[id])?;
            }
            Ok(())
        })?;
        Ok(())
    }

    /// Reads durable run evidence for audit and recovery interfaces without
    /// inferring success from the latest available transcript.
    pub fn backup_run(&self, id: i64) -> Result<BackupRun> {
        let _job = self.jobs.enter(None)?;
        let stored =
            self.store
                .read(Db::AgentHistoryEvidence, move |tx| {
                    tx.query_one(
                "SELECT id, status, started_at, finished_at, report_json FROM ah_backup_runs
                    WHERE id=?1",
                [id],
                |row| Ok((
                    row.get::<i64>(0)?, row.get::<String>(1)?, row.get::<i64>(2)?,
                    row.get::<Option<i64>>(3)?, row.get::<Option<String>>(4)?,
                )),
            ).optional()
                })?
                .ok_or_else(|| Error::NotFound("backup run".into()))?;
        Ok(BackupRun {
            id: stored.0,
            status: stored.1,
            started_at: stored.2,
            finished_at: stored.3,
            report: stored.4.map(|json| decode(&json)).transpose()?,
        })
    }

    /// Recovery is bounded and only changes running attempts; finished outcomes
    /// are immutable even when opening the archive after a process exit.
    pub(super) fn recover_backup_runs(&self) -> Result<()> {
        loop {
            let now = chrono::Utc::now().timestamp_millis();
            let changed =
                self.store.write(Db::AgentHistoryEvidence, Priority::Batch, move |tx| {
                    tx.execute(
                        "UPDATE ah_backup_runs SET status='interrupted', finished_at=?1,
                        report_json=coalesce(report_json, ?2) WHERE id IN (SELECT id FROM
                        ah_backup_runs WHERE status='running' ORDER BY id LIMIT ?3)",
                        params![
                            now,
                            r#"{"status":"interrupted","reason":"owner_restarted"}"#,
                            RECOVERY_BATCH
                        ],
                    )
                })?;
            if changed == 0 {
                return Ok(());
            }
        }
    }
}

fn validate_parser_version(version: &str) -> Result<()> {
    if version.is_empty() || version.len() > 256 {
        return Err(Error::InvalidInput("parser identity must be 1..=256 bytes".into()));
    }
    Ok(())
}

fn encode(value: &impl Serialize) -> Result<String> {
    let mut output = BoundedJson { bytes: Vec::new(), exceeded: false };
    if let Err(error) = serde_json::to_writer(&mut output, value) {
        return Err(if output.exceeded {
            Error::LimitExceeded("archive report exceeds 16 MiB".into())
        } else {
            Error::Json(error)
        });
    }
    String::from_utf8(output.bytes)
        .map_err(|_| Error::Integrity("JSON serialization produced invalid UTF-8".into()))
}

fn decode<T: serde::de::DeserializeOwned>(json: &str) -> Result<T> {
    if json.len() > REPORT_BYTES {
        return Err(Error::LimitExceeded("stored archive report exceeds 16 MiB".into()));
    }
    Ok(serde_json::from_str(json)?)
}

struct BoundedJson {
    bytes: Vec<u8>,
    exceeded: bool,
}

impl Write for BoundedJson {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        if bytes.len() > REPORT_BYTES.saturating_sub(self.bytes.len()) {
            self.exceeded = true;
            return Err(io::Error::other("archive report size limit"));
        }
        self.bytes.extend_from_slice(bytes);
        Ok(bytes.len())
    }

    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

fn operation(message: &'static str) -> vault_store::Error {
    vault_store::Error::Operation(Box::new(io::Error::new(io::ErrorKind::InvalidInput, message)))
}
