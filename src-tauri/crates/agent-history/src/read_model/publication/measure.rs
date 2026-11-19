//! Target-size, disk-backed measurements of the public reader and append path.
//! Synthetic text, ordinals and source lengths are generated independently of the archive.
use super::*;
use crate::{Archive, Harness, SourceRoot, capture::BackupOptions};
use serde_json::{Value, json};
use std::{
    fs,
    io::{BufWriter, Write},
    path::Path,
    sync::Mutex,
    time::Instant,
};
use vault_store::{Priority, params};

thread_local! {static SCAN:std::cell::Cell<(u64,u64,std::time::Duration)>=const{std::cell::Cell::new((0,0,std::time::Duration::ZERO))};}
pub(super) fn measured_scan(report: &ScanReport, time: std::time::Duration) {
    SCAN.set((report.decoded_bytes, report.physical_records, time));
}
#[allow(dead_code)]
#[path = "../../../examples/measurement/mod.rs"]
mod measurement;
const ID: &str = "11111111-1111-4111-8111-111111111111";
fn native(rank: usize, length: usize) -> Vec<u8> {
    let mut value = if rank == 0 {
        json!({"type":"session_meta","payload":{"id":ID,"cwd":"/bench","timestamp":"2026-11-10T12:00:00Z"}})
    } else if rank % 2 == 1 {
        json!({"type":"response_item","payload":{"type":"message","role":"user","id":format!("row-{rank}"),"content":""}})
    } else {
        json!({"type":"uninterpreted","payload":{"id":format!("hidden-{rank}")}})
    };
    let visible = rank % 2 == 1;
    if !visible {
        value["padding"] = Value::String(String::new())
    }
    let size = length - serde_json::to_vec(&value).unwrap().len() - 1;
    let mut text = format!("record-{rank} ");
    let mut rng = rank as u64 + 1;
    while text.len() < size {
        rng = rng.wrapping_mul(6364136223846793005).wrapping_add(1);
        text.push(if text.len().is_multiple_of(4) {
            (b'A' + ((rng >> 32) % 26) as u8) as char
        } else {
            b" synthetic text "[text.len() % 16] as char
        });
    }
    if visible {
        value["payload"]["content"] = text.into()
    } else {
        value["padding"] = text.into()
    }
    let mut raw = serde_json::to_vec(&value).unwrap();
    raw.push(b'\n');
    assert_eq!(raw.len(), length);
    raw
}
fn lengths(total: usize, records: usize) -> Vec<usize> {
    let large = (total / records * 18).min(206000);
    let mut sizes = (0..records)
        .map(|i| match i % 100 {
            0..=1 => large,
            2..=19 => 0,
            _ => 605,
        })
        .collect::<Vec<_>>();
    let mut rest = total - sizes.iter().sum::<usize>();
    let mut medium = sizes.iter().filter(|s| **s == 0).count();
    for n in sizes.iter_mut().filter(|n| **n == 0) {
        *n = rest / medium;
        rest -= *n;
        medium -= 1;
    }
    sizes
}
fn generate(path: &Path, sizes: &[usize], zstd: bool) {
    let out = BufWriter::new(fs::File::create(path).unwrap());
    let write = |out: &mut dyn Write| {
        for (i, &n) in sizes.iter().enumerate() {
            out.write_all(&native(i, n)).unwrap();
        }
    };
    if zstd {
        let mut z = zstd::stream::Encoder::new(out, 1).unwrap();
        write(&mut z);
        z.finish().unwrap().flush().unwrap();
    } else {
        let mut out = out;
        write(&mut out);
        out.flush().unwrap();
    }
}
#[derive(Serialize, Clone, Copy)]
struct Inventory {
    versions: i64,
    page_pieces: i64,
    logical_bytes: i64,
    retired_publications: i64,
    unique_blobs: i64,
    compressed_bytes: i64,
}
fn inventory(a: &Archive) -> Inventory {
    a.store
        .read(DB, |tx| {
            tx.query_one(
                "SELECT (SELECT count(*) FROM ah_reader_version),count(*),coalesce(sum(nbytes),0),
                (SELECT count(*) FROM ah_reader_retired),(SELECT count(*) FROM ah_reader_blob),
                (SELECT coalesce(sum(length(data)),0) FROM ah_reader_blob) FROM ah_reader_page",
                (),
                |r| {
                    Ok(Inventory {
                        versions: r.get(0)?,
                        page_pieces: r.get(1)?,
                        logical_bytes: r.get(2)?,
                        retired_publications: r.get(3)?,
                        unique_blobs: r.get(4)?,
                        compressed_bytes: r.get(5)?,
                    })
                },
            )
        })
        .unwrap()
}

#[test]
#[ignore = "release target measurements for WORK-AH-READ-MODEL"]
fn append_and_sparse_windows_at_target() {
    let encrypted = std::env::var_os("AH_MEASURE_ENCRYPTED").is_some();
    let password = if encrypted { b"reader-fixture".as_slice() } else { b"".as_slice() };
    for (total, zstd) in [(264_000_000, false), (30 * 1024 * 1024, true)] {
        let sizes = lengths(total, 23_812);
        let t = tempfile::tempdir().unwrap();
        let root = t.path().join("codex");
        let sessions = root.join("sessions");
        fs::create_dir_all(&sessions).unwrap();
        let mut name = format!("rollout-2026-11-10T12-00-00-{ID}.jsonl");
        if zstd {
            name.push_str(".zst")
        }
        let file = sessions.join(name);
        generate(&file, &sizes, zstd);
        let source = SourceRoot { harness: Harness::Codex, root };
        let archive = t.path().join("archive");
        let a = Arc::new(if encrypted {
            Archive::create_with_params(
                &archive,
                password,
                vault_crypto::KdfParams { memory_kib: 32, iterations: 1, lanes: 1 },
            )
            .unwrap()
        } else {
            Archive::create_unencrypted(&archive).unwrap()
        });
        let opts = BackupOptions { read_git: false, ..Default::default() };
        a.backup(std::slice::from_ref(&source), &opts).unwrap();
        let id = a.list_sessions(&Default::default()).unwrap().items.remove(0).id;
        let h = a.reader_header(&id).unwrap();
        assert_eq!(h.visible_records, 11_906);
        assert!(!a.reader_sources(&h, None, 1).unwrap()[0].rendered);
        let first = h.visible_records - 200;
        drop(a);
        let mut cold = Vec::new();
        let mut warm = Vec::new();
        for _ in 0..40 {
            let evicted = measurement::evict(&archive);
            let a = Archive::open(&archive, password).unwrap();
            for cached in [false, true] {
                let start = Instant::now();
                let rows =
                    a.reader_visible_window(&h, None, first, 200, &Default::default()).unwrap();
                let elapsed = start.elapsed();
                if cached {
                    warm.push(elapsed)
                } else if evicted {
                    cold.push(elapsed)
                }
                assert_eq!(rows.len(), 200);
                for (i, r) in rows.into_iter().enumerate() {
                    let n = 2 * (first as usize + i) + 1;
                    assert_eq!(r.ordinal.0, n as u64);
                    let expected: Value = serde_json::from_slice(&native(n, sizes[n])).unwrap();
                    assert_eq!(r.parts[0].text, expected["payload"]["content"]);
                }
            }
        }
        println!(
            "{}",
            json!({"measurement":"sparse_visible_tail","encrypted":encrypted,"decoded_bytes":total,"native_bytes":file.metadata().unwrap().len(),"single_frame":zstd,
            "cold":(!cold.is_empty()).then(||measurement::timings(cold)),"warm":measurement::timings(warm)})
        );
        if !zstd {
            let a = Arc::new(Archive::open(&archive, password).unwrap());
            let before = inventory(&a);
            let row = native(23813, 700);
            fs::OpenOptions::new().append(true).open(&file).unwrap().write_all(&row).unwrap();
            let seen = Arc::new(Mutex::new(None));
            let stage = seen.clone();
            let reader = a.clone();
            let start = Instant::now();
            let raw_commit = Arc::new(Mutex::new(None));
            let raw = raw_commit.clone();
            let opts = BackupOptions {
                read_git: false,
                progress: Some(Arc::new(move |p| {
                    if p.phase == "raw_committed" {
                        *raw.lock().unwrap() = Some(start.elapsed())
                    }
                    if p.phase == "projection_staged" {
                        let staged = start.elapsed();
                        let peak = inventory(&reader);
                        let (records,pages,bytes)=reader.store.read(DB,|tx|tx.query_one("SELECT v.records,count(*),sum(p.nbytes) FROM ah_reader_page p JOIN ah_reader_version v ON v.token=p.owner WHERE v.state='ready' GROUP BY v.token,v.records",(),|r|Ok((r.get::<i64>(0)?,r.get::<i64>(1)?,r.get::<i64>(2)?)))).unwrap();
                        *stage.lock().unwrap() =
                            Some((staged, peak, records, pages, bytes, start.elapsed() - staged));
                    }
                })),
                ..Default::default()
            };
            a.backup(&[source], &opts).unwrap();
            let elapsed = start.elapsed();
            let (staged, peak, records, pages, bytes, probe) = seen.lock().unwrap().unwrap();
            let after = inventory(&a);
            assert_eq!(records, 23813);
            assert_eq!((peak.versions, after.versions, after.retired_publications), (2, 1, 0));
            let head = a.reader_header(&id).unwrap();
            assert_eq!(
                a.reader_window(&head, None, 23812, 1, &Default::default()).unwrap()[0]
                    .native_id
                    .as_deref(),
                Some("row-23813")
            );
            let (decoded, framed, scan_time) = SCAN.get();
            assert_eq!((decoded, framed), (total as u64 + 700, 23813));
            let captured = raw_commit.lock().unwrap().unwrap();
            println!(
                "{}",
                json!({"measurement":"one_message_append","encrypted":encrypted,"original_bytes":total,"append_bytes":row.len(),"rescanned_bytes":decoded,"rescanned_records":framed,"reader_scan_and_page_build_ms":scan_time.as_secs_f64()*1000.,"pages_written":pages,"logical_page_bytes_written":bytes,"before":before,"peak":peak,"after":after,"new_unique_blob_bytes":peak.compressed_bytes-before.compressed_bytes,"capture_ms":captured.as_secs_f64()*1000.,"raw_commit_to_projection_staged_ms":(staged-captured).as_secs_f64()*1000.,"measurement_probe_ms":probe.as_secs_f64()*1000.,"backup_ms":elapsed.as_secs_f64()*1000.})
            );
        }
    }
}

mod plan_corpus;

#[test]
#[ignore = "release query plans over 120,000 retained roots"]
fn plans_on_many_roots() {
    use crate::{Artifact, ArtifactKind};
    use sha2::{Digest, Sha256};
    let tmp = tempfile::tempdir().unwrap();
    let a = Archive::create_unencrypted(&tmp.path().join("archive")).unwrap();
    a.synchronize_search().unwrap();
    let roots: i64 = std::env::var("AH_PLAN_ROOTS").map_or(120000, |s| s.parse().unwrap());
    assert!(roots > 0);
    plan_corpus::seed(&a, roots);
    // Reserve distinct real parser handles after the synthetic index population.
    a.store
        .write(DB, Priority::Batch, |tx| {
            tx.execute("UPDATE ah_meta SET value='120000' WHERE key='next_projection_id'", ())
        })
        .unwrap();
    let root = SourceRoot { root: tmp.path().join("codex"), harness: Harness::Codex };
    let sessions = root.root.join("sessions");
    fs::create_dir_all(&sessions).unwrap();
    let base = native(0, 605);
    fs::write(sessions.join(format!("rollout-2026-11-10T12-00-00-{ID}.jsonl")), &base).unwrap();
    let child = "22222222-2222-4222-8222-222222222222";
    let mut meta: Value = serde_json::from_slice(&base).unwrap();
    meta["payload"]["id"] = child.into();
    meta["payload"]["history_base"] =
        json!({"thread_id":ID,"end_ordinal_exclusive":1,"end_byte_offset":base.len()});
    fs::write(
        sessions.join(format!("rollout-2026-11-10T13-00-00-{child}.jsonl")),
        format!("{meta}\n"),
    )
    .unwrap();
    a.store.capture_query_plans(DB, true).unwrap();
    a.backup(std::slice::from_ref(&root), &BackupOptions { read_git: false, ..Default::default() })
        .unwrap();
    // Exercise the exact empty-child and expiry statements. The metadata source never has a reader.
    let conversation = a
        .store
        .read(DB, |tx| {
            tx.query_one(
                "SELECT conversation_id FROM ah_sessions WHERE harness='codex' LIMIT 1",
                (),
                |r| Ok(r.get::<String>(0)?),
            )
        })
        .unwrap();
    let head = a.reader_header(&conversation).unwrap();
    assert!(a.reader_children(&head, None, 5).unwrap().is_empty());
    for n in 0..2u8 {
        let artifact = Artifact {
            kind: ArtifactKind::Sqlite,
            path: root.root.join("state.sqlite"),
            relative_path: "state.sqlite".into(),
            project_hint: None,
            generation: None,
            current: true,
        };
        let id = a.begin_revision("state", &root, &artifact).unwrap();
        a.append_chunk(id, 0, vec![n]).unwrap();
        a.finish_revision(id, &hex::encode(Sha256::digest([n])), 1).unwrap();
    }
    let plans = a.store.take_query_plans(DB).unwrap();
    assert!(
        !plans
            .iter()
            .any(|p| p.sql.contains("UPDATE ah_reader_version SET state='retired' WHERE source="))
    );
    // Exercise the positive update against an indexed version (selecting a retained revision keeps the fixture).
    a.store
        .write(DB, Priority::Batch, |tx| super::catalog::expire_versions(tx, "bench-000001", 1))
        .unwrap();
    let plans = plans.into_iter().chain(a.store.take_query_plans(DB).unwrap());
    let mut seen = std::collections::BTreeSet::new();
    for p in plans {
        let name = if p.sql.contains("FROM ah_reader_head WHERE parent=?1 AND conversation>") {
            assert!(
                p.steps.iter().any(|step| step.contains("ah_reader_children (parent=?")),
                "{:?}",
                p.steps
            );
            "parent_zero_children"
        } else if p.sql.contains("SELECT EXISTS(SELECT 1 FROM ah_reader_version WHERE source=") {
            "expiry_probe"
        } else if p.sql.contains("UPDATE ah_reader_version SET state='retired' WHERE source=") {
            "expiry_update"
        } else if p.sql.contains("p.rollout_native_id=?2") {
            "lineage_candidates"
        } else if p.sql.contains("v.prefix_digest, v.own_ordinal") {
            "lineage_proof"
        } else {
            continue;
        };
        if seen.insert(name) {
            println!(
                "{}",
                json!({"measurement":"reader_query_plan","roots":roots,"native_body_records":roots*120,"query":name,"sql":p.sql,"steps":p.steps})
            );
        }
    }
    assert_eq!(seen.len(), 5);
}

/// Two displayable records bound a real retained revision of bookkeeping rows. Time
/// only the public window; separately count the actual page fetches with plan capture.
#[test]
#[ignore = "release 14.4M-record hidden-gap measurement"]
fn long_hidden_gap_at_scale() {
    use crate::{Artifact, ArtifactKind, SessionMetadata, archive::SLICE_BYTES};
    use sha2::{Digest, Sha256};
    let n = 14_400_000usize;
    let t = tempfile::tempdir().unwrap();
    let path = t.path().join("archive");
    let a = Archive::create_unencrypted(&path).unwrap();
    let root = SourceRoot { harness: Harness::ClaudeCode, root: t.path().join("claude") };
    let file = "projects/p/sparse.jsonl";
    let artifact = Artifact {
        path: root.root.join(file),
        relative_path: file.into(),
        kind: ArtifactKind::Transcript,
        current: true,
        generation: None,
        project_hint: None,
    };
    let row = |id: &str| {
        format!(
            "{{\"type\":\"user\",\"uuid\":\"{id}\",\"message\":{{\"role\":\"user\",\"content\":\"{id}\"}}}}\n"
        )
    };
    let mut raw = Vec::with_capacity(3 * n + 150);
    raw.extend(row("first").bytes());
    for _ in 0..n - 2 {
        raw.extend_from_slice(b"{}\n");
    }
    raw.extend(row("last").bytes());
    let bytes = raw.len() as u64;
    let id = a.begin_revision("sparse", &root, &artifact).unwrap();
    for (i, chunk) in raw.chunks(SLICE_BYTES).enumerate() {
        a.append_chunk(id, i as u64, chunk.to_vec()).unwrap();
    }
    a.finish_revision(id, &hex::encode(Sha256::digest(raw)), bytes).unwrap();
    let parse = a.prepare_projection("sparse", id, "sparse-measure", true).unwrap().unwrap();
    a.publish_projection_attempt(
        &parse,
        &SessionMetadata { native_id: "sparse".into(), ..Default::default() },
        true,
    )
    .unwrap();
    let h = a.reader_header(&a.list_sessions(&Default::default()).unwrap().items[0].id).unwrap();
    assert_eq!((h.physical_records, h.visible_records), (n as u64, 2));
    let read = |a: &Archive| {
        let start = Instant::now();
        let rows = a.reader_visible_window(&h, None, 0, 2, &Default::default()).unwrap();
        let elapsed = start.elapsed();
        assert_eq!(
            rows.iter().map(|r| (r.ordinal.0, r.parts[0].text.as_str())).collect::<Vec<_>>(),
            [(0, "first"), (n as u64 - 1, "last")]
        );
        elapsed
    };
    a.store.capture_query_plans(DB, true).unwrap();
    read(&a);
    let loads = a
        .store
        .take_query_plans(DB)
        .unwrap()
        .iter()
        .filter(|p| p.sql.contains("SELECT first_rank FROM ah_reader_page"))
        .count();
    assert!(loads < 32, "{loads} page fetches");
    drop(a);
    let mut cold = Vec::new();
    let mut warm = Vec::new();
    for _ in 0..40 {
        let evicted = measurement::evict(&path);
        let a = Archive::open(&path, b"").unwrap();
        let elapsed = read(&a);
        if evicted {
            cold.push(elapsed)
        }
        warm.push(read(&a));
    }
    println!(
        "{}",
        json!({"measurement":"hidden_gap","records":n,"source_bytes":bytes,
        "hidden_records":n-2,"visible_ordinals":[0,n-1],"page_loads":loads,
        "cold":(!cold.is_empty()).then(||measurement::timings(cold)),"warm":measurement::timings(warm)})
    );
}
