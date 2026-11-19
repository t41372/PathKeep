//! Measures framing and bounded archive windows against independently generated native bytes.
//! Temporary benchmark pages have no archive compatibility contract.
#[path = "measurement/background.rs"]
mod background;
#[path = "measurement/corpus.rs"]
mod corpus;
mod measurement;
use corpus::Corpus;

fn main() {
    let corpus =
        Corpus::new(measurement::size("AH_RECORDS", 600), measurement::size("AH_BYTES", 1_200_000));
    measure(&corpus);
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn small_window_oracle_rejects_shifted_bytes_and_marker_positions() {
        let corpus = Corpus::new(601, 1_200_001);
        let row = corpus.row(415);
        assert!(corpus.verify(414, &row, corpus.start(415)).is_err());
        assert!(corpus.verify(415, &row[1..], corpus.start(415)).is_err());
        assert!(corpus.verify(415, &row, corpus.start(415) + 1).is_err());
    }
}

#[path = "measurement/pages.rs"]
mod pages;
use agent_history::{Archive, read_model::*};
use std::{io::Read, time::Instant};
use vault_store::Store;

const PASSWORD: &[u8] = b"synthetic-window-benchmark";
type Entry = (RecordLocator, RecordContext);

fn build_pages(store: &Store, archive: &Archive, revision: i64) {
    let mut entries = Vec::new();
    let mut first = 0;
    let flush = |page: &mut Vec<Entry>, first: usize| {
        let bytes = serde_json::to_vec(page).unwrap();
        assert!(bytes.len() <= 64 * 1024);
        pages::insert(store, vec![(first, zstd::stream::encode_all(bytes.as_slice(), 1).unwrap())]);
        page.clear();
    };
    archive
        .scan_revision(revision, &Default::default(), &Default::default(), &mut |r, c, _| {
            entries.push((r, c.clone()));
            if entries.len() == pages::ENTRIES {
                flush(&mut entries, first);
                first += pages::ENTRIES;
            }
            Ok(())
        })
        .unwrap();
    if !entries.is_empty() {
        flush(&mut entries, first)
    }
}
fn load_window(store: &Store, first: usize) -> Vec<Entry> {
    let mut result = Vec::new();
    for (_, blob) in pages::covering(store, first, 200) {
        let page: Vec<Entry> = serde_json::from_reader(
            zstd::stream::Decoder::new(blob.as_slice()).unwrap().take(65537),
        )
        .unwrap();
        result.extend(
            page.into_iter()
                .filter(|(r, _)| (first as u64..first as u64 + 200).contains(&r.ordinal.0)),
        );
    }
    result
}
fn measure(corpus: &Corpus) {
    for form in ["plain", "multi", "single"] {
        let temp = tempfile::tempdir().unwrap();
        let root = temp.path().join("archive");
        let database = temp.path().join("outline");
        let archive = Archive::create_with_params(
            &root,
            PASSWORD,
            vault_crypto::KdfParams { memory_kib: 32, iterations: 1, lanes: 1 },
        )
        .unwrap();
        let revision = corpus.retain(temp.path(), &archive, form);
        let identity = archive.revision(revision).unwrap();
        let store = measurement::open(&database, true);
        pages::init(&store);
        build_pages(&store, &archive, revision);
        drop(archive);
        drop(store);
        let evicted = measurement::evict(temp.path());
        let archive = Archive::open(&root, PASSWORD).unwrap();
        let mut count = 0;
        let start = Instant::now();
        let pulse = std::sync::Arc::new(std::sync::Mutex::new((start, std::time::Duration::ZERO)));
        let observer = pulse.clone();
        let job = JobControl {
            progress: Some(std::sync::Arc::new(move |_| {
                let now = Instant::now();
                let mut times = observer.lock().unwrap();
                times.1 = times.1.max(now - times.0);
                times.0 = now;
            })),
            ..Default::default()
        };
        archive
            .scan_revision(revision, &Default::default(), &job, &mut |record, _, value| {
                assert_eq!(
                    (record.ordinal.0, record.start.0, record.length),
                    (
                        count as u64,
                        corpus.start(count),
                        corpus.start(count + 1) - corpus.start(count)
                    )
                );
                assert_eq!(value.unwrap()["uuid"], format!("native-{count}"));
                count += 1;
                Ok(())
            })
            .unwrap();
        assert_eq!(count, corpus.records());
        println!(
            "{}",
            serde_json::json!({"measurement":"physical_framing","form":form,"records":count,"decoded_bytes":corpus.start(count),"evicted":evicted,"longest_progress_gap_ms":pulse.lock().unwrap().1.as_secs_f64()*1000.,"ms":start.elapsed().as_secs_f64()*1000.})
        );
        drop(archive);
        let mut backup_sequence = 0;
        for (label, first) in [("head", 0), ("middle", (count - 200) / 2), ("tail", count - 200)] {
            let mut cold = Vec::new();
            let mut warm = Vec::new();
            let mut during_backup = Vec::new();
            for _ in 0..measurement::size("AH_SAMPLES", 2) {
                let evicted = measurement::evict(temp.path());
                let archive = Archive::open(&root, PASSWORD).unwrap();
                let store = measurement::open(&database, true);
                for attempt in 0..2 {
                    let elapsed = read_checked_window(&archive, &store, &identity, first, corpus);
                    if attempt == 1 {
                        warm.push(elapsed)
                    } else if evicted {
                        cold.push(elapsed)
                    }
                }
                during_backup.push(background::during_backup(
                    &archive,
                    &temp.path().join("background"),
                    backup_sequence,
                    || read_checked_window(&archive, &store, &identity, first, corpus),
                ));
                backup_sequence += 1;
            }
            println!(
                "{}",
                serde_json::json!({"measurement":"window","form":form,"position":label,"during_backup":measurement::timings(during_backup),"first":first,"last":first+199,
                "cold":(!cold.is_empty()).then(||measurement::timings(cold)),"warm":measurement::timings(warm)})
            );
        }
    }
}

fn read_checked_window(
    archive: &Archive,
    store: &Store,
    identity: &agent_history::archive::RevisionInfo,
    first: usize,
    corpus: &Corpus,
) -> std::time::Duration {
    let start = Instant::now();
    let outline = load_window(store, first);
    let mut rows = Vec::new();
    archive
        .read_window(identity, &outline, 16 * 1024 * 1024, &Default::default(), &mut |row| {
            rows.push(row);
            Ok(())
        })
        .unwrap();
    let elapsed = start.elapsed();
    assert_eq!(rows.len(), 200);
    for (rank, row) in (first..).zip(rows) {
        assert_eq!(row.record.ordinal.0, rank as u64);
        corpus.verify(rank, &row.raw.bytes, row.record.start.0).unwrap();
        let interpreted = row.interpretation.unwrap();
        let event = interpreted.event;
        assert_eq!(event.native_id.unwrap(), format!("native-{rank}"));
        assert!(event.text.ends_with(&corpus.marker(rank)));
        assert_eq!(interpreted.parts[0].text, event.text);
    }

    elapsed
}
