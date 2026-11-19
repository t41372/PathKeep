//! Measures compressed and incompressible evidence reads through real Turso.
//! All databases and inputs belong to the benchmark.
mod measurement;
use sha2::{Digest, Sha256};
use std::{
    io::{self, Read},
    time::{Duration, Instant},
};
use vault_store::{Db, Priority, Store, params};
fn synthetic(size: usize) -> Vec<u8> {
    let mut bytes = Vec::new();
    let mut index = 0_u64;
    while bytes.len() < size {
        let random = (0..12)
            .map(|part| hex::encode(Sha256::digest(format!("synthetic-{index}-{part}"))))
            .collect::<String>();
        let row = serde_json::json!({"type":"assistant","uuid":format!("event-{index}"),"timestamp":"2026-11-13T12:00:00Z","message":{"role":"assistant","model":"synthetic-model","content":[{"type":"text","text":format!("Review the source and preserve every record. {} {random}", "The synthetic function returns its result after validation. ".repeat(4))}],"usage":{"input_tokens":1000,"output_tokens":100}}});
        bytes.extend(serde_json::to_vec(&row).unwrap());
        bytes.push(b'\n');
        index += 1;
    }
    bytes.truncate(size);
    bytes
}

#[cfg(test)]
mod tests {
    #[test]
    fn blob_oracle_rejects_a_valid_blob_from_the_wrong_row() {
        let raw = vec![3; 256];
        let compressed = zstd::stream::encode_all(raw.as_slice(), 1).unwrap();
        let other = sha2::Sha256::digest([4; 256]);
        assert!(super::verify_blob(&compressed, &other, raw.len()).is_err());
    }
    use sha2::Digest;
}

fn verify_blob(blob: &[u8], expected: &[u8], size: usize) -> io::Result<()> {
    let mut hash = Sha256::new();
    let n = io::copy(&mut zstd::stream::Decoder::new(blob)?.take(size as u64 + 1), &mut hash)?;
    if n != size as u64 || &hash.finalize()[..] != expected {
        return Err(io::Error::new(io::ErrorKind::InvalidData, "source slice digest mismatch"));
    }
    Ok(())
}

fn read(store: &Store, id: usize, expected: &[u8], size: usize) -> Duration {
    let start = Instant::now();
    let blob: Vec<u8> = store
        .read(Db::AgentHistoryEvidence, move |tx| {
            tx.query_one("SELECT data FROM blobs WHERE id=?1", [id as i64], |r| Ok(r.get(0)?))
        })
        .unwrap();
    verify_blob(&blob, expected, size).unwrap();
    start.elapsed()
}
fn write(store: &Store, id: usize, data: Vec<u8>) {
    store
        .write(Db::AgentHistoryEvidence, Priority::Batch, move |tx| {
            tx.execute("INSERT INTO blobs VALUES(?1,?2)", params![id as i64, data])?;
            Ok(())
        })
        .unwrap()
}

fn measure(size: usize, rows: usize, samples: usize, encrypted: bool, noise: bool) {
    let temp = tempfile::tempdir().unwrap();
    let store = measurement::open(temp.path(), encrypted);
    store
        .write(Db::AgentHistoryEvidence, Priority::Batch, |tx| {
            tx.execute("CREATE TABLE blobs(id INTEGER PRIMARY KEY,data BLOB NOT NULL)", ())?;
            Ok(())
        })
        .unwrap();
    let mut raw = if noise {
        let mut state = 0x9182_7364_5bad_afe3_u64;
        let mut bytes = vec![0; size];
        for chunk in bytes.chunks_exact_mut(8) {
            state ^= state << 13;
            state ^= state >> 7;
            state ^= state << 17;
            chunk.copy_from_slice(&state.to_le_bytes());
        }
        bytes
    } else {
        synthetic(size)
    };
    let mut hashes = Vec::new();
    let mut writes = Vec::new();
    let mut stored_bytes = 0;
    for id in 0..rows {
        raw[..8].copy_from_slice(&(id as u64).to_le_bytes());
        hashes.push(Sha256::digest(&raw));
        let blob = zstd::stream::encode_all(raw.as_slice(), 3).unwrap();
        stored_bytes += blob.len();
        let start = Instant::now();
        write(&store, id, blob);
        writes.push(start.elapsed());
    }
    for (id, digest) in hashes.iter().enumerate() {
        read(&store, id, digest, size);
    }
    drop(store);
    let mut cold = Vec::new();
    let mut warm = Vec::new();
    let mut mixed = Vec::new();
    let extra = zstd::stream::encode_all(raw.as_slice(), 3).unwrap();
    for sample in 0..samples {
        let id = (sample.wrapping_mul(2654435761) + 1) % rows;
        let evicted = measurement::evict(temp.path());
        let store = measurement::open(temp.path(), encrypted);
        let first = read(&store, id, &hashes[id], size);
        if evicted {
            cold.push(first)
        }
        warm.push(read(&store, id, &hashes[id], size));
        let gate = std::sync::Barrier::new(2);
        std::thread::scope(|writer| {
            writer.spawn(|| {
                gate.wait();
                write(&store, rows + sample, extra.clone())
            });
            gate.wait();
            mixed.push(read(&store, id, &hashes[id], size));
        });
        read(&store, rows + sample, hashes.last().unwrap(), size);
    }
    println!(
        "{}",
        serde_json::json!({"measurement":"blob","encrypted":encrypted,"incompressible":noise,
        "raw_bytes":size,"stored_bytes_mean":stored_bytes/rows,"writes":measurement::timings(writes),
        "cold":(!cold.is_empty()).then(||measurement::timings(cold)),"warm":measurement::timings(warm),"during_write":measurement::timings(mixed)})
    );
}
fn main() {
    let unit = measurement::size("AH_BLOB_MIB", 1) * 1024 * 1024;
    for encrypted in [false, true] {
        for noise in [false, true] {
            for bytes in [unit, 4 * unit] {
                measure(
                    bytes,
                    measurement::size("AH_BLOB_ROWS", 3),
                    measurement::size("AH_SAMPLES", 2),
                    encrypted,
                    noise,
                )
            }
        }
    }
}
