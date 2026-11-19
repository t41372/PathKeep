//! Measures framing and bounded archive windows against independently generated native bytes.
//! Temporary benchmark pages have no archive compatibility contract.
#[path = "measurement/background.rs"]
mod background;
#[path = "measurement/corpus.rs"]
mod corpus;
mod measurement;
#[path = "measurement/rendered.rs"]
mod rendered;
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
use vault_store::{Db, Store};

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
        let mut page = Vec::new();
        zstd::stream::Decoder::new(blob.as_slice())
            .unwrap()
            .take(65537)
            .read_to_end(&mut page)
            .unwrap();
        assert!(page.len() <= 65536);
        let page: Vec<Entry> = serde_json::from_slice(&page).unwrap();
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
        let attempt = archive
            .prepare_projection(form, revision, "window-measurement/1", true)
            .unwrap()
            .unwrap();
        archive
            .publish_projection_attempt(
                &attempt,
                &agent_history::SessionMetadata { native_id: form.into(), ..Default::default() },
                true,
            )
            .unwrap();
        let conversation = archive.list_sessions(&Default::default()).unwrap().items.remove(0).id;
        let publication = archive.reader_header(&conversation).unwrap();
        let member = archive.reader_sources(&publication, None, 1).unwrap().remove(0);
        let store = measurement::open(&database, true, Db::AgentHistoryReadModel);
        pages::init(&store);
        build_pages(&store, &archive, revision);
        store.pause().unwrap().checkpoint().unwrap();
        let db_file = database.join(vault_layout::database_path(Db::AgentHistoryReadModel));
        let outline_bytes = db_file.metadata().unwrap().len();
        let building = Instant::now();
        let mut builder = rendered::Builder::new(&store);
        archive
            .scan_revision(
                revision,
                &Default::default(),
                &Default::default(),
                &mut |record, ctx, value| {
                    builder.add(record.ordinal.0, ctx, value.unwrap());
                    Ok(())
                },
            )
            .unwrap();
        builder.flush();
        let compressed_bytes = builder.compressed_bytes;
        println!(
            "{}",
            serde_json::json!({"measurement":"rendered_build","form":form,"ms":building.elapsed().as_secs_f64()*1000.})
        );
        drop(archive);
        drop(store);
        let evicted = measurement::evict(temp.path());
        let archive = Archive::open(&root, PASSWORD).unwrap();
        let mut count = 0;
        let start = Instant::now();
        let pulse = std::sync::Arc::new(std::sync::Mutex::new((
            start,
            std::time::Duration::ZERO,
            ReadPhase::Framing,
            ReadPhase::Framing,
        )));
        let observer = pulse.clone();
        let job = JobControl {
            progress: Some(std::sync::Arc::new(move |progress| {
                let now = Instant::now();
                let mut times = observer.lock().unwrap();
                if now - times.0 > times.1 {
                    times.1 = now - times.0;
                    times.3 = times.2;
                }
                times.2 = progress.phase;
                times.0 = now;
            })),
            ..Default::default()
        };
        let scan = archive
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
        // Include the final interval (last pulse through EOF verification).
        let (last, gap, last_phase, mut slow_phase) = *pulse.lock().unwrap();
        let final_gap = last.elapsed();
        if final_gap > gap {
            slow_phase = last_phase;
        }
        let gap = gap.max(final_gap);
        assert_eq!(count, corpus.records());
        assert_eq!(member.rendered, scan.max_frame_bytes > 32 * 1024 * 1024);
        println!(
            "{}",
            serde_json::json!({"measurement":"physical_framing","form":form,"records":count,"decoded_bytes":corpus.start(count),"evicted":evicted,"max_frame_bytes":scan.max_frame_bytes,"selective":member.rendered,"gap_after_phase":slow_phase,"longest_progress_gap_ms":gap.as_secs_f64()*1000.,"ms":start.elapsed().as_secs_f64()*1000.})
        );
        drop(archive);
        let mut backup_sequence = 0;
        for (label, first) in [("head", 0), ("middle", (count - 200) / 2), ("tail", count - 200)] {
            for path in ["window", "rendered_window", "production_window"] {
                let mut cold = Vec::new();
                let mut warm = Vec::new();
                let mut during_backup = Vec::new();
                for sample in 0..measurement::size("AH_SAMPLES", 2) {
                    let evicted = measurement::evict(temp.path());
                    let archive = Archive::open(&root, PASSWORD).unwrap();
                    let store = measurement::open(&database, true, Db::AgentHistoryReadModel);
                    let read = |profile: bool| {
                        if path == "window" {
                            read_checked_window(
                                &archive,
                                &store,
                                &identity,
                                first,
                                corpus,
                                profile.then_some(form),
                            )
                        } else if path == "production_window" {
                            let start = Instant::now();
                            let rows = archive
                                .reader_window(
                                    &publication,
                                    None,
                                    first as u64,
                                    200,
                                    &Default::default(),
                                )
                                .unwrap();
                            let elapsed = start.elapsed();
                            assert_eq!(rows.len(), 200);
                            for (rank, row) in (first..).zip(rows) {
                                let native: serde_json::Value =
                                    serde_json::from_slice(&corpus.row(rank)).unwrap();
                                assert_eq!(row.ordinal.0, rank as u64);
                                assert_eq!(row.native_id.as_deref(), native["uuid"].as_str());
                                assert_eq!(row.parts.len(), 1);
                                assert_eq!(row.parts[0].text, native["message"]["content"]);
                            }
                            elapsed
                        } else {
                            rendered::checked_window(&store, first, corpus)
                        }
                    };
                    for attempt in 0..2 {
                        let elapsed = read(sample == 0 && attempt == 0);
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
                        || read(false),
                    ));
                    backup_sequence += 1;
                }
                println!(
                    "{}",
                    serde_json::json!({"measurement":path,"form":form,"position":label,"during_backup":measurement::timings(during_backup),"first":first,"last":first+199,
                "cold":(!cold.is_empty()).then(||measurement::timings(cold)),"warm":measurement::timings(warm)})
                );
            }
        }
        let store = measurement::open(&database, true, Db::AgentHistoryReadModel);
        rendered::append_sample(&store, &database, corpus, compressed_bytes, outline_bytes, form);
    }
}

fn read_checked_window(
    archive: &Archive,
    store: &Store,
    identity: &agent_history::archive::RevisionInfo,
    first: usize,
    corpus: &Corpus,
    profile: Option<&str>,
) -> std::time::Duration {
    let start = Instant::now();
    let outline = load_window(store, first);
    let lookup = start.elapsed();
    let decoded = std::sync::Arc::new(std::sync::atomic::AtomicU64::new(0));
    let mut job = JobControl::default();
    if profile.is_some() {
        let done = decoded.clone();
        job.progress = Some(std::sync::Arc::new(move |pulse| {
            done.store(pulse.done, std::sync::atomic::Ordering::Relaxed)
        }));
    }
    let mut first_row = None;
    let mut rows = Vec::new();
    archive
        .read_window(identity, &outline, 16 * 1024 * 1024, &job, &mut |row| {
            first_row.get_or_insert_with(|| start.elapsed());
            rows.push(row);
            Ok(())
        })
        .unwrap();
    let elapsed = start.elapsed();
    if let Some(form) = profile {
        let record = &outline[0].0;
        let native_replay = record.frame.map_or(0, |f| record.start.0 - f.decoded.0);
        println!(
            "{}",
            serde_json::json!({"measurement":"raw_window_profile","form":form,"first":first,
            "lookup_ms":lookup.as_secs_f64()*1000.,"first_row_ms":first_row.unwrap().as_secs_f64()*1000.,
            "total_ms":elapsed.as_secs_f64()*1000.,"decoded_work":decoded.load(std::sync::atomic::Ordering::Relaxed),
            "prefix_replay_bytes":native_replay})
        );
    }
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
