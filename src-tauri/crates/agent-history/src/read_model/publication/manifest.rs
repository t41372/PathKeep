//! Validates an existing publication before retaining its token. Hash ordered logical
//! rows in bounded batches so a stored input digest alone never authorizes a no-op.
use super::*;
use crate::Archive;
use sha2::{Digest, Sha256};
use vault_store::{OptionalRow, params};

impl Archive {
    pub(super) fn serves_current_versions(&self, owner: &str) -> Result<bool> {
        let owner = owner.to_owned();
        Ok(self.store.read(DB,move|tx|tx.query_one(
            "SELECT NOT EXISTS(SELECT 1 FROM ah_reader_member m LEFT JOIN ah_reader_version v
             ON v.token=m.version LEFT JOIN ah_sessions s ON s.id=m.source
             WHERE m.publication=?1 AND m.version<>'' AND
             (v.token IS NULL OR v.state<>'published' OR v.attempt IS NOT s.projection_id OR v.source IS NOT m.source OR v.codec<>?2))
             AND NOT EXISTS(SELECT 1 FROM ah_reader_line l LEFT JOIN ah_reader_version v ON v.token=l.version
             LEFT JOIN ah_sessions s ON s.id=l.source WHERE l.publication=?1 AND (v.token IS NULL OR v.state<>'published' OR v.source IS NOT l.source OR v.attempt IS NOT s.projection_id OR v.codec<>?2))
             AND NOT EXISTS(SELECT 1 FROM ah_reader_member m JOIN ah_reader_version v ON v.token=m.version
             WHERE m.publication=?1 AND v.pieces<>(SELECT count(*) FROM ah_reader_page p JOIN ah_reader_blob b ON b.hash=p.hash WHERE p.owner=v.token))
             AND NOT EXISTS(SELECT 1 FROM (SELECT DISTINCT version FROM ah_reader_line WHERE publication=?1) l
             JOIN ah_reader_version v ON v.token=l.version WHERE v.pieces<>(SELECT count(*) FROM ah_reader_page p JOIN ah_reader_blob b ON b.hash=p.hash WHERE p.owner=v.token))",
            params![owner,CODEC],|r|Ok(r.get::<i64>(0)?!=0)))?)
    }

    pub(super) fn publication_manifest(
        &self,
        conversation: &str,
        owner: &str,
        input: &str,
        current: Option<&str>,
        cancel: &CancelToken,
    ) -> Result<Vec<u8>> {
        let mut hash = Sha256::new();
        for field in [conversation, owner, input, current.unwrap_or("")] {
            hash.update((field.len() as u64).to_le_bytes());
            hash.update(field);
        }
        for (table, key_column, offset, columns) in [
            (
                "ah_reader_member",
                "source",
                None,
                "source,version,descriptor,records,complete,attachments,reason,visible,stamp",
            ),
            (
                "ah_reader_line",
                "branch",
                Some("first_rank"),
                "branch,first_rank,version,source,records,first_visible,visible,stamp",
            ),
            ("ah_reader_dependency", "target", None, "target"),
            ("ah_reader_route", "target", None, "target"),
        ] {
            hash.update(table.as_bytes());
            let rank = offset.unwrap_or("CAST(0 AS INTEGER)");
            let mut after = String::new();
            let mut last_rank = -1i64;
            let sql = format!(
                "SELECT {key_column},{rank},{columns} FROM {table}
                WHERE publication=?1 AND ({key_column},{rank})>(?2,?3)
                ORDER BY {key_column},{rank} LIMIT 128"
            );
            loop {
                cancel.check().map_err(|_| Error::Cancelled)?;
                let key = owner.to_owned();
                let cursor = after.clone();
                let query = sql.clone();
                let batch = self.store.read(DB, move |tx| {
                    tx.query(&query, params![key, cursor, last_rank], |r| {
                        let mut row_hash = Sha256::new();
                        for i in 2..r.column_count() {
                            vault_store::hash_value(&mut row_hash, r.get_value(i)?);
                        }
                        Ok((r.get::<String>(0)?, r.get::<i64>(1)?, row_hash.finalize()))
                    })
                })?;
                if batch.is_empty() {
                    break;
                }
                for (name, n, record) in batch {
                    after = name;
                    last_rank = n;
                    hash.update(record);
                }
            }
        }
        Ok(hash.finalize().to_vec())
    }
}

pub(super) fn stamp<T: Serialize>(
    owner: &str,
    domain: &str,
    value: &T,
) -> vault_store::Result<Vec<u8>> {
    let bytes = postcard::to_allocvec(&(domain, owner, value)).map_err(crate::query::operation)?;
    Ok(Sha256::digest(bytes).to_vec())
}

pub(super) fn check_pieces(tx: &vault_store::Session<'_>, owner: &str) -> vault_store::Result<()> {
    verify_version(tx, owner)?;
    let ok=tx.query_one("SELECT pieces>0 AND pieces=(SELECT count(*) FROM ah_reader_page p
        JOIN ah_reader_blob b ON b.hash=p.hash WHERE p.owner=?1) FROM ah_reader_version WHERE token=?1",
        [owner],|r|Ok(r.get::<i64>(0)?!=0)).optional()?;
    if ok != Some(true) {
        return Err(crate::query::operation(Error::ReaderRebuildRequired));
    }
    Ok(())
}

pub(super) const MEMBER: &str =
    "source,version,descriptor,records,complete,attachments,reason,visible,stamp";
pub(super) fn decode_member(
    r: &vault_store::Row,
    owner: &str,
) -> vault_store::Result<membership::MemberRow> {
    let row =
        (r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?, r.get(4)?, r.get(5)?, r.get(6)?, r.get(7)?);
    if r.get::<Option<Vec<u8>>>(8)? != Some(stamp(owner, "member", &row)?) {
        return Err(crate::query::operation(Error::ReaderRebuildRequired));
    }
    Ok(row)
}
pub(super) fn member(
    tx: &vault_store::Session<'_>,
    owner: &str,
    source: &str,
) -> vault_store::Result<Option<membership::MemberRow>> {
    tx.query_one(
        &format!("SELECT {MEMBER} FROM ah_reader_member WHERE publication=?1 AND source=?2"),
        params![owner, source],
        |r| decode_member(r, owner),
    )
    .optional()
}
pub(super) fn decode_line(
    r: &vault_store::Row,
    owner: &str,
) -> vault_store::Result<membership::LineRow> {
    let row = (r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?, r.get(4)?, r.get(5)?, r.get(6)?);
    if r.get::<Option<Vec<u8>>>(7)? != Some(stamp(owner, "line", &row)?) {
        return Err(crate::query::operation(Error::ReaderRebuildRequired));
    }
    Ok(row)
}
pub(super) fn valid_head(tx: &vault_store::Session<'_>, id: &str) -> vault_store::Result<bool> {
    tx.query_one("SELECT h.publication,h.current_source,h.input_digest,h.parent,h.parent_status,h.awaiting_rebuild,h.members,h.manifest,h.stamp,
        w.publication IS h.publication AND w.parent IS h.parent AND w.stamp IS h.stamp
        FROM ah_reader_head h LEFT JOIN ah_reader_serving w ON w.conversation=h.conversation WHERE h.conversation=?1",[id],|r|{
        let owner:String=r.get(0)?;
        let Some(manifest)=r.get::<Option<Vec<u8>>>(7)? else{return Ok(false)};
        let fields=(id,r.get::<String>(1)?,r.get::<String>(2)?,r.get::<Option<String>>(3)?,r.get::<String>(4)?,r.get::<i64>(5)?,r.get::<i64>(6)?,manifest);
        Ok(r.get::<i64>(9)?!=0 && r.get::<Option<Vec<u8>>>(8)?==Some(stamp(&owner,"head",&fields)?))
    }).optional().map(|v|v.unwrap_or(false))
}

struct VersionProof {
    fields: Vec<u8>,
    stored: Option<Vec<u8>>,
}

fn version_fields(
    tx: &vault_store::Session<'_>,
    token: &str,
) -> vault_store::Result<Option<VersionProof>> {
    tx.query_one(
        "SELECT attempt,source,revision,digest,byte_len,codec,records,record_limit,decoded_limit,
                max_frame,format_version,header_ordinal,leaf_count,visible_total,rank_pages,
                nonempty_blocks,lookup_pages,directory_pages,postings,ambiguous_edges,pieces,
                repair_generation,stamp FROM ah_reader_version WHERE token=?1",
        [token],
        |row| {
            let mut hash = Sha256::new();
            let end = row.column_count() - 1;
            for i in 0..end {
                vault_store::hash_value(&mut hash, row.get_value(i)?);
            }
            Ok(VersionProof { fields: hash.finalize().to_vec(), stored: row.get(end)? })
        },
    )
    .optional()
}
pub(super) fn sign_version(tx: &vault_store::Session<'_>, token: &str) -> vault_store::Result<()> {
    let proof = version_fields(tx, token)?
        .ok_or_else(|| crate::query::operation(Error::ReaderRebuildRequired))?;
    tx.execute(
        "UPDATE ah_reader_version SET stamp=?2 WHERE token=?1",
        params![token, stamp(token, "version", &proof.fields)?],
    )?;
    Ok(())
}
pub(super) fn verify_version(
    tx: &vault_store::Session<'_>,
    token: &str,
) -> vault_store::Result<()> {
    let valid = if let Some(proof) = version_fields(tx, token)? {
        proof.stored == Some(stamp(token, "version", &proof.fields)?)
    } else {
        false
    };
    if !valid {
        return Err(crate::query::operation(Error::ReaderRebuildRequired));
    }
    Ok(())
}
