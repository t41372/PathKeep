//! Owns one reviewed replacement per open vault generation. Only an install journal
//! can outlive the owner and still authorize publication.
use super::Prepared;
use crate::ProjectPaths;
use anyhow::{Result, ensure};
use std::{
    fs,
    path::Path,
    sync::{Arc, Mutex, mpsc},
    time::{Duration, Instant},
};
const PREFIX: &str = "maintenance-agent-preview-";
const TTL: Duration = Duration::from_secs(15 * 60);

pub(crate) fn stage(paths: &ProjectPaths) -> std::io::Result<tempfile::TempDir> {
    fs::create_dir_all(&paths.staging_dir)?;
    tempfile::Builder::new().prefix(PREFIX).tempdir_in(&paths.staging_dir)
}

pub(crate) fn is_preview(path: &Path) -> bool {
    path.file_name().and_then(|s| s.to_str()).is_some_and(|s| s.starts_with(PREFIX))
}

pub(crate) fn clean_abandoned(paths: &ProjectPaths) -> std::io::Result<()> {
    match fs::symlink_metadata(&paths.staging_dir) {
        Ok(m) if !m.is_dir() => {
            return Err(std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                "staging is not a directory",
            ));
        }
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(()),
        Err(e) => return Err(e),
        _ => {}
    }
    let entries = match fs::read_dir(&paths.staging_dir) {
        Ok(e) => e,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(()),
        Err(e) => return Err(e),
    };
    for entry in entries {
        let entry = entry?;
        if is_preview(&entry.path()) {
            let kind = entry.file_type()?;
            if kind.is_dir() {
                fs::remove_dir_all(entry.path())?
            } else if kind.is_file()
                && entry
                    .file_name()
                    .to_str()
                    .is_some_and(|n| n.starts_with("maintenance-agent-preview-handle-"))
            {
                fs::remove_file(entry.path())?;
            }
        }
    }
    Ok(())
}

struct Review {
    plan: Prepared,
    deadline: Instant,
    _dismiss: mpsc::Sender<()>,
}

#[derive(Default)]
pub(crate) struct Cache(Mutex<State>);
#[derive(Default)]
struct State {
    generation: u64,
    plan: Option<Review>,
}

impl Cache {
    pub(crate) fn clear(&self) -> bool {
        let mut state = self.0.lock().unwrap();
        state.generation = state.generation.wrapping_add(1);
        state.plan.take().is_some()
    }
    pub(super) fn generation(&self) -> u64 {
        self.0.lock().unwrap().generation
    }
    pub(super) fn publish(self: &Arc<Self>, generation: u64, plan: Prepared) -> Result<()> {
        self.publish_for(generation, plan, TTL)
    }

    fn publish_for(self: &Arc<Self>, generation: u64, plan: Prepared, ttl: Duration) -> Result<()> {
        let mut state = self.0.lock().unwrap();
        ensure!(state.generation == generation, "conversation-repair-preview-stale");
        let token = plan.preview.token.clone();
        let weak = Arc::downgrade(self);
        let (dismiss, wait) = mpsc::channel();
        let deadline = Instant::now() + ttl;
        std::thread::Builder::new().name("conversation-preview-expiry".into()).spawn(
            move || {
                if wait.recv_timeout(ttl) == Err(mpsc::RecvTimeoutError::Timeout)
                    && let Some(cache) = weak.upgrade()
                {
                    let mut slot = cache.0.lock().unwrap();
                    if slot.plan.as_ref().is_some_and(|r| r.plan.preview.token == token) {
                        slot.plan.take();
                    }
                }
            },
        )?;
        state.generation = state.generation.wrapping_add(1);
        state.plan = Some(Review { plan, deadline, _dismiss: dismiss });
        Ok(())
    }

    pub(super) fn take(&self, token: &str, source: Option<&Path>) -> Result<Prepared> {
        let mut slot = self.0.lock().unwrap();
        ensure!(
            slot.plan.as_ref().is_some_and(|r| r.deadline > Instant::now()
                && r.plan.preview.token == token
                && r.plan.preview.copy_path.as_deref().map(Path::new) == source),
            "conversation-repair-preview-stale"
        );
        Ok(slot.plan.take().unwrap().plan)
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn expired_plan_releases_the_directory_without_another_request() {
        use super::*;
        let cache = Arc::new(Cache::default());
        let stage = tempfile::tempdir().unwrap();
        let path = stage.path().to_owned();
        let preview = super::super::ConversationRepairPreview {
            token: "expired".into(),
            action: "startFresh",
            copy_path: None,
            source_op: None,
            created_at: None,
            conversations: "0".into(),
            conversations_only_in_copy: None,
            conversations_recovered_whole: None,
            conversations_recovered_partially: None,
            skipped_optional_records: "0".into(),
            unreadable_optional_sections: "0".into(),
            kept_as: String::new(),
            review_sources_before_backup: true,
            next_backup_recaptures_tool_files: true,
            archive_only_conversations_unreadable: true,
        };
        cache
            .publish_for(
                0,
                Prepared {
                    stage,
                    preview,
                    relative_copy: Default::default(),
                    originals: vec![],
                    policy: String::new(),
                    staged_identity: None,
                    source_identity: None,
                    held_identity: None,
                    shared_identity: vec![],
                },
                Duration::from_millis(10),
            )
            .unwrap();
        let timeout = Instant::now() + Duration::from_secs(5);
        while path.exists() && Instant::now() < timeout {
            std::thread::sleep(Duration::from_millis(5));
        }
        assert!(!path.exists());
        assert!(cache.take("expired", None).is_err());
    }
}
