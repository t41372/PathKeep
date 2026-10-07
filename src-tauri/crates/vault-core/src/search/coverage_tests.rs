//! Responsibilities: prove public search surfaces can reach beyond historical limits.
//! Not responsible for: performance measurements or frontend presentation.
//! Failure modes: the 25,000-page window, 50,000-visit scan, 200 fuzzy candidates,
//! palette first-page stop, or export page size makes old records unreachable.
use super::{SearchRequest, SearchSession};
use crate::{ExportFormat, ExportRequest, HistoryQuery, Vault};
use vault_store::{Db, Priority};

#[test]
fn lexical_fuzzy_regex_palette_and_export_reach_past_old_limits() {
    for key in [None, Some([107; 32])] {
        let started = std::time::Instant::now();
        let report = |phase: &str| {
            eprintln!("coverage encrypted={} {phase}: {:?}", key.is_some(), started.elapsed())
        };
        let vault = Vault::open_temp_with_key(key).unwrap();
        report("seeding");
        super::tests::seed(&vault, 25_007);
        vault
            .store()
            .write(Db::Archive, Priority::Batch, |s| {
                s.execute("UPDATE urls SET title='common ancientcommand' WHERE page_id=1", ())?;
                crate::archive::search_documents::refresh(s, 1)
            })
            .unwrap();
        vault.search().cycle().unwrap();
        report("indexed");
        for fuzzy in [false, true] {
            let mut session = SearchSession::start(
                &vault,
                SearchRequest {
                    query: if fuzzy { "comon" } else { "common" }.into(),
                    fuzzy,
                    page_size: 1000,
                    ..Default::default()
                },
            )
            .unwrap();
            let mut ids = Vec::new();
            loop {
                let step = session.finish_page().unwrap();
                assert_eq!(step.count.value, 25_007, "fuzzy={fuzzy}");
                assert!(step.count.exact);
                ids.extend(step.items.into_iter().map(|item| item.page_id));
                let Some(cursor) = step.cursor else { break };
                session.next_page(&cursor).unwrap();
            }
            ids.sort_unstable();
            assert_eq!(ids, (1..=25_007).collect::<Vec<_>>());
            report(if fuzzy { "fuzzy complete" } else { "lexical complete" });
        }
        // The oldest visit is beyond the old 50,000-newest visit scan; palette
        // requests only six rows, but its ranking walk still covers every page.
        for regex in [false, true] {
            let mut session = SearchSession::start(
                &vault,
                SearchRequest {
                    query: "ancientcommand".into(),
                    regex,
                    page_size: 6,
                    ..Default::default()
                },
            )
            .unwrap();
            let page = session.finish_page().unwrap();
            assert_eq!(page.count.value, 1);
            assert_eq!(page.items[0].page_id, 1);
        }
        report("regex and palette complete");
        let export = crate::export_history(
            &vault,
            ExportRequest {
                export_id: None,
                query: HistoryQuery {
                    q: Some("common".into()),
                    limit: Some(6),
                    ..Default::default()
                },
                format: ExportFormat::Jsonl,
            },
        )
        .unwrap();
        report("export complete");
        assert_eq!(export.count, 25_007);
        assert_eq!(std::fs::read_to_string(export.path).unwrap().lines().count(), 25_007);
    }
}
