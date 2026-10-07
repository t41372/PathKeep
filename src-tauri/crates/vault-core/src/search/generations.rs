//! Responsibilities: publish complete generations and retain directories used by old readers.
//! Not responsible for: canonical SQL, query semantics or cryptography.
//! Failure modes: renaming breaks lazy readers, crashes publish an incomplete replacement,
//! a corrupt pointer escapes its root, or cancelled builds leave unbounded disk usage.
//! Tests: owner recovery/rebuild tests and vault-search crash contracts.
use anyhow::{Context, Result};
use std::{
    path::{Path, PathBuf},
    sync::Weak,
};
use vault_search::SearchIndex;

pub(super) struct Generations {
    root: PathBuf,
    current: String,
    key: Option<vault_crypto::DataKey>,
    retired: Vec<(PathBuf, Weak<()>)>,
}
impl Generations {
    pub fn open(
        root: &Path,
        key: Option<&vault_crypto::DataKey>,
    ) -> Result<(Self, SearchIndex, bool)> {
        std::fs::create_dir_all(root)?;
        let key = key.map(|key| vault_crypto::DataKey::from_bytes((*key.expose_bytes()).into()));
        let name = match std::fs::read(root.join("current")) {
            Ok(bytes) => String::from_utf8(bytes).ok().filter(|name| generation_name(name)),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => None,
            Err(error) => return Err(error.into()),
        };
        let mut generations =
            Self { root: root.to_owned(), current: String::new(), key, retired: Vec::new() };
        if let Some(name) = name {
            match SearchIndex::open(&root.join(&name), generations.key.as_ref(), Default::default())
            {
                Ok(index) => {
                    let initial = index.initial_build_incomplete()?;
                    generations.current = name;
                    generations.collect()?;
                    return Ok((generations, index, initial));
                }
                Err(vault_search::Error::RebuildNeeded { .. }) => {}
                Err(error) => return Err(error.into()),
            }
        }
        let (name, index) = generations.create()?;
        generations.point_to(&name)?;
        generations.current = name;
        generations.collect()?;
        Ok((generations, index, true))
    }
    pub fn create(&self) -> Result<(String, SearchIndex)> {
        let mut bytes = [0u8; 16];
        getrandom::fill(&mut bytes)?;
        let name = hex::encode(bytes);
        let index =
            SearchIndex::create(&self.root.join(&name), self.key.as_ref(), Default::default())?;
        Ok((name, index))
    }
    pub fn publish(&mut self, name: String, old: &SearchIndex) -> Result<()> {
        self.point_to(&name)?;
        self.retired.push((self.root.join(&self.current), old.generation_lease()));
        self.current = name;
        Ok(())
    }
    fn point_to(&self, name: &str) -> Result<()> {
        crate::durable_io::atomic_durable_write(&self.root.join("current"), name.as_bytes())
            .context("publishing the search generation")
    }
    pub fn discard(&self, name: &str) -> Result<()> {
        std::fs::remove_dir_all(self.root.join(name))?;
        Ok(())
    }
    pub fn collect(&mut self) -> Result<()> {
        self.retired.retain(|(_, lease)| lease.strong_count() > 0);
        for entry in std::fs::read_dir(&self.root)? {
            let entry = entry?;
            let name = entry.file_name();
            let name = name.to_string_lossy();
            if generation_name(&name)
                && name != self.current
                && !self.retired.iter().any(|(path, _)| *path == entry.path())
            {
                std::fs::remove_dir_all(entry.path())?;
            }
        }
        Ok(())
    }
}
fn generation_name(name: &str) -> bool {
    name.len() == 32 && name.bytes().all(|byte| byte.is_ascii_hexdigit())
}
