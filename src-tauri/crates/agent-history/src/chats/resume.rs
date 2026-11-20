//! Verified terminal commands for saved CLI sessions. This module only formats text.
use crate::{
    Archive, Result,
    read_model::{ReaderPublication, SourceMember},
};
use vault_store::{Db, params};

impl Archive {
    pub(super) fn resume_command(&self, head: &ReaderPublication) -> Result<Option<String>> {
        let publication = head.publication.0.clone();
        let source = head.current_source.clone();
        let (member, version, cwd, origin) = self.store.read(Db::AgentHistoryReadModel, move |s| {
            s.query_one(
                "SELECT m.descriptor,s.harness_version,s.project_path,s.origin FROM ah_reader_member m
            JOIN ah_sessions s ON s.id=m.source WHERE m.publication=?1 AND m.source=?2",
                params![publication, source],
                |r| {
                    Ok((
                        serde_json::from_str::<SourceMember>(&r.get::<String>(0)?)
                            .map_err(crate::query::operation)?,
                        r.get::<Option<String>>(1)?,
                        r.get::<Option<String>>(2)?,
                        r.get::<String>(3)?,
                    ))
                },
            )
        })?;
        if origin != "cli"
            || member.observation.missing_since.is_some()
            || member.parent_id.is_some()
        {
            return Ok(None);
        }
        Ok(command(
            &member.harness,
            version.as_deref(),
            &member.native_id,
            cwd.as_deref(),
            &member.root,
        ))
    }
}

fn command(
    tool: &str,
    version: Option<&str>,
    id: &str,
    cwd: Option<&str>,
    root: &str,
) -> Option<String> {
    let cwd = cwd.filter(|s| !s.is_empty())?;
    if id.len() != 36
        || !id.bytes().enumerate().all(|(i, c)| {
            if [8, 13, 18, 23].contains(&i) { c == b'-' } else { c.is_ascii_hexdigit() }
        })
    {
        return None;
    }
    let (env, invocation) = match (tool, version?) {
        (
            "claude-code",
            "2.1.138" | "2.1.260" | "2.1.319" | "2.1.324" | "2.1.321" | "2.1.322" | "2.1.323"
            | "2.1.325" | "2.1.326" | "2.1.327" | "2.1.328",
        ) => ("CLAUDE_CONFIG_DIR", "claude --resume"),
        ("codex", "0.166.0" | "0.167.1" | "0.170.0") => ("CODEX_HOME", "codex resume"),
        _ => return None,
    };
    if [root, cwd].iter().any(|s| s.contains(['\n', '\r', '\0'])) {
        return None;
    }
    #[cfg(not(windows))]
    {
        let quote = |s: &str| format!("'{}'", s.replace('\'', "'\"'\"'"));
        Some(format!("cd {} && {}={} {} {}", quote(cwd), env, quote(root), invocation, id))
    }
    #[cfg(windows)]
    {
        let quote = |s: &str| format!("'{}'", s.replace('\'', "''"));
        Some(format!(
            "Set-Location -LiteralPath {}; if ($?) {{ $env:{}={}; {} {} }}",
            quote(cwd),
            env,
            quote(root),
            invocation,
            id
        ))
    }
}
