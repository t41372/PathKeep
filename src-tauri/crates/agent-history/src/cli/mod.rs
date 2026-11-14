//! Responsibilities: connect explicit CLI commands to the public library boundary.
//! Not responsible for: native source interpretation, archive transactions or indexing.
mod args;
mod locale;
mod output;

use agent_history::{Archive, Error, capture, export::ExportFormat, query, search::SearchQuery};
use args::{Arguments, Command};
use locale::Locale;
use serde::Serialize;
use serde_json::{Value, json};
use std::ffi::OsString;
use std::io::{Read, Write};
use std::path::Path;
use std::process::ExitCode;
use zeroize::Zeroizing;

type Result<T> = std::result::Result<T, Failure>;

/// Command failures expose stable translated messages, never secret argv values or
/// source records that may be present in lower-level decoder diagnostics.
struct Failure {
    code: &'static str,
}

impl Failure {
    fn invalid() -> Self {
        Self { code: "invalid_input" }
    }
}

impl From<Error> for Failure {
    fn from(error: Error) -> Self {
        Self { code: error.code() }
    }
}

impl From<std::io::Error> for Failure {
    fn from(_: std::io::Error) -> Self {
        Self { code: "io_error" }
    }
}

impl From<serde_json::Error> for Failure {
    fn from(_: serde_json::Error) -> Self {
        Self { code: "invalid_json" }
    }
}

pub(super) fn main_entry() -> ExitCode {
    let raw: Vec<OsString> = std::env::args_os().skip(1).collect();
    let locale = Locale::from_arguments(&raw);
    let response = Arguments::parse(&raw).and_then(|arguments| run(arguments, locale));
    let (value, success) = match response {
        Ok(value) => (value, true),
        Err(failure) => {
            (json!({"error":{"code":failure.code,"message":locale.error(failure.code)}}), false)
        }
    };
    let mut stdout = std::io::stdout().lock();
    if serde_json::to_writer(&mut stdout, &value).is_err() || stdout.write_all(b"\n").is_err() {
        return ExitCode::FAILURE;
    }
    if success { ExitCode::SUCCESS } else { ExitCode::FAILURE }
}

fn run(arguments: Arguments, locale: Locale) -> Result<Value> {
    if arguments.command == Command::Help {
        return Ok(json!({
            "description": locale.description(),
            "usage": locale.usage(),
            "guidance": locale.guidance(),
        }));
    }
    if arguments.command == Command::Plan {
        return encode(capture::preview(&arguments.sources())?);
    }
    if matches!(
        arguments.command,
        Command::Backup | Command::Reparse | Command::Export | Command::RawExport
    ) && !arguments.flag("--execute")
    {
        return Err(Failure { code: "execute_required" });
    }
    validate_required(&arguments)?;
    let root = arguments.path("--archive")?;
    let password = password(&arguments, &mut std::io::stdin().lock())?;
    if arguments.command == Command::Init {
        let archive = if arguments.flag("--no-encryption") {
            Archive::create_unencrypted(&root)?
        } else {
            Archive::create(&root, &password)?
        };
        drop(archive);
        return Ok(json!({"status":"complete"}));
    }
    let archive = Archive::open(&root, &password)?;
    // The password's only job is unwrapping the data key. Drop its allocation now,
    // rather than retaining it for the rest of a potentially long backup/export.
    drop(password);
    match arguments.command {
        Command::Backup => {
            let options = capture::BackupOptions {
                read_git: !arguments.flag("--no-git"),
                ..Default::default()
            };
            encode(archive.backup(&arguments.sources(), &options)?)
        }
        Command::Mark => {
            let boolean =
                |name| -> Result<Option<bool>> { Ok(arguments.text(name)?.map(|s| s == "true")) };
            encode(archive.mark_conversation(
                arguments.required("--conversation")?,
                &agent_history::archive::ConversationMark {
                    starred: boolean("--star")?,
                    put_away: boolean("--put-away")?,
                    trash: boolean("--trash")?,
                    title: if arguments.flag("--clear-title") {
                        Some(None)
                    } else {
                        arguments.text("--title")?.map(|s| Some(s.into()))
                    },
                },
                arguments.flag("--execute"),
            )?)
        }
        Command::Folders => encode(
            archive.folders(
                arguments.text("--path")?,
                arguments
                    .text("--after")?
                    .map(|s| s.parse::<i64>().map_err(|_| Failure::invalid()))
                    .transpose()?,
                arguments.limit()?,
            )?,
        ),
        Command::Reparse => encode(archive.reparse_source(
            arguments.required("--source-id")?,
            &capture::BackupOptions::default(),
        )?),
        Command::List => encode(archive.list_sessions(&query::SessionQuery {
            harness: arguments.harness()?,
            project_path: owned(&arguments, "--project")?,
            put_away: arguments.text("--put-away")?.unwrap_or("exclude").into(),
            trash: arguments.text("--trash")?.unwrap_or("exclude").into(),
            starred: arguments.flag("--starred"),
            sort: match arguments.text("--sort")? {
                Some("created") => query::SessionSort::Created,
                _ => query::SessionSort::Updated,
            },
            limit: arguments.limit()?,
            after: owned(&arguments, "--after")?,
        })?),
        Command::Sources => encode(archive.list_sources(&query::SourceQuery {
            harness: arguments.harness()?,
            kind: arguments.kind()?,
            limit: arguments.limit()?,
            after: owned(&arguments, "--after")?,
        })?),
        Command::Events => encode(archive.events(&query::EventQuery {
            session_id: arguments.required("--session")?.to_owned(),
            limit: arguments.limit()?,
            after: owned(&arguments, "--after")?,
        })?),
        Command::Revisions => encode(archive.revisions(
            arguments.required("--source-id")?,
            arguments.limit()?,
            owned(&arguments, "--after")?,
        )?),
        Command::Search => encode(archive.search(&SearchQuery {
            text: arguments.required("--query")?.to_owned(),
            harness: arguments.harness()?,
            project_path: owned(&arguments, "--project")?,
            limit: arguments.limit()?,
        })?),
        Command::SyncSearch => {
            if arguments.flag("--rebuild") {
                encode(archive.rebuild_search()?)
            } else {
                encode(archive.synchronize_search()?)
            }
        }
        Command::Verify => encode(archive.verify()?),
        Command::Export | Command::RawExport => export(&archive, &arguments, &root),
        Command::Help | Command::Init | Command::Plan => {
            unreachable!("handled before archive open")
        }
    }
}

fn export(archive: &Archive, arguments: &Arguments, root: &Path) -> Result<Value> {
    let destination = arguments.path("--output")?;
    let source_roots = archive.source_roots()?;
    let report = if arguments.command == Command::RawExport {
        let revision = arguments.revision()?;
        output::write_export(&destination, root, &source_roots, |writer| {
            archive.write_raw(revision, writer)
        })?
    } else {
        let session = arguments.required("--session")?;
        let format: ExportFormat = arguments.text("--format")?.unwrap_or("jsonl").parse()?;
        output::write_export(&destination, root, &source_roots, |writer| {
            archive.write_normalized(session, format, writer)
        })?
    };
    encode(report)
}

fn validate_required(arguments: &Arguments) -> Result<()> {
    match arguments.command {
        Command::Mark => {
            arguments.required("--conversation")?;
        }
        Command::Events | Command::Export => {
            arguments.required("--session")?;
        }
        Command::Revisions | Command::Reparse => {
            arguments.required("--source-id")?;
        }
        Command::Search => {
            arguments.required("--query")?;
        }
        Command::RawExport => {
            arguments.revision()?;
        }
        _ => {}
    }
    if matches!(arguments.command, Command::Export | Command::RawExport) {
        arguments.path("--output")?;
    }
    Ok(())
}

fn owned(arguments: &Arguments, name: &str) -> Result<Option<String>> {
    Ok(arguments.text(name)?.map(str::to_owned))
}

fn password(arguments: &Arguments, input: &mut dyn Read) -> Result<Zeroizing<Vec<u8>>> {
    if !arguments.flag("--password-stdin") {
        return Ok(Zeroizing::new(Vec::new()));
    }
    const MAX_PASSWORD_BYTES: usize = 64 * 1024;
    let mut bytes = Zeroizing::new(Vec::new());
    input.take((MAX_PASSWORD_BYTES + 1) as u64).read_to_end(&mut bytes)?;
    if bytes.len() > MAX_PASSWORD_BYTES {
        return Err(Failure::invalid());
    }
    if bytes.last() == Some(&b'\n') {
        bytes.pop();
        if bytes.last() == Some(&b'\r') {
            bytes.pop();
        }
    }
    if bytes.is_empty() {
        return Err(Failure::invalid());
    }
    Ok(bytes)
}

/// JSON IDs remain exact when a future JavaScript client consumes the CLI. Native
/// event payloads are opaque and are deliberately not traversed or rewritten.
fn encode(value: impl Serialize) -> Result<Value> {
    let mut value = serde_json::to_value(value)?;
    stringify_ids(&mut value);
    if let Some(items) = value.get_mut("items").and_then(Value::as_array_mut) {
        for item in items {
            stringify_ids(item);
        }
    }
    Ok(value)
}

fn stringify_ids(value: &mut Value) {
    let Some(object) = value.as_object_mut() else {
        return;
    };
    for (key, value) in object {
        if (key == "id" || key.ends_with("_id")) && value.is_number() {
            *value = Value::String(value.to_string());
        }
    }
}
