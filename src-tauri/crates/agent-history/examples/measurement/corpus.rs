//! Generates native Claude rows of known length and marker position. This oracle
//! uses no decoder, JSON parser, archive slices or outline pages.
use agent_history::{Archive, Artifact, ArtifactKind, Harness, SourceRoot, archive::SLICE_BYTES};
use sha2::{Digest, Sha256};
use std::{
    fs::File,
    io::{BufReader, Read, Write},
    path::Path,
};

pub struct Corpus {
    starts: Vec<usize>,
}

impl Corpus {
    pub fn new(records: usize, total: usize) -> Self {
        assert!(records >= 200 && total / records >= 256);
        let median = 605.min(total / records / 2);
        let large = 206_000.min(total / records * 18);
        // Two large rows per hundred; eighteen medium rows absorb the remainder.
        let mut lengths: Vec<_> = (0..records)
            .map(|n| match n % 100 {
                0..=1 => large,
                2..=19 => 0,
                _ => median,
            })
            .collect();
        let mut remaining = total.checked_sub(lengths.iter().sum()).unwrap();
        let mut medium = lengths.iter().filter(|v| **v == 0).count();
        for len in lengths.iter_mut().filter(|v| **v == 0) {
            *len = remaining / medium;
            remaining -= *len;
            medium -= 1;
        }
        let mut starts = vec![0];
        for len in lengths {
            starts.push(starts.last().unwrap() + len);
        }
        assert_eq!(starts.last(), Some(&total));
        Self { starts }
    }
    pub fn records(&self) -> usize {
        self.starts.len() - 1
    }
    pub fn start(&self, rank: usize) -> u64 {
        self.starts[rank] as u64
    }
    pub fn marker(&self, rank: usize) -> String {
        format!("marker-{rank:08}")
    }
    pub fn row(&self, rank: usize) -> Vec<u8> {
        let prefix = format!(
            "{{\"type\":\"user\",\"uuid\":\"native-{rank}\",\"message\":{{\"role\":\"user\",\"content\":\""
        );
        let suffix = format!("{}\"}}}}\n", self.marker(rank));
        self.fill(rank, prefix, suffix, self.starts[rank + 1] - self.starts[rank])
    }
    pub fn array_element(&self, rank: usize) -> Vec<u8> {
        let prefix = format!("{{\"role\":\"user\",\"id\":\"native-{rank}\",\"content\":\"");
        let suffix = format!("{}\"}}", self.marker(rank));
        let length =
            self.starts[rank + 1] - self.starts[rank] - 1 - usize::from(rank + 1 == self.records());
        self.fill(rank, prefix, suffix, length)
    }
    fn fill(&self, rank: usize, prefix: String, suffix: String, length: usize) -> Vec<u8> {
        let mut bytes = prefix.into_bytes();
        let mut random = (rank as u64) + 1;
        while bytes.len() < length - suffix.len() {
            // Repeated native prose mixed with variable bytes; a giant repeated byte
            // would turn the single-frame seek into an unrealistically small input.
            random = random.wrapping_mul(6364136223846793005).wrapping_add(1);
            bytes.push(if bytes.len() % 4 == 0 {
                b'A' + ((random >> 32) % 26) as u8
            } else {
                b" synthetic text "[bytes.len() % 16]
            });
        }
        bytes.extend(suffix.bytes());
        bytes
    }
    pub fn verify(&self, rank: usize, raw: &[u8], start: u64) -> Result<(), &'static str> {
        if raw != self.row(rank) || start != self.start(rank) {
            return Err("wrong raw bytes or physical offset");
        }
        let marker = self.marker(rank);
        let position = raw
            .windows(marker.len())
            .position(|s| s == marker.as_bytes())
            .ok_or("missing marker")?;
        if start + position as u64 != self.start(rank + 1) - 4 - marker.len() as u64 {
            return Err("wrong marker position");
        }
        Ok(())
    }
    pub fn retain(&self, root: &Path, archive: &Archive, form: &str) -> i64 {
        let name = match form {
            "array" => "api_conversation_history.json",
            "plain" => "chat.jsonl",
            _ => "chat.jsonl.zst",
        };
        let path = root.join(name);
        let mut file = File::create(&path).unwrap();
        if form == "array" {
            file.write_all(b"[").unwrap();
            for rank in 0..self.records() {
                if rank > 0 {
                    file.write_all(b",").unwrap();
                }
                file.write_all(&self.array_element(rank)).unwrap();
            }
            file.write_all(b"]").unwrap();
        } else if form == "plain" {
            for rank in 0..self.records() {
                file.write_all(&self.row(rank)).unwrap()
            }
        } else {
            let group = if form == "single" { self.records() } else { 64 };
            for first in (0..self.records()).step_by(group) {
                let mut encoder = zstd::stream::Encoder::new(&mut file, 1).unwrap();
                encoder.include_checksum(true).unwrap();
                for rank in first..self.records().min(first + group) {
                    encoder.write_all(&self.row(rank)).unwrap()
                }
                encoder.finish().unwrap();
            }
        }
        drop(file);
        let source = SourceRoot {
            harness: if form == "array" { Harness::Cline } else { Harness::ClaudeCode },
            root: root.into(),
        };
        let artifact = Artifact {
            kind: ArtifactKind::Transcript,
            path: path.clone(),
            relative_path: name.into(),
            generation: None,
            current: true,
            project_hint: None,
        };
        let revision = archive.begin_revision(form, &source, &artifact).unwrap();
        let file = File::open(&path).unwrap();
        let size = file.metadata().unwrap().len();
        let mut input = BufReader::new(file);
        let mut digest = Sha256::new();
        for (ordinal, offset) in (0..size).step_by(SLICE_BYTES).enumerate() {
            let mut chunk = vec![0; (size - offset).min(SLICE_BYTES as u64) as usize];
            input.read_exact(&mut chunk).unwrap();
            digest.update(&chunk);
            archive.append_chunk(revision, ordinal as u64, chunk).unwrap();
        }
        drop(input);
        std::fs::remove_file(path).unwrap();
        archive
            .finish_revision(revision, &hex::encode(digest.finalize()), size)
            .unwrap()
            .revision_id
    }
}
