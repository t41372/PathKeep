//! Resolve only proven repository relationships and explicit user rules, using saved paths.
//! Folder comparisons retain the path conventions of the source, including on a moved archive.
use super::{Choices, D, E};
use crate::{Archive, Result, capture::git::GitObservation, query::operation};
use std::collections::{BTreeMap, BTreeSet};
use vault_store::{Priority, params};

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Grouping {
    pub read_git: bool,
    pub home: Option<String>,
    pub temporary: Vec<String>,
}
impl Default for Grouping {
    fn default() -> Self {
        use crate::capture::ToolsHome;
        let (home, temporary) = match ToolsHome::resolve() {
            Some(ToolsHome::Isolated(home)) => {
                let temp = home.join("tmp").to_string_lossy().into_owned();
                (Some(home), vec![temp])
            }
            Some(ToolsHome::Native { home, .. }) => (
                Some(home),
                vec![
                    "/tmp".into(),
                    "/private/tmp".into(),
                    "/private/var/folders".into(),
                    std::env::temp_dir().to_string_lossy().into_owned(),
                ],
            ),
            None => (None, Vec::new()),
        };
        Self { read_git: true, home: home.map(|p| p.to_string_lossy().into_owned()), temporary }
    }
}
fn windows(path: &str) -> bool {
    path.as_bytes().get(1) == Some(&b':') || path.starts_with("\\\\")
}
pub(super) fn key(path: &str) -> String {
    let s = if windows(path) { path.replace('\\', "/").to_lowercase() } else { path.into() };
    if s == "/" { s } else { s.trim_end_matches('/').to_owned() }
}
pub(super) fn name(path: &str) -> &str {
    path.trim_end_matches(['/', '\\'])
        .rsplit(['/', '\\'])
        .next()
        .filter(|x| !x.is_empty())
        .unwrap_or(path)
}
pub(super) fn parent(path: &str) -> Option<&str> {
    let path = path.trim_end_matches(['/', '\\']);
    let n = path.rfind(['/', '\\'])?;
    Some(if n == 0 { &path[..1] } else { &path[..n] })
}
fn ancestors(path: &str) -> impl Iterator<Item = &str> {
    std::iter::successors(Some(path), |s| parent(s).filter(|p| p != s))
}
#[derive(Default, Clone)]
pub(super) struct Observation {
    pub exists: Option<bool>,
    pub git: Option<GitObservation>,
}
#[derive(Clone)]
pub(super) struct Folder {
    pub path: String,
    pub own: String,
    pub natural: String,
    pub project: String,
    pub role: &'static str,
    pub reason: &'static str,
    pub observation: Observation,
}
#[derive(Clone, Default)]
pub(super) struct Directory {
    pub folders: BTreeMap<String, Folder>,
    pub mains: BTreeMap<String, String>,
    pub labels: BTreeMap<String, String>,
    pub kinds: BTreeMap<String, &'static str>,
}
impl Archive {
    pub(super) fn resolve_folders(
        &self,
        policy: &Grouping,
        choices: &Choices,
    ) -> Result<Directory> {
        let paths = self.store.read(D, |s| {
            s.query("SELECT path FROM ah_project_paths ORDER BY path", (), |r| {
                Ok(r.get::<String>(0)?)
            })
        })?;
        let mut observations: BTreeMap<String, Observation> = self.store.read(E, |s| {
            let rows = s.query(
                "SELECT f.path,f.exists_on_disk,coalesce(f.git_json,CASE WHEN f.exists_on_disk=0 THEN
                (SELECT p.git_json FROM ah_folder_observations p WHERE p.path=f.path AND p.git_json IS NOT NULL ORDER BY p.id DESC LIMIT 1) END)
                FROM (SELECT path,max(id) AS id FROM ah_folder_observations GROUP BY path) latest JOIN ah_folder_observations f ON f.id=latest.id",
                (),
                |r| {
                    let git = r.get::<Option<String>>(2)?.map(|j| serde_json::from_str(&j)).transpose().map_err(operation)?;
                    Ok((r.get::<String>(0)?, Observation { exists: Some(r.get::<i64>(1)? != 0), git }))
                },
            )?;
            Ok(rows.into_iter().collect())
        })?;
        for p in paths {
            observations.entry(p).or_default();
        }
        let mut repositories = BTreeMap::new();
        if policy.read_git {
            for observation in observations.values() {
                if let Some(git) = &observation.git {
                    let main = if name(&git.common_dir) == ".git" {
                        parent(&git.common_dir).unwrap_or(&git.top_level)
                    } else {
                        &git.common_dir
                    };
                    repositories.insert(key(main), main.to_owned());
                    repositories.insert(key(&git.top_level), main.to_owned());
                }
            }
        }
        // A worktree can be the only captured cwd. Its common directory still names the main root.
        for root in repositories.values() {
            observations.entry(root.clone()).or_default();
        }
        let home = policy.home.as_deref().map(key);
        let temps: BTreeSet<_> = policy.temporary.iter().map(|s| key(s)).collect();
        let mut identities: BTreeSet<_> = observations.keys().map(|s| key(s)).collect();
        if identities
            .iter()
            .any(|k| home.as_ref() == Some(k) || ancestors(k).any(|p| temps.contains(p)))
        {
            identities.insert(String::new()); // Loose folders have no main path.
        }
        let mut ids: BTreeMap<String, String> = self.store.read(E, |s| {
            Ok(s.query("SELECT path,id FROM ah_project_identity", (), |r| {
                Ok((r.get(0)?, r.get(1)?))
            })?
            .into_iter()
            .collect())
        })?;
        identities.retain(|path| !ids.contains_key(path));
        if !identities.is_empty() {
            let added = self.store.write(E, Priority::Batch, move |s| {
                let mut insert = s.prepare("INSERT OR IGNORE INTO ah_project_identity(path,id) VALUES(?1,'p-'||lower(hex(randomblob(16))))")?;
                let mut added = Vec::new();
                for path in identities {
                    insert.execute([path.as_str()])?;
                    let id = s.query_one("SELECT id FROM ah_project_identity WHERE path=?1", [path.as_str()], |r| {
                        Ok(r.get::<String>(0)?)
                    })?;
                    added.push((path, id));
                }
                Ok(added)
            })?;
            ids.extend(added);
        }
        let mut directory = Directory::default();
        for (path, observation) in observations {
            let k = key(&path);
            let scratch = home.as_ref() == Some(&k) || ancestors(&k).any(|p| temps.contains(p));
            let repo = ancestors(&k)
                .find_map(|anchor| repositories.get(anchor).map(|main| (anchor, main)));
            let (main, role, reason) = if scratch {
                ("", "temp", if home.as_ref() == Some(&k) { "home" } else { "temporary" })
            } else if let Some((anchor, main)) = repo {
                if key(main) == k {
                    (main.as_str(), "main", "repository")
                } else if anchor != key(main) {
                    (main.as_str(), "worktree", "commonGitDirectory")
                } else {
                    (main.as_str(), "subfolder", "insideRepository")
                }
            } else {
                (path.as_str(), "main", "folder")
            };
            let natural = ids[&key(main)].clone();
            let own = ids[&k].clone();
            directory.mains.insert(own.clone(), path.clone());
            directory.mains.insert(natural.clone(), main.to_owned());
            directory.kinds.entry(natural.clone()).or_insert(if scratch {
                "scratch"
            } else if repo.is_some() {
                "repo"
            } else {
                "folder"
            });
            let assigned = choices.rules.get(&k);
            let project = assigned.map(|r| r.project.clone()).unwrap_or_else(|| natural.clone());
            let (role, reason) = if assigned.is_none() {
                (role, reason)
            } else if project == own {
                ("main", "userDetached")
            } else {
                ("userAdded", "userAssigned")
            };
            directory.folders.insert(
                path.clone(),
                Folder { path, own, natural, project, role, reason, observation },
            );
        }
        for (path, id) in ids {
            directory.mains.entry(id).or_insert(path);
        }
        for (id, main) in &directory.mains {
            let label = choices.names.get(id).cloned().unwrap_or_else(|| name(main).to_owned());
            directory.labels.insert(id.clone(), label);
            directory.kinds.entry(id.clone()).or_insert("folder");
        }
        Ok(directory)
    }
    pub(super) fn publish_folders(&self, directory: &Directory) -> Result<()> {
        let rows = directory
            .folders
            .values()
            .map(|f| {
                (
                    f.path.clone(),
                    (
                        f.project.clone(),
                        directory.labels[&f.project].clone(),
                        directory.kinds[&f.project].to_owned(),
                    ),
                )
            })
            .collect::<Vec<_>>();
        self.store.write(D, Priority::Batch, move |s| {
            let previous: BTreeMap<_, _> = s
                .query("SELECT path,project,label,kind FROM ah_project_folder", (), |r| {
                    Ok((
                        r.get::<String>(0)?,
                        (r.get::<String>(1)?, r.get::<String>(2)?, r.get::<String>(3)?),
                    ))
                })?
                .into_iter()
                .collect();
            let mut upsert = s.prepare("INSERT OR REPLACE INTO ah_project_folder VALUES(?1,?2,?3,?4)")?;
            let mut assign = s.prepare("UPDATE ah_chat_summary SET project_id=?2 WHERE folder=?1 AND project_id IS NOT ?2")?;
            let mut regrouped = false;
            for (path, fields) in rows {
                if previous.get(&path) == Some(&fields) {
                    continue;
                }
                // New folders join the next search publication. Only a changed existing
                // binding can make a previously delivered project's chip or filter stale.
                regrouped |= previous.contains_key(&path);
                let (id, label, kind) = fields;
                upsert.execute(params![path.as_str(), id.as_str(), label, kind])?;
                if previous.get(&path).is_none_or(|(old, _, _)| old != &id) {
                    assign.execute(params![path, id])?;
                }
            }
            if regrouped {
                s.execute("UPDATE ah_chat_clock SET publication=publication+1 WHERE id=1", ())?;
                s.execute(
                    "UPDATE ah_meta SET value=CAST(CAST(value AS INTEGER)+1 AS TEXT) WHERE key='project_epoch'",
                    (),
                )?;
            }
            Ok(())
        })?;
        Ok(())
    }
}
