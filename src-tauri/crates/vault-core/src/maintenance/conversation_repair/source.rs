//! Verify the members selected from a listed copy. Its unreadable marker describes
//! the original observation; only independently verified evidence can seed a rebuild.
use super::{
    ArtifactId, CompanionUse, DataKey, Db, Path, PathBuf, Result, Vault, ensure, fs,
    member_identity, owned, project_paths_with_root, snapshot,
};
use anyhow::Context;

fn header_binding(vault: &Vault, root: &Path) -> Result<String> {
    let parent = vault.paths().artifact_path(ArtifactId::NativeSnapshots);
    ensure!(
        root.parent() == Some(parent.as_path()) && fs::symlink_metadata(root)?.is_dir(),
        "conversation-copy-not-listed"
    );
    for dir in [vault.paths().artifact_path(ArtifactId::RawSnapshots), parent] {
        ensure!(fs::symlink_metadata(dir)?.is_dir(), "conversation-copy-unsafe-path");
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
            ensure!(fs::symlink_metadata(parent)?.is_dir(), "conversation-copy-unsafe-path");
        }
        if let Ok(meta) = fs::symlink_metadata(&member) {
            ensure!(
                meta.is_file()
                    || meta.is_dir() && root.join(vault_layout::UNREADABLE_CONVERSATIONS).is_file(),
                "conversation-copy-unsafe-path"
            );
        }
        binding.push(member_identity(&member)?);
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
                Some(m) if m.is_file() => Some(snapshot::hash_file(&absolute).map_err(|e| {
                    vault_store::Error::PreserveFile {
                        path: absolute,
                        source: std::io::Error::other(e),
                    }
                })?),
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
                return Err(vault_store::Error::PreserveFile { path: absolute, source }.into());
            }
        };
        ensure!(!meta.is_symlink(), "conversation-copy-unsafe-path");
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
            source.parent() == Some(parent.as_path()) && fs::symlink_metadata(source)?.is_dir(),
            "conversation-copy-not-listed"
        );
        for part in [vault.paths().artifact_path(ArtifactId::RawSnapshots), parent.clone()] {
            ensure!(fs::symlink_metadata(part)?.is_dir(), "conversation-copy-unsafe-path");
        }
        for file in ["config.json", snapshot::SEAL] {
            ensure!(
                fs::symlink_metadata(source.join(file))?.is_file(),
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
        ensure!(fs::metadata(&seal_path)?.len() <= 128 * 1024, "conversation-copy-invalid-seal");
        let seal = fs::read(seal_path)?;
        let proof: Vec<snapshot::FileProof> = serde_json::from_slice(&seal)?;
        let marked = info.conversations_unreadable
            || proof.iter().any(|p| p.path == Path::new(vault_layout::UNREADABLE_CONVERSATIONS));
        info.conversations_unreadable = marked;
        let paths = project_paths_with_root(source);
        let key = if crate::load_config(&paths)?.archive_mode == crate::ArchiveMode::Encrypted {
            let same = vault.paths().artifact_path(ArtifactId::Envelope);
            let incoming = paths.artifact_path(ArtifactId::Envelope);
            ensure!(fs::symlink_metadata(&incoming)?.is_file(), "conversation-copy-unsafe-path");
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
        let held = Path::new(vault_layout::HELD_CONVERSATIONS);
        let main = Path::new(vault_layout::database_path(Db::AgentHistoryEvidence));
        let mut older = Vec::new();
        for member in &self.proof {
            if let Ok(relative) = member.path.strip_prefix(held)
                && let Some(first) = relative.components().next()
                && let Some(n) = first
                    .as_os_str()
                    .to_str()
                    .and_then(|s| s.strip_prefix("older-"))
                    .and_then(|n| n.parse::<u64>().ok())
                && relative == Path::new(first.as_os_str()).join(main)
            {
                older.push((n, held.join(first.as_os_str())));
            }
        }
        older.sort();
        older.dedup();
        [PathBuf::new(), held.to_owned()]
            .into_iter()
            .chain(older.into_iter().map(|(_, p)| p))
            .filter(|prefix| self.proof.iter().any(|p| p.path == prefix.join(main)))
            .collect()
    }
    /// Each candidate has its own directory. A failed proof cannot contribute
    /// any members to the next candidate.
    pub fn copy(&self, roles: &[Db], prefix: &Path, work: &Path) -> Result<Option<Binding>> {
        let mut members = Vec::new();
        for &db in roles {
            let main = Path::new(vault_layout::database_path(db));
            fs::create_dir_all(work.join(main.parent().unwrap()))?;
            for suffix in vault_layout::database_spec(db).companions.suffixes(CompanionUse::Install)
            {
                let name = PathBuf::from(format!("{}{suffix}", main.display()));
                let relative = prefix.join(&name);
                let mut proofs = self.proof.iter().filter(|p| p.path == relative);
                let proof = proofs.next();
                ensure!(proofs.next().is_none(), "conversation-copy-duplicate-role");
                match (member_metadata(self.root, &relative)?, proof) {
                    (None, None) if !suffix.is_empty() => {}
                    (Some(meta), Some(proof)) if meta.is_file() => {
                        let dest = work.join(&name);
                        fs::copy(self.root.join(&relative), &dest)?;
                        if snapshot::hash_file(&dest)? != proof.sha256 {
                            if self.info.conversations_unreadable {
                                return Ok(None);
                            }
                            anyhow::bail!("snapshot-digest-mismatch: {}", relative.display());
                        }
                    }
                    _ => return Ok(None),
                }
                members.push((relative, proof.map(|p| p.sha256.clone())));
            }
        }
        Ok(Some(Binding { header: self.header.clone(), members }))
    }
}
