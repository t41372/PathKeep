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

- **The adapter names it.** From native evidence it returns a conversation key
  that is the same when the file grows, when the tool writes another file for
  the same conversation, when the tool moves the file, and when the same
  conversation is found under a second root. A subagent's key is never its
  parent's key.
- **The key is the native id plus an origin mark.** The mark is something every
  file of one conversation shares and an unrelated conversation with the same
  native id would not. Each adapter says what it is. For Claude Code and Pi it
  is a digest of the transcript's first record, which is written once. Codex and
  DeepSeek have none, so the id alone is the key: every rollout of a Codex
  thread carries its own time in its session record, and DeepSeek's generations
  are separate files whose headers could not be compared on real data. A tool
  that rewrites one JSON document each time has no steady content, so its mark
  is the source. Two files under different roots with the same native id are one
  conversation when their marks match, and two conversations when they do not.
  A transcript whose first record is still being written has no conversation
  yet; it gets one, with the id the finished file gives, when the record is
  whole. The core's existing contract,
  that an id reused under another root must not merge two histories, holds for
  strangers and no longer splits copies.
- **The id is derived, not issued.** The conversation id is
  `sha256("pathkeep-agent-conversation-v1", harness, key)`. It needs no lookup
  and no counter, and it is the same after a reparse, on a rebuilt read model
  and on another machine.
- **A parent link is the parent's native id, resolved when read.** A subagent's
  file does not contain its parent's origin mark, so it cannot compute the
  parent's id. It records the parent's native id (and, where the tool's layout
  gives it, where the parent's transcript sits). The link resolves to the
  parent's conversation as soon as the parent is in the archive, whichever of
  the two was captured first.
- **A fork is not a child.** A parent is the conversation that spawned this one
  as a subagent. A conversation the user forked from another (Codex
  `forked_from_id`, a Claude Code transcript that begins with another session's
  records, a DeepSeek `parentSession` without `origin: "subagent"`) carries a
  separate "forked from" link, resolved the same way. It is a top-level
  conversation: it is not nested under the one it came from, and putting that
  one in the trash leaves it where it is.
- **A conversation with a published source is always reachable.** It is listed,
  its user state can be read and changed, and exactly one of its sources is
  current, whatever happens to its other files. A DeepSeek generation that has
  been captured and cannot be read still supersedes the older ones (the tool
  itself never falls back): the conversation stays listed under the same id with
  its user state, marked as not fully read, and its line is empty until the
  generation can be read. The older generation does not stand in for it.
- **A conversation has one or more sources, and the adapter says how they
  relate.** A source is either a transcript that stands on its own, or one that
  continues another source from a stated boundary. For Codex the boundary is two
  numbers that count differently: the byte offset is a position in the named
  rollout's own file, the ordinal counts along that rollout's whole history
  (all 70 local rollouts that name a base agree; the 27 whose base itself
  inherits show the difference). A continuation is complete when every base
  down its chain is in the archive with a valid boundary, which is decided when
  read. An attachment that was not kept is counted on the conversation and does
  not make it incomplete (user, 2026-11-13). The reader shows the current
  line: the source the tool itself would open, with the part it inherits read
  from the source it continues. Records in the other sources that are not on
  that line are the conversation's earlier branches. When the adapter finds no
  native id, the key is derived from the source id and the conversation has
  exactly that one source.
- **Copies are told apart by content, never by capture time.** Two roots can hold
  the same conversation (a second profile, an old copy of the tool's folder added
  later). A source whose records are a prefix of another's adds nothing and is
  listed as a copy. When two copies each hold records the other lacks, both are
  kept as branches and the one with the later last record is the current line.
- **A record is addressed** by conversation id, source and ordinal (its position
  in that source), plus the native record id where the tool writes one. Links
  and search hits carry both and resolve by native id first.

What the local files show (counts only, 2026-11-13), and the rule each gives:

| Tool        | Observed                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                | Key                                                                                                                                                                                                                    |
| ----------- | ----------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | ---------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Claude Code | 562 of 564 main transcripts are named after the session id their records carry. The other 2 begin with records copied from another session and are named after their own, later id                                                                                                                                                                                                                                                                                                                                                                                                                                      | The file name. A copied beginning is a "continued from" link, and its usage is not counted twice                                                                                                                       |
| Claude Code | 7,112 of 7,284 subagent files carry their **parent's** session id; 132 carry none. All have an agent id; 3,072 sit in nested workflow folders                                                                                                                                                                                                                                                                                                                                                                                                                                                                           | The parent's session id (the folder above `subagents/`) plus the file's path below `subagents/`                                                                                                                        |
| Codex       | 2,577 rollouts, 2,107 of them compressed. 92 files in 54 threads are named `<thread>_<rollout>`: after a revert the same thread gets another rollout. 68 of them name the rollout they inherit from and where it ends (31 the thread's first rollout, 37 a later one, so there are chains); 24 inherit nothing. None repeats the earlier rollout's records. All 92 carry the thread id; every file of a thread has its own time in its session record, and none repeats the first rollout's. Within a thread the file-name times all differ, and the rollout with the latest last record is always the latest-named one | The stable thread id. Rollouts are sources of one conversation. One that inherits reads its beginning from the rollout it names, which may itself inherit. The current line is the rollout with the latest last record |
| Pi          | 39 files, each with its own id; 38 file names contain it                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                | The header id                                                                                                                                                                                                          |

Today's rule (session id = source id) makes the user's stars and deletions
depend on an absolute path, gives a new session to every extra file of one
conversation, and takes a subagent's native id from records that carry its
parent's id.

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

### What is kept of each kind of source

Slices of 4 MiB (section 6) make a retained tail sixteen times dearer than
today's 256 KiB, so what is retained has to be decided with them. The adapter
declares the kind; the rule follows from it.

| Kind                                                            | Examples                                                                                            | Kept                                                                                                                                                                                                                               |
| --------------------------------------------------------------- | --------------------------------------------------------------------------------------------------- | ---------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Transcript the tool appends to                                  | Claude Code, Codex, Pi, DeepSeek JSONL                                                              | Every revision. A revision that is a byte prefix of the next keeps its record and shares the next one's slices (section 6). A revision that is not a prefix is kept whole: the tool changed history, which is what PathKeep is for |
| Transcript the tool rewrites as one document                    | Continue sessions, Cline and Roo conversation arrays                                                | Every revision, whole, as today. These three tools are not offered in v0.6 (user, 2026-11-13), so nothing more is built for them. When they are, a revision whose records are all in the next one should keep only its record      |
| File written once                                               | Attachments, Claude Code file snapshots, generated images                                           | The one revision. A changed file is a new revision                                                                                                                                                                                 |
| The tool's own state that says something its transcripts do not | Codex `state_N.sqlite` (thread titles, projects and their folders), prompt history, session indexes | The latest snapshot keeps its bytes. An earlier one keeps only its record (digest, length, time) once the new one is verified                                                                                                      |
| The tool's own index of its transcripts                         | Codex `thread_history_N.sqlite`                                                                     | Not captured. It is rebuilt by the tool from the rollouts PathKeep already keeps                                                                                                                                                   |

On this machine `thread_history_1.sqlite` is 2.4 GB and changes whenever Codex is
used. Its tables are a projection of the rollouts (each item row carries a
rollout ordinal, and a state table records how far each rollout has been read).
Today every changed dump is stored as a new revision with each cell hex-encoded,
which doubles it: about 5 GB per backup for data the archive already has.

A SQLite source stays a logical dump read in one transaction, for the reasons the
core gives. Text cells that are valid UTF-8 are written as text, not hex.

## 3. What the user writes

Three kinds of data now exist, and each has one rule:

| Kind       | Examples                                                                    | Rule                                                                  |
| ---------- | --------------------------------------------------------------------------- | --------------------------------------------------------------------- |
| Evidence   | Raw revisions, observations                                                 | Append only. Removed only when the user deletes the whole kind's data |
| User state | Star, put-away, title override, projects and their folders, sections, trash | Written only by the user's action. Never rebuilt, never lost          |
| Read model | Conversation summaries, turns, outlines, search index                       | Disposable. Rebuilt from evidence and user state at any time          |

User state is stored with the evidence, not with the read model, so dropping and
rebuilding the catalog cannot touch it. It is keyed by identifiers that outlive a
publication: conversation ids for conversations, PathKeep's own project ids for
projects, folder paths for folder membership.

- **Conversation state**: starred, put away, in the trash, a title the user typed.
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

## 4. Trash now, permanent deletion later

v0.6 does not delete single conversations from the archive. The user decided on
2026-11-13 that putting a conversation away or moving it to the trash is enough
for now.

- **Put away** hides a conversation from lists. Search and insights still
  include it.
- **Trash** is a soft delete. The conversation and its subagents are in no list,
  no search result and no number. Nothing is removed: its sources keep being
  backed up and it comes back exactly as it was. Both are flags in user state.

The interface says what the trash is. A conversation in the trash is still
stored in the archive, in the clear if the archive is not encrypted. Until
permanent deletion exists, the only ways to remove agent conversations from disk
are deleting the whole kind's saved data or all data, and both are whole files
being removed.

**Why not yet.** Removing one conversation for good is more than deleting rows:

- Deleted rows leave their pages in the database file and deleted documents
  leave their terms in index segments, so the database has to be rewritten by
  copying it and the segments merged. On an archive of several gigabytes that is
  gigabytes written per deletion.
- A safety snapshot taken earlier still holds the conversation and would have to
  be dropped whole and taken again.
- It must not come back at the next backup while the tool still has the file,
  which needs a record of deletions that capture checks before storing bytes.
- Files the tool shares between conversations (prompt history, session indexes,
  state databases) hold pieces of it and cannot lose them and stay byte-exact.
- Conversations that continue from it (a copied beginning, an inherited prefix)
  hold or need its content.

**What the format keeps so it can be added.** Every revision lists its slices,
so slices no surviving revision refers to can be found. Sources map to
conversations through the adapter's key, so a conversation's files can be named
before their bytes are stored. Neither needs a format change later.

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
  reader still reaches any record by decompressing one slice. The read timings
  were taken with slices in memory. Reading and writing blobs of a megabyte or
  more through encrypted Turso is not measured yet; it is the first step of
  building this, and if it is slow the slice is 1 MiB (0.31×).
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
counted as zero. A record that a fork, a resume or a revert copied or inherited
from another transcript is counted where it was first produced, not again in the
copy.

**Agent time** is the union of turn intervals, so parallel agents count once per
clock second; the per-conversation sum is kept beside it. Day buckets split at
local midnight in the insights time zone, so a day never exceeds 24 hours.

**Projects** are grouped only by what is certain. The folder a conversation
worked in maps to its git top-level folder. A worktree maps to its main
repository through the common git directory. A folder that lies inside a known
repository's top-level folder belongs to that repository even after the folder
is gone, because the path shows it. Everything else is a project of its own.
What is only likely is a suggestion the user confirms: folders that share a
remote but not a repository, a tool's worktree folder kept somewhere else whose
repository can no longer be read, a folder that is gone and another of the same
name. Each suggestion says what it rests on.

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
(today not cancellable), reparse, verify, rebuild. A backup of one
harness, or of the agent kind alone, is a first-class request.

## Clauses this amends

`agent-history-core.md` and `docs/features/agent-history.md` are rewritten when
each part lands. The first part has (the evidence format: sections 1 to 4 and
the slices of section 6), and the core document now says so; the row about the
archive's own root and key waits for section 5. These are the sentences that
change:

| Today's text                                                                             | Becomes                                                                                                                                            |
| ---------------------------------------------------------------------------------------- | -------------------------------------------------------------------------------------------------------------------------------------------------- |
| "all archive/projection/search/spill files stay encrypted"                               | They follow the archive's encryption choice, which may be none (user, 2026-11-13). Spill files are always encrypted                                |
| "Conversation text cannot authorize arbitrary filesystem reads"                          | Still true. One bounded read is added: git facts of a folder named by a session's structured working-directory field (user, 2026-11-13; section 2) |
| "Retain previous successful versions when a source is rewritten, truncated, disappears…" | Kept for anything a later revision does not contain. A revision wholly contained in its successor keeps its record, not a second copy of its bytes |
| "The standalone archive has its own root and data key"                                   | In the app it is part of the PathKeep archive and uses its key (section 5). The CLI keeps a root of its own                                        |
| SQLite sources: every changed dump is a revision                                         | Latest snapshot only; the tool's index of its own transcripts is not captured                                                                      |

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

## Measurements

Taken 2026-11-13 on the development machine (Apple M5 Max) over its own
transcripts, read in place. Only byte counts and timings were produced; no
content was printed, copied or sent anywhere. The scripts used Python's zstd and
json, so the timings are upper bounds for a Rust reader on this machine, not
numbers for the target machine.

**What is there.** 8,357 uncompressed transcripts (Claude Code, Codex, Pi),
13.43 GB, 2.04 million records. 20,653 records are larger than 64 KiB and hold
49 % of all bytes. The largest transcript is 264 MB with 23,812 records (median
record 605 bytes, 99th percentile 206 KB). The core's 2.9× was measured on a
synthetic Pi transcript; if it held for these files they would need about 39 GB.
That is an extrapolation, not a measurement.

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
the level buys almost nothing, the slice size buys a third. The slice-size table
is a Claude Code sample; Codex and Pi compressed less at 256 KiB (0.44× and
0.42×) and their ratio at 4 MiB is not measured.

**Other sources.** Claude Code file snapshots 235 MB in 11,575 files; Codex
generated images 258 MB; Codex `state_5.sqlite` 43 MB and
`thread_history_1.sqlite` 2,448 MB; the Claude desktop app's local agent
sessions 444 MB.

**Reading from 4 MiB compressed slices**, the largest transcript:

| Read                                    | Time   |
| --------------------------------------- | ------ |
| Last 200 records (open a conversation)  | 6 ms   |
| 200 records in the middle (seek)        | 12 ms  |
| Scan the whole 264 MB for a byte string | 271 ms |

**Still to measure**, in the block that builds each part: window reads and a
full regex scan on the 4-core / 8 GB target; the size of the search index and of
the read model on real data.
