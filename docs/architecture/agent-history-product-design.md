# Agent history: format and read model before the first release

Status: design, 2026-11-13. Not built. [agent-history-core.md](agent-history-core.md)
describes the crate as it is today; this page says what changes before v0.6.0
ships it and why. When a part lands, the core page, the
[feature spec](../features/agent-history.md) and the research notes are rewritten
to match, and that part is removed from this page.

## Why now

The `agent-history` crate is in no release and no build of the app writes its
archive. Its schema can be changed in place today. After the first release every
change is a registered migration over people's data, and some facts (whether a
folder was a git worktree before it was deleted) can never be recovered if the
first release did not record them.

The decisions below are ordered by that: what cannot be recovered later comes
first, what can be rebuilt from stored bytes comes last.

## What the product asks for and what the core gives today

| The product shows                                              | The core today                                                                                                     |
| -------------------------------------------------------------- | ------------------------------------------------------------------------------------------------------------------ |
| One conversation with its subagents inside it                  | One session per file. A subagent is a separate session linked by a native id, and its id can equal its parent's    |
| "Only PathKeep has this"                                       | Nothing records whether a source file is still on disk                                                             |
| Projects: a git repository with its worktrees, deleted folders | The exact working-directory string of each session. Nothing about the folder                                       |
| Stars, put-away, renamed titles, pinned projects, sections     | No user-written state                                                                                              |
| Delete a conversation from PathKeep                            | No delete                                                                                                          |
| Tokens, cost and agent time per turn, day, project             | Present only inside each event's JSON payload                                                                      |
| Search everything, streamed, phrase and regex, by role         | Top 100 events, AND of terms; refuses to answer from the first publish of a backup until the end-of-run index sync |
| A list of thousands that pages while a backup runs             | List cursors are invalid after any publish anywhere                                                                |
| An archive that is not three times the data                    | 2.9× the source: raw bytes uncompressed, each record again as JSON, its text a third time, then the index          |
| One archive, one password, or none                             | Its own key envelope and data key; always encrypted                                                                |

## 1. Sources and conversations are different things

A **source** stays what it is now: one file, identified by harness, root and
relative path, with its retained revisions. Nothing about raw capture depends on
what the file means.

A **conversation** is new and is what everything the user sees hangs on.

- The adapter names it. From native evidence it returns a conversation key that
  is the same when the file grows, when the tool writes a new generation file,
  when the tool moves the file (Codex active → archived), and when the same
  conversation is found under a second root. A subagent's key is never its
  parent's key: for Claude Code it is the parent session plus the agent id.
- The conversation id is `sha256("pathkeep-agent-conversation-v1", harness, key)`.
  It needs no lookup and no counter, so a parent link can be written before the
  parent has been captured, and the id is the same after a reparse, on a rebuilt
  catalog and on another machine.
- A conversation has one or more sources. The reader shows the head source: the
  current one with the highest generation, then the most recent capture. The
  others are listed as copies. When the adapter finds no native id, the key is
  derived from the source id and the conversation has exactly that one source.
- An event is addressed by conversation id and ordinal (the record's position in
  the head source), plus the native event id where the tool writes one. Links
  and search hits carry both and resolve by native id first.

Today's rule (session id = source id) makes the user's stars and deletions
depend on an absolute path, gives DeepSeek a new session whenever a generation
file appears, and lets a Claude subagent share its parent's native id.

## 2. What a capture records besides bytes

These are observations. They are appended, never rewritten, and they are the
only record once the thing observed is gone.

**Per source, per run that scanned its root:** seen or missing. A conversation is
"only in PathKeep" when every one of its sources was missing in the last run that
could read their roots. A root that could not be read (unmounted disk, no
permission) says nothing about its files.

**Per folder a conversation worked in:** whether it exists and, when it is inside
a git work tree, the top-level folder, the common git directory (which is how a
worktree names its main repository), the remote URLs and the checked-out branch.
A new observation is stored only when something differs from the last one.

### Reading `.git` (amends the core's rule on what text may authorize)

The core says conversation text cannot authorize file reads. That stays true
for content. The user decided on 2026-11-13 that PathKeep may read the git
location and remotes of project folders, so the rule gains one bounded exception:

- The folder path comes only from the structured working-directory field of a
  session record that an adapter extracted. Never from message text, tool
  arguments or tool output.
- PathKeep walks up from that folder to the first `.git` entry and reads a fixed
  set of small files: `.git` when it is a file, `HEAD`, `commondir`, `gitdir` and
  `config`. Each is opened without following symlinks and capped in size. It
  parses them itself. It never runs `git`, never reads the index, objects or
  hooks, and never lists the work tree.
- Userinfo is removed from remote URLs before they are stored
  (`https://user:token@host/…` keeps only the host and path).
- The Preview of enabling agent history lists that these files are read, and a
  setting turns it off. With it off, projects are grouped by path only.

## 3. What the user writes

Three kinds of data now exist, and each has one rule:

| Kind       | Examples                                                                        | Rule                                                         |
| ---------- | ------------------------------------------------------------------------------- | ------------------------------------------------------------ |
| Evidence   | Raw revisions, observations                                                     | Append only. Removed only by the user's deletion (section 4) |
| User state | Star, put-away, title override, projects and their folders, sections, deletions | Written only by the user's action. Never rebuilt, never lost |
| Read model | Conversation summaries, turns, outlines, search index                           | Disposable. Rebuilt from evidence and user state at any time |

User state is stored with the evidence, not with the read model, so dropping and
rebuilding the catalog cannot touch it. It is keyed by identifiers that outlive a
publication: conversation ids for conversations, PathKeep's own project ids for
projects, folder paths for folder membership.

- **Conversation state**: starred, put away, a title the user typed.
- **Projects**: a project is a row with an id PathKeep generates the first time a
  folder is seen, and an optional name. Which folders belong to it is a list of
  rules over folder paths, each marked as PathKeep's own grouping or the user's.
  A regrouping never rewrites a rule the user made. Removing a folder the user
  added puts it back where PathKeep's grouping would have it.
- **Sections**: the pinned area is a tree of sections and projects with an order,
  and whether each section is collapsed.
- **Suggestions answered**: a "these folders may be one project" suggestion the
  user declined is remembered so it does not come back.

This follows what browsing history does with notes, tags and stars: user-written
rows in the canonical store, keyed by a natural key, untouched by index rebuilds.

## 4. Deleting a conversation

PathKeep keeps a conversation after the tool deletes it. That is the point of the
product, and it is also why the user needs a way out: conversations hold pasted
keys and passwords, and with an unencrypted archive they sit on disk in the clear.

Deleting removes, for the conversation and its subagents:

1. every revision of every source of the conversation, and each raw chunk no
   other revision refers to;
2. attachments only this conversation refers to;
3. its read-model rows and its documents in the search index;
4. its user state.

It leaves a deletion record: harness, conversation key, conversation id and the
time. No title, no folder, no content.

- **It must not come back.** Capture checks the deletion records before it stores
  bytes. A file whose conversation key, or whose parent's key, is in the list is
  skipped and counted in the run report as skipped by the user's deletion. This
  needs the key before the bytes are stored, so adapters gain a cheap way to name
  a conversation from the path and the first records. Removing the deletion
  record ("allow backup again") makes the next run store the file if the tool
  still has it.
- **It must be gone from the files.** Deleted rows leave their pages inside the
  database file, and deleted documents leave their terms inside index segments
  until a merge. After a deletion PathKeep rewrites the affected database by
  copying it (the storage layer already copies and verifies a database table by
  table for a password change) and merges the affected index segments. The
  delete dialog reports done only after both. A run that is interrupted finishes
  the rewrite the next time the archive opens.
- **Files the tool shares between conversations.** Some captured files hold
  pieces of many conversations: Claude Code's prompt history, Codex's session
  index and state databases. A raw revision of such a file cannot lose one
  conversation and stay byte-exact. Rule: when a conversation is deleted, stored
  revisions of shared files that mention it are deleted too, and later captures
  of those files leave out records of deleted conversations and are marked as
  filtered rather than native. Where a shared file's records cannot be tied to a
  conversation, the delete dialog says so and offers to stop keeping that file.
  Which files this covers is settled per adapter from the research notes, with a
  test for each.

- **Safety snapshots.** A snapshot is a full copy of the durable databases taken
  before risky maintenance. One taken before a deletion still holds the
  conversation. Its purpose is recovery from failed maintenance, not undoing
  what the user chose to delete, so a deletion also removes the agent-history
  database from older snapshots, and the dialog's "what goes" list says so.
- **What PathKeep cannot reach** is said plainly in the dialog and the feature
  spec: files the user exported, and backups of the PathKeep folder made by the
  system or by the user.

Putting a conversation away is only a flag in user state. It hides the
conversation from lists; search and insights still include it.

## 5. One archive, one key, or none

The crate opens its own root with its own key envelope and refuses an empty
password. The product has one archive: one encryption choice, one lock, one
password change, one recovery flow, one "delete all my data". The user decided
on 2026-11-13 that agent history follows the archive's encryption choice, and
the archive's default is no encryption. (This overturns "all archive files stay
encrypted" in the core page.)

- **The store is part of the PathKeep archive.** Its databases are roles of the
  one `Vault`, opened by the owner process with the archive's data key or with
  none: a canonical database under `archive/` (evidence, observations, user
  state) and a read-model database and search index under `derived/`. The crate
  stops owning a root, an envelope, an owner lock and a password; it is handed
  open store handles and a search directory.
- **Its own key labels.** Sub-keys are derived from the archive's data key under
  labels of their own. Today the crate reuses the labels of the browsing
  databases, which is safe only because it has a different data key.
- **Its own databases, not tables in the browsing ones.** Each database has one
  writer, so an agent backup does not queue behind a browsing backup, and damage
  to one kind does not take the other down.
- **Names.** `agent.db`, `Db::Agent` and `KeyPurpose::AgentDatabase` already mean
  the assistant's own chats. The new roles are named for agent history, never
  plain "agent".
- **Every lifecycle list learns the new roles in one place.** Snapshots, the
  move bundle, the at-rest check, password change and encryption on/off, storage
  usage and wipe each keep their own list of databases and folders today. Before
  the new roles are added, those lists are replaced by one table that says, per
  role and folder, whether it is canonical or rebuildable, snapshotted, moved,
  and wiped. Two tests guard it: after a wipe the data root holds nothing the
  table does not account for, and the at-rest check fails on a file it does not
  know. A kind that "delete all my data" forgets would be the worst bug this
  work could ship.
- **The standalone CLI stays** as the crate's end-to-end entry point. It builds
  the same store roles over a root of its own, so the crate's tests do not need
  the desktop app.
- **Reads use the vault's reader pool and priority lanes**, so the list, the
  reader and search run at interactive priority beside a backup at batch
  priority.

In the app, agent history is reached like everything else: Tauri commands run in
the owner process and call the crate through `vault-worker`; the scheduled job
and the tray's "Back up now" back up every enabled kind.

## 6. Storage

The numbers behind these choices are under "Measurements" at the end.

- **Raw slices are 4 MiB and compressed.** Each slice is stored zstd-compressed
  (level 3). Its address stays the SHA-256 of the uncompressed bytes, so
  deduplication, verification and byte-exact export are unchanged. A source the
  tool already compressed is stored as it is. 256 KiB slices cut through the
  repetition inside a transcript and store 0.39× of the source; 4 MiB stores
  0.26×, close to compressing the whole file as one stream (0.24×), and a
  reader still reaches any record by decompressing one slice.
- **An appended file does not leave a trail of tails.** Most revisions of a live
  transcript are the previous revision plus more lines. Such a revision keeps
  its record (digest, length, time) and its full slices, which the next revision
  shares, and gives up its last partial slice; its bytes are the first N bytes
  of its successor. Capture knows a revision is a prefix because the running
  hash at the old length equals the old digest. Without this, an hourly backup
  of a conversation in progress keeps a new partial slice every hour, forever.
- **The read model holds no copy of the content.** Today each record is stored
  again as JSON and its text a third time. Instead:
  - per conversation, a summary row (title, times, counts, totals) and an
    **outline**: one compact entry per record with its kind, role, time, parent,
    turn, and where its bytes are in the head revision;
  - per turn, a row of facts for insights (section 7);
  - the text itself is read from the raw chunks when a reader window, a search
    snippet or find-in-conversation asks for it, by the same adapter code that
    builds the outline.
- **Consequences.** A parser fix shows in the reader without reprojecting
  anything. The "raw records" view is the same read as the normal view. A
  conversation costs a few rows, not one row per record, so deleting or
  reprojecting it is cheap, and 14.4 million records are 14.4 million outline
  entries inside blobs rather than 14.4 million table rows.
- **Cost.** Reading depends on adapters at read time: opening a conversation
  decompresses the slice under the window and parses those records. A proxy on
  the largest local transcript (264 MB) reads a 200-record window in about 10 ms
  and scans the whole conversation in under 0.3 s on the development machine.
  The implementation repeats this on the 4-core / 8 GB target before the read
  path is called done. If a window read there misses the 100 ms feedback rule,
  the fallback is to keep each record's rendered text in compressed pages beside
  the outline, which still avoids the uncompressed second and third copies.

## 7. The read model

Rebuildable, versioned by parser version, replaced per conversation in one
publication as today.

- **Conversation summary**: the row the list, the project pages and ⌘K read.
  Title and where it came from (the user's, the tool's, or the first prompt),
  start and last activity, harness, folder, parent conversation, models, counts
  (turns, tool calls, subagents, lines changed), totals (tokens by kind, agent
  time, recorded cost), flags (has a pull request, has edits, has no usage
  record), size.
- **Outline**: what the reader needs to lay out a conversation without reading
  its text. It marks which records are shown: a record that mirrors another
  (Codex writes both a response item and a UI event) is marked as a mirror, a
  record on an abandoned branch carries its branch, a compaction is a boundary.
  Subagents are conversations of their own with a parent link and the ordinal in
  the parent where they were launched; the reader nests them.
- **Turns**: one row per turn (a user prompt to the end of the agent's work):
  start and end, agent time and whether the tool recorded it or it was derived
  from timestamps, tokens by kind, model, tool calls, lines changed. Insights
  and the per-turn footer read these rows.

Usage rules are per harness and live in the adapter, with the evidence in the
research notes: Claude Code repeats `message.usage` on every streamed row of one
response, so usage counts once per `message.id`; Codex writes cumulative
`token_count` records, so a turn's usage is a difference; Pi writes usage and
cost per message. A conversation with no usage record has no totals. It is never
counted as zero.

**Agent time** is the union of turn intervals, so parallel agents count once per
clock second; the per-conversation sum is kept beside it. Day buckets split at
local midnight in the insights time zone, so a day never exceeds 24 hours.

**Projects** are grouped from folder observations: the folder a conversation
worked in maps to its git top-level folder; a worktree maps to the main
repository through the common git directory; a folder with no observation falls
back to path rules for the tools' own worktree locations; everything else is a
project of its own. Folders that share a remote but not a repository are a
suggestion, not a grouping.

## 8. Queries

- **Lists page by key, not by publication.** A page is "the next N after this
  sort key and id". A backup that publishes while the user scrolls adds or moves
  rows; it does not invalidate the cursor.
- **Lists filter and sort on the summary.** Harness, project, time range, flags,
  put-away; by last activity, start, agent time, tokens, turns. Counts per group
  come with the page.
- **Search streams and is not capped.** Results arrive grouped by conversation
  with an exact total when the scan ends, and can be cancelled. Terms, phrases
  (adjacent terms), regex and role scope. Titles are indexed. Tool inputs are
  searchable for every harness (Claude Code's are not today). A snippet returns
  the offsets of its matches. Regex uses the index to narrow candidates by the
  literals in the pattern and otherwise scans, reporting progress.
- **Search answers during a backup.** It reads the last committed index and says
  how far the index is behind, instead of refusing until the run ends.
- **The reader reads windows.** By ordinal range, from the end, or around a
  native event id; find-in-conversation streams matches for one conversation.
- **Insights read turns.** Totals, series by day, by project, by harness, by
  model, hour-of-week, for a time range and filters, computed off the UI thread
  and cached per archive generation.
- **More than one read at a time.** A long search must not hold up a list page.

## 9. Long work

Every operation that can take more than a moment reports progress with totals
where they are knowable (discovery count first, then bytes and files done) and
takes a cancel flag: preview, backup, the index sync at the end of a backup
(today not cancellable), reparse, verify, deletion, rebuild. A backup of one
harness, or of the agent kind alone, is a first-class request.

## What stays

Raw capture and its guarantees: read-only sources, no harness is launched, every
byte kept, whole-file hashing instead of trusting size and time, retained
revisions, per-file validation, the reference rules for attachments, reparse
from stored bytes without touching the source, honest coverage reports. The
adapters' discovery and format knowledge. Turso for relational data and Tantivy
for text, per ADR-012.

## Rejected

- **Keeping the per-record table and compressing it.** It still stores content
  twice, keeps one row per record at 14.4 million, and keeps the unstable row ids
  that search hits use.
- **Storing agent tables inside the browsing archive database.** One writer for
  both kinds, one schema history, and a damaged agent table would take browsing
  history down with it.
- **Keeping session id = source id and mapping user state through a lookup
  table.** The mapping would itself be user state that breaks when a path
  changes.
- **Deleting by hiding.** A hidden conversation still holds the secret the user
  wanted gone.

## Measurements

Taken 2026-11-13 on the development machine (Apple M5 Max) over its own
transcripts, read in place. Only byte counts and timings were produced; no
content was printed, copied or sent anywhere. The scripts used Python's zstd and
json, so the timings are upper bounds for a Rust reader on this machine, not
numbers for the target machine.

**What is there.** 8,357 uncompressed transcripts (Claude Code, Codex, Pi),
13.43 GB, 2.04 million records. 20,653 records are larger than 64 KiB and hold
49 % of all bytes. The largest transcript is 264 MB with 23,812 records (median
record 605 bytes, 99th percentile 206 KB). At today's 2.9× these transcripts
would need about 39 GB of archive.

**Stored size by slice size**, zstd level 3, each slice compressed on its own, on
every fourth Claude Code transcript (2.94 GB):

| Slice                                  | Stored / source |
| -------------------------------------- | --------------- |
| 256 KiB                                | 0.393           |
| 1 MiB                                  | 0.307           |
| 4 MiB                                  | 0.262           |
| 16 MiB                                 | 0.251           |
| Whole file, one stream, 128 MiB window | 0.239           |

Over all 13.43 GB, 256 KiB slices store 0.422× at level 3 and 0.413× at level 9:
the level buys almost nothing, the slice size buys a third.

**Reading from 4 MiB compressed slices**, the largest transcript:

| Read                                    | Time   |
| --------------------------------------- | ------ |
| Last 200 records (open a conversation)  | 6 ms   |
| 200 records in the middle (seek)        | 12 ms  |
| Scan the whole 264 MB for a byte string | 271 ms |

**Still to measure**, in the block that builds each part: window reads and a
full regex scan on the 4-core / 8 GB target; the size of the search index and of
the read model on real data; how long the copy that follows a deletion takes on
an archive of this size.
