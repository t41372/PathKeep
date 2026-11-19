//! Launches a read at the raw-commit callback of a separate synthetic Claude backup.
//! The writer can finish before the read runs. This sample does not measure overlap.
use agent_history::{Archive, Harness, SourceRoot, capture::BackupOptions};
use std::{
    fs,
    io::{BufWriter, Write},
    path::Path,
    sync::{Arc, mpsc},
};

pub fn after_raw_commit<T>(
    archive: &Archive,
    root: &Path,
    sequence: usize,
    read: impl FnOnce() -> T,
) -> T {
    let folder = root.join("projects/benchmark");
    fs::create_dir_all(&folder).unwrap();
    let mut output = BufWriter::new(fs::File::create(folder.join("producer.jsonl")).unwrap());
    for n in 0..crate::measurement::size("AH_BACKUP_RECORDS", 512) {
        writeln!(output,"{{\"sessionId\":\"producer\",\"type\":\"user\",\"uuid\":\"{n}\",\"message\":{{\"role\":\"user\",\"content\":\"concurrent capture {sequence} {n}\"}}}}").unwrap();
    }
    output.flush().unwrap();
    drop(output);
    std::thread::scope(|scope| {
        let (started, ready) = mpsc::sync_channel(0);
        let producer = scope.spawn(move || {
            let options = BackupOptions {
                read_git: false,
                progress: Some(Arc::new(move |progress| {
                    if progress.phase == "raw_committed" {
                        started.send(()).unwrap()
                    }
                })),
                ..Default::default()
            };
            let report = archive
                .backup(&[SourceRoot { harness: Harness::ClaudeCode, root: root.into() }], &options)
                .unwrap();
            assert_eq!(report.captured, 1);
        });
        ready.recv().expect("backup reached raw commit");
        let result = read();
        producer.join().unwrap();
        result
    })
}
