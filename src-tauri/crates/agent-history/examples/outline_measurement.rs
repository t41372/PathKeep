//! Measures random rank reads over independently decodable synthetic outline pages.
//! The benchmark owns its temporary database; it does not publish a read model.
mod measurement;
fn main() {
    measure(measurement::size("AH_ENTRIES", 1025), measurement::size("AH_READS", 12), true);
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn partial_last_page_and_page_boundary_have_distinct_offsets() {
        measure(513, 8, false);
    }
}

#[path = "measurement/pages.rs"]
mod pages;
use pages::ENTRIES;
use sha2::{Digest, Sha256};
use std::{io::Read, time::Instant};

// A two-byte length delta, role/flags, millisecond delta, and native ID prefix.
// All fields fit within the 64 KiB decoded-page prototype budget.
const ENTRY_BYTES: usize = 21;

fn measure(entries: usize, reads: usize, evict: bool) {
    let tmp = tempfile::tempdir().unwrap();
    let store = measurement::open(tmp.path(), true, vault_store::Db::AgentHistoryReadModel);
    pages::init(&store);
    let mut batch = Vec::new();
    let mut position = 0u64;
    let mut stored = 0;
    for first in (0..entries).step_by(ENTRIES) {
        let mut page = position.to_le_bytes().to_vec();
        for ordinal in first..entries.min(first + ENTRIES) {
            // Common small lengths fit in two bytes. The page restarts at an absolute offset.
            let length = 605 + (ordinal % 17) as u16;
            page.extend(length.to_le_bytes());
            page.push((ordinal % 4) as u8);
            page.extend(((ordinal % 1000) as u16).to_le_bytes());
            page.extend(&Sha256::digest((ordinal as u64).to_le_bytes())[..16]);
            position += u64::from(length);
        }
        let data = zstd::stream::encode_all(&page[..], 1).unwrap();
        stored += data.len();
        batch.push((first, data));
        if batch.len() == 64 {
            pages::insert(&store, std::mem::take(&mut batch))
        }
    }
    pages::insert(&store, batch);
    drop(store);
    let mut ranks = vec![0, entries - 1, 255.min(entries - 1), 256.min(entries - 1)];
    let mut state = 0xace0023u64;
    for _ in 0..reads {
        state = state.wrapping_mul(6364136223846793005).wrapping_add(1);
        ranks.push((state >> 24) as usize % entries);
    }
    let mut cold = Vec::new();
    let mut warm = Vec::new();
    for rank in ranks {
        let evicted = evict && measurement::evict(tmp.path());
        let store = measurement::open(tmp.path(), true, vault_store::Db::AgentHistoryReadModel);
        for attempt in 0..2 {
            let start = Instant::now();
            let (first, blob) = pages::covering(&store, rank, 1).pop().unwrap();
            let mut data = Vec::new();
            zstd::stream::Decoder::new(blob.as_slice())
                .unwrap()
                .take(65537)
                .read_to_end(&mut data)
                .unwrap();
            assert!(data.len() <= 65536);
            let base = u64::from_le_bytes(data[..8].try_into().unwrap());
            let offsets = &data[8..];
            let diff = rank - first;
            let length = |index: usize| {
                u16::from_le_bytes(
                    offsets[index * ENTRY_BYTES..index * ENTRY_BYTES + 2].try_into().unwrap(),
                )
            };
            let byte = base + (0..diff).map(|i| u64::from(length(i))).sum::<u64>();
            let len = length(diff);
            let entry = &offsets[diff * ENTRY_BYTES..(diff + 1) * ENTRY_BYTES];
            let elapsed = start.elapsed();
            assert_eq!(entry[2], (rank % 4) as u8);
            assert_eq!(&entry[5..], &Sha256::digest((rank as u64).to_le_bytes())[..16]);
            // Closed-form sum is independent of page boundaries and the encoding loop.
            let n = rank as u64;
            let rem = n % 17;
            assert_eq!(byte, 605 * n + n / 17 * 136 + rem * rem.saturating_sub(1) / 2);
            assert_eq!(u64::from(len), 605 + rem);
            if attempt == 1 {
                warm.push(elapsed)
            } else if evicted {
                cold.push(elapsed)
            }
        }
    }
    println!(
        "{}",
        serde_json::json!({"measurement":"rank","entries":entries,"page_entries":ENTRIES,"page_blob_bytes":stored,
        "cold":(!cold.is_empty()).then(||measurement::timings(cold)),"warm":measurement::timings(warm)})
    );
}
