//! Responsibilities: measure synthetic compressed blobs through encrypted vault-store.
//! Not responsible for: production formats or reading native user histories.
use sha2::{Digest, Sha256};
use std::{path::Path, time::Instant};
use vault_crypto::{DataKey, KeyPurpose, TempCryptIo};
use vault_store::{Config, Db, Priority, Store, params};

fn open(root: &Path, key: &DataKey) -> Store {
    let mut config = Config::new(root);
    config.databases = vec![Db::Scratch, Db::SourceEvidence];
    config.keys[Db::Scratch as usize] =
        Some(*key.derive(KeyPurpose::ScratchDatabase).expose_bytes());
    config.keys[Db::SourceEvidence as usize] =
        Some(*key.derive(KeyPurpose::SourceEvidenceDatabase).expose_bytes());
    config.io = Some(TempCryptIo::new(key).unwrap());
    config.readers_per_database = std::num::NonZeroUsize::new(1).unwrap();
    Store::open(config).unwrap()
}

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

fn read(store: &Store, id: i64) -> f64 {
    let start = Instant::now();
    let bytes: Vec<u8> = store
        .read(Db::SourceEvidence, move |tx| {
            tx.query_one("SELECT data FROM blobs WHERE id=?1", [id], |r| Ok(r.get(0)?))
        })
        .unwrap();
    std::hint::black_box(bytes);
    start.elapsed().as_secs_f64() * 1000.0
}

// On macOS a no-cache read asks UBC to discard the consumed pages. This affects
// only our synthetic files; it does not flush the user's machine-wide cache.
#[cfg(target_os = "macos")]
fn evict(root: &Path) {
    use std::{fs::File, os::fd::AsRawFd};
    for entry in walkdir::WalkDir::new(root) {
        let entry = entry.unwrap();
        if entry.file_type().is_file() {
            let mut file = File::open(entry.path()).unwrap();
            assert_eq!(unsafe { libc::fcntl(file.as_raw_fd(), libc::F_NOCACHE, 1) }, 0);
            std::io::copy(&mut file, &mut std::io::sink()).unwrap();
        }
    }
}

#[cfg(not(target_os = "macos"))]
fn evict(_root: &Path) {}

fn main() {
    for mib in [1, 4] {
        let temp = tempfile::tempdir().unwrap();
        let key = DataKey::from_bytes(zeroize::Zeroizing::new([0x39; 32]));
        let raw = synthetic(mib * 1024 * 1024);
        let blob = zstd::stream::encode_all(raw.as_slice(), 3).unwrap();
        let store = open(temp.path(), &key);
        store
            .write(Db::SourceEvidence, Priority::Batch, |tx| {
                tx.execute("CREATE TABLE blobs(id INTEGER PRIMARY KEY,data BLOB NOT NULL)", ())?;
                Ok(())
            })
            .unwrap();
        let mut writes = Vec::new();
        for id in 0..32 {
            let data = blob.clone();
            let start = Instant::now();
            store
                .write(Db::SourceEvidence, Priority::Batch, move |tx| {
                    tx.execute("INSERT INTO blobs VALUES(?1,?2)", params![id, data])?;
                    Ok(())
                })
                .unwrap();
            writes.push(start.elapsed().as_secs_f64());
        }
        drop(store);
        let mut cold = Vec::new();
        let mut warm = Vec::new();
        for id in 0..12 {
            evict(temp.path());
            let store = open(temp.path(), &key);
            cold.push(read(&store, id));
            for _ in 0..8 {
                warm.push(read(&store, id));
            }
        }
        cold.sort_by(f64::total_cmp);
        warm.sort_by(f64::total_cmp);
        println!(
            "{}",
            serde_json::json!({"raw_mib":mib,"stored_bytes":blob.len(),"cold_write_stored_mib_s":blob.len() as f64 / writes[0] / 1048576.0,"warm_write_stored_mib_s":31.0 * blob.len() as f64 / writes[1..].iter().sum::<f64>() / 1048576.0,"cold_connection_read_ms_median":cold[cold.len()/2],"cold_connection_read_ms_max":cold[cold.len()-1],"warm_read_ms_median":warm[warm.len()/2],"warm_read_ms_p95":warm[warm.len()*95/100],"cache_note":"cold = reopened Turso after F_NOCACHE sweep of synthetic files; no global cache purge"})
        );
    }
}
