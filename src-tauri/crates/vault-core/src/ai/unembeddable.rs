//! A lineage remembers content for which its model returned no signal. Only the writer
//! opens this file. Search reads the published vector plane, which contains usable rows only.
use super::VectorStore;
use anyhow::Result;
use std::{
    collections::HashSet,
    fs::{File, OpenOptions},
    io::{BufReader, Read, Write},
    path::PathBuf,
};

pub(super) struct Unembeddable {
    path: PathBuf,
    keys: HashSet<u64>,
}

impl Unembeddable {
    /// Each record is one little-endian content key. A torn final append can be retried.
    pub fn open(store: &VectorStore) -> Result<Self> {
        let path = store.path().with_extension("pkskip");
        let mut keys = HashSet::new();
        if path.try_exists()? {
            let file = OpenOptions::new().read(true).write(true).open(&path)?;
            let size = file.metadata()?.len();
            if size % 8 != 0 {
                file.set_len(size - size % 8)?;
            }
            let mut reader = BufReader::new(file);
            let mut bytes = [0; 8];
            for _ in 0..size / 8 {
                reader.read_exact(&mut bytes)?;
                keys.insert(u64::from_le_bytes(bytes));
            }
        }
        Ok(Self { path, keys })
    }

    pub fn contains(&self, key: u64) -> bool {
        self.keys.contains(&key)
    }

    pub fn record(&mut self, keys: impl Iterator<Item = u64>) -> Result<()> {
        let mut bytes = Vec::new();
        for key in keys.filter(|key| self.keys.insert(*key)) {
            bytes.extend_from_slice(&key.to_le_bytes());
        }
        if !bytes.is_empty() {
            let mut file = File::options().create(true).append(true).open(&self.path)?;
            file.write_all(&bytes)?;
            drop(file);
            crate::durable_io::fsync_file_durably(&self.path)?;
        }
        Ok(())
    }
}
