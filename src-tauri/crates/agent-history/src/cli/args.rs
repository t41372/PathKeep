//! Responsibilities: reject ambiguous command lines before reading secrets or files.
//! Not responsible for: archive access, filesystem mutation or localized messages.
use super::{Failure, Result};
use agent_history::model::{ArtifactKind, Harness, SourceRoot};
use std::collections::{BTreeMap, BTreeSet};
use std::ffi::OsString;
use std::path::PathBuf;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Command {
    Help,
    Mark,
    Folders,
    Init,
    Plan,
    Backup,
    Reparse,
    List,
    Sources,
    Events,
    Revisions,
    Search,
    Export,
    RawExport,
    Verify,
    SyncSearch,
}

impl Command {
    fn parse(value: &str) -> Result<Self> {
        Ok(match value {
            "mark" => Self::Mark,
            "folders" => Self::Folders,
            "help" | "--help" | "-h" => Self::Help,
            "init" => Self::Init,
            "plan" => Self::Plan,
            "backup" => Self::Backup,
            "reparse" => Self::Reparse,
            "list" => Self::List,
            "sources" => Self::Sources,
            "events" => Self::Events,
            "revisions" => Self::Revisions,
            "search" => Self::Search,
            "export" => Self::Export,
            "raw-export" => Self::RawExport,
            "verify" => Self::Verify,
            "sync-search" => Self::SyncSearch,
            _ => return Err(Failure::invalid()),
        })
    }

    fn accepts(self, name: &str) -> bool {
        if name == "--locale" {
            return true;
        }
        if !matches!(self, Self::Help | Self::Plan)
            && matches!(name, "--archive" | "--password-stdin")
        {
            return true;
        }
        match self {
            Self::Help | Self::Verify => false,
            Self::Init => name == "--no-encryption",
            Self::Mark => matches!(
                name,
                "--conversation"
                    | "--star"
                    | "--put-away"
                    | "--trash"
                    | "--title"
                    | "--clear-title"
                    | "--execute"
            ),
            Self::Folders => matches!(name, "--path" | "--after" | "--limit"),
            Self::SyncSearch => name == "--rebuild",
            Self::Plan => name == "--source",
            Self::Backup => matches!(name, "--source" | "--execute" | "--no-git"),
            Self::Reparse => matches!(name, "--source-id" | "--execute"),
            Self::List => {
                matches!(
                    name,
                    "--harness"
                        | "--project"
                        | "--limit"
                        | "--after"
                        | "--sort"
                        | "--put-away"
                        | "--trash"
                        | "--starred"
                )
            }
            Self::Sources => matches!(name, "--harness" | "--kind" | "--limit" | "--after"),
            Self::Events => matches!(name, "--session" | "--limit" | "--after"),
            Self::Revisions => matches!(name, "--source-id" | "--limit" | "--after"),
            Self::Search => matches!(name, "--query" | "--harness" | "--project" | "--limit"),
            Self::Export => matches!(name, "--session" | "--format" | "--output" | "--execute"),
            Self::RawExport => matches!(name, "--revision" | "--output" | "--execute"),
        }
    }
}

pub(super) struct Arguments {
    pub command: Command,
    values: BTreeMap<String, OsString>,
    flags: BTreeSet<String>,
    sources: Vec<SourceRoot>,
}

impl Arguments {
    pub fn parse(arguments: &[OsString]) -> Result<Self> {
        let command = match arguments.first() {
            Some(value) => Command::parse(value.to_str().ok_or_else(Failure::invalid)?)?,
            None => Command::Help,
        };
        let mut result =
            Self { command, values: BTreeMap::new(), flags: BTreeSet::new(), sources: vec![] };
        let mut remaining = arguments.iter().skip(1);
        while let Some(name) = remaining.next() {
            let name = name.to_str().ok_or_else(Failure::invalid)?;
            if !command.accepts(name) {
                return Err(Failure::invalid());
            }
            if matches!(
                name,
                "--password-stdin"
                    | "--execute"
                    | "--rebuild"
                    | "--no-encryption"
                    | "--no-git"
                    | "--starred"
                    | "--clear-title"
            ) {
                if !result.flags.insert(name.to_owned()) {
                    return Err(Failure::invalid());
                }
                continue;
            }
            let value = remaining.next().ok_or_else(Failure::invalid)?;
            if name == "--source" {
                let value = value.to_str().ok_or_else(Failure::invalid)?;
                let (harness, root) = value.split_once('=').ok_or_else(Failure::invalid)?;
                if root.is_empty() {
                    return Err(Failure::invalid());
                }
                result.sources.push(SourceRoot { harness: harness.parse()?, root: root.into() });
            } else if result.values.insert(name.to_owned(), value.clone()).is_some() {
                return Err(Failure::invalid());
            }
        }
        result.validate()?;
        Ok(result)
    }

    fn validate(&self) -> Result<()> {
        if self.values.contains_key("--limit") {
            self.limit()?;
        }
        if self.values.contains_key("--harness") {
            self.harness()?;
        }
        if self.values.contains_key("--kind") {
            self.kind()?;
        }
        if let Some(sort) = self.text("--sort")? {
            if !matches!(sort, "created" | "updated") {
                return Err(Failure::invalid());
            }
        }
        if let Some(format) = self.text("--format")? {
            if !matches!(format, "jsonl" | "markdown") {
                return Err(Failure::invalid());
            }
        }
        if self.values.contains_key("--revision") {
            self.revision()?;
        }
        if let Some(locale) = self.text("--locale")? {
            if !matches!(locale, "en" | "zh-CN" | "zh-TW") {
                return Err(Failure::invalid());
            }
        }
        match self.command {
            Command::Init if self.flag("--no-encryption") && self.flag("--password-stdin") => {
                return Err(Failure::invalid());
            }
            Command::Mark => {
                for flag in ["--star", "--put-away", "--trash"] {
                    if self.text(flag)?.is_some_and(|value| !matches!(value, "true" | "false")) {
                        return Err(Failure::invalid());
                    }
                }
                if self.flag("--clear-title") && self.text("--title")?.is_some() {
                    return Err(Failure::invalid());
                }
            }
            Command::List
                if self
                    .text("--put-away")?
                    .is_some_and(|v| !matches!(v, "exclude" | "only" | "include"))
                    || self.text("--trash")?.is_some_and(|v| !matches!(v, "exclude" | "only")) =>
            {
                return Err(Failure::invalid());
            }
            _ => {}
        }
        Ok(())
    }

    pub fn flag(&self, name: &str) -> bool {
        self.flags.contains(name)
    }

    pub fn text(&self, name: &str) -> Result<Option<&str>> {
        self.values.get(name).map(|value| value.to_str().ok_or_else(Failure::invalid)).transpose()
    }

    pub fn required(&self, name: &str) -> Result<&str> {
        self.text(name)?.filter(|value| !value.is_empty()).ok_or_else(Failure::invalid)
    }

    pub fn path(&self, name: &str) -> Result<PathBuf> {
        self.values
            .get(name)
            .filter(|value| !value.is_empty())
            .map(PathBuf::from)
            .ok_or_else(Failure::invalid)
    }

    pub fn limit(&self) -> Result<usize> {
        let limit = self
            .text("--limit")?
            .unwrap_or("100")
            .parse::<usize>()
            .map_err(|_| Failure::invalid())?;
        let maximum = if self.command == Command::Search { 100 } else { 500 };
        if !(1..=maximum).contains(&limit) {
            return Err(Failure::invalid());
        }
        Ok(limit)
    }

    pub fn harness(&self) -> Result<Option<Harness>> {
        self.text("--harness")?.map(str::parse).transpose().map_err(Into::into)
    }

    pub fn kind(&self) -> Result<Option<ArtifactKind>> {
        Ok(match self.text("--kind")? {
            None => None,
            Some("transcript") => Some(ArtifactKind::Transcript),
            Some("attachment") => Some(ArtifactKind::Attachment),
            Some("metadata") => Some(ArtifactKind::Metadata),
            Some("sqlite") => Some(ArtifactKind::Sqlite),
            Some(_) => return Err(Failure::invalid()),
        })
    }

    pub fn revision(&self) -> Result<i64> {
        let id = self.required("--revision")?.parse::<i64>().map_err(|_| Failure::invalid())?;
        if id <= 0 {
            return Err(Failure::invalid());
        }
        Ok(id)
    }

    pub fn sources(&self) -> Vec<SourceRoot> {
        if self.sources.is_empty() {
            agent_history::capture::default_sources()
        } else {
            self.sources.clone()
        }
    }
}
