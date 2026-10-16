# Semantic index control

`WORK-AI-INDEX-CONTROL` backend contract (2026-10-15). No schema changes.

The chosen index is the configured embedding provider ID and its default model.
`semantic_index` reads readiness from the ledger primary key, completion rows through
provider-ID prefixes, and active jobs through the existing state index. It never
counts archive visits or embedding rows during status polling.

The serving index is chosen when ready, otherwise the latest ready pair whose
provider remains configured, otherwise chosen as partial or empty. Worker context
resolves that pair with the provider's current address and secret and the pair's
model. History and MCP use that runtime; Ask gives it to both search tools and
run_code for the turn. Ranked search adds `previousModel` only after reading the semantic plane of a pair
that differs from chosen; blank queries, date sorting, empty indexes and drift fallbacks do not. The in-app sentinel uses the same static engine for query and
build, with the current vault's model paths. One process-wide strong `Arc` per canonical
model directory retains the engine between requests. Directory, verified-marker and model-file
size/mtime changes invalidate it; cache misses reverify hashes before loading. Query embedding
and drift validation share the selected engine. This retains model memory until replacement or
removal instead of rereading and converting the weights for every search.

Queue payload JSON gains an optional model snapshot. Existing columns keep their
meaning: stop 1 cancels, stop 2 pauses, and error_code distinguishes user, service
and queue pauses. Queue resume only touches NULL pause reasons. A stop wins over
a concurrent success or failure in the same Intelligence transaction. Readiness
checks the persisted stop flag in its final transaction too.

Legacy payloads without a recorded model cannot run, including queued cursor-zero jobs.
Startup cancels every active such job and keeps the index files. A new explicit incremental
request can fill the missing visits without binding old work to today's model.

User enqueue and automatic enqueue make their decision inside the owner write
transaction. User requests reuse active builds for the same pair and cancel other
writers, including clear jobs; automatic work requires a ready chosen ledger and no active same-pair job.
Explicit reset abandons old jobs and queues a clean rebuild. Running jobs keep
their lease until they acknowledge cancellation. Claims allow one index writer;
the worker also holds one mutex from claim through physical cleanup, so a new
claim never waits behind cleanup with an aging lease. Configuration is reloaded
inside that guard. Explicit run-now uses a nonblocking lock attempt and returns status
when another build owns it. Poisoning does not wedge this data-free execution guard.
Build status excludes clear-only jobs and prefers a running build over other matching work.

Only new durable cursor progress renews retry attempts. Typed embedding-service
errors survive anyhow contexts; exhausted retries pause with the complete error
chain. A drain remains alive while a queued retry waits for its available_at,
without depending on UI polling. It sleeps until the earliest due time, capped at 30 seconds,
and exits when paused or no queued index job remains. Local database, configuration and consistency
errors remain failures.

Job start retains chosen and serving. Once the job's success is settled, a complete
full-archive pass retires every other pair, including ledger rows and all vector
files. An incremental first fill is such a pass; the fullRebuild flag controls the
wipe, not whether a finished replacement may retire its predecessor. First fills and wiping builds write `building` (`clearing` for clear-only).
An update that does not wipe leaves an already `ready` ledger and its full-pass coverage
watermark and timestamps intact; interruption or working-set completion cannot demote that index. Only `building` or
`clearing` can become `incomplete`. Working-set updates also preserve the full-pass watermark and timestamps of a ready pair,
so unscanned visits or enrichment remain stale. A successful full-archive incremental pass
advances that coverage. Working-set fills of an unfinished index remain incomplete.
Cleanup failure is logged and appended to the job summary, preserving the chosen index and
build outcome. Start-of-job cleanup failure also allows the build to proceed; later jobs retry
remaining deletions. A changed selection during a build retains the current chosen and serving
pairs instead of deleting the fallback.

When the background drain finishes handling an index job (succeeded, failed, cancelled, paused
by the service, or waiting for a retry), the desktop shell emits `pathkeep://ai-index-job-ended`
with the job's id and state; the dev bridge forwards it. The app reads its snapshot again on it,
whichever screen is open, so History's semantic search and Settings learn about builds and
after-backup updates that end while they are not watching the queue.

Feature acceptance belongs to reviewer E2E. Rust regressions cover consent,
stop/failure races, legacy recovery, cursor durability and serving-model isolation.
