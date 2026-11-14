//! Responsibilities: normalize approved roots and reject overlap with archive data.
//! Not responsible for: reading source bodies or interpreting native identity.
use crate::{Archive, Error, Result, SourceRoot};
use std::fs;

pub(super) fn normalize_sources(
    archive: &Archive,
    sources: &[SourceRoot],
) -> Result<Vec<SourceRoot>> {
    let mut selected = Vec::with_capacity(sources.len());
    for source in sources {
        let original_link = fs::symlink_metadata(&source.root)
            .is_ok_and(|metadata| metadata.file_type().is_symlink());
        let resolved =
            fs::canonicalize(&source.root).or_else(|_| std::path::absolute(&source.root))?;
        if resolved.starts_with(&archive.root) || archive.root.starts_with(&resolved) {
            return Err(Error::InvalidInput("archive and source roots must be disjoint".into()));
        }
        let root = if original_link { source.root.clone() } else { resolved };
        if !selected
            .iter()
            .any(|prior: &SourceRoot| prior.harness == source.harness && prior.root == root)
        {
            selected.push(SourceRoot { harness: source.harness, root });
        }
    }
    Ok(selected)
}
