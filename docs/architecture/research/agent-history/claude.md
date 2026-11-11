# Claude family: storage and backup research

Research date: 2026-11-11. This records evidence for an independent, read-only
backup library. It is not a promise that every researched surface is implemented.
The capability matrix and implementation notes must state the shipped subset.

## Evidence and confidence

- **Official contract:** current Anthropic documentation describes locations,
  retention, supported export/resume interfaces, and explicitly warns that JSONL
  is an internal format. These pages are mutable; the access date above matters.
- **Official implementation:** the Python Agent SDK reader was inspected at
  [`3cf3446b0fa6c97042e85b245bba07459f22f946`](https://github.com/anthropics/claude-agent-sdk-python/blob/3cf3446b0fa6c97042e85b245bba07459f22f946/src/claude_agent_sdk/_internal/sessions.py).
  Its UI-style projection is useful evidence, not a lossless archive API.
- **Local observation:** authorized, read-only macOS inspection counted files and
  field names. No conversation text, paths identifying projects, account IDs,
  credentials, or source files were copied into this document or repository.
- **Community implementation/report:** pinned code establishes what a tool does;
  an issue establishes a reported failure, not a proven upstream root cause.
  Claude Code and Claude Desktop application persistence internals are not fully
  published in the public Claude Code repository.

## Sources to discover

`C` below is the configured Claude root: `CLAUDE_CONFIG_DIR` when explicitly
supplied, otherwise the user's `.claude` directory. On Windows that default is
under `%USERPROFILE%`. Discovery must accept additional roots; a desktop-launched
PathKeep process does not necessarily inherit a CLI shell's environment.
[Settings files](https://code.claude.com/docs/en/settings)

| Surface                             | Local source                                                                                                       | Interpretation                                                         |
| ----------------------------------- | ------------------------------------------------------------------------------------------------------------------ | ---------------------------------------------------------------------- |
| Claude Code CLI / SDK / IDE         | `C/projects/<project>/<session-id>.jsonl`                                                                          | Transcript event stream; enumerate files directly                      |
| Historical transcript revisions     | `<session>.orphaned-*.jsonl`, `<session>.jsonl.superseded-*`                                                       | Preserve as distinct artifacts, including names not ending in `.jsonl` |
| Subagents                           | `C/projects/<project>/<session>/subagents/**/agent-*.jsonl` plus `.meta.json`                                      | Nested workflow directories also occur                                 |
| Large tool output / returned images | `C/projects/<project>/<session>/tool-results/`                                                                     | Relevant content can be absent from the main JSONL                     |
| Other session evidence              | `C/file-history/<session>/`, `C/uploads/<session>/`, session `mcp-tasks/`, task lists, referenced paste/image data | Preserve available owned artifacts; account for missing references     |
| Claude Desktop Code metadata        | `<DesktopData>/claude-code-sessions/<account>/<org>/local_*.json`                                                  | Links Desktop IDs to one or more CLI transcript IDs                    |
| Legacy/local Cowork                 | `<DesktopData>/local-agent-mode-sessions/<account>/<org>/local_<id>.json` and matching directory                   | Card, audit stream, nested Claude transcripts, uploads, outputs        |
| General Claude chat / cloud Cowork  | Account-managed cloud data; official downloaded export where available                                             | Local Electron caches are not evidence of a complete archive           |

Observed macOS `<DesktopData>` is
`~/Library/Application Support/Claude`. Windows reports identify
`%APPDATA%\Claude`; packaged/MSIX installs may redirect that tree. Linux tooling
uses `~/.config/Claude`; native Linux discovery must also allow an explicit data
root and must be verified on Linux before claiming coverage. WSL is a separate
Linux home and source instance, not automatically the Windows profile. See the
[Windows loss report](https://github.com/anthropics/claude-code/issues/93385) and
the [Linux wrapper's path handling](https://github.com/johnzfitch/claude-cowork-linux/blob/master/docs/FAQ.md).

The official [directory reference](https://code.claude.com/docs/en/claude-directory)
documents transcript revisions, sidecars, and attachment locations. Image-cache
layout is versioned: releases through 2.1.274 used `C/image-cache/<session>`;
newer releases use a session `images/` directory under Claude's temporary tree.
That tree is controlled by `CLAUDE_CODE_TMPDIR`. OS cleanup can remove it before
a backup runs. Preserve available images and report unavailable references;
do not infer that a successfully copied transcript contains every original file.

Do not recursively copy the entire Claude/Electron profile by default. It mixes
conversation artifacts with login state, caches, plugins, package trees and
unrelated application configuration. Discovery should identify owned transcript
and artifact roots, record exclusions, and never follow a transcript-supplied
absolute path outside approved roots without an explicit collection policy.

## Claude Code records and identity

The official [session documentation](https://code.claude.com/docs/en/sessions)
describes project-name sanitization, truncation beyond 200 characters, a hash
suffix, and `CLAUDE_CODE_PROJECT_DIR_NAME` as a configurable override. Consequently,
a project folder name is neither a reversible path encoding nor a project ID.
Use recorded `cwd` as evidence, retain the exact source locator, and keep logical
project association separate from archive identity. A renamed/deleted working
directory must not make its transcript undiscoverable.

Local sampling found 7,782 JSONL files in the ordinary projects tree. Fifteen
recent files included versions 2.1.319, 2.1.321 and 2.1.322. The following are
observed fields, not a closed schema:

| Layer              | Fields / examples                                                                                                                                                                                                                      | Backup consequence                                                              |
| ------------------ | -------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | ------------------------------------------------------------------------------- |
| Envelope           | `type`, `uuid`, `parentUuid`, `logicalParentUuid`, `sessionId`, `session_id`, `timestamp`, `cwd`, `version`                                                                                                                            | Retain raw bytes and unknown fields; expose provenance separately               |
| Origin / branching | `entrypoint`, `isSidechain`, `agentId`, `sourceToolAssistantUUID`, `sourceToolUseID`, `sessionKind`                                                                                                                                    | Keep graph and origin evidence rather than flattening all streams into one chat |
| Message            | `message.role`, `message.id`, `message.content`, `message.model`, `message.usage`                                                                                                                                                      | Content can be a string or typed blocks                                         |
| Blocks             | `text`, `thinking`, `tool_use`, `tool_result`, `image`, `tool_addition`                                                                                                                                                                | Tool results can nest blocks; image data may be inline base64                   |
| Other events       | `attachment`, `api-request`, `api-request-blob`, `api-request-shape`, `mode`, `session-owner`, `permission-mode`, `file-history-snapshot`, `file-history-delta`, `cost-state`, `queue-operation`, `last-prompt`, `ai-title`, `pr-link` | Unknown/non-message records are still source data, never discard on backup      |
| System events      | `compact_boundary`, `turn_duration`, `stop_hook_summary`, `local_command`, `away_summary`                                                                                                                                              | A rendered chat is a projection, not the archive                                |

Archive artifact identity needs a source instance and relative source path.
Native session/message IDs are searchable attributes, not a globally unique key
across independent profiles, copies, forks or harnesses. Record order and byte
positions must survive even when an event has no ID or timestamp.

In a 60-file local sample, 5,381 assistant `message.id` values appeared more than
once within their file. The corresponding records can be different content
blocks of the same model response with distinct envelope UUIDs. Deduplicating
events by `message.id` loses content; summing each row's `usage` can double count
model activity. Preserve raw usage with provenance and define a separate,
version-aware response-usage projection before presenting totals as authoritative.

### Branches, compaction and subagents

The official SDK reader establishes several important failure modes:

- It walks `parentUuid` and deliberately does not follow `logicalParentUuid`
  across compaction for its current-conversation view. That is insufficient for
  an archive that should keep pre-compaction history still present on disk.
- Parallel tool calls can create sibling assistant records sharing `message.id`,
  and tool results branch from different parents. A single latest-leaf walk
  drops real calls/results. Its `_recover_orphaned_parallel_tool_results` repairs
  that projection using call IDs, parent links and writer identity.
- Subagents can be nested under `subagents/workflows/<runId>/`. Their `.meta.json`
  sidecar has `toolUseId` and sometimes `parentAgentId`, linking them to a parent
  invocation. Losing the sidecar loses a recoverable relationship.
- Its visible view excludes sidechain/meta/team messages and filters event types.
  Its error-tolerant parser skips malformed lines. Neither behavior belongs in
  the byte-preserving backup layer.

Retain every graph edge even when its target is absent; do not invent an edge to
the previous chronological row. Dangling parents have been reported in the
[TypeScript SDK issue tracker](https://github.com/anthropics/claude-agent-sdk-typescript/issues/287).
Represent incomplete graph evidence explicitly. A future UI can offer a native
active branch, all recorded branches, and chronological evidence as distinct
views over the same retained data.

## Desktop Code and Cowork are different collections

The local Desktop Code inspection found 448 session cards; 447 carried
`cliSessionId`. A card can also contain `priorCliSessionIds`,
`lastAssistantCliSessionId`, `forkedFromSessionId`, `cwd`, `originCwd`,
`worktreePath`, title and archived state. One card had prior CLI session IDs.
Do not assume one Desktop session permanently equals one transcript file.

418 of those cards were marked `adoptedFromOtherSurface`; two had
`fromCliTranscript`. Therefore membership in the Desktop session list does not
prove that a session originated there. Preserve both facts: source records say
`entrypoint=cli` or another origin, while Desktop metadata says the session is
available/adopted there. Return `unknown` for missing origin rather than guess.

The local Cowork inspection found 38 session cards with matching `cliSessionId`
fields. Their directories contained:

```text
local_<desktop-session-id>.json
local_<desktop-session-id>/
  audit.jsonl
  .audit-key
  .claude/projects/<project>/<cli-session-id>.jsonl
  .claude/projects/<project>/<cli-session-id>/subagents/...
  .claude/projects/<project>/<cli-session-id>/tool-results/...
  uploads/...
  outputs/...
```

Nested transcripts sampled versions 2.1.138–2.1.260 and consistently recorded
`entrypoint=local-agent`. The audit stream is a different representation, with
`session_id`, `_audit_timestamp`, `_audit_hmac`, and events such as `system`,
`result`, `rate_limit_event`, and `command_lifecycle`. Preserve it independently;
indexing both streams as if each were new messages double counts conversation
and usage. A body can remain when its sidebar card is missing, or vice versa.
Discover both, attach available evidence, and report metadata-only sessions.

Keep `.audit-key` opaque if preserving a Cowork session's audit artifacts. Do
not claim signature verification without implementing and testing its actual
algorithm; the observed field names alone do not establish that algorithm.
The key is not a reason to capture unrelated authentication stores.

Current [official Cowork guidance](https://support.claude.com/en/articles/15520349-use-claude-cowork-on-web-desktop-and-mobile)
says new Pro/Max/Ultra tasks run in the cloud from October 6, 2026; older local
tasks remain local. It offers task-data download and bulk Cowork export, with a
documented `/port-task-data` workflow into Code. This is a major completeness
boundary: finding local Cowork directories does not mean all account sessions
are there. Cloud fetching is outside the current local-core scope.

For ordinary Claude conversations, the supported [account export](https://support.claude.com/en/articles/9450526-export-your-claude-data)
includes chat data but excludes original uploaded files. It does not support
import into another personal account. A future local-export adapter should
preserve the downloaded original before making a normalized projection, and
mark absent attachments. Do not present an Electron cache copy as a complete
general-chat backup.

## Existing implementations inspected

These projects are evidence, not dependencies or code to execute on user data.

| Project / pinned revision                                                                                                                                                                         | Concrete mechanism                                                                                                                                                         | Useful lesson / limitation                                                                                                                                                                                            |
| ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | -------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | --------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| [vega-llc/cowork-migrate, `9fcd07dc`](https://github.com/vega-llc/cowork-migrate/blob/9fcd07dc5a7d3be189712a28a3631ab3f6c3ca9a/migrate.sh)                                                        | Copies cards plus whole session directories to staging; installs skipped/forced sessions; rewrites `/Users/<name>/` strings; verifies JSON, audit presence and file counts | Captures uploads/outputs as well as transcripts. Plain recursive copy is not a consistent live snapshot; count equality is not byte verification; global text replacement can alter conversation evidence             |
| [XDUwt/claude-desktop-history, `03b43534`](https://github.com/XDUwt/claude-desktop-history/blob/03b435348e68d472d578a5086903d557e969104a/01-backup.sh)                                            | `tar` of Desktop cards/Cowork tree; `--full` adds projects and file history                                                                                                | Default lightweight backup explicitly excludes Code transcript bodies. PathKeep must state captured scope instead of equating metadata with complete history                                                          |
| [Same revision: Cowork migration](https://github.com/XDUwt/claude-desktop-history/blob/03b435348e68d472d578a5086903d557e969104a/05-migrate-cowork.sh)                                             | Copies body and card, changes account display fields and workspace paths, requires app exit and preview                                                                    | Account/org folders and account state are distinct; do not infer current Desktop account from CLI login. Same-file/in-place migration remains outside this task                                                       |
| [Same revision: Chat export](https://github.com/XDUwt/claude-desktop-history/blob/03b435348e68d472d578a5086903d557e969104a/06-export-chat.sh)                                                     | Reads official `conversations.json` (array or wrapper), renders message text plus attachment names to Markdown                                                             | A useful export-schema reference. Markdown loses structured blocks and attachment bytes; it cannot substitute for raw preservation                                                                                    |
| [simonw/claude-code-transcripts, `316fd093`](https://github.com/simonw/claude-code-transcripts/blob/316fd093aca3c9c0ad8aa70092711aee3adacc6e/src/claude_code_transcripts/__init__.py)             | Recursively discovers JSONL and renders typed content to HTML; optional `--include-json` copies raw input                                                                  | Its parser keeps user/assistant only, drops UUID/parent links, skips malformed lines, and normally excludes agent/warmup/no-summary sessions. Useful renderer, insufficient archive discovery                         |
| [ZeroSumQuant/claude-conversation-extractor, `a12b72e3`](https://github.com/ZeroSumQuant/claude-conversation-extractor/blob/a12b72e3be2e23d4ed1956fd67a09472b311e783/src/extract_claude_logs.py)  | Scans `~/.claude/projects`, builds a role/text/time list, exports JSON/Markdown/HTML                                                                                       | Even JSON is a lossy projection. Nested tool results are omitted; malformed entries silently skipped; session-ID prefixes are used in output naming                                                                   |
| [garrettmoss/restore-claude-history, `55730b85`](https://github.com/garrettmoss/restore-claude-history/blob/55730b852d7abe04b92830c75470c64cde491b19/backup_claude_history.py)                    | SessionStart hook copies JSONL when source grows; recursively includes subagents; atomically replaces a manifest                                                           | Protects a larger prior file against truncation but loses same-size rewrites and shorter new revisions. Payload copy itself is neither versioned nor atomically published                                             |
| [BasedGPT/claude-code-session-recovery, `9bb05a46`](https://github.com/BasedGPT/claude-code-session-recovery/blob/9bb05a4615de94645fed9a21b17d697843cf7a7d/tools/sessions/backup_claude_state.py) | Enumerates account/org cards; regular-file traversal; stat/open/fstat/read/stat checks; SHA-256 manifest; verifies temporary ZIP before atomic publication                 | Strong integrity reference. Per-file stability still is not a global snapshot; same-day ZIP replacement is not indefinite revision retention. Optional FTS5 database is this tool's index, not Claude's native format |
| [DazzleML/Claude-Session-Backup, `059f7fb9`](https://github.com/DazzleML/Claude-Session-Backup/blob/059f7fb983541b20f7df703dad2c6bee41395e61/claude_session_backup/commands.py)                   | Git retains raw history, SQLite indexes sessions; discovers session subtrees/file-history/logger sidecars; tracks deletions                                                | Backup lock coordinates its own writers, not Claude. Indexing and staging live sources can see different revisions. Git `-text` attributes avoid line-ending conversion                                               |
| [ccusage/ccusage, `d06f55bb`](https://github.com/ccusage/ccusage/blob/d06f55bb8937479fea5014fd36a0e7b9286fb630/rust/adapters/claude/src/lib.rs)                                                   | Rust Claude adapter reads usage-bearing lines, merges repeated response usage with request/session evidence                                                                | Usage normalization is a separate derived view. Prefix heuristics for missing request IDs can mismerge gateway IDs; never reuse these rules to discard archive events                                                 |
| [abhinav-adtechs/Claude-Backup, `8a5a5ac6`](https://github.com/abhinav-adtechs/Claude-Backup/blob/8a5a5ac6983058def2f61e11bb8a1e63d1f84a96/scheduled-task/SKILL.md)                               | Natural-language scheduled-agent instructions, mtime-based export, model summaries                                                                                         | This is not a deterministic backup engine. The instructions skip system/tool detail and subagent content; summary availability cannot be a backup prerequisite                                                        |

The accompanying [migration experience notes](https://github.com/XDUwt/claude-desktop-history/blob/03b435348e68d472d578a5086903d557e969104a/LESSONS.md)
are a June 2026 macOS observation, not a current cross-platform contract. Their
statement that Cowork is entirely local predates the official cloud transition.

Further implementation lessons:

- The recovery toolkit's
  [transcript audit](https://github.com/BasedGPT/claude-code-session-recovery/blob/9bb05a4615de94645fed9a21b17d697843cf7a7d/tools/transcript_audit.py)
  bounds binary line reads and graph work, and reports invalid UTF-8, NUL,
  non-object JSON, cycles, missing parents and exhausted budgets. Its
  [metadata synthesis](https://github.com/BasedGPT/claude-code-session-recovery/blob/9bb05a4615de94645fed9a21b17d697843cf7a7d/tools/sessions/synth_session_metadata.py)
  cannot recover missing original MCP configuration. Synthesized cards restore
  discoverability, not original state.
- The Git backup project's
  [FTS importer](https://github.com/DazzleML/Claude-Session-Backup/blob/059f7fb983541b20f7df703dad2c6bee41395e61/claude_session_backup/fts5_importer.py)
  uses content hashing and session reindexing to handle reorder/deletion. Its
  [restore bug report](https://github.com/DazzleML/Claude-Session-Backup/issues/37)
  documents a destructive write through a dangling transcript symlink: restoring
  the target first made the subsequent alias write truncate it. Earlier tests
  moved the entire footprint away and accidentally removed the dangling link
  needed to reproduce the bug. Realistic partial deletion and destination links
  belong in backup/export regression scenarios.
- ccusage's
  [path discovery](https://github.com/ccusage/ccusage/blob/d06f55bb8937479fea5014fd36a0e7b9286fb630/rust/adapters/claude/src/paths.rs)
  accepts multiple comma-separated roots as its own extension. Do not attribute
  that syntax to Claude's `CLAUDE_CONFIG_DIR`. Its usage logic distinguishes
  response ID, request ID, sidechain replay and richer cache details; naive
  per-row sums are demonstrably inadequate.
- The transcript renderer's
  [README at the pinned revision](https://github.com/simonw/claude-code-transcripts/blob/316fd093aca3c9c0ad8aa70092711aee3adacc6e/README.md)
  warns that its unofficial web APIs stopped working. Local correctness must not
  depend on an authenticated private web endpoint remaining unchanged.

## Reliability rules derived from the evidence

1. **Enumerate transcripts independently of indexes.** Reports of a
   [stale session index](https://github.com/anthropics/claude-code/issues/22114)
   and [Desktop card migration failure](https://github.com/anthropics/claude-code/issues/45710)
   show why a sidebar/index is a hint. Capture cards separately and report their
   missing referents without hiding files absent from the index.
2. **Version source bytes, not just growing files.** Live records can be partially
   written; compaction/GC, restore, crash recovery or user editing can replace,
   shrink or rewrite a file. Size/mtime alone is not proof of equal content.
   Retain the previous successful revision if a later read fails.
3. **Read stable snapshots with bounded retries.** Compare source identity and
   metadata before/after streaming; check content verification where the design
   requires it. Record an unstable result instead of reporting complete success.
   Multi-file per-file stability is not an atomic cross-file snapshot: document
   that consistency boundary and capture metadata/body mismatches explicitly.
4. **Separate preservation from parse coverage.** Malformed JSON or an unknown
   type must not stop raw capture. Mark parse gaps with artifact and byte/line
   position. Treat an incomplete tail distinctly from a malformed middle record.
5. **Preserve relationships without trusting paths.** Symlinks/reparse points,
   traversal components, huge attachments, and special files need explicit
   handling. A reference to a source-code file is not blanket permission to
   crawl that project or the whole home directory.
6. **No destructive mirroring.** Source cleanup is a recorded observation, never
   permission to remove archived history. A missing root, denied directory or
   transient read error must not be misreported as source deletion.
7. **Make retention/version boundaries visible.** Official
   [settings documentation](https://code.claude.com/docs/en/settings-reference)
   gives a 30-day default cleanup period; `0` is invalid. Desktop/Cowork retention
   changed at 2.1.248 and managed rules can override exceptions. Persistence can
   also be disabled. PathKeep cannot recover bytes the harness never stored or
   removed before its first successful capture.

## Resume and migration research; no execution

Current official interfaces accept a session ID/name or an absolute transcript
path with `claude --resume`; fork options are available. Cross-project duplicate
IDs can make UUID lookup ambiguous. Restoring a file alone is therefore not a
promise of resumability, particularly for Desktop cards, renamed worktrees,
external artifacts and older versions. Store a structured resume assessment with
version/evidence and missing prerequisites, not a shell string assembled from
untrusted paths. Launching a harness can run hooks, access providers, and change
files, so this task does not launch resumed sessions or repair native indexes.

The safer future project-move feature starts with archive-only aliases from old
to new project locations. Native migration is a separate operation requiring
format-specific preflight, source snapshots, conflict detection, app lifecycle
coordination and rollback. Replacing path substrings throughout JSONL rewrites
historical text and should not be the default migration algorithm.

## Failure scenarios to write before adapter implementation

- CLI and Desktop refer to the same transcript; Desktop adopts a CLI session;
  one Desktop card has multiple CLI transcript IDs.
- No card/index, stale card, deleted body, malformed card; multiple account/org
  roots all remain discoverable.
- Parent and nested workflow subagent use the same session ID; metadata sidecar
  links a subagent to a parent tool call; dangling parent and graph cycle.
- Compaction leaves old history; branches copy earlier records; parallel calls
  share `message.id`; multiple usage-bearing blocks represent one response.
- Nested tool-result blocks, inline images, spilled tool outputs, old image cache,
  newer temp images and an unavailable attachment all retain accurate coverage.
- JSONL has a partial last line, malformed middle line, unknown event, non-object
  JSON value, very large line, invalid UTF-8 or absent timestamp/UUID.
- Append during copy, same-size rewrite, truncate/regrow, atomic replacement,
  source disappears, disk-full destination, interruption before manifest commit.
- Custom config roots, pinned project-directory names, long/Unicode paths,
  Windows/MSIX/WSL roots, symlink escape and unreadable directories.
- Cowork audit and nested transcript both describe the same messages; outputs
  contain a large package tree; nested credential/config files are not silently
  swept into a transcript-only backup.

## Adapter implementation notes

The adapter's integration tests were written before its production module. The
implementation separates streaming record interpretation (`adapters/claude.rs`)
from artifact discovery (`adapters/claude/discovery.rs`). Its structured output
keeps every parseable JSON object, physical ordinal and raw payload; it does not
select an active branch or compute billed usage. Unknown schemas remain evidence.

Code Desktop cards remain raw metadata: this version does not enrich normalized
titles or join `priorCliSessionIds` from those cards. Cowork prefers nested native
transcripts for indexing and keeps the audit stream raw; when no native transcript
is found, it indexes the audit as a fallback. An existing but corrupt native
transcript does not automatically switch to the audit representation. Historical
orphaned/superseded files are marked non-current and retained as raw evidence.

Discovery includes selected config and Desktop trees, history/task metadata and
known attachment directories. Credential/config stores and general chat caches
are excluded. New temporary image directories can be explicitly selected as an
`images` source, but this adapter does not crawl system temporary directories.
Structured file/output references are returned without opening them, capped by
the shared reference budget; resolution and missing-file reporting belong to
capture. All origin/branch evidence remains in payloads when scalar metadata is
ambiguous. Capture must revalidate containment after discovery to close file races.

Only macOS format observation was performed for this research. No harness was
launched, no native record was modified, and no tests/builds were run. Windows,
Linux, cloud export completeness and native resume remain unverified here.
