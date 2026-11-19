//! Measures a 200-element evidence window in a plain native array. Separators
//! create a gap between every pair. Source generation supplies the exact byte oracle.
use super::{Corpus, PASSWORD, measurement};
use agent_history::Archive;
use std::time::Instant;

pub fn measure(corpus: &Corpus) {
    let tmp = tempfile::tempdir().unwrap();
    let root = tmp.path().join("archive");
    let archive = Archive::create_with_params(
        &root,
        PASSWORD,
        vault_crypto::KdfParams { memory_kib: 32, iterations: 1, lanes: 1 },
    )
    .unwrap();
    let id = corpus.retain(tmp.path(), &archive, "array");
    let info = archive.revision(id).unwrap();
    assert_eq!(info.byte_len, corpus.start(corpus.records()));
    let mut outline = Vec::new();
    archive
        .scan_revision(id, &Default::default(), &Default::default(), &mut |r, c, _| {
            outline.push((r, c.clone()));
            Ok(())
        })
        .unwrap();
    assert_eq!(outline.len(), corpus.records());
    drop(archive);
    for (label, first) in
        [("head", 0), ("middle", (outline.len() - 200) / 2), ("tail", outline.len() - 200)]
    {
        let mut cold = Vec::new();
        let mut warm = Vec::new();
        for _ in 0..measurement::size("AH_SAMPLES", 2) {
            let evicted = measurement::evict(tmp.path());
            let archive = Archive::open(&root, PASSWORD).unwrap();
            for attempt in 0..2 {
                let mut rows = Vec::new();
                let start = Instant::now();
                archive
                    .read_window(
                        &info,
                        &outline[first..first + 200],
                        16 << 20,
                        &Default::default(),
                        &mut |r| {
                            rows.push(r);
                            Ok(())
                        },
                    )
                    .unwrap();
                let elapsed = start.elapsed();
                assert_eq!(rows.len(), 200);
                for (rank, row) in (first..).zip(rows) {
                    let bytes = corpus.array_element(rank);
                    let native: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
                    assert_eq!(row.record.start.0, corpus.start(rank) + 1);
                    assert_eq!(row.record.ordinal.0, rank as u64);
                    assert_eq!(row.raw.bytes, bytes);
                    let event = row.interpretation.unwrap();
                    assert_eq!(event.event.text, native["content"]);
                    assert_eq!(event.parts[0].text, native["content"]);
                }
                if attempt == 1 {
                    warm.push(elapsed);
                } else if evicted {
                    cold.push(elapsed);
                }
            }
        }
        println!(
            "{}",
            serde_json::json!({"measurement":"native_array_window","position":label,"decoded_bytes":info.byte_len,"records":outline.len(),"first":first,"last":first+199,"cold":(!cold.is_empty()).then(||measurement::timings(cold)),"warm":measurement::timings(warm)})
        );
    }
}
