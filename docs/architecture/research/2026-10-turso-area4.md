# Turso area 4: intelligence, queues, agent storage and enrichment

This report records the work before the area-3 rebase and the earlier local disk limit.
See [the Modal integration report](2026-10-turso-area4-modal.md) for the resolved search
integration, passing gates and the subsequent scale measurements.

Native intelligence/runtime, AI queue/indexing, enrichment and caller ports are present.
Agent-store work is retained unchanged from the preceding implementation. Acceptance is
not complete: four tests reach excluded area-3 search code, and the encrypted scale rebuild
cannot complete within the requested 15 GB data budget with this materialization layout.
Both guarded benchmark attempts removed their data after stopping.

## Storage and decisions

Public operations receive `&Vault`; SQL helpers receive `&vault_store::Session`.
Intelligence has its own writer and reader pool, independent of Archive edits. Its
sixteen read views select durable generations; unfinished generations and cursors are
committed together, and obsolete rows are reclaimed in batches. Full replacements keep
published rows available. Ordinary appends extend facts/daily/navigation and replace
aggregates. Late arrivals, visibility regression and recipe changes require chronological
rebuilds. An unfinished build whose source changed is replaced rather than resumed against
an obsolete upper watermark.

Generation rows are durable projection state, not disposable staging. No TEMP tables or
VACUUM INTO are used. The small per-page refind accumulator and durable session-domain
counts avoid materializing the archive or replaying a whole long session after a crash.
The consolidated v1 schema changes directly; no new schema versions are added here.
Four redundant profile-prefixed fact/assignment indexes are removed. Navigation already
has generation/session and generation/trail access paths, and point reads use visit/generation
indexes. The trail-assignment index omits NULL trail IDs. Native catalog and render-plan tests
verify these access paths without duplicating an index entry for every visit.

Canonical enrichment is keyed by `(canonical_url, content_source)`. Nullable history/profile
IDs are provenance, not identity. A stored-result marker commits with the cache, then native
Archive text/outbox publication completes separately and can be replayed without refetching.
The fetch/extraction behavior is unchanged. Unit transport fixtures do not resolve DNS;
network socket tests remain explicitly ignored.

AI backfill traverses the entire captured visit-ID range and maps every visit to its page
vector. Working-set pages enumerate every eligible canonical URL, with scores used as queue
priorities. Metadata counters commit with replacements, cleanup and doctor repair. Queue
startup recovery preserves terminal work. Intelligence claims capture trigger watermarks,
and runtime invalidation revisions prevent an older completion from hiding a newer change.

## Failure modes

| Owner                           | Failure modes covered                                                                                                                                                                                             |
| ------------------------------- | ----------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Store Session/statement helpers | Borrowed values escape their owner; RETURNING is not drained; prepared reads bypass plan capture; Intelligence cannot see committed Archive facts; attached Archive writes escape its writer.                     |
| Build state/pipeline/rows       | Lost cursors; duplicated replay; partial generation publication; stale source/rule resume; archive writes blocked by a rebuild; first-domain classification changes at a batch boundary.                          |
| Daily/navigation/refind stages  | Split sessions or trails; duplicated members; inconsistent daily uniqueness/category selection; huge open-session replay; whole-archive refind buffering; stale counts after replay.                              |
| Intelligence reads              | Full canonical scans; timestamp/score ties lose continuation; a long session is excluded by a lookback; evidence samples hide visits; anniversaries count visits as sessions; cached metadata or end dates drift. |
| Runtime                         | Duplicate claims; failed decoding rolls back quarantine; a completion loses a newer trigger; a stale completion overwrites invalidation; reads perform recovery; latest-job caps hide history.                    |
| AI queue/indexing/read model    | In-flight work is stranded after owner death; completed work repeats; cursor skips unchanged rows; page limits become total limits; metadata totals drift on replacement, rollback or cleanup.                    |
| Working set/agent tools         | Signal pools drop pages; canonical variants split; oversized reports or notes lose bytes; UTF-8 is split; volatile timers invalidate every continuation.                                                          |
| Enrichment                      | Legacy candidates miss native pages; parked jobs never return; a page without visits is ignored; cache fan-out misses later variants; a crash refetches already stored content or duplicates publication.         |
| Callers                         | A request opens another owner, loads the wrong root, loses cursor fields, or uses stale consent/configuration.                                                                                                    |

## Request and response changes

Wire fields use camelCase; agent-tool arguments retain snake_case. Cursors are opaque and
are reused with the same filters/sort. A limit is a page or batch size, never total coverage.

| Surface                                                                          | Contract                                                                                                                                                                                                                                                                                                          |
| -------------------------------------------------------------------------------- | ----------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Top sites, refind pages, hub pages, frequent searches, top concepts              | Requests add `cursor`; responses become `{ items, nextCursor }`. Existing limit fields are page sizes.                                                                                                                                                                                                            |
| Stable sources, friction signals, reopened investigations, observed interactions | `ScopedPageRequest` adds optional `limit` and `cursor` to date/profile scope; result is `{ items, nextCursor }`, including inside worker section/overview envelopes. Sparse observed pages may be empty with a continuation.                                                                                      |
| Sessions, trails, query families                                                 | Offset `page` is replaced by `cursor`; results replace `page` with `nextCursor`, retain totals and `pageSize`, and report the applied size. Sessions use exact interval overlap.                                                                                                                                  |
| Recent search queries                                                            | `page` is replaced by `cursor`; result `{ rows, total, nextCursor, pageSize }`. Every sort has stable continuation; family sorting retains only a page in Rust.                                                                                                                                                   |
| Path flows                                                                       | Request adds `cursor`; response/overview section becomes `{ items, nextCursor }`. All-time reads use published aggregates; date scopes stream navigation windows.                                                                                                                                                 |
| Domain detail                                                                    | Request adds `pageSize`, `pagesCursor`, `searchesCursor`; `topPages` and `landingSearches` become pages. `pageCount` and `landingSearchCount` remain exact for the whole scope.                                                                                                                                   |
| Query-family detail                                                              | Adds `cursor`, `pageSize`; `relatedTrails` becomes a page.                                                                                                                                                                                                                                                        |
| Search effectiveness                                                             | Request adds `topicsCursor`, `topicsLimit`; `hardestTopics` and `topResolvingSources` become pages. Continue resolving sources through the stable-sources endpoint.                                                                                                                                               |
| Refind detail                                                                    | Adds `visitsCursor`, `trailsCursor`, `pageSize`; `relatedTrails` becomes a page; explanation adds `profileId` and `visitIdsNextCursor`. `recentDays` covers every matching day.                                                                                                                                   |
| Refind explanation                                                               | Request adds `profileId`, `cursor`, `pageSize`; visit IDs are a complete cursor traversal, not the saved sample.                                                                                                                                                                                                  |
| Entity explanation                                                               | Request adds `cursor`, `pageSize`; response adds `participatingVisitIdsNextCursor`. Session, trail, family, habit, compare and reopened/reference evidence all page completely; factors remain whole-entity values.                                                                                               |
| Session detail                                                                   | `{ request: { sessionId, visitsCursor?, trailsCursor?, pageSize? } }`; retains visit/trail arrays and adds `visitsNextCursor`, `trailsNextCursor`.                                                                                                                                                                |
| Trail detail                                                                     | `{ request: { trailId, cursor?, pageSize? } }`; retains member array and adds `membersNextCursor`.                                                                                                                                                                                                                |
| Anniversary entries                                                              | Adds `topDomainsNextCursor`; continue through top-sites with the entry's date and the same profile.                                                                                                                                                                                                               |
| AI/runtime recent jobs                                                           | AI/index/runtime status snapshots add `recentJobsNextCursor`. New `list_ai_jobs` and `list_intelligence_jobs` take `{ cursor?, limit? }` and return `{ items, nextCursor }`. AI cursors are integer IDs; intelligence cursors are `{ updatedAt, id }`. Owner IPC/CLI names are `ai-jobs` and `intelligence-jobs`. |
| Enrichment request/payload                                                       | `historyId` and `profileId` are nullable/optional; native canonical page identity is authoritative.                                                                                                                                                                                                               |
| Rebuild and AI indexing                                                          | Existing `limit` is a batch size; rebuild progress includes `overview` while preparing cached views.                                                                                                                                                                                                              |
| `intelligence_report`                                                            | Row cursor is forwarded; `session_detail` also accepts `trails_cursor` independently of its visit `cursor`. `output_cursor` pages oversized JSON as `{ jsonFragment, reportRevision, nextOutputCursor, totalBytes }`. Changed content returns `restartRequired`.                                                  |
| `list_annotations`                                                               | Full notes replace 120-byte previews; `output_cursor` continues oversized text with the same fragment envelope.                                                                                                                                                                                                   |
| Public intelligence snapshot                                                     | Adds `topDomainsNextCursor`.                                                                                                                                                                                                                                                                                      |
| Embed cards                                                                      | Request adds `cursor`; result becomes `{ items, nextCursor }`.                                                                                                                                                                                                                                                    |
| Widget and local-host cards                                                      | Keep their arrays, adding `highlightsNextCursor` and `embedCardsNextCursor` respectively.                                                                                                                                                                                                                         |

Agent conversation/message contracts are retained: conversation recency/ID cursors,
message sequence cursors, and complete atomic transcript save/load. Full transcript save
must not be given an incomplete message page.

## Coverage caps

STATUS names a 13-limit audit without enumerating it. The owned-module scan found:

| Previous limit                                                    | Outcome                                                                                                                                                   |
| ----------------------------------------------------------------- | --------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Agent conversations 200; transcript pages                         | Existing cursor implementation retained.                                                                                                                  |
| Working set 50,000; each signal 12,500                            | Removed; canonical-page traversal covers all eligible pages.                                                                                              |
| Index `limit` and working-set candidate pool                      | Batch size only; a 50,021-page build in each storage mode completes every vector/map/metadata row.                                                        |
| AI recent jobs and runtime latest 12                              | Explicit job-list continuation.                                                                                                                           |
| Rebuild newest-visit tail                                         | Batch size only; all captured facts are visited.                                                                                                          |
| Top sites/refind/hubs/stable sources/friction/reopened/path flows | Cursor pages, including score/time ties.                                                                                                                  |
| Frequent searches 100; concepts                                   | Page limits with continuation.                                                                                                                            |
| Session one-day lookback                                          | Removed; arbitrary-duration overlap is exact.                                                                                                             |
| Session/trail/family/recent-query offsets and unbounded details   | List cursors and detail navigation-ordinal pages.                                                                                                         |
| Domain pages 10, landing queries 8                                | Independent cursor pages with whole-scope totals.                                                                                                         |
| Query-family/refind related trails 8                              | Complete related-trail traversal.                                                                                                                         |
| Refind evidence sample 50                                         | Persisted sample stays bounded; public evidence traverses canonical facts.                                                                                |
| Other explanation ID vectors                                      | Session/trail/family/habit/compare/reopened IDs page completely; path-flow explanations seek their latest stored occurrence with a bounded domain window. |
| Anniversary top domains 3                                         | Continuation to the complete day/domain list.                                                                                                             |
| Widget highlights 4 / selected embed cards                        | Selected cards have complete continuation.                                                                                                                |
| Agent intelligence report 8 KB; annotation previews 120 bytes     | Complete output fragments and full notes.                                                                                                                 |
| Search, regex, hybrid/vector candidate pools and code-mode search | Area 3 owns these. Calls retain its entry point; no search implementation is changed.                                                                     |

Curated embed cards choose one representative per category. The selected card list now pages
with a cursor; widget highlights and local-host bundles expose its continuation. This is a
curated summary, not an enumeration of every underlying entity.

## Validation

| Gate                                                                     | Result                                                                                                                                                                                             |
| ------------------------------------------------------------------------ | -------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `cargo fmt --all --check`                                                | Pass.                                                                                                                                                                                              |
| `cargo clippy --workspace --all-targets --all-features -- -D warnings`   | Pass, including the latest native worker fixtures.                                                                                                                                                 |
| `cargo test -p vault-core -p vault-worker -p vault-store --no-fail-fast` | Fail only in core/worker search integrations listed below. Core: 1,817 pass, 1 fail, 36 ignored. Worker: 144 pass, 3 fail. All non-ignored integration, Store and documentation-test targets pass. |
| `bun run test:desktop-bridge:rust`                                       | Pass: 21 tests.                                                                                                                                                                                    |
| `cargo build --workspace`                                                | Pass.                                                                                                                                                                                              |

Native acceptance results include:

- Ten rebuild/runtime tests: many batches equal one pass; a 10,000-visit append equals a
  full rebuild; actual process exit resumes committed work; interactive star commits before
  completion; stale-source retargeting; long-session resume; trigger/invalidation races;
  malformed-claim quarantine; long-session overlap; production render plans.
- Sixteen list/detail pagination tests, plus complete entity-evidence paging with unchanged
  whole-entity factors, oversized agent reports, and independent session-trail agent cursors.
- A 50,021-page AI backfill in plaintext and encrypted modes covers every vector, map and
  metadata row. The working-set suite also reaches all 50,021 pages past both former caps.
- AI queue and agent journal process-death recovery; agent conversation/message traversal;
  native enrichment candidates, visitless pages, awaiting-archive reoffering, cached crash
  handoff and later canonical variants. Relevant scenarios run both plaintext and with a key.
- Worker coverage-only code compiles through a targeted `cargo rustc --cfg coverage`
  metadata check. Provider/network behavior was not executed by that check.
- Export heap assertions run in an isolated child, preventing concurrent Vault initialization
  from contaminating the process-wide allocator. Unchanged thresholds pass at 10,000 and
  100,000 visits, with about 0.9 MiB above baseline in both cases.

The remaining failing tests are:

- `ai::tests::native::indexing::static_tier_index_writes_real_vectors_and_semantic_search_finds_the_seeded_page`
- `tests::ai_worker_helpers_cover_preview_secret_and_lexical_search_flows`
- `tests::mcp_per_call_gate_refuses_search_and_status_when_mcp_consent_is_revoked_mid_session`
- `tests::mcp_surface_respects_native_visibility_and_locked_app_sessions`

Each fails at `applying migration 1: table runs already exists`: the search entry point
opens the native archive through the legacy SQLite initializer. The final worker-only rerun,
using native imports/visibility/audit fixtures, confirms 144 passes and exactly the same three failures.
`ai/search.rs` and search candidate implementations are explicitly excluded from area 4.
No test is ignored and no production compatibility copy is added to hide this boundary.
The remaining `open_intelligence_connection` and `ensure_ai_schema` helpers serve only
excluded search/maintenance consumers; their deletion depends on those ports.

A minimal local reproduction is the named core static-index/search test, run offline with
`PATHKEEP_TEST_BLOCK_NETWORK=1` and the supplied shared target directory. Index construction
succeeds before the search call reaches the legacy initializer.

## Scale measurements

One retained encrypted archive has 14,400,000 visits, 2,880,000 exact pages, 20,000 domains,
and one profile. Visits are ten seconds apart, forming one continuous session; this fixture
has no search URLs/trails. These distribution limits must be considered when interpreting
rebuild measurements. Cold means a fresh process/Store, not a flushed operating-system cache.

| Measurement                          | Final guarded attempt                                                                                                |
| ------------------------------------ | -------------------------------------------------------------------------------------------------------------------- |
| Full rebuild                         | Incomplete: stopped in refind generation after scanning 5,586,944 refind-input visits. Elapsed at stop: 1,877.458 s. |
| Observed peak RSS before stop        | 158,466,048 bytes (151.125 MiB); not a completed-rebuild peak.                                                       |
| Incremental after 10,000 visits      | Not measured: no complete baseline within the data budget.                                                           |
| Overview cold / warm                 | Not measured: no complete baseline.                                                                                  |
| Domain cold / warm                   | Not measured: no complete baseline.                                                                                  |
| Sessions cold / warm                 | Not measured: no complete baseline.                                                                                  |
| Interactive star wait during rebuild | 0.328 ms; committed while fact derivation continued.                                                                 |
| Load at rebuild start                | 6.58 / 5.99 / 5.58 (`uptime`).                                                                                       |
| Data at guard                        | 14,865,960,966 bytes; stopped with 128 MiB reserved headroom below 15 GB.                                            |
| Cleanup                              | Entire fixture removed; absence verified.                                                                            |

Fact derivation completed at 1,345.693 s, daily rollups at 1,491.707 s and session construction
at 1,662.135 s. The remaining refind/reference materialization did not fit before the guard.
This is a failed scale acceptance, not a full-rebuild timing. Measuring the incremental or
render paths against that incomplete baseline would not satisfy the requested measurement.

The preceding guarded attempt stopped earlier, at 8,908,800 session-stage visits:
14,866,006,022 bytes, 1,674.062 s, 161,153,024-byte peak RSS and 1.870 ms interactive-write wait,
with starting load 5.12 / 4.91 / 4.63. Removing redundant indexes and excluding NULL trail IDs
let the final attempt finish sessions and enter refind generation, but did not make the
complete projection fit. Both attempts stayed below 15 GB and deleted their fixtures.

Hardware `sysctl`/process inspection was denied by the sandbox, so these results do not
claim a verified 4-core/8-GB host. Load includes simultaneous test/build work. Earlier
interrupted diagnostics are not full-rebuild timings; their local SQL root causes and
minimal reproductions are in the [query-plan report](2026-10-turso-intelligence-query-plans.md).

## Not completed

The full encrypted 14.4M rebuild measurement, its 10,000-visit incremental measurement,
and complete-baseline cold/warm overview/domain/session measurements are unavailable
because the materialized data reaches the 15 GB guard first. The ignored harness is present,
but its full scale acceptance does not pass. The overall test gate remains red at the four
excluded search integrations. Removing the legacy projection opener/schema helper requires
those excluded consumers to migrate. Frontend DTO consumers, maintenance and general app
wiring were intentionally not changed.

## Agent store review

The original `agent_store.rs` combines database opening/migrations, transcripts,
agent journals and 1,200 lines of behavioural tests. The port separates transcript
reads/writes, journal operations, shared types and tests. Store owns the schema and
connections. Public operations receive `&Vault`; SQL helpers receive `&Session`.

Failure modes to preserve: transcript replacement must be atomic, existing message
and conversation timestamps must survive saves, deletion must cascade through the
journal, replayed tool results must not duplicate, citations and usage must resolve
to the latest answering run, and lazy first-turn persistence must create a parent
without overwriting a saved conversation. Pagination must not lose timestamp ties,
messages beyond a page, or conversations beyond the old 200-row ceiling.

Existing Rust tests cover these behaviours. Desktop worker tests cover all five
CRUD entry points and the existing App Lock boundary. Frontend E2E is outside this
area; no frontend files or E2E tests are changed.

## Retained agent test inventory

All 26 prior transcript/journal behaviour tests were ported to `Vault::open_temp()`.
Two names now describe the native contract: `open_agent_connection_bootstraps_database_file`
is `vault_open_installs_the_agent_schema`; `list_cap_holds_at_two_hundred_even_for_oversized_limits`
is `bounded_conversation_pages_keep_every_row_reachable`, which follows the cursor to the
remaining records. The old agent-v2 assertion inside the legacy archive-upgrade test was
removed with the SQLite agent migration runner; native v1 installation is covered by the
ported open test and Store schema tests. No transcript or journal behaviour test was deleted.

Ported tests:

- `run_trace_round_trips_header_steps_and_citations_in_order`
- `duplicate_tool_result_step_is_idempotent_on_call_id`
- `finalize_failed_run_records_error_and_load_missing_is_none`
- `begin_agent_run_rejects_blank_id_and_reopen_resets_accounting`
- `record_agent_citations_replaces_prior_set`
- `deleting_conversation_cascades_run_trace`
- `begin_agent_run_self_heals_missing_conversation_parent`
- `save_conversation_overwrites_a_begin_agent_run_stub`
- `begin_agent_run_does_not_clobber_an_existing_saved_conversation`
- `agent_run_status_tags_round_trip`
- `load_conversation_reconstructs_citations_and_usage_for_the_answering_turn`
- `load_conversation_without_a_run_leaves_turns_trace_free`
- `load_conversation_uses_the_latest_run_per_message_and_zero_tokens_yield_no_usage`
- `vault_open_installs_the_agent_schema`
- `save_then_load_roundtrips_messages_in_order`
- `explicit_title_overrides_auto_title_and_save_replaces_transcript`
- `list_orders_newest_first_and_clamps_limit`
- `load_unknown_conversation_returns_none`
- `delete_removes_conversation_and_cascades_messages`
- `rename_updates_title_and_rejects_blank`
- `resave_with_more_messages_never_truncates_history`
- `resave_preserves_per_message_created_at_and_stamps_only_new_messages`
- `resave_holds_conversation_created_at_and_advances_updated_at_in_db`
- `bounded_conversation_pages_keep_every_row_reachable`
- `save_requires_non_empty_id`
- `auto_title_falls_back_and_truncates`

Added acceptance tests, each in plaintext and with a key:

- `chats_and_messages_reach_every_row_after_reopen`: 426 conversations, including
  425 with tied recency, and 2,501 messages; complete traversal after reopening.
- `a_failed_replacement_preserves_the_saved_transcript`: a duplicate message ID
  aborts replacement without losing either summary or messages.
- `committed_agent_journal_survives_process_death`: kill a child after acknowledged
  commits, reopen, verify unfinished and finished runs, token counts and citations,
  and replay an already committed tool result without duplication.
- `journal_crash_child` is an ignored subprocess fixture, invoked by its parent test.

## Remaining-module review

The intelligence read modules compose SQL helpers; rebuild orchestration previously mixed
per-open schema installation, SQLite transactions, fallback builders and tests. The native
producer separates profile checkpoints, bounded row access, daily/navigation stages and
per-page aggregates. Reference builders remain test-only. Existing tests use
real Vault files; a small `TestDatabase` fixture helper removes repeated owner closures while
business SQL helpers continue to use `Session`.

AI tests combine provider-independent helpers, index/queue/status contracts, and search-owned
candidate/retrieval tests. The native index tests must retain their storage assertions and
provider seams; excluded search implementations are not rewritten in this area. Agent tools
separate search dispatch, code execution, deterministic reports and stars/annotations. Deterministic reports and annotations gain complete output continuation.

The retired `vault-core/examples/intelligence-benchmark.rs` driver mixed SQLite corpus copying,
legacy stage mutation, and measurements in one 1,042-line file. Its full, append, crash/recovery,
and visibility coverage now belongs to the native Vault acceptance suites; scale measurements
belong to `vault-worker/tests/archive_scale_bench/turso_intelligence.rs`. That owner uses one
encrypted corpus and removes it after measurement. The old SQLite driver is removed.

The final module split keeps production owners below the repository's giant-file threshold:
normalization/cache, fetch/handoff, local-host rendering, user-record tools, and their tests.
Fetch tests retain separate routing, storage/replay and ignored socket suites; native AI tests
separate metadata, backfill and resume contracts. These moves preserve the existing assertions.
Frontend E2E remains outside this area's allowed paths.

## Test inventory and changed files

Existing behavior tests in intelligence, site dictionary, runtime, AI queue/indexing,
agent tools, enrichment and the worker facades execute native fixture SQL through Vault.
Pure classifier/aggregate/scoring tests remain independent references. Search-owned tests
retain their original entry point. Agent-store test/code changes below belong to the
preceding work and were retained during this continuation.

The old benchmark's `skip_baseline_rebuild_requires_existing_intelligence_state` test
is retained in the native scale harness: a child read/incremental measurement requires a
published native baseline. The driver itself no longer copies SQLite databases.

The following names changed with their required behavior; the old cap assertions are
replaced by complete traversal assertions, not discarded:

| Previous test                                                                  | Native test                                                                   |
| ------------------------------------------------------------------------------ | ----------------------------------------------------------------------------- |
| `a_legacy_derived_only_repair_still_has_a_native_audit_run`                    | `a_native_derived_only_repair_keeps_its_audit_run_and_metadata_totals`        |
| `build_index_honors_limit_at_a_chunk_boundary`                                 | `build_index_uses_limit_as_page_size_and_reaches_every_candidate`             |
| `chrome_now_ms_is_in_chrome_epoch_frame`                                       | `candidate_clock_is_in_unix_epoch_frame`                                      |
| `daily_rollup_stage_falls_back_when_delta_rows_do_not_match_watermark`         | `daily_rollup_resumes_an_interrupted_fact_batch_without_missing_delta_rows`   |
| `enqueue_working_set_skips_extractor_disabled_and_visitless_candidates`        | `enqueue_working_set_skips_extractor_disabled_and_unarchived_candidates`      |
| `ensure_ai_schema_adds_tables`                                                 | `native_ai_schema_has_empty_metadata_totals`                                  |
| `ensure_ai_schema_creates_metadata_only_embedding_rows`                        | `native_ai_schema_keeps_vectors_out_of_metadata_rows`                         |
| `intelligence_report_truncates_over_budget_json_on_a_char_boundary`            | `intelligence_report_pages_over_budget_json_without_losing_unicode`           |
| `list_annotations_truncates_long_cjk_note_on_a_char_boundary`                  | `list_annotations_pages_every_byte_of_a_long_cjk_note`                        |
| `list_cap_holds_at_two_hundred_even_for_oversized_limits`                      | `bounded_conversation_pages_keep_every_row_reachable`                         |
| `load_visible_visits_hydrates_search_terms_within_the_limit_window`            | `source_pages_hydrate_search_terms`                                           |
| `load_visible_visits_limit_above_row_count_returns_all_rows`                   | `oversized_source_page_returns_all_rows`                                      |
| `load_visible_visits_limit_applies_to_unscoped_reads_across_profiles`          | `source_pages_cover_every_profile`                                            |
| `load_visible_visits_limit_keeps_the_newest_tail_in_ascending_order`           | `source_page_size_keeps_all_profile_visits_reachable`                         |
| `load_visible_visits_limit_zero_keeps_a_single_newest_visit`                   | `zero_rebuild_page_size_still_covers_all_visits`                              |
| `mcp_surface_respects_legacy_visibility_and_locked_app_sessions`               | `mcp_surface_respects_native_visibility_and_locked_app_sessions`              |
| `open_agent_connection_bootstraps_database_file`                               | `vault_open_installs_the_agent_schema`                                        |
| `per_signal_cap_is_a_strict_fraction_of_max_so_no_signal_dominates`            | `every_page_is_reachable_past_both_old_pool_limits`                           |
| `scoped_debug_rebuild_uses_legacy_fallback_and_progress_callbacks`             | `scoped_rebuild_uses_batches_and_progress_callbacks`                          |
| `structural_stage_updates_tail_assignments_incrementally`                      | `late_visits_rebuild_tail_assignments_without_duplicates`                     |
| `structural_stream_keeps_assignments_stable_across_batch_boundaries`           | `late_visit_chains_keep_assignments_stable_across_batch_boundaries`           |
| `structural_stream_reclassifies_incremental_search_event_after_landing_domain` | `late_search_events_are_reclassified_after_their_landing_domain`              |
| `truncate_str_on_char_boundary_never_splits_a_multibyte_char`                  | `output_pages_keep_multibyte_characters_and_reject_partial_character_cursors` |

Deleted SQLite-engine tests (native schema catalog, transaction and restart tests replace their engine coverage):

- `agent_schema_migrations_records_versioned_migrations`
- `clear_is_noop_when_table_absent`
- `core_claim_returns_none_when_the_claimed_row_disappears_before_payload_read`
- `ensure_core_intelligence_schema_migrates_legacy_search_query_kind`
- `ensure_core_intelligence_schema_records_versioned_migrations`
- `forward_migration_from_v1_only_applies_v2_atomically`
- `intelligence_v8_adds_w_enrich_columns_to_a_legacy_enrichment_table`
- `intelligence_v8_column_add_propagates_a_genuine_alter_error`
- `legacy_repairs_tolerate_missing_optional_intelligence_tables`
- `migration_apply_and_ledger_insert_are_one_transaction`
- `migration_checksum_is_derived_from_sql_and_changes_when_sql_changes`
- `migration_guard_rejects_recorded_checksum_drift`
- `migration_v2_creates_run_trace_tables_and_indices`
- `runtime_schema_propagates_alter_failures_other_than_a_duplicate_column`
- `schema_count_helpers_tolerate_missing_tables`
- `search_times_are_backfilled_for_databases_from_before_migration_9`
- `stale_ai_embedding_helpers_treat_missing_optional_table_as_clean`

The legacy `upgrade_v020_plaintext_archive_migrates_cleanly` test remains; only its
retired intelligence-v6 and agent-v2 forward-migration assertions were removed.

New native acceptance/regression tests, excluding the renamed tests above:

- `a_changed_archive_retargets_an_interrupted_generation` — [intelligence_turso.rs](../../../src-tauri/crates/vault-core/tests/intelligence_turso.rs)
- `a_failed_replacement_preserves_the_saved_transcript` — [agent_store_paging.rs](../../../src-tauri/crates/vault-core/tests/agent_store_paging.rs)
- `a_long_session_resumes_from_durable_domain_counts` — [intelligence_turso.rs](../../../src-tauri/crates/vault-core/tests/intelligence_turso.rs)
- `a_page_without_visits_reoffers_a_parked_candidate` — [enrichment_turso.rs](../../../src-tauri/crates/vault-core/tests/enrichment_turso.rs)
- `a_rebuild_does_not_overwrite_a_newer_invalidation` — [intelligence_turso.rs](../../../src-tauri/crates/vault-core/tests/intelligence_turso.rs)
- `agent_session_detail_forwards_the_independent_trail_cursor` — [intelligence_agent_pages.rs](../../../src-tauri/crates/vault-core/tests/intelligence_agent_pages.rs)
- `an_interrupted_build_resumes_and_an_interactive_star_commits_before_completion` — [intelligence_turso.rs](../../../src-tauri/crates/vault-core/tests/intelligence_turso.rs)
- `anniversary_domains_continue_and_deep_sessions_are_counted_once` — [intelligence_paging.rs](../../../src-tauri/crates/vault-core/tests/intelligence_paging.rs)
- `backfill_reaches_every_distinct_page_beyond_fifty_thousand` — [coverage.rs](../../../src-tauri/crates/vault-core/src/ai/tests_native/coverage.rs)
- `bounded_path_flow_tail_matches_all_reference_windows` — [intelligence_structural_aggregates.rs](../../../src-tauri/crates/vault-core/src/intelligence/intelligence_structural_aggregates.rs)
- `candidate_page_does_not_group_the_entire_archive_for_each_scalar_signal` — [working_set_turso.rs](../../../src-tauri/crates/vault-core/tests/working_set_turso.rs)
- `captures_direct_and_prepared_reads_only_when_enabled` — [query_plans.rs](../../../src-tauri/crates/vault-store/tests/query_plans.rs)
- `chats_and_messages_reach_every_row_after_reopen` — [agent_store_paging.rs](../../../src-tauri/crates/vault-core/tests/agent_store_paging.rs)
- `committed_agent_journal_survives_process_death` — [agent_store_paging.rs](../../../src-tauri/crates/vault-core/tests/agent_store_paging.rs)
- `curated_cards_and_widgets_expose_every_selected_card` — [intelligence_paging.rs](../../../src-tauri/crates/vault-core/tests/intelligence_paging.rs)
- `derived_batch_looks_up_only_requested_visit_ids` — [intelligence_generation_plans.rs](../../../src-tauri/crates/vault-core/tests/intelligence_generation_plans.rs)
- `domain_details_page_through_all_pages_and_landing_queries` — [intelligence_paging.rs](../../../src-tauri/crates/vault-core/tests/intelligence_paging.rs)
- `encrypted_intelligence_at_scale` — [turso_intelligence.rs](../../../src-tauri/crates/vault-worker/tests/archive_scale_bench/turso_intelligence.rs)
- `entity_evidence_pages_preserve_every_visit_and_whole_entity_factors` — [intelligence_explanation_pages.rs](../../../src-tauri/crates/vault-core/tests/intelligence_explanation_pages.rs)
- `every_unicode_byte_is_reachable_and_changed_reports_request_a_restart` — [report_pages.rs](../../../src-tauri/crates/vault-core/src/ai/report_pages.rs)
- `frequent_queries_and_concepts_continue_beyond_their_former_limits` — [intelligence_paging.rs](../../../src-tauri/crates/vault-core/tests/intelligence_paging.rs)
- `generation_views_seek_assignment_keys` — [intelligence_generation_plans.rs](../../../src-tauri/crates/vault-core/tests/intelligence_generation_plans.rs)
- `hub_pages_continue_beyond_the_former_ten_page_limit` — [intelligence_paging.rs](../../../src-tauri/crates/vault-core/tests/intelligence_paging.rs)
- `indexed_family_candidates_preserve_the_reference_first_match` — [intelligence_structural_build.rs](../../../src-tauri/crates/vault-core/src/intelligence/intelligence_structural_build.rs)
- `intelligence_reads_committed_archive_facts_in_both_key_modes` — [archive_attachment.rs](../../../src-tauri/crates/vault-store/tests/archive_attachment.rs)
- `journal_crash_child` — [agent_store_paging.rs](../../../src-tauri/crates/vault-core/tests/agent_store_paging.rs)
- `malformed_job_claim_commits_its_quarantine` — [intelligence_turso.rs](../../../src-tauri/crates/vault-core/tests/intelligence_turso.rs)
- `many_batches_equal_one_pass_and_incremental_equals_full` — [intelligence_turso.rs](../../../src-tauri/crates/vault-core/tests/intelligence_turso.rs)
- `materialized_all_time_flows_page_past_twenty_results` — [intelligence_paging.rs](../../../src-tauri/crates/vault-core/tests/intelligence_paging.rs)
- `native_embedding_totals_follow_replacement_and_rollback` — [store_rows.rs](../../../src-tauri/crates/vault-core/src/ai/indexing/store_rows.rs)
- `native_pages_offer_all_candidates_and_reoffer_a_prearchive_job` — [enrichment_turso.rs](../../../src-tauri/crates/vault-core/tests/enrichment_turso.rs)
- `oversized_native_reports_page_through_every_domain` — [intelligence_agent_pages.rs](../../../src-tauri/crates/vault-core/tests/intelligence_agent_pages.rs)
- `owner_queue_pages_use_the_supplied_vault_without_a_password` — [tests.rs](../../../src-tauri/crates/vault-worker/src/owner_requests/tests.rs)
- `prepared_statements_stream_and_rebind_in_both_key_modes` — [statements.rs](../../../src-tauri/crates/vault-store/tests/statements.rs)
- `process_exit_mid_rebuild_resumes_without_duplicate_facts` — [intelligence_turso.rs](../../../src-tauri/crates/vault-core/tests/intelligence_turso.rs)
- `process_exit_requeues_inflight_claim_and_preserves_completed_job` — [ai_queue_turso.rs](../../../src-tauri/crates/vault-core/tests/ai_queue_turso.rs)
- `query_family_details_page_through_every_related_trail` — [intelligence_paging.rs](../../../src-tauri/crates/vault-core/tests/intelligence_paging.rs)
- `queue_history_dispatch_preserves_every_cursor_page` — [tests.rs](../../../src-tauri/src/dev_ipc_bridge/dispatch/tests.rs)
- `queue_pages_reach_every_job_and_a_cancel_cannot_become_success` — [ai_queue_turso.rs](../../../src-tauri/crates/vault-core/tests/ai_queue_turso.rs)
- `recent_queries_have_complete_cursors_for_every_sort` — [intelligence_paging.rs](../../../src-tauri/crates/vault-core/tests/intelligence_paging.rs)
- `refind_detail_reaches_all_visits_days_and_related_trails` — [intelligence_paging.rs](../../../src-tauri/crates/vault-core/tests/intelligence_paging.rs)
- `refind_pages_reach_tied_results_and_choose_the_best_profile` — [intelligence_paging.rs](../../../src-tauri/crates/vault-core/tests/intelligence_paging.rs)
- `render_reads_use_indexed_archive_and_membership_access` — [render_plans.rs](../../../src-tauri/crates/vault-core/tests/intelligence_turso/render_plans.rs)
- `reopening_recovers_only_abandoned_claims_and_keeps_finished_work` — [ai_queue_turso.rs](../../../src-tauri/crates/vault-core/tests/ai_queue_turso.rs)
- `search_effectiveness_pages_through_every_hard_topic` — [intelligence_paging.rs](../../../src-tauri/crates/vault-core/tests/intelligence_paging.rs)
- `session_and_trail_details_page_in_navigation_order` — [intelligence_paging.rs](../../../src-tauri/crates/vault-core/tests/intelligence_paging.rs)
- `sessions_that_started_more_than_a_day_earlier_still_overlap_the_requested_day` — [intelligence_turso.rs](../../../src-tauri/crates/vault-core/tests/intelligence_turso.rs)
- `sessions_trails_and_families_continue_across_timestamp_ties` — [intelligence_paging.rs](../../../src-tauri/crates/vault-core/tests/intelligence_paging.rs)
- `sparse_observed_evidence_has_continuation_even_when_a_page_is_empty` — [intelligence_paging.rs](../../../src-tauri/crates/vault-core/tests/intelligence_paging.rs)
- `stored_content_survives_a_crash_and_serves_new_canonical_variants_without_fetching` — [storage.rs](../../../src-tauri/crates/vault-core/src/enrichment/content_fetch/tests/storage.rs)
- `summary_lists_keep_every_source_friction_signal_and_reopened_investigation` — [intelligence_paging.rs](../../../src-tauri/crates/vault-core/tests/intelligence_paging.rs)
- `top_sites_page_through_every_domain_in_each_sort_order` — [intelligence_paging.rs](../../../src-tauri/crates/vault-core/tests/intelligence_paging.rs)
- `view_timers_do_not_change_paged_content_but_runtime_state_does` — [report_pages.rs](../../../src-tauri/crates/vault-core/src/ai/report_pages.rs)
- `work_requested_during_a_claim_is_not_lost_at_completion` — [intelligence_turso.rs](../../../src-tauri/crates/vault-core/tests/intelligence_turso.rs)

The network fixtures below remain in the repository with explicit ignore reasons. They
were not executed. The three pure OG-image scheme/SERP/synthesis early-return tests run
in isolated children without reaching DNS or HTTP. No network test was replaced with a
fabricated successful transport.

- `shared_fetcher_returns_html_body_for_a_readable_article`
- `shared_fetcher_reports_rate_limited_on_429`
- `shared_fetcher_reports_unsupported_content_for_non_html`
- `shared_fetcher_reports_fetch_error_on_4xx_5xx`
- `shared_fetcher_reports_decode_error_when_html_body_exceeds_cap`
- `shared_fetcher_returns_github_repo_json`
- `shared_fetcher_treats_github_403_as_rate_limited`
- `shared_fetcher_caps_json_body`
- `shared_fetcher_follows_a_redirect_chain_to_the_final_html`
- `shared_fetcher_reports_fetch_error_on_unreachable_host`
- `shared_fetcher_reports_fetch_error_on_json_5xx`
- `read_capped_drains_under_cap_and_rejects_over_cap`
- `og_images_fetch.rs::og_image_host_guard_blocks_private_image_target`
- `og_images_fetch.rs::fetch_returns_ok_with_bytes_when_page_and_image_resolve`
- `og_images_fetch.rs::fetch_returns_missing_when_page_has_no_og_image`
- `og_images_fetch.rs::fetch_returns_http_error_on_404_page`
- `og_images_fetch.rs::fetch_returns_parse_error_on_non_html_content_type`
- `og_images_fetch.rs::fetch_rejects_unsupported_image_mime`
- `og_images_fetch.rs::fetch_returns_http_error_when_image_body_stream_aborts_mid_chunk`
- `og_images_fetch.rs::fetch_rejects_oversize_image_via_content_length_header`
- `og_images_fetch.rs::fetch_returns_parse_error_when_html_body_exceeds_cap`
- `og_images_fetch.rs::fetch_returns_http_error_when_og_image_url_is_unreachable`
- `og_images_fetch.rs::fetch_returns_http_error_when_image_endpoint_returns_404`
- `og_images_fetch.rs::fetch_recovers_from_non_utf8_html_via_lossy_decode`
- `og_images_fetch.rs::bilibili_api_path_enters_finish_image_fetch_via_mockito`
- `og_images_fetch.rs::guarded_client_stops_at_a_blocked_redirect_hop`
- `og_images_fetch.rs::guarded_client_errors_after_too_many_redirects`
- `og_images_synth.rs::resolve_image_url_via_api_success_path_returns_pic_url`
- `og_images_synth.rs::resolve_image_url_via_api_returns_none_on_http_error`
- `og_images_synth.rs::resolve_image_url_via_api_returns_none_when_body_exceeds_cap`
- `og_images_synth.rs::resolve_image_url_via_api_with_av_id_uses_aid_query_param`
- `og_images_synth.rs::resolve_image_url_via_api_strips_trailing_slash_from_base`
- `net_guard::url_target_is_blocked_covers_scheme_literal_and_resolution_paths`

Complete working-tree file inventory (relative to the branch HEAD; all changes uncommitted):

- [docs/architecture/data-model.md](../../../docs/architecture/data-model.md)
- [docs/architecture/research/2026-10-turso-area4.md](../../../docs/architecture/research/2026-10-turso-area4.md)
- [docs/architecture/research/2026-10-turso-intelligence-query-plans.md](../../../docs/architecture/research/2026-10-turso-intelligence-query-plans.md)
- [docs/architecture/store-schema.md](../../../docs/architecture/store-schema.md)
- [docs/features/core-intelligence-ultimate-design.md](../../../docs/features/core-intelligence-ultimate-design.md)
- [docs/features/deterministic-intelligence.md](../../../docs/features/deterministic-intelligence.md)
- [docs/features/intelligence.md](../../../docs/features/intelligence.md)
- [docs/plan/CHANGELOG.md](../../../docs/plan/CHANGELOG.md)
- [docs/plan/program/repo-baseline.md](../../../docs/plan/program/repo-baseline.md)
- [src-tauri/crates/vault-core/Cargo.toml](../../../src-tauri/crates/vault-core/Cargo.toml)
- `src-tauri/crates/vault-core/examples/intelligence-benchmark.rs` — removed.
- [src-tauri/crates/vault-core/src/agent_store.rs](../../../src-tauri/crates/vault-core/src/agent_store.rs)
- [src-tauri/crates/vault-core/src/agent_store/journal.rs](../../../src-tauri/crates/vault-core/src/agent_store/journal.rs)
- [src-tauri/crates/vault-core/src/agent_store/tests/journal.rs](../../../src-tauri/crates/vault-core/src/agent_store/tests/journal.rs)
- [src-tauri/crates/vault-core/src/agent_store/tests/mod.rs](../../../src-tauri/crates/vault-core/src/agent_store/tests/mod.rs)
- [src-tauri/crates/vault-core/src/agent_store/tests/transcript.rs](../../../src-tauri/crates/vault-core/src/agent_store/tests/transcript.rs)
- [src-tauri/crates/vault-core/src/agent_store/transcript.rs](../../../src-tauri/crates/vault-core/src/agent_store/transcript.rs)
- [src-tauri/crates/vault-core/src/agent_store/types.rs](../../../src-tauri/crates/vault-core/src/agent_store/types.rs)
- [src-tauri/crates/vault-core/src/ai.rs](../../../src-tauri/crates/vault-core/src/ai.rs)
- [src-tauri/crates/vault-core/src/ai/agent_tools.rs](../../../src-tauri/crates/vault-core/src/ai/agent_tools.rs)
- [src-tauri/crates/vault-core/src/ai/agent_tools/tests.rs](../../../src-tauri/crates/vault-core/src/ai/agent_tools/tests.rs)
- [src-tauri/crates/vault-core/src/ai/agent_tools/user_records.rs](../../../src-tauri/crates/vault-core/src/ai/agent_tools/user_records.rs)
- [src-tauri/crates/vault-core/src/ai/indexing/backfill.rs](../../../src-tauri/crates/vault-core/src/ai/indexing/backfill.rs)
- [src-tauri/crates/vault-core/src/ai/indexing/candidates.rs](../../../src-tauri/crates/vault-core/src/ai/indexing/candidates.rs)
- [src-tauri/crates/vault-core/src/ai/indexing/content.rs](../../../src-tauri/crates/vault-core/src/ai/indexing/content.rs)
- [src-tauri/crates/vault-core/src/ai/indexing/mod.rs](../../../src-tauri/crates/vault-core/src/ai/indexing/mod.rs)
- [src-tauri/crates/vault-core/src/ai/indexing/store_rows.rs](../../../src-tauri/crates/vault-core/src/ai/indexing/store_rows.rs)
- [src-tauri/crates/vault-core/src/ai/ledger.rs](../../../src-tauri/crates/vault-core/src/ai/ledger.rs)
- [src-tauri/crates/vault-core/src/ai/native_ledger.rs](../../../src-tauri/crates/vault-core/src/ai/native_ledger.rs)
- [src-tauri/crates/vault-core/src/ai/read_model.rs](../../../src-tauri/crates/vault-core/src/ai/read_model.rs)
- [src-tauri/crates/vault-core/src/ai/report_pages.rs](../../../src-tauri/crates/vault-core/src/ai/report_pages.rs)
- [src-tauri/crates/vault-core/src/ai/tests.rs](../../../src-tauri/crates/vault-core/src/ai/tests.rs)
- [src-tauri/crates/vault-core/src/ai/tests_native/coverage.rs](../../../src-tauri/crates/vault-core/src/ai/tests_native/coverage.rs)
- [src-tauri/crates/vault-core/src/ai/tests_native/fixtures.rs](../../../src-tauri/crates/vault-core/src/ai/tests_native/fixtures.rs)
- [src-tauri/crates/vault-core/src/ai/tests_native/indexing.rs](../../../src-tauri/crates/vault-core/src/ai/tests_native/indexing.rs)
- [src-tauri/crates/vault-core/src/ai/tests_native/indexing/metadata.rs](../../../src-tauri/crates/vault-core/src/ai/tests_native/indexing/metadata.rs)
- [src-tauri/crates/vault-core/src/ai/tests_native/indexing/resume.rs](../../../src-tauri/crates/vault-core/src/ai/tests_native/indexing/resume.rs)
- [src-tauri/crates/vault-core/src/ai/tests_native/mod.rs](../../../src-tauri/crates/vault-core/src/ai/tests_native/mod.rs)
- [src-tauri/crates/vault-core/src/ai/tests_native/status.rs](../../../src-tauri/crates/vault-core/src/ai/tests_native/status.rs)
- [src-tauri/crates/vault-core/src/ai/tests_native/working_set.rs](../../../src-tauri/crates/vault-core/src/ai/tests_native/working_set.rs)
- [src-tauri/crates/vault-core/src/ai/working_set.rs](../../../src-tauri/crates/vault-core/src/ai/working_set.rs)
- [src-tauri/crates/vault-core/src/ai_queue.rs](../../../src-tauri/crates/vault-core/src/ai_queue.rs)
- [src-tauri/crates/vault-core/src/ai_queue/store.rs](../../../src-tauri/crates/vault-core/src/ai_queue/store.rs)
- [src-tauri/crates/vault-core/src/ai_queue/tests.rs](../../../src-tauri/crates/vault-core/src/ai_queue/tests.rs)
- [src-tauri/crates/vault-core/src/ai_queue/types.rs](../../../src-tauri/crates/vault-core/src/ai_queue/types.rs)
- [src-tauri/crates/vault-core/src/archive/doctor_derived.rs](../../../src-tauri/crates/vault-core/src/archive/doctor_derived.rs)
- [src-tauri/crates/vault-core/src/archive/doctor_tests.rs](../../../src-tauri/crates/vault-core/src/archive/doctor_tests.rs)
- [src-tauri/crates/vault-core/src/archive/history/net_guard.rs](../../../src-tauri/crates/vault-core/src/archive/history/net_guard.rs)
- [src-tauri/crates/vault-core/src/archive/history/og_images_fetch.rs](../../../src-tauri/crates/vault-core/src/archive/history/og_images_fetch.rs)
- [src-tauri/crates/vault-core/src/archive/history/og_images_synth.rs](../../../src-tauri/crates/vault-core/src/archive/history/og_images_synth.rs)
- [src-tauri/crates/vault-core/src/archive/intelligence_projection.rs](../../../src-tauri/crates/vault-core/src/archive/intelligence_projection.rs)
- [src-tauri/crates/vault-core/src/archive/upgrade_v020_tests.rs](../../../src-tauri/crates/vault-core/src/archive/upgrade_v020_tests.rs)
- [src-tauri/crates/vault-core/src/enrichment.rs](../../../src-tauri/crates/vault-core/src/enrichment.rs)
- [src-tauri/crates/vault-core/src/enrichment/content_fetch.rs](../../../src-tauri/crates/vault-core/src/enrichment/content_fetch.rs)
- [src-tauri/crates/vault-core/src/enrichment/content_fetch/tests.rs](../../../src-tauri/crates/vault-core/src/enrichment/content_fetch/tests.rs)
- [src-tauri/crates/vault-core/src/enrichment/content_fetch/tests/routing.rs](../../../src-tauri/crates/vault-core/src/enrichment/content_fetch/tests/routing.rs)
- [src-tauri/crates/vault-core/src/enrichment/content_fetch/tests/storage.rs](../../../src-tauri/crates/vault-core/src/enrichment/content_fetch/tests/storage.rs)
- [src-tauri/crates/vault-core/src/enrichment/content_fetch/tests/transport.rs](../../../src-tauri/crates/vault-core/src/enrichment/content_fetch/tests/transport.rs)
- [src-tauri/crates/vault-core/src/enrichment/content_fetch_api.rs](../../../src-tauri/crates/vault-core/src/enrichment/content_fetch_api.rs)
- [src-tauri/crates/vault-core/src/enrichment/search_text.rs](../../../src-tauri/crates/vault-core/src/enrichment/search_text.rs)
- [src-tauri/crates/vault-core/src/enrichment/tests.rs](../../../src-tauri/crates/vault-core/src/enrichment/tests.rs)
- [src-tauri/crates/vault-core/src/intelligence/build_pipeline.rs](../../../src-tauri/crates/vault-core/src/intelligence/build_pipeline.rs)
- [src-tauri/crates/vault-core/src/intelligence/build_refind.rs](../../../src-tauri/crates/vault-core/src/intelligence/build_refind.rs)
- [src-tauri/crates/vault-core/src/intelligence/build_rows.rs](../../../src-tauri/crates/vault-core/src/intelligence/build_rows.rs)
- [src-tauri/crates/vault-core/src/intelligence/build_stages.rs](../../../src-tauri/crates/vault-core/src/intelligence/build_stages.rs)
- [src-tauri/crates/vault-core/src/intelligence/build_state.rs](../../../src-tauri/crates/vault-core/src/intelligence/build_state.rs)
- [src-tauri/crates/vault-core/src/intelligence/day_insights.rs](../../../src-tauri/crates/vault-core/src/intelligence/day_insights.rs)
- [src-tauri/crates/vault-core/src/intelligence/host_artifacts.rs](../../../src-tauri/crates/vault-core/src/intelligence/host_artifacts.rs)
- [src-tauri/crates/vault-core/src/intelligence/host_artifacts/tests.rs](../../../src-tauri/crates/vault-core/src/intelligence/host_artifacts/tests.rs)
- [src-tauri/crates/vault-core/src/intelligence/incremental.rs](../../../src-tauri/crates/vault-core/src/intelligence/incremental.rs)
- [src-tauri/crates/vault-core/src/intelligence/intelligence_core_persist.rs](../../../src-tauri/crates/vault-core/src/intelligence/intelligence_core_persist.rs)
- [src-tauri/crates/vault-core/src/intelligence/intelligence_daily_rollup_state.rs](../../../src-tauri/crates/vault-core/src/intelligence/intelligence_daily_rollup_state.rs)
- [src-tauri/crates/vault-core/src/intelligence/intelligence_daily_rollups.rs](../../../src-tauri/crates/vault-core/src/intelligence/intelligence_daily_rollups.rs)
- [src-tauri/crates/vault-core/src/intelligence/intelligence_domain.rs](../../../src-tauri/crates/vault-core/src/intelligence/intelligence_domain.rs)
- [src-tauri/crates/vault-core/src/intelligence/intelligence_explain.rs](../../../src-tauri/crates/vault-core/src/intelligence/intelligence_explain.rs)
- [src-tauri/crates/vault-core/src/intelligence/intelligence_explain_helpers.rs](../../../src-tauri/crates/vault-core/src/intelligence/intelligence_explain_helpers.rs)
- [src-tauri/crates/vault-core/src/intelligence/intelligence_navigation.rs](../../../src-tauri/crates/vault-core/src/intelligence/intelligence_navigation.rs)
- [src-tauri/crates/vault-core/src/intelligence/intelligence_outputs.rs](../../../src-tauri/crates/vault-core/src/intelligence/intelligence_outputs.rs)
- [src-tauri/crates/vault-core/src/intelligence/intelligence_overview.rs](../../../src-tauri/crates/vault-core/src/intelligence/intelligence_overview.rs)
- [src-tauri/crates/vault-core/src/intelligence/intelligence_overview_snapshot.rs](../../../src-tauri/crates/vault-core/src/intelligence/intelligence_overview_snapshot.rs)
- [src-tauri/crates/vault-core/src/intelligence/intelligence_rebuild.rs](../../../src-tauri/crates/vault-core/src/intelligence/intelligence_rebuild.rs)
- [src-tauri/crates/vault-core/src/intelligence/intelligence_refind.rs](../../../src-tauri/crates/vault-core/src/intelligence/intelligence_refind.rs)
- [src-tauri/crates/vault-core/src/intelligence/intelligence_schema.rs](../../../src-tauri/crates/vault-core/src/intelligence/intelligence_schema.rs)
- `src-tauri/crates/vault-core/src/intelligence/intelligence_schema_sql.rs` — removed.
- [src-tauri/crates/vault-core/src/intelligence/intelligence_search_metrics.rs](../../../src-tauri/crates/vault-core/src/intelligence/intelligence_search_metrics.rs)
- [src-tauri/crates/vault-core/src/intelligence/intelligence_search_queries.rs](../../../src-tauri/crates/vault-core/src/intelligence/intelligence_search_queries.rs)
- [src-tauri/crates/vault-core/src/intelligence/intelligence_sessions.rs](../../../src-tauri/crates/vault-core/src/intelligence/intelligence_sessions.rs)
- [src-tauri/crates/vault-core/src/intelligence/intelligence_shared.rs](../../../src-tauri/crates/vault-core/src/intelligence/intelligence_shared.rs)
- [src-tauri/crates/vault-core/src/intelligence/intelligence_structural_aggregates.rs](../../../src-tauri/crates/vault-core/src/intelligence/intelligence_structural_aggregates.rs)
- [src-tauri/crates/vault-core/src/intelligence/intelligence_structural_build.rs](../../../src-tauri/crates/vault-core/src/intelligence/intelligence_structural_build.rs)
- [src-tauri/crates/vault-core/src/intelligence/intelligence_structural_persist.rs](../../../src-tauri/crates/vault-core/src/intelligence/intelligence_structural_persist.rs)
- [src-tauri/crates/vault-core/src/intelligence/intelligence_structural_stage.rs](../../../src-tauri/crates/vault-core/src/intelligence/intelligence_structural_stage.rs)
- [src-tauri/crates/vault-core/src/intelligence/intelligence_structural_state.rs](../../../src-tauri/crates/vault-core/src/intelligence/intelligence_structural_state.rs)
- [src-tauri/crates/vault-core/src/intelligence/intelligence_structural_stream.rs](../../../src-tauri/crates/vault-core/src/intelligence/intelligence_structural_stream.rs)
- [src-tauri/crates/vault-core/src/intelligence/intelligence_summary.rs](../../../src-tauri/crates/vault-core/src/intelligence/intelligence_summary.rs)
- `src-tauri/crates/vault-core/src/intelligence/intelligence_visit_derive.rs` — removed.
- [src-tauri/crates/vault-core/src/intelligence/intelligence_visit_records.rs](../../../src-tauri/crates/vault-core/src/intelligence/intelligence_visit_records.rs)
- [src-tauri/crates/vault-core/src/intelligence/mod.rs](../../../src-tauri/crates/vault-core/src/intelligence/mod.rs)
- [src-tauri/crates/vault-core/src/intelligence/paging.rs](../../../src-tauri/crates/vault-core/src/intelligence/paging.rs)
- [src-tauri/crates/vault-core/src/intelligence/path_flows.rs](../../../src-tauri/crates/vault-core/src/intelligence/path_flows.rs)
- [src-tauri/crates/vault-core/src/intelligence/phase_four.rs](../../../src-tauri/crates/vault-core/src/intelligence/phase_four.rs)
- [src-tauri/crates/vault-core/src/intelligence/phase_three.rs](../../../src-tauri/crates/vault-core/src/intelligence/phase_three.rs)
- [src-tauri/crates/vault-core/src/intelligence/refind_evidence.rs](../../../src-tauri/crates/vault-core/src/intelligence/refind_evidence.rs)
- [src-tauri/crates/vault-core/src/intelligence/search_queries.rs](../../../src-tauri/crates/vault-core/src/intelligence/search_queries.rs)
- [src-tauri/crates/vault-core/src/intelligence/session_details.rs](../../../src-tauri/crates/vault-core/src/intelligence/session_details.rs)
- [src-tauri/crates/vault-core/src/intelligence/site_dictionary/classification.rs](../../../src-tauri/crates/vault-core/src/intelligence/site_dictionary/classification.rs)
- [src-tauri/crates/vault-core/src/intelligence/site_dictionary/mod.rs](../../../src-tauri/crates/vault-core/src/intelligence/site_dictionary/mod.rs)
- [src-tauri/crates/vault-core/src/intelligence/site_dictionary/overrides.rs](../../../src-tauri/crates/vault-core/src/intelligence/site_dictionary/overrides.rs)
- [src-tauri/crates/vault-core/src/intelligence/site_dictionary/search_rules.rs](../../../src-tauri/crates/vault-core/src/intelligence/site_dictionary/search_rules.rs)
- [src-tauri/crates/vault-core/src/intelligence/site_dictionary/tests.rs](../../../src-tauri/crates/vault-core/src/intelligence/site_dictionary/tests.rs)
- [src-tauri/crates/vault-core/src/intelligence/site_dictionary/types.rs](../../../src-tauri/crates/vault-core/src/intelligence/site_dictionary/types.rs)
- [src-tauri/crates/vault-core/src/intelligence/tests/batch_equivalence.rs](../../../src-tauri/crates/vault-core/src/intelligence/tests/batch_equivalence.rs)
- [src-tauri/crates/vault-core/src/intelligence/tests/fixtures.rs](../../../src-tauri/crates/vault-core/src/intelligence/tests/fixtures.rs)
- [src-tauri/crates/vault-core/src/intelligence/tests/frequent_searches.rs](../../../src-tauri/crates/vault-core/src/intelligence/tests/frequent_searches.rs)
- [src-tauri/crates/vault-core/src/intelligence/tests/schema_overview.rs](../../../src-tauri/crates/vault-core/src/intelligence/tests/schema_overview.rs)
- [src-tauri/crates/vault-core/src/intelligence/tests/stage_rebuild.rs](../../../src-tauri/crates/vault-core/src/intelligence/tests/stage_rebuild.rs)
- [src-tauri/crates/vault-core/src/intelligence/tests/structural_incremental.rs](../../../src-tauri/crates/vault-core/src/intelligence/tests/structural_incremental.rs)
- [src-tauri/crates/vault-core/src/intelligence_catalog.rs](../../../src-tauri/crates/vault-core/src/intelligence_catalog.rs)
- [src-tauri/crates/vault-core/src/intelligence_runtime/claims.rs](../../../src-tauri/crates/vault-core/src/intelligence_runtime/claims.rs)
- [src-tauri/crates/vault-core/src/intelligence_runtime/enqueue.rs](../../../src-tauri/crates/vault-core/src/intelligence_runtime/enqueue.rs)
- [src-tauri/crates/vault-core/src/intelligence_runtime/job_control.rs](../../../src-tauri/crates/vault-core/src/intelligence_runtime/job_control.rs)
- [src-tauri/crates/vault-core/src/intelligence_runtime/mod.rs](../../../src-tauri/crates/vault-core/src/intelligence_runtime/mod.rs)
- [src-tauri/crates/vault-core/src/intelligence_runtime/recovery.rs](../../../src-tauri/crates/vault-core/src/intelligence_runtime/recovery.rs)
- [src-tauri/crates/vault-core/src/intelligence_runtime/snapshot.rs](../../../src-tauri/crates/vault-core/src/intelligence_runtime/snapshot.rs)
- [src-tauri/crates/vault-core/src/intelligence_runtime/tests_queue.rs](../../../src-tauri/crates/vault-core/src/intelligence_runtime/tests_queue.rs)
- [src-tauri/crates/vault-core/src/intelligence_runtime/tests_runtime.rs](../../../src-tauri/crates/vault-core/src/intelligence_runtime/tests_runtime.rs)
- [src-tauri/crates/vault-core/src/intelligence_sections.rs](../../../src-tauri/crates/vault-core/src/intelligence_sections.rs)
- [src-tauri/crates/vault-core/src/lib.rs](../../../src-tauri/crates/vault-core/src/lib.rs)
- [src-tauri/crates/vault-core/src/models/core_intelligence/analytics.rs](../../../src-tauri/crates/vault-core/src/models/core_intelligence/analytics.rs)
- [src-tauri/crates/vault-core/src/models/core_intelligence/exports.rs](../../../src-tauri/crates/vault-core/src/models/core_intelligence/exports.rs)
- [src-tauri/crates/vault-core/src/models/core_intelligence/mod.rs](../../../src-tauri/crates/vault-core/src/models/core_intelligence/mod.rs)
- [src-tauri/crates/vault-core/src/models/core_intelligence/overview.rs](../../../src-tauri/crates/vault-core/src/models/core_intelligence/overview.rs)
- [src-tauri/crates/vault-core/src/models/core_intelligence/paging.rs](../../../src-tauri/crates/vault-core/src/models/core_intelligence/paging.rs)
- [src-tauri/crates/vault-core/src/models/core_intelligence/reads.rs](../../../src-tauri/crates/vault-core/src/models/core_intelligence/reads.rs)
- [src-tauri/crates/vault-core/src/models/core_intelligence/requests.rs](../../../src-tauri/crates/vault-core/src/models/core_intelligence/requests.rs)
- [src-tauri/crates/vault-core/src/models/intelligence.rs](../../../src-tauri/crates/vault-core/src/models/intelligence.rs)
- [src-tauri/crates/vault-core/src/models/intelligence_pages.rs](../../../src-tauri/crates/vault-core/src/models/intelligence_pages.rs)
- [src-tauri/crates/vault-core/src/models/mod.rs](../../../src-tauri/crates/vault-core/src/models/mod.rs)
- [src-tauri/crates/vault-core/src/snapshot_reads.rs](../../../src-tauri/crates/vault-core/src/snapshot_reads.rs)
- [src-tauri/crates/vault-core/src/store_schema/agent.rs](../../../src-tauri/crates/vault-core/src/store_schema/agent.rs)
- [src-tauri/crates/vault-core/src/store_schema/intelligence.rs](../../../src-tauri/crates/vault-core/src/store_schema/intelligence.rs)
- [src-tauri/crates/vault-core/src/store_schema/source_evidence.rs](../../../src-tauri/crates/vault-core/src/store_schema/source_evidence.rs)
- [src-tauri/crates/vault-core/src/store_schema/tests/agent.rs](../../../src-tauri/crates/vault-core/src/store_schema/tests/agent.rs)
- [src-tauri/crates/vault-core/src/store_schema/tests/intelligence.rs](../../../src-tauri/crates/vault-core/src/store_schema/tests/intelligence.rs)
- [src-tauri/crates/vault-core/src/store_schema/tests/mod.rs](../../../src-tauri/crates/vault-core/src/store_schema/tests/mod.rs)
- [src-tauri/crates/vault-core/src/store_schema/tests/source_evidence.rs](../../../src-tauri/crates/vault-core/src/store_schema/tests/source_evidence.rs)
- [src-tauri/crates/vault-core/src/test_support.rs](../../../src-tauri/crates/vault-core/src/test_support.rs)
- [src-tauri/crates/vault-core/src/test_support/native.rs](../../../src-tauri/crates/vault-core/src/test_support/native.rs)
- [src-tauri/crates/vault-core/src/utils.rs](../../../src-tauri/crates/vault-core/src/utils.rs)
- [src-tauri/crates/vault-core/src/vault.rs](../../../src-tauri/crates/vault-core/src/vault.rs)
- [src-tauri/crates/vault-core/tests/agent_store_paging.rs](../../../src-tauri/crates/vault-core/tests/agent_store_paging.rs)
- [src-tauri/crates/vault-core/tests/ai_queue_turso.rs](../../../src-tauri/crates/vault-core/tests/ai_queue_turso.rs)
- [src-tauri/crates/vault-core/tests/enrichment_turso.rs](../../../src-tauri/crates/vault-core/tests/enrichment_turso.rs)
- [src-tauri/crates/vault-core/tests/intelligence_agent_pages.rs](../../../src-tauri/crates/vault-core/tests/intelligence_agent_pages.rs)
- [src-tauri/crates/vault-core/tests/intelligence_explanation_pages.rs](../../../src-tauri/crates/vault-core/tests/intelligence_explanation_pages.rs)
- [src-tauri/crates/vault-core/tests/intelligence_generation_plans.rs](../../../src-tauri/crates/vault-core/tests/intelligence_generation_plans.rs)
- [src-tauri/crates/vault-core/tests/intelligence_paging.rs](../../../src-tauri/crates/vault-core/tests/intelligence_paging.rs)
- [src-tauri/crates/vault-core/tests/intelligence_turso.rs](../../../src-tauri/crates/vault-core/tests/intelligence_turso.rs)
- [src-tauri/crates/vault-core/tests/intelligence_turso/render_plans.rs](../../../src-tauri/crates/vault-core/tests/intelligence_turso/render_plans.rs)
- [src-tauri/crates/vault-core/tests/lmstudio_agent_e2e.rs](../../../src-tauri/crates/vault-core/tests/lmstudio_agent_e2e.rs)
- [src-tauri/crates/vault-core/tests/working_set_turso.rs](../../../src-tauri/crates/vault-core/tests/working_set_turso.rs)
- [src-tauri/crates/vault-store/README.md](../../../src-tauri/crates/vault-store/README.md)
- [src-tauri/crates/vault-store/src/engine.rs](../../../src-tauri/crates/vault-store/src/engine.rs)
- [src-tauri/crates/vault-store/src/lib.rs](../../../src-tauri/crates/vault-store/src/lib.rs)
- [src-tauri/crates/vault-store/src/query_plans.rs](../../../src-tauri/crates/vault-store/src/query_plans.rs)
- [src-tauri/crates/vault-store/src/session.rs](../../../src-tauri/crates/vault-store/src/session.rs)
- [src-tauri/crates/vault-store/src/sql.rs](../../../src-tauri/crates/vault-store/src/sql.rs)
- [src-tauri/crates/vault-store/src/statements.rs](../../../src-tauri/crates/vault-store/src/statements.rs)
- [src-tauri/crates/vault-store/src/worker.rs](../../../src-tauri/crates/vault-store/src/worker.rs)
- [src-tauri/crates/vault-store/tests/archive_attachment.rs](../../../src-tauri/crates/vault-store/tests/archive_attachment.rs)
- [src-tauri/crates/vault-store/tests/query_plans.rs](../../../src-tauri/crates/vault-store/tests/query_plans.rs)
- [src-tauri/crates/vault-store/tests/statements.rs](../../../src-tauri/crates/vault-store/tests/statements.rs)
- [src-tauri/crates/vault-worker/src/app.rs](../../../src-tauri/crates/vault-worker/src/app.rs)
- [src-tauri/crates/vault-worker/src/archive_flows.rs](../../../src-tauri/crates/vault-worker/src/archive_flows.rs)
- [src-tauri/crates/vault-worker/src/cli.rs](../../../src-tauri/crates/vault-worker/src/cli.rs)
- [src-tauri/crates/vault-worker/src/context.rs](../../../src-tauri/crates/vault-worker/src/context.rs)
- [src-tauri/crates/vault-worker/src/intelligence.rs](../../../src-tauri/crates/vault-worker/src/intelligence.rs)
- [src-tauri/crates/vault-worker/src/intelligence/agent_store.rs](../../../src-tauri/crates/vault-worker/src/intelligence/agent_store.rs)
- [src-tauri/crates/vault-worker/src/intelligence/ai_queue.rs](../../../src-tauri/crates/vault-worker/src/intelligence/ai_queue.rs)
- [src-tauri/crates/vault-worker/src/intelligence/chat.rs](../../../src-tauri/crates/vault-worker/src/intelligence/chat.rs)
- [src-tauri/crates/vault-worker/src/intelligence/content_fetch.rs](../../../src-tauri/crates/vault-worker/src/intelligence/content_fetch.rs)
- [src-tauri/crates/vault-worker/src/intelligence/route_queries.rs](../../../src-tauri/crates/vault-worker/src/intelligence/route_queries.rs)
- [src-tauri/crates/vault-worker/src/intelligence/runtime.rs](../../../src-tauri/crates/vault-worker/src/intelligence/runtime.rs)
- [src-tauri/crates/vault-worker/src/intelligence/section_queries.rs](../../../src-tauri/crates/vault-worker/src/intelligence/section_queries.rs)
- [src-tauri/crates/vault-worker/src/lib.rs](../../../src-tauri/crates/vault-worker/src/lib.rs)
- [src-tauri/crates/vault-worker/src/mcp.rs](../../../src-tauri/crates/vault-worker/src/mcp.rs)
- [src-tauri/crates/vault-worker/src/owner_requests/mod.rs](../../../src-tauri/crates/vault-worker/src/owner_requests/mod.rs)
- [src-tauri/crates/vault-worker/src/owner_requests/tests.rs](../../../src-tauri/crates/vault-worker/src/owner_requests/tests.rs)
- [src-tauri/crates/vault-worker/src/tests.rs](../../../src-tauri/crates/vault-worker/src/tests.rs)
- [src-tauri/crates/vault-worker/tests/archive_scale_bench.rs](../../../src-tauri/crates/vault-worker/tests/archive_scale_bench.rs)
- [src-tauri/crates/vault-worker/tests/archive_scale_bench/turso_intelligence.rs](../../../src-tauri/crates/vault-worker/tests/archive_scale_bench/turso_intelligence.rs)
- [src-tauri/crates/vault-worker/tests/archive_scale_bench/turso_reads.rs](../../../src-tauri/crates/vault-worker/tests/archive_scale_bench/turso_reads.rs)
- [src-tauri/src/commands/archive.rs](../../../src-tauri/src/commands/archive.rs)
- [src-tauri/src/commands/intelligence/ai.rs](../../../src-tauri/src/commands/intelligence/ai.rs)
- [src-tauri/src/commands/intelligence/core.rs](../../../src-tauri/src/commands/intelligence/core.rs)
- [src-tauri/src/commands/intelligence/runtime.rs](../../../src-tauri/src/commands/intelligence/runtime.rs)
- [src-tauri/src/dev_ipc_bridge/dispatch.rs](../../../src-tauri/src/dev_ipc_bridge/dispatch.rs)
- [src-tauri/src/dev_ipc_bridge/dispatch/tests.rs](../../../src-tauri/src/dev_ipc_bridge/dispatch/tests.rs)
- [src-tauri/src/dev_ipc_bridge/payloads.rs](../../../src-tauri/src/dev_ipc_bridge/payloads.rs)
- [src-tauri/src/lib.rs](../../../src-tauri/src/lib.rs)
- [src-tauri/src/worker_bridge/app.rs](../../../src-tauri/src/worker_bridge/app.rs)
- [src-tauri/src/worker_bridge/intelligence/ai.rs](../../../src-tauri/src/worker_bridge/intelligence/ai.rs)
- [src-tauri/src/worker_bridge/intelligence/core.rs](../../../src-tauri/src/worker_bridge/intelligence/core.rs)
- [src-tauri/src/worker_bridge/intelligence/runtime.rs](../../../src-tauri/src/worker_bridge/intelligence/runtime.rs)
- [src-tauri/src/worker_bridge/mod.rs](../../../src-tauri/src/worker_bridge/mod.rs)

`src/` and `tests/e2e/` are unchanged. No dependency, lockfile, upstream draft, gate log or
artifacts directory was added. Gate logs and temporary scripts remain outside the repository.
