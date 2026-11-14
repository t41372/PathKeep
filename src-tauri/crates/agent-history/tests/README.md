# Agent history contract tests

The original core tests and historical platform evidence are recorded in
`docs/architecture/research/agent-history/validation/`. The current fix-round
regressions below were run red before their fixes; the crate-only gate validates
this working tree. Historical results are not new-format platform validation.

The CLI is the production entry point, with real files, compressed native logs,
encrypted storage, reopen operations and actual exports. No archive or parser is
mocked. All content is synthetic; native schemas are grounded in the pinned
sources in `docs/architecture/research/agent-history/`.

Failure contracts:

- Different harnesses remain separate. Claude/Pi origin marks separate native-ID
  strangers; Codex/DeepSeek use their native ID alone. Documents use a source mark.
  Byte-identical or prefix copies under multiple roots share a conversation where
  the adapter's key is shared.
- Compressed logs, all branches, unknown events and duplicate assistant response
  IDs must survive backup; lower DeepSeek generations remain raw evidence.
- A plan must not initialize an archive, and a write requires explicit execution.
- Source bytes and modification times must survive discovery, backup and export.
- A same-size rewrite with the original timestamp, truncation, deletion, or a
  partial final JSON record must not erase an older retained revision.
- Missing or escaping referenced files produce reason-class diagnostics and
  unavailable-reference counts without lowering conversation completeness; text
  in a conversation does not grant access to unrelated files.
- Search must include the end of a conversation larger than 8 MiB, survive
  reopening, and find newly appended content without retaining replaced content.
- Pagination must cover each conversation/physical event address exactly once with a stable cursor.
- Raw exports reproduce compressed or malformed source bytes exactly; normalized
  exports retain native payloads and Markdown remains a readable derivative.
- Wrong passwords, encrypted-state corruption and occupied output paths cannot
  silently produce successful results or destroy existing data.

Every CLI scenario writes `fixture.json` and `report.json` below
`artifacts/agent-history/<scenario>/`. The report includes expected/actual checks
and command results even when an assertion panics. Re-run with
`cargo test --manifest-path src-tauri/Cargo.toml -p agent-history --test cli_e2e`.
Crash injection, live writer races and platform-specific permission behavior are
separate integrity contracts; this CLI suite does not pretend to cover them.

## First implementation round (historical adaptations)

The five supplied contract files and their assertions are unchanged. New focused
cases in `identity_recovery.rs` cover reassignment of a current source away from a
conversation with a surviving copy, and same-native-ID parent disambiguation by
root/layout. `retention_recovery.rs` covers state A→B→A and prefix→rewrite→append,
including an already-open reader and different producer slice boundaries. Their
failure modes were recorded before the corresponding implementation/fixes.

Existing test changes, with the authority for each:

- Contract §7.2 (conversation IDs, source membership, physical event addresses):
  `four_native_harnesses_remain_separate_and_export_their_exact_evidence`,
  `rewrite_truncation_partial_tail_and_deletion_keep_every_raw_revision`,
  `attachment_references_do_not_grant_access_outside_the_source_root`,
  `missing_inherited_rollout_is_reported_without_discarding_the_child`,
  `same_byte_reparse_keeps_old_rows_visible_and_invalidates_old_event_cursor_on_publish`,
  `keyset_pages_keep_timestamp_ties_nulls_and_distinct_source_identities`,
  `events_never_mix_published_and_staged_revisions`,
  `large_native_events_page_by_bytes_without_dropping_a_record`,
  `failed_new_normalization_does_not_claim_the_old_projection_covers_latest_bytes`,
  `metadata_pages_bound_bytes_without_truncating_or_skipping_native_titles`,
  `reparsing_identical_bytes_publishes_a_distinct_projection_and_expires_event_cursors`,
  `larger_parser_budget_recovers_deleted_source_without_new_raw_revision`,
  `retained_attachment_coverage_survives_deleted_source_roots`,
  `missing_retained_attachment_is_partial_and_does_not_read_new_live_file`,
  `publication_ahead_of_search_is_explicit_and_replacement_removes_old_text`,
  `native_ids_do_not_merge_harnesses_and_unicode_filters_are_exact`,
  `a_large_message_keeps_terms_after_eight_mebibytes_and_index_stays_encrypted`.
  These obtain a conversation ID from real membership rather than passing a source
  key to conversation APIs. Timestamp ties compare the derived IDs; metadata
  assertions still identify the exact originating sources. Normalized event
  assertions retain every prior field and additionally check source_id.
- Contract §7.4 (SQLite v2):
  `logical_database_exports_identify_the_retained_format_without_claiming_native_bytes`,
  `committed_wal_rows_and_metadata_belong_to_one_snapshot`, and
  `unaddressable_hidden_rowid_is_reported_instead_of_invented` expect v2/bare UTF-8
  strings; invalid text/blob storage-class and snapshot assertions stay intact.
- Contract §7.5 (thread_history excluded):
  `discovery_captures_compressed_archives_metadata_and_assets_without_credentials`
  expects eight artifacts and one SQLite source, instead of nine and two.
- The brief explicitly requires deleting the unused legacy wrappers and moving
  tests onto the attempt/versioned-report API. The following test bodies/helpers
  now use actual ProjectionAttempt tokens or explicit fixture parser versions,
  preserving the old failure/atomicity assertions:
  `new_projection_is_visible_only_after_publication_and_advances_search_generation_once`,
  `publishing_new_raw_bytes_invalidates_old_search_coverage_before_any_parser_runs`,
  `parse_coverage_and_native_references_survive_reopen`,
  `invalid_or_oversized_reports_leave_existing_state_untouched`,
  `a_source_cannot_project_another_sources_revision`,
  `a_published_projection_cannot_be_appended_or_published_twice`,
  `duplicate_ordinals_roll_back_the_entire_batch`,
  `superseded_projection_work_cannot_publish`,
  `bounded_cleanup_preserves_current_projection_and_all_raw_revisions`,
  `a_failed_newer_parse_keeps_prior_results_but_marks_their_coverage_incomplete`,
  `verification_checks_complete_raw_revisions_without_requiring_parseable_messages`,
  and `event_index_pages_seek_the_published_projection_without_a_sorter`.
  Shared publishing helpers for all seven `export_stream` cases, `query_export`
  and `search_integrity` cases use this same real API. No legacy test wrapper is
  introduced. Export cases also address the resulting conversation IDs:
  `raw_export_is_byte_exact_while_normalized_export_preserves_all_paged_evidence`,
  `markdown_cannot_promote_native_content_to_active_markup`,
  `output_failure_never_returns_a_successful_export_report`,
  `replacing_a_projection_during_export_cannot_mix_two_revisions`,
  `republishing_identical_raw_bytes_during_export_rejects_the_changed_projection`,
  `staged_reparse_keeps_the_published_export_available_until_publication` and the
  logical-database export case listed above.
- `capture_crash::options` supplies the added read_git option. No crash assertion
  changed. No fixture mistake was repaired and no test was ignored or removed.

Uncovered by this run: Windows/Linux runtime of the new format, real home
histories, target-machine 14.4M/multi-source profiling, physical power loss,
exhaustive state-cache/retention crash checkpoints, and scheduled git-directory
replacement races. Historical validation results for the old format are not a
substitute for these checks. No mutation tests or desktop build run in this block.

## One fix round (resume-2)

Current contract assertions remain unchanged, including the two replacement tests
strengthened by the reviewer. Rustfmt changed layout only in the supplied new
suites. The document fixture call in `conversation_stability.rs` borrows `question`
with `std::slice::from_ref` instead of cloning it, solely to satisfy strict Clippy;
its serialized bytes and every assertion are identical. No lint waiver was added.

Only three existing test meanings changed in this round, each authorized by §10:

- `codex_adapter::referenced_history_is_preserved_without_claiming_a_complete_linear_thread`:
  the native fork is checked through forked_from_session_id; parent is absent (§10 fork link).
- `deepseek_adapter::preserves_fork_cut_and_latest_title_without_mistaking_parent_for_launch_origin`:
  parentSession without subagent origin is checked as a fork (§10 fork link).
- `query_export::keyset_pages_keep_timestamp_ties_nulls_and_distinct_source_identities`:
  the two Codex files with one thread ID are one conversation with both source
  members; Pi strangers and exact pagination/order checks remain (§10 same-thread identity).

Mechanical `forked_from_session_id: None` additions let existing typed fixture
constructors compile without changing their meaning. The unchanged-ID assertions
in `events_never_mix_published_and_staged_revisions` and
`replacing_a_projection_during_export_cannot_mix_two_revisions` were preserved.

The new contract tests pin reclassification retention, per-tool keys, unfinished
first records, rewritten document state, forks, source ordering, chain reads,
DeepSeek unreadable-generation reachability, unrelated-publication cursors and
git alias/unreadability observations. Additional focused regressions were written
and run red before the corresponding fixes:

- `a_state_reader_reports_expiry_instead_of_missing_slice_damage`: a state
  snapshot expires after the reader consumes its first byte.
- `raw_export_keeps_the_existing_expired_revision_error`: the same expiry must
  remain the existing not_found classification at the export boundary.
- `a_metadata_read_does_not_wait_for_the_publication_owner`: an independently
  held publication owner cannot serialize a metadata read.
- `republishing_an_inherited_source_during_export_invalidates_the_export`: a
  same-byte base reparse after the export header invalidates the whole export.
- `first_record_gating_propagates_retained_integrity_failure`: raw-store damage
  must not become a decoder/source limitation.
- `one_conversations_sources_cannot_grow_sql_parameters_past_the_page_bound`:
  600 synthetic retained copies exercise real store plans and source membership.
- `retiring_the_current_source_selects_a_remaining_copy_and_preserves_state`:
  regression fails with re-election disabled, rather than accepting a vanished ID.
- `search_cannot_mix_old_trash_filter_with_new_hit_state`: a one-shot test
  observer performs a real mark between real filter and hit reads; SQL and
  Tantivy execution remain in both test and production paths.
- `divergent_copies_of_a_physical_rollout_do_not_supply_an_arbitrary_prefix`:
  equal lengths/native IDs do not settle a cross-root inherited prefix.
- `a_larger_parser_budget_does_not_rename_a_known_native_conversation`:
  a skipped large header cannot create temporary identity and lose a star.
- `invalid_git_text_is_reported_as_unreadable`: unreadable UTF-8 facts are not
  silently treated as absent git configuration.
- `native_parent_and_rollout_ids_never_become_paths_to_read`: an ID with a slash
  cannot capture an unrelated file, even inside an explicitly approved root.

The existing process-exit capture/reparse tests still run against real stores.
This is not exhaustive crash scheduling or physical power-loss testing. Windows,
Linux, adversarial directory-swap scheduling and target-machine 14.4M/multi-source
profiling were not rerun. All fixtures are synthetic; no real home histories,
network services, harness executables, desktop build or new dependency are used.

## Second fix round (resume-3)

The amended contract tests retain every assertion. They pin nested Codex native
boundaries, a late base completing its child without reparse, and attachment
availability being separate from completeness. All fixtures remain synthetic.

Five existing tests changed meaning only where the three resume-3 amendments
require it:

- `codex_adapter::referenced_history_is_preserved_without_claiming_a_complete_linear_thread`:
  parsing the child's own records is complete and no longer emits a permanent
  `codex-lineage-unresolved`; native boundary, fork and payload checks remain.
- `codex_adapter::remote_only_media_is_a_reported_source_limit_and_inline_bytes_remain_native`:
  unavailable remote media does not lower completeness; its diagnostic has the
  redacted `remote` class and inline native evidence remains checked.
- `codex_adapter::remote_media_in_tool_outputs_and_legacy_audio_is_not_silently_complete`:
  the same remote-reference rule applies to tool output and legacy audio.
- `capture::references::tests::unresolved_diagnostics_do_not_expose_untrusted_reference_contents`:
  detail accepts only the allowed reason classes instead of requiring null;
  target/path secrecy assertions remain.
- `reparse_source::missing_retained_attachment_is_partial_and_does_not_read_new_live_file`:
  the run remains partial, while the conversation is complete with one unavailable
  attachment and the saved `missing` reason. No-live-read, search and revision
  assertions remain.

Adding empty `NativeEvent.references` to typed test constructors is mechanical;
the field streams per-record capture references and is skipped in serialized
events. It does not change fixture bytes or existing assertions.

Additional failure modes were written as tests and run red before their fixes:

- `attachment_line::inherited_attachment_counts_exclude_the_abandoned_tail_and_keep_reparse_reasons`:
  only reference occurrences in an inherited prefix count; offline reparse keeps
  the original redacted reason after the roots disappear.
- `attachment_line::repeated_references_count_as_occurrences_while_capture_stays_deduplicated`:
  repeated references count twice but reading/capturing one target is deduplicated.
- `prefix_coverage::inherited_parse_gaps_only_affect_the_records_on_the_current_line`:
  malformed records before a cut lower completeness, while abandoned-tail failures
  do not; list, search, verify and normalized export agree.

The full local suite has 288 passing tests. Existing real-store process-exit
tests still run; they do not exhaust attachment-counter publication/recovery
checkpoints or physical power loss. No Windows/Linux, target-machine profiling,
real-transcript backup, desktop build or external service was used this round.
