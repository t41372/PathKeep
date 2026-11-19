//! Verify the members selected from a listed copy. Its unreadable marker describes
//! the original observation; only independently verified evidence can seed a rebuild.
use super::{
    ArchiveFileRole, ArtifactId, CompanionUse, DataKey, Db, Path, PathBuf, Result, Vault, ensure,
    fs, member_identity, owned, project_paths_with_root, snapshot,
};
use anyhow::Context;
use snapshot::SnapshotMemberFailure;

fn metadata(path: &Path, role: ArchiveFileRole) -> Result<fs::Metadata> {
    Ok(fs::symlink_metadata(path).map_err(|e| vault_store::Error::preserve(role, path, e))?)
}

fn header_binding(vault: &Vault, root: &Path) -> Result<String> {
    let parent = vault.paths().artifact_path(ArtifactId::NativeSnapshots);
    ensure!(
        root.parent() == Some(parent.as_path())
            && metadata(root, ArchiveFileRole::Browsing)?.is_dir(),
        "conversation-copy-not-listed"
    );
    for dir in [vault.paths().artifact_path(ArtifactId::RawSnapshots), parent] {
        ensure!(
            metadata(&dir, ArchiveFileRole::Browsing)?.is_dir(),
            "conversation-copy-unsafe-path"
        );
    }
    let mut binding = Vec::new();
    for path in [
        "config.json",
        snapshot::SEAL,
        vault_layout::UNREADABLE_CONVERSATIONS,
        vault_layout::spec(ArtifactId::Envelope).path.relative(),
    ] {
        let member = root.join(path);
        if let Some(parent) = member.parent()
            && parent != root
            && parent.try_exists()?
        {
            ensure!(
                metadata(parent, ArchiveFileRole::Browsing)?.is_dir(),
                "conversation-copy-unsafe-path"
            );
        }
        if let Ok(meta) = fs::symlink_metadata(&member) {
            ensure!(
                meta.is_file()
                    || meta.is_dir() && root.join(vault_layout::UNREADABLE_CONVERSATIONS).is_file(),
                "conversation-copy-unsafe-path"
            );
        }
        binding.push(member_identity(root, Path::new(path))?);
    }

    Ok(crate::utils::sha256_hex(&serde_json::to_vec(&binding)?))
}

pub(super) struct Binding {
    header: String,
    members: Vec<(PathBuf, Option<String>)>,
}

impl Binding {
    pub(super) fn verify(&self, vault: &Vault, root: &Path) -> Result<bool> {
        if header_binding(vault, root)? != self.header {
            return Ok(false);
        }
        for (path, expected) in &self.members {
            let absolute = root.join(path);
            let actual = match member_metadata(root, path)? {
                Some(m) if m.is_file() => {
                    Some(snapshot::hash_file(&absolute, ArchiveFileRole::for_member(path))?)
                }
                None => None,
                Some(_) => return Ok(false),
            };
            if actual != *expected {
                return Ok(false);
            }
        }
        Ok(true)
    }
}

fn member_metadata(root: &Path, path: &Path) -> Result<Option<fs::Metadata>> {
    let mut absolute = root.to_owned();
    for part in path {
        absolute.push(part);
        let meta = match fs::symlink_metadata(&absolute) {
            Ok(m) => m,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(None),
            Err(source) => {
                return Err(vault_store::Error::preserve(
                    ArchiveFileRole::for_member(path),
                    &absolute,
                    source,
                )
                .into());
            }
        };
        if meta.is_symlink() {
            return Err(SnapshotMemberFailure {
                code: "conversation-copy-unsafe-path",
                path: absolute,
                role: ArchiveFileRole::for_member(path),
            }
            .into());
        }
        if absolute == root.join(path) {
            return Ok(Some(meta));
        }
        ensure!(meta.is_dir(), "conversation-copy-unsafe-path");
    }
    Ok(None)
}

pub(super) struct CopySource<'a> {
    root: &'a Path,
    pub info: crate::RecoverySnapshot,
    pub key: Option<DataKey>,
    pub digest: String,
    proof: Vec<snapshot::FileProof>,
    header: String,
}
impl<'a> CopySource<'a> {
    pub fn open(vault: &Vault, source: &'a Path, password: Option<&str>) -> Result<Self> {
        let header = header_binding(vault, source)?;
        let parent = vault.paths().artifact_path(ArtifactId::NativeSnapshots);
        ensure!(
            source.parent() == Some(parent.as_path())
                && metadata(source, ArchiveFileRole::Browsing)?.is_dir(),
            "conversation-copy-not-listed"
        );
        for part in [vault.paths().artifact_path(ArtifactId::RawSnapshots), parent.clone()] {
            ensure!(
                metadata(&part, ArchiveFileRole::Browsing)?.is_dir(),
                "conversation-copy-unsafe-path"
            );
        }
        for file in ["config.json", snapshot::SEAL] {
            ensure!(
                metadata(&source.join(file), ArchiveFileRole::Browsing)?.is_file(),
                "conversation-copy-unsafe-path"
            );
        }
        crate::admission::require_control(source)?;
        let mut info = crate::maintenance::list_snapshots(vault.paths())?
            .into_iter()
            .find(|c| Path::new(&c.path) == source)
            .context("conversation-copy-not-listed")?;
        if info.source_op == "maintenance"
            && source.file_name().is_some_and(|n| n.to_string_lossy().starts_with("rekey-"))
        {
            info.source_op = "rekey".into();
        }

        let seal_path = source.join(snapshot::SEAL);
        if metadata(&seal_path, ArchiveFileRole::Browsing)?.len() > 128 * 1024 {
            return Err(SnapshotMemberFailure::new(
                "conversation-copy-invalid-seal",
                source,
                Path::new(snapshot::SEAL),
            )
            .into());
        }
        let seal = fs::read(&seal_path)
            .map_err(|e| vault_store::Error::preserve(ArchiveFileRole::Browsing, &seal_path, e))?;
        let proof: Vec<snapshot::FileProof> = serde_json::from_slice(&seal).with_context(|| {
            SnapshotMemberFailure::new(
                "conversation-copy-invalid-seal",
                source,
                Path::new(snapshot::SEAL),
            )
        })?;
        let marked = info.conversations_unreadable
            || proof.iter().any(|p| p.path == Path::new(vault_layout::UNREADABLE_CONVERSATIONS));
        info.conversations_unreadable = marked;
        let paths = project_paths_with_root(source);
        let key = if crate::load_config(&paths)?.archive_mode == crate::ArchiveMode::Encrypted {
            let same = vault.paths().artifact_path(ArtifactId::Envelope);
            let incoming = paths.artifact_path(ArtifactId::Envelope);
            ensure!(
                metadata(&incoming, ArchiveFileRole::Browsing)?.is_file(),
                "conversation-copy-unsafe-path"
            );
            if let Some(password) = password {
                Some(Vault::unlock_data_key(&paths, password)?)
            } else if same.is_file()
                && vault_crypto::read_envelope_file(&same)?
                    == vault_crypto::read_envelope_file(&incoming)?
            {
                vault.data_key().map(owned)
            } else {
                anyhow::bail!("archive-password-required: enter the copy password");
            }
        } else {
            None
        };

        Ok(Self { root: source, info, key, digest: crate::utils::sha256_hex(&seal), proof, header })
    }
    pub fn evidence_prefixes(&self) -> Vec<PathBuf> {
        let main = Path::new(vault_layout::database_path(Db::AgentHistoryEvidence));
        let mut held: Vec<_> = self
            .proof
            .iter()
            .filter_map(|member| snapshot::held_evidence_prefix(&member.path))
            .collect();
        held.sort_unstable();
        held.dedup();
        self.proof
            .iter()
            .any(|member| member.path == main)
            .then(PathBuf::new)
            .into_iter()
            .chain(held.into_iter().map(|(_, prefix)| prefix.to_owned()))
            .collect()
    }
    /// Each candidate has its own directory. A failed proof cannot contribute
    /// any members to the next candidate.
    pub fn copy(&self, roles: &[Db], prefix: &Path, work: &Path) -> Result<Binding> {
        let mut members = Vec::new();
        for &db in roles {
            let main = Path::new(vault_layout::database_path(db));
            for suffix in vault_layout::database_spec(db).companions.suffixes(CompanionUse::Install)
            {
                let name = PathBuf::from(format!("{}{suffix}", main.display()));
                let relative = prefix.join(&name);
                let mut proofs = self.proof.iter().filter(|p| p.path == relative);
                let proof = proofs.next();
                if proofs.next().is_some() {
                    return Err(SnapshotMemberFailure::new(
                        "conversation-copy-duplicate-role",
                        self.root,
                        &relative,
                    )
                    .into());
                }
                match (member_metadata(self.root, &relative)?, proof) {
                    (None, None) if !suffix.is_empty() => {}
                    (Some(meta), Some(proof)) if meta.is_file() => {
                        let dest = work.join(&name);
                        vault_store::copy_archive_file(
                            ArchiveFileRole::for_member(&relative),
                            &self.root.join(&relative),
                            &dest,
                        )?;
                        if snapshot::hash_file(&dest, ArchiveFileRole::for_member(&relative))?
                            != proof.sha256
                        {
                            return Err(SnapshotMemberFailure::new(
                                "snapshot-digest-mismatch",
                                self.root,
                                &relative,
                            )
                            .into());
                        }
                    }
                    _ => {
                        return Err(SnapshotMemberFailure::new(
                            "conversation-copy-incomplete",
                            self.root,
                            &relative,
                        )
                        .into());
                    }
                }
                members.push((relative, proof.map(|p| p.sha256.clone())));
            }
        }
        Ok(Binding { header: self.header.clone(), members })
    }
}
