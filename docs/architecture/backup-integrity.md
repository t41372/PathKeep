# Browser history backup integrity

Browser row ids are not visit identities. Up to v0.5.0-dev, native backup treated them as if
they were, and lost new history while saying it succeeded. This page describes what browsers
actually do with their history files, how a backup identifies a visit instead, how much of a
file each backup reads, and what a run reports. Code: `archive/ingest/{identity,read_plan,
consumer,writes}.rs`, `browser_history_parser::native_visits`, `store_schema/backup_identity.rs`.

## What went wrong

Three paths could each drop new history:

1. The next backup read only `visits.id > last_visit_id`. Chrome numbers its visits from 1
   again after it clears all history or rebuilds a damaged file, so every new visit was below
   the old mark.
2. `UNIQUE(profile, source_visit_id)` with `INSERT OR IGNORE`: a new visit whose row id an older
   visit once had was silently ignored. Firefox gives a deleted row's id to its next visit.
3. URLs were found by their browser row id, so a reused id rewrote an old URL row's address,
   and every visit and search term pointing at it moved to the new address.

The run still reported success, because nothing failed. On one real Chrome profile Chrome
rebuilt the history file in place twice in three months, and the archive kept nothing after the
first rebuild.

## What browsers do

Verified in Chromium and Firefox source, and on a copy of one real Chrome profile's history
(46,295 visits; counted, not read).

- **Chromium.** `visits.id` is `INTEGER PRIMARY KEY AUTOINCREMENT`, so a deleted row's id is
  not given out again while the table exists. "Clear all history" drops and recreates the
  table: ids start at 1 and its `sqlite_sequence` row is gone. Razing a damaged file empties it
  in place, with the same effect. `visit_time` (microseconds since 1601) is written once and
  never changes. `(url, visit_time)` is not unique: a redirect's hops share one time (78 such
  pairs in that copy). `visit_duration` is written when the tab closes, sometimes days
  later. Sync writes older visits after newer ones: 7,843 of 15,843 openers in that copy point
  at a higher id. Downloads are numbered max+1, so a deleted newest download's id comes
  back. Sync matches incoming visits by time.
- **Firefox.** `moz_historyvisits.id` is a plain rowid: deleting the newest visits frees their
  ids for the next ones. Sync gives a device's own visits back after a reset with new ids,
  times cut to the millisecond, their kind (`visit_type`) kept, and no opener; it tells a
  returned visit by its millisecond and kind. `moz_places_metadata` rows are rewritten in
  place.
- **Safari.** `history_visits.id` is AUTOINCREMENT; `visit_time` is a double (seconds since 2001) that can tell apart times less than a microsecond apart.

## A visit's identity

A native visit is its profile, its page's exact address and the browser's own time, kept
without loss in `visits.source_visit_time`: `chromium-us:<µs>`, `firefox-us:<µs>`,
`safari-f64:<bits>`. Among a profile's archived visits at the same millisecond and address
(`archived_at`, through the profile/time index, which also holds visits hidden behind another
source's copy):

1. the one with the same browser time is the visit; for Firefox, the one with the same time and
   kind (transition), as Firefox and its Sync tell two visits at one address and time apart
   (Sync's times are cut to the millisecond, so a link and a typed visit within one come back at
   one time). The import preview and the openers a read did not reach (`VisitMark.kind`) use
   the same test. When this capture already wrote or found it, the row is another row of that
   visit (a redirect hop, which Chrome writes with one time and two kinds) and the two merge:
   the longest duration, the first row's opener, a parent either row names;
2. for Firefox only, when one of the two times is a whole millisecond, the other row of the same
   kind it has not been matched to is probably the same visit given back by Sync, told the way
   Firefox's Sync tells one. Probably, not surely: the row is written as its own visit with its
   own time, hidden behind that one and counted once (`same_as`, as another source's copy is,
   `same-visit-across-sources.md`), and what it opened points at the visit that counts. Both
   rows stay; nothing is merged (2026-11-02, the user's decision);
3. a visit archived by version 1, which has no browser time and is not yet matched: it takes
   the capture's time from then on. Version 1 kept visits at one address and millisecond apart
   by their transition and title, so several can wait: the row takes the one with its
   transition (a renumbered file keeps that, not the row id), then the one read from its row id,
   and the first only when nothing tells them apart;
4. otherwise the row is a new visit. Nothing is left out because it looks familiar.

Rows with a browser time have no event fingerprint: one built from the millisecond would make
two visits less than a millisecond apart into one. Takeout keeps its fingerprints and its
existing identity ([same-visit-across-sources.md](same-visit-across-sources.md)).

- **URLs** are found by `(profile, address)`. Version 1 could leave more than one row for an
  address; the first one is used. A newer read updates the title, counts and last visit.
- **Search terms** carry their page's address from the parser (`ParsedSearchTerm.url`) and are
  matched by it, not by the page's row id.
- **Downloads** are their guid, or, for the oldest rows without one, where they were saved and
  when they started. Their state, sizes and paths change while they run and after, and nothing
  in a row says which of two states is later, so each keeps when the copy of the file it was
  read from was taken (`observed_at_ms`, migration 21; the copy's start for a backup or an
  import, the checkpoint's time for a replay; a row from before has its `recorded_at`). A copy
  taken earlier leaves it as it is: a checkpoint replayed after a backup saw a download finish
  does not put it back in progress, and still adds downloads only the checkpoint has. Every
  backup reads all downloads: the table is small and its ids come back. URLs need no such time
  (their last visit says which read is newer), and a visit's other fields only grow.

### What a later read changes

The time and address never change. A later read of a visit brings in a longer duration (Chrome
only ever writes it once the tab closes), the mark that Sync knows it (it never goes off), a
visited-link id the visit did not have, and the visit that opened it when the read can name it
and it had none. A read never takes these back, so replaying an older checkpoint cannot undo a
later backup. Its opener's row id, transition and referrer stay as first read: Insights
classified the visit by them. A read whose payload equals the saved one writes nothing; the
payload includes the row id, so the first full read after a renumbering rewrites each
still-present visit's payload once.

### Openers

`visits.parent_visit_id` names the visit that opened a visit by archive id. A capture keeps
each row's archive id (`identity::Capture`; its size is the capture's, about 50 MB for two
million rows): a child whose opener came earlier in the read gets it at once; one whose opener
came later (Sync) gets it when the read ends, in bounded transactions; one whose opener is
outside the rows read gets it from the browser file's record of that row, matched to the
archive by identity. Those openers are only in memory until they are written, so a read of the rows after
the last one saves the visit mark it started from with each batch and the one it reached only
after them: a read cut off before (the process ending, a stop, a file that fails part way) is
read again from where it started, and a visit Sync wrote with an old time gets its opener from the next backup,
not the next read of every row. A read of every row saves the row it reached instead: the rest of
it, cut off, reads its visits from the first again anyway, and needs that row to know the file
is still the one it was reading. An opener once named stays: Chrome points a deleted visit's
children at the visit that opened it, and the archive still has the deleted one. A redirect's
later hop names the earlier one; merged into one visit, that is not an opener. Version 1 visits got theirs when the archive was updated, from the row id they
named (migration 15), before any backup could give one of them its browser time: a link then
survives either visit being matched to its browser row again, or neither. Navigation follows
`parent_visit_id` alone, and so does Insights (structural version 13): a visit's opener there
is its archive id, so a redirect's hops are one visit and a renumbered file's ids do not meet
old ones. An opener the browser names but the archive never had is still a link for
classification and matches no visit. Insights builds visits once and then adds the ones after
them, so an opener given to a visit it already built (Sync bringing the opener in a later
backup, a read cut off before naming it) reaches its site flows and trails only at its next
full build; Navigation follows the link at once (`WORK-INSIGHTS-LATE-OPENERS`).

Observed interactions read a visit's duration from its own row, which every read keeps
current. Only version 1 visits are looked up in source evidence by row id, as before, and only
in evidence version 1's own runs captured (`identity_upgrade.last_legacy_run`, migration 16):
a later visit given the same row id writes its evidence under that id too.

## How much a backup reads

`read_plan::plan` decides per profile:

- **The rows after the last one read, and the last 24 hours again** — while the row the last
  backup stopped at still holds the visit it held (`profile_watermarks.last_visit_key` /
  `last_visit_url`). The browser has added rows since and not numbered them again. When that
  row is gone, an AUTOINCREMENT table whose `sqlite_sequence` has reached its id is still safe
  (a deleted row; its id is never given out again). Either way no row at or below it may be a
  visit newer than the newest one any backup read (`newest_visit_ms`): every such row was read
  before and browser times never change, so one is a new visit something wrote under an old id
  below the sequence. The browser's time index answers that from the newest rows only. A row
  whose time is past year 9999, which the parser keeps as that bound, counts on neither side:
  it would otherwise read as newer than itself, and every backup would read every row. And the
  row with the lowest id (`first_visit_id` / `_key` / `_url`, saved with each read) may not be
  lower, nor hold another visit: rows are only added above the highest id and taken away, so in
  the same table the lowest id only rises (Chrome drops visits older than 90 days from the
  bottom every day). Chrome clearing all history makes the table again from id 1, and once Sync
  has written older visits back past the cursor and the cursor row is deleted, the sequence
  vouches for it and none of those visits is newer than any read; only the lowest row shows
  it. The 24 hours bring in durations and openers written after the visit.

  These are evidence, not proof: a table made again whose lowest row and cursor row were both
  deleted since, with nothing newer below the cursor, passes them. The weekly read of every row
  is what catches that; a stronger proof would be a full read on every backup.

- **Every row** — on the first backup, the first one after this update (it gives version 1
  visits their browser times), once a week (tabs left open for days, openers Sync filled in), and
  when the row the last backup stopped at holds another visit or is gone without a sequence to
  vouch for it, or the lowest row is lower or another visit. The run then notes
  `source-ids-reset`. The cursors go back to the start first:
  they only move forward otherwise, and a renumbered file's ids are lower — and the read is
  marked under way (`full_scan_started_at`).
- **The rest of a read of every row that stopped part way** — while the row it stopped at
  still holds the same visit and the lowest row is the one it started with, or higher
  (otherwise every row again): the browser may have made the file again in between, and the
  rest of the read would then go on from the old file's cursors and never reach the new file's
  first pages. A read of every row saves the row it reached with each batch for this, and the
  lowest row with its first batch, even one cut off before any visit. Its URL phase carries on from its
  cursor; its visits are read from the first again, because the openers they were waiting for
  were only in memory. Visits it already saved are matched, not written, which is fast. The
  mark clears, and `last_full_scan_at` is set, only when a read reaches the end; a checkpoint
  replay leaves it alone.

A weekly full read costs what the browser file holds, not the archive: its rows are probed and
compared, and unchanged ones write nothing. Source evidence for unchanged rows is deduplicated
by content.

Checkpoint replay and browser history files the user imports read every row and use the same
identity, so replaying a checkpoint adds no copies. A replay saves none of the profile's cursors:
its rows are numbered by an older copy of the browser file, and a cursor taken from it, past rows
the live file added since and then deleted with its sequence past them, would have the next
backup read only after them. The import preview asks what the import
would do, remembering what it already matched: a version 1 visit counts once, as the import
stamps it once, and rows of one visit anywhere in the file are one, a version 1 visit's other
rows included once the import would have stamped it (the identities the import gives, new or
stamped, are kept as 64-bit hashes, about 8 MB for a million; a collision, about one in
180,000 previews of 14.4 million new rows, counts one new visit as already there, while the
import itself compares exact identities, and the exact pairs would take gigabytes), so its
"already in your archive" is what the import skips and its Import button is there while
something is missing.

## What a run says

Each selected profile ends with an outcome: `complete`, `unavailable { code }` (its files could
not be found or opened: `source-profile-not-discovered`, `source-unreadable`,
`source-permission-denied`) or `failed { code }` (they opened but could not be read to the end:
`source-read-failed`, `source-stream-failed`; what was read before is saved and counted, also
when the file fails after its rows, while its openers or page-less rows are read; an archive
error there fails the backup). The run is
`success` when every selected profile is complete, `partial` when the backup ran and any
profile was not, and `failed` only when the backup itself failed (archive, disk, lock), with
its original error. `partial` moves the scheduler's due clock when at least one profile was read to the end (one that kept nothing does not, so the next scheduled run tries again); health and "last successful
backup" count only `success`. A scope with no profile at all is `backup-no-profiles`, not a
success that read nothing.

Rows the browser itself cannot show — a visit whose page row is gone — have no address to save.
The run counts them in a `source-visits-without-page` note per profile and stays `success`.
A first backup that is `partial` still finishes setup; its result says that some browsers could
not be read and that Backup shows which.

Link previews are fetched after the run has replied (`archive_flows/previews.rs`): one pass at
a time, the newest waiting request kept, each pass reading the settings current when it starts.

## Version 1 archives

Opening one runs Archive migrations 2–21 (`backup_identity.rs`), one DDL statement each: the
two visit columns, the row-id index made unique only among visits without a browser time
(version 1 ones until a backup reads their row again, and Takeout's synthetic ids), the URL and
download row-id uniqueness dropped, the download guid and path/time indexes, the five watermark
columns; then the opener backfill (15) — one pass over the visits in 50,000-visit transactions
that resume where they stopped, shown on the update screen — the version 1 run boundary
(16), the three columns of the lowest visit row (17–19), the Firefox Sync copy mark (20) and
the time a download was observed (21). The SQLCipher converter runs both again after it copies version 1's facts into the new
archive (`conversion::adopt_version_one_facts`). No other data is rewritten at open. The first backup reads every row and gives the visits
the browser still has their browser times; visits the browser no longer has stay as version 1
left them. Damage version 1 already did — a URL row whose address a reused id rewrote, a new
visit it ignored that the browser has since dropped — is not repaired: nothing records which
address was right.

## Measurements

A million generated visits, encrypted, release build, on an M5 Max
([raw outputs and method](research/2026-11-backup-integrity-1m/README.md)): the first backup
reads and writes them in 157 s; the next backup with nothing new takes 0.6 s; the weekly full
read 32 s. A version 1 archive of the same history opens after the update in 4 s, and its first
backup, which gives every visit its browser time, takes 38 s and adds nothing. 14.4M visits and
the target machine are not measured yet (`WORK-BACKUP-INTEGRITY-SCALE` in BACKLOG).

## Limits

- An opener that is a Firefox Sync copy cut to the millisecond, named only from outside the
  rows read, is not found.
- Two genuinely different Firefox visits of one kind at one millisecond and address, one of
  them exactly on the millisecond, are counted as one: both are kept, the later one hidden
  behind the other (about one in a thousand times for two such visits in one millisecond, and
  Firefox's Sync takes them for one too). Without the rule a reset Sync would count the
  device's whole history a second time. Two rows of one read are never matched to each other
  this way.
