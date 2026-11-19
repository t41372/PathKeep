# Step 3 publication and target-size verification

The base revision is `3f197c06`. The Linux review on that revision established the 32 MiB selective-rendering decision. On 2026-11-19 the final local release run used arm64 macOS, disk-backed synthetic evidence, and the full 264,000,000-byte / 23,812-record corpus. Each position has 40 cold samples, 40 warm samples and 40 reads launched at the backup’s raw-commit callback. The production and rendered-page paths compared each returned ordinal, native ID and complete message with generated source content. The evidence path in this run checked the exact raw bytes, marker positions and message suffix.

```sh
AH_BYTES=264000000 AH_RECORDS=23812 AH_SAMPLES=40 cargo run --release -p agent-history --example window_measurement
```

## Production window

Cells are p95 / p99 in milliseconds. Each read resolves a real reader publication, with the proper membership, selected source pages and evidence identity checks.

| Native transport | Physical window |          Cold |          Warm | After raw commit |
| ---------------- | --------------- | ------------: | ------------: | ---------------: |
| plain            | 0–199           | 19.68 / 22.03 | 17.40 / 20.41 |    17.12 / 17.38 |
| plain            | 11806–12005     | 21.34 / 26.08 | 19.12 / 20.35 |    19.95 / 31.29 |
| plain            | 23612–23811     | 18.66 / 26.13 | 17.41 / 19.77 |    17.39 / 17.74 |
| multi            | 0–199           | 20.29 / 22.50 | 37.42 / 66.67 |   56.97 / 120.92 |
| multi            | 11806–12005     | 22.39 / 29.76 | 20.78 / 62.55 |    18.70 / 69.38 |
| multi            | 23612–23811     | 26.18 / 28.53 | 19.51 / 25.89 |    20.25 / 23.52 |
| single           | 0–199           | 12.94 / 24.06 | 12.60 / 61.81 |    26.91 / 43.16 |
| single           | 11806–12005     | 10.78 / 15.50 | 16.08 / 36.89 |    18.29 / 40.65 |
| single           | 23612–23811     |  8.52 / 11.12 |  7.86 / 11.78 |     7.80 / 10.79 |

| Native transport | Largest decoded native frame (bytes) | Rendered pages | Scan ms | Longest progress gap ms | Preceding phase |
| ---------------- | -----------------------------------: | -------------- | ------: | ----------------------: | --------------- |
| plain            |                                    0 | False          | 1479.70 |                    1.20 | FetchingSlice   |
| multi            |                            1,084,264 | False          | 1337.25 |                    4.72 | FetchingSlice   |
| single           |                          264,000,000 | True           | 1318.48 |                    4.78 | FetchingSlice   |

The previously reported Linux gaps of 166 and 120 ms spanned a scan that only reported decoded transport bytes. One call to `RawReader::load_chunk` fetched a whole stored blob, inflated it if applicable and authenticated up to 4 MiB before the framer could report again. Those aggregate logs cannot distinguish a slow database request, computation or scheduling delay. A scan with a progress observer now transfers up to 256 KiB per SQL request and inflates/hashes in 64 KiB blocks. Verification progress describes work on the pending slice. No unverified native bytes reach the visitor. An individual database operation still depends on the engine and operating system. The [JSON lines](validation/window-step3-macos.jsonl) include the phase preceding the largest gap for the Linux rerun.

Linux eviction checks process descriptors by filesystem device and inode before advising away pages. Each residency check unmaps and closes its own descriptor. A remaining handle or resident page yields a JSON diagnostic on stderr naming the file and triggers a bounded backoff and retry. A persistent failure invalidates the cold sample.

The JSON measurements named `rendered_storage` and `rendered_build` refer to the independent comparison table. The production builder reinterprets the affected retained source and reuses identical content-addressed blobs. This run measures production reads and does not establish a production append-time bound.

## Fault checks

The restored agent-history crate ran **338 tests**, and **29 owner lifecycle tests** passed. Formatting and strict all-target Clippy passed. Real mode, snapshot, restore, move, reopen, password change, pause and wipe cases exercise a Codex tail that cannot be read under the test's replay budget without its published pages.

Each row below is a separate source edit in an isolated copy using the shared Cargo target. Every mutation ran the complete non-E2E crate suite before restoration. Forcing evidence replay also failed the worker mode-switch/snapshot/restore case at its published-tail assertion. The named `cli_e2e` target and desktop E2E were excluded. Counts are passing / failing tests. The attachment-prefix, scan-progress and forced-evidence mutations also had unrelated CLI storage errors on their first runs. Their rows use the repeats under a separately named profile inside the shared target. Additional failures in a run can cover the same invariant from another entry point. The [per-run results](validation/step3-mutation-results.jsonl) include the initial and repeated runs.

| Broken behavior                                                                        | Catching test                                                            | Passed / failed |
| -------------------------------------------------------------------------------------- | ------------------------------------------------------------------------ | --------------: |
| Accumulate decoded lengths across native frames                                        | `frame_extent_counts_decoded_bytes_across_record_boundaries`             |         336 / 2 |
| Select exactly 32 MiB as well as longer frames                                         | `only_a_frame_strictly_above_the_limit_selects_rendering`                |         337 / 1 |
| Accept a stale renderer key                                                            | `long_frame_cache_is_selective_and_missing_or_stale_text_reads_evidence` |         337 / 1 |
| Abort on a missing rendered page                                                       | `long_frame_cache_is_selective_and_missing_or_stale_text_reads_evidence` |         337 / 1 |
| Accept a mismatched page binding                                                       | `long_frame_cache_is_selective_and_missing_or_stale_text_reads_evidence` |         337 / 1 |
| Trust the local revision handle without verifying source and digest                    | `equal_revision_handles_in_a_foreign_catalog_never_authorize_a_header`   |         337 / 1 |
| Leave unreachable retired source versions                                              | `replacement_collects_private_pages_and_keeps_shared_rendered_blobs`     |         337 / 1 |
| Switch after a newer dependency ticket supersedes the builder                          | `stale_membership_ticket_leaves_old_head_and_recovers_dirty_work`        |         337 / 1 |
| Skip dirty-head recovery on open                                                       | `stale_membership_ticket_leaves_old_head_and_recovers_dirty_work`        |         337 / 1 |
| Leave a pre-crash parser attempt writable                                              | `i_abandoned_staging_survives_reopen_and_stale_builder_cannot_switch`    |         337 / 1 |
| Keep a Pi run contiguous across a different parent                                     | `b_pi_alternate_leaf_and_stale_head`                                     |         337 / 1 |
| Read the elected member for an explicitly selected earlier branch                      | `c_late_base_publishes_clipped_line_and_keeps_abandoned_branch`          |         337 / 1 |
| Skip the captured unreadable successor                                                 | `d_unreadable_higher_generation_has_no_inherited_rendering`              |         336 / 2 |
| Forget an unresolved native rollout dependency                                         | `base_from_another_root_refreshes_a_dependent_without_recapturing_it`    |         337 / 1 |
| Include the base tail after the exclusive cut                                          | `c_late_base_publishes_clipped_line_and_keeps_abandoned_branch`          |         334 / 4 |
| Declare a native cycle complete                                                        | `cycle_stops_at_visited_source_in_the_published_line`                    |         337 / 1 |
| Treat an included parser gap as covered                                                | `prefix_gaps_and_attachments_follow_the_exclusive_cut`                   |         336 / 2 |
| Count attachment loss in the abandoned suffix                                          | `prefix_gaps_and_attachments_follow_the_exclusive_cut`                   |         336 / 2 |
| Require the live source root to rebuild reader pages                                   | `offline_page_rebuild_preserves_native_projection_and_evidence`          |         336 / 2 |
| Drop the final short native-ID merge run                                               | `sorted_lookup_pages_preserve_repeated_ids_and_raw_provenance`           |         337 / 1 |
| Advance a visibility bitmap by one row regardless of its population                    | `visible_ranks_skip_non_display_records_before_the_requested_window`     |         337 / 1 |
| Suppress publication when source presence changes                                      | `late_parent_and_missing_source_update_membership`                       |         337 / 1 |
| Leave an expired state source published                                                | `replacing_retained_state_removes_its_reader_publication`                |         337 / 1 |
| Remove verification-phase progress                                                     | `scan_reports_verified_work_inside_the_first_storage_slice`              |         337 / 1 |
| Accept residency on the first eviction attempt                                         | `transient_residency_retries_the_named_file`                             |         335 / 3 |
| Hash the random page owner into the input fingerprint                                  | `offline_page_rebuild_preserves_native_projection_and_evidence`          |         336 / 2 |
| Report an existing unbuilt conversation as absent                                      | `development_catalog_without_reader_tables_data_rebuilds_offline`        |         337 / 1 |
| Choose the earliest recorded leaf                                                      | `b_pi_alternate_leaf_and_stale_head`                                     |         337 / 1 |
| Always replay evidence for a long-frame display window                                 | `long_frame_cache_is_selective_and_missing_or_stale_text_reads_evidence` |         335 / 3 |
| Evict while a synthetic file handle is still open                                      | `transient_residency_retries_the_named_file`                             |         335 / 3 |
| Reuse the obsolete renderer when backing up unchanged input                            | `long_frame_cache_is_selective_and_missing_or_stale_text_reads_evidence` |         337 / 1 |
| Store rendered bytes for a short-frame source while leaving its eligibility flag false | `only_a_frame_strictly_above_the_limit_selects_rendering`                |         337 / 1 |
| Expose only the own length of an inherited source branch                               | `c_late_base_publishes_clipped_line_and_keeps_abandoned_branch`          |         337 / 1 |
| Drop the validated base prefix digest                                                  | `c_late_base_publishes_clipped_line_and_keeps_abandoned_branch`          |         337 / 1 |
| Treat an exhausted staging budget as a successfully built short publication            | `i_abandoned_staging_survives_reopen_and_stale_builder_cannot_switch`    |         337 / 1 |
| Trust a decodable payload with the wrong content digest                                | `replacement_collects_private_pages_and_keeps_shared_rendered_blobs`     |         337 / 1 |
| Keep damaged data on a content-hash conflict during offline rebuild                    | `replacement_collects_private_pages_and_keeps_shared_rendered_blobs`     |         337 / 1 |

One fault used a valid compressed payload containing different text under an existing content hash. The reader fell back to evidence, but the first rebuild still hit the replay budget because it kept the damaged blob on key conflict. Rebuilding now replaces differing bytes under that key. The restored page then opens under the one-byte replay budget.

## Append cost and the incremental-page decision (2026-11-19)

One 700-byte append to the 264,000,000-byte plain source, backed up through the production path on arm64 macOS
(release): 10.17 s end to end in a plaintext archive, 11.76 s encrypted. Of that, raw commit through the staged
projection took 3.21 s / 3.78 s and the reader rescan with its page writes 1.31 s / 1.33 s (264,000,700 bytes,
23,813 records, 283 new page pieces; 566 pieces and 344 unique blobs at the peak of old plus new, one generation
and 283 pieces after backup-end collection). Raw data: `validation/reader-step3-reviewed-macos.jsonl`.

Decision: the reader keeps rescanning the whole source on change; no incremental pages for append-only sources.
The rescan is about an eighth of the backup's time for this, the largest single transcript the product targets, and
the backup runs in the background. If appends to very large transcripts need to get faster, the larger costs are
the raw commit and the projection and search synchronisation, not the reader pages.
