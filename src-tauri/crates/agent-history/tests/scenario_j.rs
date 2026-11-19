//! Acceptance oracle for source-order windows over the same decoded corpus in
//! three native transports. Timing and cache eviction belong to the examples.
#[path = "../examples/measurement/corpus.rs"]
mod corpus;
use agent_history::Archive;
use corpus::Corpus;

fn check(records: usize, bytes: usize, first: usize, last: usize) {
    let corpus = Corpus::new(records, bytes);
    for form in ["plain", "multi", "single"] {
        let tmp = tempfile::tempdir().unwrap();
        let archive = Archive::create_unencrypted(&tmp.path().join("archive")).unwrap();
        let revision = corpus.retain(tmp.path(), &archive, form);
        let info = archive.revision(revision).unwrap();
        let mut tail = Vec::new();
        let report = archive
            .scan_revision(
                revision,
                &Default::default(),
                &Default::default(),
                &mut |record, ctx, _| {
                    if record.ordinal.0 >= first as u64 {
                        tail.push((record, ctx.clone()))
                    }
                    Ok(())
                },
            )
            .unwrap();
        assert_eq!(report.physical_records, records as u64);
        assert_eq!(report.objects, records as u64);
        assert_eq!(corpus.start(records), bytes as u64);
        let mut ranks = Vec::new();
        archive
            .read_window(&info, &tail, 16 * 1024 * 1024, &Default::default(), &mut |row| {
                let rank = first + ranks.len();
                corpus.verify(rank, &row.raw.bytes, row.record.start.0).unwrap();
                assert!(row.interpretation.unwrap().event.text.ends_with(&corpus.marker(rank)));
                ranks.push(row.record.ordinal.0 as usize);
                Ok(())
            })
            .unwrap();
        assert_eq!(ranks, (first..=last).collect::<Vec<_>>(), "{form}");
        assert_eq!(ranks.len(), 200);
    }
}

#[test]
fn exact_scenario_j() {
    check(23_812, 264_000_000, 23_612, 23_811)
}

#[test]
fn small_tail_oracle() {
    check(600, 1_200_000, 400, 599)
}
