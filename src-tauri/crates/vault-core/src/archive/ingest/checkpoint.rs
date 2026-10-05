//! Responsibilities: retain a staged browser checkpoint and its Turso provenance.
//! Not responsible for: live-file staging, whole-archive snapshots or scheduling.
//! Failure modes: a failed copy advances checkpoint time; ledger points at missing
//! bytes; multi-profile checkpoints collide. Tests: ingest core and backup suites.

use super::{parser::Watermark, *};
use vault_store::{Db, Priority, params};

pub(super) fn create_checkpoint(
    vault: &crate::Vault,
    run_id: i64,
    snapshot: &ProfileSnapshot,
    hash: &str,
    watermark: &Watermark,
) -> Result<()> {
    let reason = if watermark.last_schema_hash.as_deref() != Some(hash) {
        "source-schema-changed"
    } else {
        "periodic-checkpoint"
    };
    let dir = vault
        .paths()
        .raw_snapshots_dir
        .join(filesystem_safe_path_segment(&snapshot.profile.profile_id))
        .join(format!("{}-{run_id}", now_rfc3339().replace(':', "-")));
    fs::create_dir_all(&dir)?;
    let mut copied = Vec::new();
    let mut bytes = 0;
    for (source, name) in std::iter::once((&snapshot.history_path, "History"))
        .chain(snapshot.favicons_path.as_ref().map(|p| (p, "Favicons")))
    {
        let target = dir.join(name);
        bytes += fs::copy(source, &target)?;
        copied.push((target.display().to_string(), file_sha256_hex(&target)?));
    }
    let checksum = sha256_hex(&serde_json::to_vec(&copied)?);
    let path = dir.display().to_string();
    vault.store().write(Db::Archive,Priority::Batch,move |s| {
        s.execute("INSERT INTO snapshots(run_id,file_path,file_size,checksum,reason,created_at) VALUES(?1,?2,?3,?4,?5,?6)",
            params![run_id,path,bytes as i64,checksum,reason,now_rfc3339()])?;
        crate::archive::totals::add(s, "snapshots", 1)?;
        Ok(())
    })?;
    Ok(())
}
