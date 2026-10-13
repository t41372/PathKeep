# Turso maintenance and SQLCipher conversion

## Validation

All requested gates passed on Modal Linux microVMs. The final Rust edits after
the full suite only complete module documentation; all 79 changed Rust files were
compared against that tested source with no non-documentation differences. Final
formatting, Clippy and workspace build also passed on those files. Generated benchmark data stayed in Sandbox `/tmp`.

| Check                                                 | Result                                                                                          |
| ----------------------------------------------------- | ----------------------------------------------------------------------------------------------- |
| Workspace formatting                                  | Passed on the final files                                                                       |
| Workspace Clippy, all targets/features, `-D warnings` | Passed on the final files                                                                       |
| Five-package suite                                    | Passed as one command (exit 0), including worker 152/152; core has 1,907 unit tests             |
| Worker fixture retry                                  | 152 passed, 0 failed, 1 process-child test ignored by the parent runner                         |
| Desktop bridge                                        | 24 passed, 0 failed                                                                             |
| Desktop post-publication recovery                     | 1 passed; its ignored child runs inside that test                                               |
| Desktop bundle round trip                             | 1 passed                                                                                        |
| Complete crypto suite, including scanner regression   | Passed, including the plaintext control and atomic-replacement scanner test                     |
| Workspace build                                       | Passed                                                                                          |
| 14.4M benchmark                                       | Passed: complete 14.4M-visit / 2.9M-page run, including conversion, index build and mode switch |

Two concurrent VM runs ended before their commands completed, with the exact
runner error `modal-offload: the Sandbox stopped during the command (exit 0): see the Sandbox's logs in the Modal dashboard`.
The gate Sandbox was `sb-01M49ZB03XP3YFH36FV59YYRCZ`; the benchmark Sandbox was
`sb-01M49WD0WQ11W7V1WJC1AB655B`. The available watchdog output did not identify
why they ended. No timing/RSS result from either incomplete run is used, and
all earlier gVisor measurements are excluded.

## VM benchmark results

Measured on a 4-vCPU / 8-GiB Linux 7.2.9 microVM, Intel Xeon Platinum 8375C
at 2.90 GHz. Input: 14,400,000 encrypted SQLCipher visits and 2,900,000 distinct
pages. Fixture generation and compilation are excluded; all generated files are
under Sandbox `/tmp`.

| Operation                                                              |  Seconds | Process memory observation                  |
| ---------------------------------------------------------------------- | -------: | ------------------------------------------- |
| Conversion before indexing (copy, normalized proofs, documents/outbox) | 6612.523 | 277,144 KiB sampled `/proc/self/statm` peak |
| Following Tantivy rebuild                                              |   81.048 | 757,552 KiB sampled `/proc/self/statm` peak |
| Publication after the index                                            |    0.103 | Included in the whole-process peak below    |
| Complete conversion, including index and publication                   | 6693.674 | 755,508 KiB kernel `VmHWM`                  |
| Encrypted → plaintext mode switch                                      |  602.572 | 1,073,908 KiB kernel `VmHWM`                |

The phase samples and kernel high-water reading use different Linux RSS
accounting interfaces; they are recorded separately rather than combined into
one measurement. The complete ignored benchmark passed with exit 0; its fixture
was removed from `/tmp` on exit. The mode-switch timer includes verified database
copying, the rebuilt index, the safety snapshot and publication.

## Canonical collisions and restart behavior

Enrichment jobs and refind rows merge in Scratch before publication. Scratch
uses the target archive's encryption mode; encrypted conversions keep these
proofs encrypted.
The smallest source row ID anchors each merged row. Content comes from the
newest timestamp (RFC3339 offsets compare as instants); equal timestamps use the
larger source ID. Job attempts and refind counters/scores are added, JSON counters
are added, and JSON arrays are unioned in deterministic order. Non-counter scalar
values—including IDs and explicit nulls—come from the newest content. Job priority
keeps the most urgent value and creation time keeps the earliest instant. Refind
keeps the earliest/last observed time bounds. Every trigger survives with its
job ID remapped to the merged anchor; retired AUTOINCREMENT IDs remain retired.

Each attempt reconstructs the expected aggregates from the immutable originals,
then replaces only those staged tables in bounded pages and compares their typed
count/digest proofs. Cancellation or a process exit cannot double the counters on
resume. Tests use two `%7E` / `~` variants, timezone-offset timestamps, counters,
sets, source profiles and both trigger references in both storage modes.

Session startup, password/keychain unlock, desktop assessment and initialization
recover a pending installation before interpreting mode, envelope or legacy-file
presence. Recovery retains the owner's reservation. Existing native bootstrap
uses the stored generation directly; configuration changes remain on the Settings
save path. Real child processes exit during row conversion, the first install or
preserve rename, and after publication. Restart either resumes conversion, rolls
back to the old native generation, or finishes the published generation.

## Existing boundaries and replacement map

`archive/maintenance.rs` mixes retention, rekey, snapshot restore, three recovery
protocols and their tests (3,386 lines). `archive/at_rest.rs` mixes SQLite rewrites,
header inspection, snapshot discovery and launch recovery (2,693 lines).
`migration.rs` mixes bundle format, ZIP I/O, file installation, rollback and tests
(3,086 lines). Before this port, the public worker wrappers refused these operations.

The port separates database copying/checkpoint admission in vault-store, file
installation/recovery in vault-core, and SQLCipher decoding in
vault-convert-sqlcipher. Password changes use vault-crypto's atomic envelope
replacement. Search rebuilding stays with the existing owner indexer. Retention,
snapshot inventory and bundle path/checksum validation retain their contracts.

## Required failure coverage

- Rewrap: wrong password, accidental data rewrite or data-key rotation, loss of
  both envelopes at an interrupted replacement, keychain auto-unlock regression.
- Database copy: missing rows or sequence high-water marks, changed NULL/blob/real
  values, plaintext encrypted output, skipped tables, unbounded row collection,
  ignored cancellation, attachment left behind after an error.
- Snapshots: uncheckpointed data, a write during file copying, inconsistent
  evidence/archive references, queue not resumed after a failed copy.
- Installation: original removed before verification, mixed generations after a
  crash, stale WAL replay, configuration/envelope disagreement, owner lock replaced.
- Bundles: incorrect password, omitted agent/evidence rows, derived files leaked,
  tampered manifest or entries, unsafe paths, overwrite without confirmation,
  interruption at each journal boundary.
- Conversion: stored legacy derived keys reused, percent-escaped URL annotations
  detached from pages, duplicate rows after resume, missing search documents,
  mismatched counts/digests allowed to install, legacy files removed.

## Coverage boundary

Existing desktop bridge tests cover worker command serialization and the bundle
round trip. The restored worker bundle tests exercise export, preview and import
into a fresh real root. Engine and core acceptance tests use real temporary
plaintext/encrypted stores and generated SQLCipher fixtures. Frontend E2E belongs
to area 6 and is unchanged here. All executable validation runs on Modal. A populated 20,000-page lookup diagnostic
confirmed that IN lists of 1, 64 and 512 URLs use the covering URL index with SEARCH.

## Native maintenance decisions

- Encrypted ATTACH is supported by the pinned 0.8.1 probes. Copies use it in
  4,096-row transactions on the Batch lane, including explicit destination keys,
  typed SHA-256 proofs and sequence high-water marks. No VACUUM INTO or TEMP tables.
- Store admission drains at a committed batch boundary. The same owner-lock inode
  spans closing files and reopening the installed generation. A failed/cancelled
  copy has not touched live files; prepared installations roll back, published
  installations finish cleanup.
- Password changes authenticate the current password and atomically rewrap the
  envelope. They do not rotate data keys, write a database run, rebuild search or
  change keychain entries. Previously saved snapshots retain their own envelopes.
- Mode switches retain a verified safety snapshot in the destination mode before
  publication. This avoids creating a new plaintext safety copy when enabling
  encryption. Historical snapshots and retained SQLCipher inputs keep their own
  mode/password; conversion never deletes those originals.
- Bundle format 2 contains encrypted archive, evidence and agent databases. Its
  six payload entries also include the envelope, encrypted config and an encrypted
  streaming provenance TAR. Audit, raw snapshots and retained sidecars travel with
  their database references; derived search/intelligence, models, credentials,
  schedules and intelligence body blobs are rebuilt or reconfigured locally. The
  destination retains its own App Lock policy alongside its local secret; a fresh
  root starts without App Lock and can configure it locally.
- Immutable audit files retain their bytes and hashes. Imported root aliases let
  locked review resolve their old paths; mutable database artifact locators are
  rebased in bounded pages. Browser source paths and history URLs are unchanged.
- Conversion retains source rowids and resumable batch cursors. URL identities,
  favorites, annotation keys, enrichment job keys and embedding content hashes
  use current vault-core rules. Canonical star collisions retain the newest star
  timestamp and available source profile, and are verified against a normalized
  on-disk proof in Scratch.
- SQLCipher remains a library in the application while the converter ships.
  Workspace rusqlite uses `bundled-sqlcipher-vendored-openssl`; browser databases
  are opened without a key. It returns to `bundled` when the converter is removed.
- Old archive-only SQLCipher safety snapshots are converted before native restore.
  The report identifies reconstructed empty source evidence; those old snapshots
  cannot supply evidence or agent data they never contained.

## Command contract for the upgrade and maintenance screens

Types below describe serialized camelCase payloads. `ArchiveMode` remains
`"Plaintext" | "Encrypted"`. No root `src/` or `tests/e2e/` changes are included.

### Upgrade

`assess_archive_upgrade({ databaseKey?: string | null })` adds the optional typed
SQLCipher password. An encrypted source needs that password to count its tables;
missing/wrong passwords return `archive-password-required` / `archive-password-wrong`.
After startup journal recovery, the assessment reads metadata/counts and never
creates a native destination or starts conversion.

```ts
type UpgradeAssessment = {
  pending: boolean
  currentSchemaVersion: number // 0 before conversion, 1 when native exists
  targetSchemaVersion: 1
  phases: {
    phase: 'storageConversion' | 'contentVerification' | 'searchReprojection'
    phaseLabel: string
    pending: boolean
    streamed: boolean
    estimatedTotal: number // retained field name; row/document units, not duration
  }[]
  conversion: {
    required: boolean
    sourceEncrypted: boolean
    sourceFormat: 'sqlcipher'
    targetFormat: 'turso'
    sourceBytes: number
    requiredFreeBytes: number
    filesTouched: string[]
    filesRetained: string[]
    tables: { database: string; table: string; rows: number }[]
    resumable: boolean
  } | null
}
```

`requiredFreeBytes` is a staging/search reserve of three times the source database and existing WAL/SHM
bytes, not a guaranteed upper bound for arbitrary content. Retained files are
explicitly listed and are never removed by conversion. Source counts include
legacy tables; native facts and normalized-key proofs are verified before swapping.

`initialize_archive({ config, databaseKey?: string | null }) -> AppSnapshot` keeps
its shape and starts/resumes conversion before publishing the native session. An
existing native archive opens directly with its stored configuration, after
pending-journal recovery; the retained SQLCipher inputs are not reopened.
The existing `pathkeep://archive-upgrade` event adds optional `database` and `table`:

```ts
type UpgradeProgress = {
  phase:
    | 'storageConversion'
    | 'contentVerification'
    | 'searchReprojection'
    | 'finalizing'
    | 'schemaMigration'
    | 'registrableDomainBackfill'
    | 'intelligence'
  phaseLabel: string
  database?: string
  table?: string
  processed: number
  total: number
  done: boolean
}
```

Counters reset for each database/table. `total: 0` is indeterminate. Verification
counts both source and destination rows. Finalizing with `done: true` is the only
successful terminal upgrade event; cancellation/failure returns the command error.

### Password and mode

The request and return types are unchanged:

```ts
type RekeyRequest = {
  newMode: 'Plaintext' | 'Encrypted'
  newKey: string | null
  currentKey?: string | null
}
// preview_rekey_archive({ request }) -> RekeyPreview
// rekey_archive({ request }) -> AppSnapshot
```

For encrypted-to-encrypted password changes, preview `snapshotPath` and
`tempDatabasePath` are empty strings; there is one envelope-rewrap step and no
`same-mode-rewrite` warning. Mode changes preview the staged destination and the
new-mode safety snapshot. `currentKey` remains required for an encrypted source;
wrong current passwords remain `archive-password-wrong`.

`reconcile_archive_encryption({}) -> { repaired, fromMode, toMode }` retains its
shape and repairs stale native config after Store's authenticated preflight.
Changing `archiveMode` through ordinary configuration save/initialization returns
`encryption-mode-change-requires-rekey` so callers use the preview/execute flow.

### Bundle

`export_app_data({ targetPath, exportPassword }) -> ExportedBundle` adds the
mandatory export password. Export always creates a fresh encrypted bundle,
including when the source Vault is plaintext or keychain-unlocked. It refuses an
output path that would overwrite managed app data (`export-target-is-managed`).

`preview_app_data_import({ bundlePath }) -> ImportPreview` retains its fields:
`manifest`, `schemaUpToDate`, `migrationsToApply`, `bytesToExtract`,
`exclusionNotes: { path, reason }[]`, `willOverwriteExisting`.

`apply_app_data_import({ bundlePath, options: { confirmOverwrite,
sourceArchiveKey } }) -> ImportResult` retains its shape. `sourceArchiveKey` now
means the **export password**, independent of the source archive password.
Missing/wrong keys retain the `source_archive_key required` /
`source_archive_key invalid` prefixes. Native bundles have `formatVersion: 2`,
`archiveSchemaVersion: 1`, `archiveMode: "encrypted"`, and six payload entries.
`migrationsApplied` is empty and `finalSchemaVersion` is 1. The prior generation
is retained under `maintenance.bak-*` when importing over an existing root.

### Progress, cancellation and restoration

Two new commands are independent of SQL admission:

```ts
// get_maintenance_progress({}) -> MaintenanceProgress | null
// cancel_maintenance({}) -> boolean // whether an active operation was signalled
type MaintenanceProgress = {
  phase: string
  database: string | null
  table: string | null
  processed: number
  total: number
}
```

Phase values are `rewrapping`, `copying`, `verifying`, `indexing`, `snapshotting`,
`exporting`, `importing`, `restoring`, `pruning`, `merging`, `converting`, `projecting`,
`publishing`, then `completed`, `cancelled` or `failed`. Table counters are local
to the named table, not an overall percentage. Import/restore currently report
an indeterminate phase while verifying and rebuilding. Cancellation is checked
between pages or I/O chunks; Turso cannot interrupt one executing index DDL.

`run_full_archive_restore({ request: { snapshotPath }, key?: string | null })`
retains `FullArchiveRestoreReport`: `runId`, `restoredSnapshotPath`, `restoredMode`,
`quarantineDir`, `sourceEvidenceRebuilt`, `warnings`. Native snapshot paths are
directories and preserve all four databases; old `.sqlite` snapshots use the
converter and report `sourceEvidenceRebuilt: true`. `quarantineDir` points at the
retained previous native generation. Browser checkpoint preview/replay, snapshot
inventory, retention and wipe keep their existing command shapes.

If publication succeeds but storing/clearing the new keychain entry fails, the
session adopts the installed archive and reports `archive-installed-keychain-unavailable`.
It must not report a rollback or retry the already-published operation blindly.

### Copy for frontend integration

The new phase keys are supplied here because the upgrade screen is owned by the
frontend task. Backend rekey preview sentences already support all three locales.

| Key / phase                                | en                               | zh-CN                | zh-TW                |
| ------------------------------------------ | -------------------------------- | -------------------- | -------------------- |
| `archiveUpgrade.phase.storageConversion`   | Converting archive               | 正在转换存档         | 正在轉換封存         |
| `archiveUpgrade.phase.contentVerification` | Verifying archive contents       | 正在核对存档内容     | 正在核對封存內容     |
| `merging`                                  | Merging matching pages           | 正在合并相同页面     | 正在合併相同頁面     |
| `copying`                                  | Copying databases                | 正在复制数据库       | 正在複製資料庫       |
| `verifying`                                | Verifying copied data            | 正在核对复制的数据   | 正在核對複製的資料   |
| `indexing`                                 | Building search index            | 正在建立搜索索引     | 正在建立搜尋索引     |
| `snapshotting`                             | Saving safety snapshot           | 正在保存安全快照     | 正在儲存安全快照     |
| `rewrapping`                               | Changing archive password        | 正在更改存档密码     | 正在變更封存密碼     |
| `cancelled`                                | Cancelled; original archive kept | 已取消；原存档已保留 | 已取消；原封存已保留 |
| Export password                            | Export password                  | 导出密码             | 匯出密碼             |

## Test inventory

This is the inventory at the time of the conversion work. The 2026-10-12 test cut removed
several of these tests, among them the ignored `conversion_url_lookup_plan` diagnostic; the
test files are the current list.

Restored worker tests:

- `export_app_data_writes_a_bundle_at_the_requested_target`
- `preview_import_reads_a_bundle_written_by_export_app_data`
- `apply_import_restores_an_exported_bundle_onto_a_fresh_project_root`

Ported behavior tests replace the temporary migration refusals for rekey,
full restore/reconciliation, unsafe config-only mode changes and dev-IPC rekey.
The existing desktop `migration_and_browse_insights_impls_cover_local_desktop_flows`
again asserts a real export/preview/import round trip. Legacy SQLCipher regression
fixtures remain. No SQLite/SQLCipher engine tests were deleted.

New tests cover every table's bundle round trip in both source modes, encrypted
provenance replay at an empty destination, missing/altered bundle entries, wrong
passwords, managed-target refusal, prepared/published installation boundaries,
startup rollback for mode/import/restore, orphan staging cleanup, last-snapshot
retention, incomplete snapshot manifests, native wipe with the owner lock retained,
keychain continuity, atomic envelope process kills, all-database mode copies and
search, checkpoint snapshots during multi-batch backup, prepared batch rollback,
SQLCipher conversion parity with area-1 ingestion, old-rule stars/notes, canonical
star collisions, agent/evidence preservation, conversion cancellation and abrupt
process-exit resumption, digest refusal and legacy snapshot restoration.

Converter collision tests also cover offset-bearing timestamps, numeric IDs kept
separate from counters, nested counters/sets, remapped triggers and cancellation
after staged merged rows have been installed. Real restart tests cover conversion
copy, conversion installation, mode preservation and post-publication bootstrap.

Owner startup/CLI fixtures now complete real SQLCipher conversion before testing
scheduled backup, startup waiting, unreadable-keychain reuse and App Lock behavior.
Their original behavioral assertions remain; the production conversion requirement
is unchanged. The unchanged-name ports are:

- `owner_requests::tests::due_only_request_waits_for_startup_session_open`
- `owner_requests::tests::scheduled_backup_uses_the_open_owners_key_when_keyring_is_unavailable`
- `tests::app_snapshot_and_worker_cli_cover_main_local_flows`
- `tests::mcp_surface_respects_legacy_visibility_and_locked_app_sessions`
- `dev_ipc_bridge::dispatch::tests::ui_snapshot_waits_for_startup_open`

Portable crypto tests cover purpose-key separation, wrong-key/name/truncation,
bounded round trips and cancellation. No frontend E2E tests were changed.

### Added, restored or renamed test entry points

These 55 entry points include the three restored bundle tests and four renamed
refusal tests. Four process-child tests are invoked by their parent acceptance
tests; the large benchmark and lookup-plan diagnostic are ignored by default.
Existing assertions also changed in the unchanged-name ports listed above.

- [crates/vault-convert-sqlcipher/src/transfer/stars.rs](../../../src-tauri/crates/vault-convert-sqlcipher/src/transfer/stars.rs):
  - `proof_pages_seek_both_parts_of_the_favorite_key`

- [crates/vault-convert-sqlcipher/tests/benchmark.rs](../../../src-tauri/crates/vault-convert-sqlcipher/tests/benchmark.rs):
  - `conversion_url_lookup_plan`
  - `encrypted_14_4m_conversion_and_mode_switch`

- [crates/vault-convert-sqlcipher/tests/collisions.rs](../../../src-tauri/crates/vault-convert-sqlcipher/tests/collisions.rs):
  - `old_variants_merge_content_counters_sets_and_triggers_even_after_resume`

- [crates/vault-convert-sqlcipher/tests/conversion.rs](../../../src-tauri/crates/vault-convert-sqlcipher/tests/conversion.rs):
  - `a_changed_staged_value_refuses_the_swap`
  - `a_process_exit_between_tables_resumes_committed_rows`
  - `assessment_includes_committed_legacy_wal_bytes_and_retained_sidecars`
  - `conversion_crash_child`
  - `encrypted_conversion_recomputes_keys_preserves_original_and_builds_search`
  - `interrupted_conversion_resumes_without_duplicate_visits`
  - `stars_that_now_share_a_canonical_key_keep_the_latest_star_time`

- [crates/vault-convert-sqlcipher/tests/ingest_parity.rs](../../../src-tauri/crates/vault-convert-sqlcipher/tests/ingest_parity.rs):
  - `conversion_matches_native_ingestion_for_the_same_browser_facts`

- [crates/vault-convert-sqlcipher/tests/snapshot.rs](../../../src-tauri/crates/vault-convert-sqlcipher/tests/snapshot.rs):
  - `legacy_full_snapshot_restores_to_turso_in_both_modes`

- [crates/vault-core/src/archive/ingest/concurrency_tests.rs](../../../src-tauri/crates/vault-core/src/archive/ingest/concurrency_tests.rs):
  - `safety_snapshot_during_backup_restores_a_consistent_committed_prefix`

- [crates/vault-core/src/maintenance/install_tests.rs](../../../src-tauri/crates/vault-core/src/maintenance/install_tests.rs):
  - `a_published_generation_finishes_cleanup_without_restoring_old_files`
  - `every_interrupted_swap_rolls_back`

- [crates/vault-core/src/maintenance/rewrap_crash_tests.rs](../../../src-tauri/crates/vault-core/src/maintenance/rewrap_crash_tests.rs):
  - `rewrap_child`
  - `rewrap_survives_a_process_kill_at_every_envelope_boundary`

- [crates/vault-core/src/maintenance/tests.rs](../../../src-tauri/crates/vault-core/src/maintenance/tests.rs):
  - `closing_a_finished_staged_index_does_not_cancel_the_maintenance_operation`
  - `incomplete_snapshot_manifest_cannot_create_empty_replacement_databases`
  - `mode_switch_round_trips_all_databases_and_cancel_keeps_original`
  - `password_change_only_rewraps_and_the_remembered_data_key_still_opens`
  - `retention_keeps_a_native_safety_snapshot_and_updates_its_native_ledger`
  - `snapshot_restores_evidence_and_agent_alongside_the_archive`
  - `startup_discards_unpublished_copy_files_but_keeps_conversion_resume_state`
  - `startup_rolls_back_a_mode_switch_interrupted_after_the_first_install`
  - `startup_rolls_back_interrupted_bundle_import_and_snapshot_restore`
  - `wipe_removes_native_databases_index_and_envelope_without_replacing_owner_lock`

- [crates/vault-core/src/migration/native_tests.rs](../../../src-tauri/crates/vault-core/src/migration/native_tests.rs):
  - `export_cannot_replace_the_live_archive_or_its_configuration`
  - `export_then_import_into_an_empty_root_round_trips_native_tables`
  - `native_preview_rejects_an_inventory_missing_a_database`
  - `portable_import_keeps_app_lock_policy_with_its_local_secret`
  - `portable_provenance_is_encrypted_relocated_and_replayable_on_an_empty_root`
  - `wrong_export_password_and_changed_database_bytes_preserve_the_target`

- [crates/vault-crypto/tests/portable.rs](../../../src-tauri/crates/vault-crypto/tests/portable.rs):
  - `artifacts_round_trip_without_plaintext_and_authenticate_their_names`
  - `cancelled_artifact_has_no_readable_complete_payload`

- [crates/vault-crypto/tests/watcher.rs](../../../src-tauri/crates/vault-crypto/tests/watcher.rs):
  - `an_atomic_replacement_is_scanned_alongside_the_retained_original`

- [crates/vault-store/tests/maintenance.rs](../../../src-tauri/crates/vault-store/tests/maintenance.rs):
  - `attached_copy_preserves_values_and_deleted_sequence_high_water_in_both_modes`
  - `cancelled_copy_releases_attachment_and_leaves_source_usable`
  - `pause_checkpoint_copy_and_resume_land_between_write_batches`
  - `repeated_insert_rebinds_nulls_and_rolls_back_the_whole_failed_batch`

- [crates/vault-worker/src/maintenance_jobs.rs](../../../src-tauri/crates/vault-worker/src/maintenance_jobs.rs):
  - `cancel_reaches_the_running_operation_and_preserves_a_terminal_state`

- [crates/vault-worker/src/migration.rs](../../../src-tauri/crates/vault-worker/src/migration.rs):
  - `apply_import_restores_an_exported_bundle_onto_a_fresh_project_root`
  - `export_app_data_writes_a_bundle_at_the_requested_target`
  - `preview_import_reads_a_bundle_written_by_export_app_data`

- [crates/vault-worker/src/session/recovery_tests.rs](../../../src-tauri/crates/vault-worker/src/session/recovery_tests.rs):
  - `conversion_restart_child`
  - `restart_recovers_before_conversion_and_keychain_decisions`

- [crates/vault-worker/src/session/tests.rs](../../../src-tauri/crates/vault-worker/src/session/tests.rs):
  - `legacy_storage_waits_for_explicit_conversion_before_session_open`

- [crates/vault-worker/src/tests.rs](../../../src-tauri/crates/vault-worker/src/tests.rs):
  - `full_restore_and_reconciliation_use_the_native_owner`
  - `mode_changes_require_the_rekey_command_instead_of_a_config_edit`
  - `rekey_requires_the_current_password_and_rewraps_the_native_envelope`
  - `remembered_data_key_auto_unlocks_after_password_rewrap`

- [src/dev_ipc_bridge/dispatch/tests.rs](../../../src-tauri/src/dev_ipc_bridge/dispatch/tests.rs):
  - `dispatch_rekey_requires_the_current_password_and_keeps_keychain_unlock`

- [src/worker_bridge/recovery_tests.rs](../../../src-tauri/src/worker_bridge/recovery_tests.rs):
  - `assessment_and_bootstrap_finish_a_published_mode_switch_after_process_exit`
  - `published_mode_child`

## Module failure-mode map

Each implementation/test module carries its own Responsibilities / Not responsible
for / Failure modes header. The maintenance-specific groups are:

| Module                                           | Failures guarded                                                                                                                                                                                                    |
| ------------------------------------------------ | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| vault-store `maintenance`, `copy`, `session`     | Uncheckpointed files; cross-batch writes during copying; lost rows/NULL/blob/real values or sequence high-water marks; ignored cancellation; leaked attachment; partial batch commit; unbounded statement retention |
| vault-core `maintenance/mod`                     | Wrong-password mutation; data-key rotation on rewrap; omitted databases; early publication; search opened with the old mode/key                                                                                     |
| `maintenance/install`                            | Mixed generations; missing originals; stale WAL; non-idempotent rollback; config published ahead of files; replaced owner lock                                                                                      |
| `maintenance/snapshot`                           | Inconsistent archive/evidence snapshot; missing envelope or manifest inventory; corrupted bytes; replacing a live archive before verification                                                                       |
| `maintenance/audit`, `retention`                 | Lost locked review; treating audit failure as rollback; pruning the last recovery point; deleting active backup staging; stale snapshot ledger/totals                                                               |
| `migration/native`, `portable_artifacts`         | Wrong export key; missing agent/evidence/provenance; plaintext config/TAR; derived leakage; tampered entries; overwrite without confirmation; stale source-machine paths; cancelled copy publication                |
| vault-crypto `portable`                          | Cross-purpose key reuse; unauthenticated file names/truncation; plaintext temporary files/cache; unbounded reads; missed cancellation                                                                               |
| vault-core `conversion`, `conversion/enrichment` | Different identity rules from ingest; missing documents/outbox; stale counters; missing canonical variants; corpus-sized enrichment fan-out                                                                         |
| converter `source`, `transfer`, `transfer/stars` | Legacy writes; lost rowids; copied old derived strings; digest mismatch accepted; canonical collisions losing newest favorite time                                                                                  |
| converter `collisions`                           | Lost newest content, counters or sets; dangling triggers; repeated attempts double counters; normalized count/digest proof omitted                                                                                  |
| converter `lib`, `snapshot`                      | Incomplete preview; duplicate crash-resumed rows; early swap; deleted SQLCipher originals; wrong-password/corrupt legacy snapshot installed                                                                         |
| worker `maintenance_jobs`, session adapters      | Overlapping maintenance replaces progress; cancellation queued behind SQL; false successful terminal state; stale session/keychain after installation                                                               |

The extra crypto control-index run exposed a test-scanner defect: `Watcher` kept
only the first hard link for a pathname. An atomic replacement of `meta.json`
therefore left the scanner inspecting the initial empty metadata, causing the
plaintext control to miss `pk-payload-sentinel`. The scanner now inspects both the
retained link and final live path. `tests/watcher.rs` reproduces that replacement;
the existing control-index assertions are unchanged.

## Reproducing validation

Run through `~/.agents/skills/modal-offload/offload run -- '<command>'` from this
checkout. Gate output stays in Sandbox `/tmp` logs and is summarized there; it
is not copied into the repository. `.modal-offload.toml` is a local, uncommitted
configuration. The gates use a non-root Linux microVM and four concurrent Rust
tests. No macOS Keychain or Windows runtime behavior is claimed by that run.

The ignored benchmark uses a separate 4-vCPU, 8-GiB microVM, release code and a
fresh encrypted SQLCipher fixture under `/tmp`. It has 14,400,000 visits and
2,900,000 distinct URL strings. Setup is excluded from timings. Conversion
includes normalization, per-table verification and document/outbox projection;
the following Tantivy build is reported separately. Mode-switch time includes
copy verification, search rebuild, its safety snapshot and installation. Whole-process RSS uses Linux VM `VmHWM`. Conversion and mode switching run in
separate child processes. Within the conversion child, pre-index and index-phase
RSS are sampled from `/proc/self/statm` every millisecond; those values are sampled
process peaks for each phase. Total conversion and mode-switch peaks are kernel
high-water values. None includes total machine memory. Mode-switch
RSS includes opening the source archive; its timer starts after that open.

```sh
cd src-tauri
cargo fmt --all --check
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo test -p vault-core -p vault-worker -p vault-store -p vault-search -p vault-convert-sqlcipher
cd ..
bun run test:desktop-bridge:rust
cd src-tauri
cargo build --workspace
cargo test -p vault-crypto
cargo test --release -p vault-convert-sqlcipher --test benchmark \
  encrypted_14_4m_conversion_and_mode_switch -- --ignored --nocapture --test-threads=1
```

The target-sized benchmark is launched with explicit Sandbox resources (not CPU
pinning inside a larger Sandbox):

```sh
~/.agents/skills/modal-offload/offload run --cpu 4 --memory-gib 8 --timeout-min 720 -- \
  'cd src-tauri && CARGO_BUILD_JOBS=2 cargo test --release -p vault-convert-sqlcipher --test benchmark encrypted_14_4m_conversion_and_mode_switch -- --ignored --nocapture --test-threads=1'
```

## Changed files

- [docs/architecture/data-model.md](../../../docs/architecture/data-model.md)
- [docs/architecture/decisions/012-turso-storage-engine.md](../../../docs/architecture/decisions/012-turso-storage-engine.md)
- [docs/architecture/research/2026-10-turso-maintenance.md](../../../docs/architecture/research/2026-10-turso-maintenance.md)
- [docs/architecture/store-schema.md](../../../docs/architecture/store-schema.md)
- [docs/architecture/turso-migration-design.md](../../../docs/architecture/turso-migration-design.md)
- [docs/features/archive.md](../../../docs/features/archive.md)
- [docs/plan/CHANGELOG.md](../../../docs/plan/CHANGELOG.md)
- [src-tauri/Cargo.lock](../../../src-tauri/Cargo.lock)
- [src-tauri/Cargo.toml](../../../src-tauri/Cargo.toml)
- [src-tauri/crates/vault-convert-sqlcipher/Cargo.toml](../../../src-tauri/crates/vault-convert-sqlcipher/Cargo.toml)
- [src-tauri/crates/vault-convert-sqlcipher/README.md](../../../src-tauri/crates/vault-convert-sqlcipher/README.md)
- [src-tauri/crates/vault-convert-sqlcipher/src/collisions/mod.rs](../../../src-tauri/crates/vault-convert-sqlcipher/src/collisions/mod.rs)
- [src-tauri/crates/vault-convert-sqlcipher/src/collisions/record.rs](../../../src-tauri/crates/vault-convert-sqlcipher/src/collisions/record.rs)
- [src-tauri/crates/vault-convert-sqlcipher/src/lib.rs](../../../src-tauri/crates/vault-convert-sqlcipher/src/lib.rs)
- [src-tauri/crates/vault-convert-sqlcipher/src/snapshot.rs](../../../src-tauri/crates/vault-convert-sqlcipher/src/snapshot.rs)
- [src-tauri/crates/vault-convert-sqlcipher/src/source.rs](../../../src-tauri/crates/vault-convert-sqlcipher/src/source.rs)
- [src-tauri/crates/vault-convert-sqlcipher/src/transfer.rs](../../../src-tauri/crates/vault-convert-sqlcipher/src/transfer.rs)
- [src-tauri/crates/vault-convert-sqlcipher/src/transfer/stars.rs](../../../src-tauri/crates/vault-convert-sqlcipher/src/transfer/stars.rs)
- [src-tauri/crates/vault-convert-sqlcipher/tests/benchmark.rs](../../../src-tauri/crates/vault-convert-sqlcipher/tests/benchmark.rs)
- [src-tauri/crates/vault-convert-sqlcipher/tests/collisions.rs](../../../src-tauri/crates/vault-convert-sqlcipher/tests/collisions.rs)
- [src-tauri/crates/vault-convert-sqlcipher/tests/conversion.rs](../../../src-tauri/crates/vault-convert-sqlcipher/tests/conversion.rs)
- [src-tauri/crates/vault-convert-sqlcipher/tests/ingest_parity.rs](../../../src-tauri/crates/vault-convert-sqlcipher/tests/ingest_parity.rs)
- [src-tauri/crates/vault-convert-sqlcipher/tests/snapshot.rs](../../../src-tauri/crates/vault-convert-sqlcipher/tests/snapshot.rs)
- [src-tauri/crates/vault-convert-sqlcipher/tests/support/memory.rs](../../../src-tauri/crates/vault-convert-sqlcipher/tests/support/memory.rs)
- [src-tauri/crates/vault-core/Cargo.toml](../../../src-tauri/crates/vault-core/Cargo.toml)
- [src-tauri/crates/vault-core/src/archive/at_rest.rs](../../../src-tauri/crates/vault-core/src/archive/at_rest.rs)
- [src-tauri/crates/vault-core/src/archive/ingest/concurrency_tests.rs](../../../src-tauri/crates/vault-core/src/archive/ingest/concurrency_tests.rs)
- [src-tauri/crates/vault-core/src/archive/ingest/consumer.rs](../../../src-tauri/crates/vault-core/src/archive/ingest/consumer.rs)
- [src-tauri/crates/vault-core/src/archive/maintenance.rs](../../../src-tauri/crates/vault-core/src/archive/maintenance.rs)
- [src-tauri/crates/vault-core/src/archive/mod.rs](../../../src-tauri/crates/vault-core/src/archive/mod.rs)
- [src-tauri/crates/vault-core/src/archive/schema.rs](../../../src-tauri/crates/vault-core/src/archive/schema.rs)
- [src-tauri/crates/vault-core/src/conversion.rs](../../../src-tauri/crates/vault-core/src/conversion.rs)
- [src-tauri/crates/vault-core/src/conversion/enrichment.rs](../../../src-tauri/crates/vault-core/src/conversion/enrichment.rs)
- [src-tauri/crates/vault-core/src/fault_inject.rs](../../../src-tauri/crates/vault-core/src/fault_inject.rs)
- [src-tauri/crates/vault-core/src/lib.rs](../../../src-tauri/crates/vault-core/src/lib.rs)
- [src-tauri/crates/vault-core/src/maintenance/audit.rs](../../../src-tauri/crates/vault-core/src/maintenance/audit.rs)
- [src-tauri/crates/vault-core/src/maintenance/install.rs](../../../src-tauri/crates/vault-core/src/maintenance/install.rs)
- [src-tauri/crates/vault-core/src/maintenance/install_tests.rs](../../../src-tauri/crates/vault-core/src/maintenance/install_tests.rs)
- [src-tauri/crates/vault-core/src/maintenance/mod.rs](../../../src-tauri/crates/vault-core/src/maintenance/mod.rs)
- [src-tauri/crates/vault-core/src/maintenance/retention.rs](../../../src-tauri/crates/vault-core/src/maintenance/retention.rs)
- [src-tauri/crates/vault-core/src/maintenance/rewrap_crash_tests.rs](../../../src-tauri/crates/vault-core/src/maintenance/rewrap_crash_tests.rs)
- [src-tauri/crates/vault-core/src/maintenance/snapshot.rs](../../../src-tauri/crates/vault-core/src/maintenance/snapshot.rs)
- [src-tauri/crates/vault-core/src/maintenance/tests.rs](../../../src-tauri/crates/vault-core/src/maintenance/tests.rs)
- [src-tauri/crates/vault-core/src/migration.rs](../../../src-tauri/crates/vault-core/src/migration.rs)
- [src-tauri/crates/vault-core/src/migration/native.rs](../../../src-tauri/crates/vault-core/src/migration/native.rs)
- [src-tauri/crates/vault-core/src/migration/native_tests.rs](../../../src-tauri/crates/vault-core/src/migration/native_tests.rs)
- [src-tauri/crates/vault-core/src/migration/portable_artifacts.rs](../../../src-tauri/crates/vault-core/src/migration/portable_artifacts.rs)
- [src-tauri/crates/vault-core/src/models/archive.rs](../../../src-tauri/crates/vault-core/src/models/archive.rs)
- [src-tauri/crates/vault-core/src/search/indexer.rs](../../../src-tauri/crates/vault-core/src/search/indexer.rs)
- [src-tauri/crates/vault-core/src/store_schema/mod.rs](../../../src-tauri/crates/vault-core/src/store_schema/mod.rs)
- [src-tauri/crates/vault-core/src/store_schema/tests/mod.rs](../../../src-tauri/crates/vault-core/src/store_schema/tests/mod.rs)
- [src-tauri/crates/vault-core/src/vault.rs](../../../src-tauri/crates/vault-core/src/vault.rs)
- [src-tauri/crates/vault-core/src/vault/password.rs](../../../src-tauri/crates/vault-core/src/vault/password.rs)
- [src-tauri/crates/vault-crypto/README.md](../../../src-tauri/crates/vault-crypto/README.md)
- [src-tauri/crates/vault-crypto/src/keys.rs](../../../src-tauri/crates/vault-crypto/src/keys.rs)
- [src-tauri/crates/vault-crypto/src/lib.rs](../../../src-tauri/crates/vault-crypto/src/lib.rs)
- [src-tauri/crates/vault-crypto/src/portable.rs](../../../src-tauri/crates/vault-crypto/src/portable.rs)
- [src-tauri/crates/vault-crypto/src/search_dir/mod.rs](../../../src-tauri/crates/vault-crypto/src/search_dir/mod.rs)
- [src-tauri/crates/vault-crypto/tests/common/mod.rs](../../../src-tauri/crates/vault-crypto/tests/common/mod.rs)
- [src-tauri/crates/vault-crypto/tests/envelope.rs](../../../src-tauri/crates/vault-crypto/tests/envelope.rs)
- [src-tauri/crates/vault-crypto/tests/portable.rs](../../../src-tauri/crates/vault-crypto/tests/portable.rs)
- [src-tauri/crates/vault-crypto/tests/watcher.rs](../../../src-tauri/crates/vault-crypto/tests/watcher.rs)
- [src-tauri/crates/vault-store/Cargo.toml](../../../src-tauri/crates/vault-store/Cargo.toml)
- [src-tauri/crates/vault-store/README.md](../../../src-tauri/crates/vault-store/README.md)
- [src-tauri/crates/vault-store/src/copy.rs](../../../src-tauri/crates/vault-store/src/copy.rs)
- [src-tauri/crates/vault-store/src/engine.rs](../../../src-tauri/crates/vault-store/src/engine.rs)
- [src-tauri/crates/vault-store/src/lib.rs](../../../src-tauri/crates/vault-store/src/lib.rs)
- [src-tauri/crates/vault-store/src/maintenance.rs](../../../src-tauri/crates/vault-store/src/maintenance.rs)
- [src-tauri/crates/vault-store/src/session.rs](../../../src-tauri/crates/vault-store/src/session.rs)
- [src-tauri/crates/vault-store/src/worker.rs](../../../src-tauri/crates/vault-store/src/worker.rs)
- [src-tauri/crates/vault-store/tests/maintenance.rs](../../../src-tauri/crates/vault-store/tests/maintenance.rs)
- [src-tauri/crates/vault-worker/Cargo.toml](../../../src-tauri/crates/vault-worker/Cargo.toml)
- [src-tauri/crates/vault-worker/src/app.rs](../../../src-tauri/crates/vault-worker/src/app.rs)
- [src-tauri/crates/vault-worker/src/archive_flows.rs](../../../src-tauri/crates/vault-worker/src/archive_flows.rs)
- [src-tauri/crates/vault-worker/src/current_password.rs](../../../src-tauri/crates/vault-worker/src/current_password.rs)
- [src-tauri/crates/vault-worker/src/lib.rs](../../../src-tauri/crates/vault-worker/src/lib.rs)
- [src-tauri/crates/vault-worker/src/maintenance_jobs.rs](../../../src-tauri/crates/vault-worker/src/maintenance_jobs.rs)
- [src-tauri/crates/vault-worker/src/migration.rs](../../../src-tauri/crates/vault-worker/src/migration.rs)
- [src-tauri/crates/vault-worker/src/owner_requests/tests.rs](../../../src-tauri/crates/vault-worker/src/owner_requests/tests.rs)
- [src-tauri/crates/vault-worker/src/security.rs](../../../src-tauri/crates/vault-worker/src/security.rs)
- [src-tauri/crates/vault-worker/src/session.rs](../../../src-tauri/crates/vault-worker/src/session.rs)
- [src-tauri/crates/vault-worker/src/session/recovery_tests.rs](../../../src-tauri/crates/vault-worker/src/session/recovery_tests.rs)
- [src-tauri/crates/vault-worker/src/session/tests.rs](../../../src-tauri/crates/vault-worker/src/session/tests.rs)
- [src-tauri/crates/vault-worker/src/tests.rs](../../../src-tauri/crates/vault-worker/src/tests.rs)
- [src-tauri/src/command_error.rs](../../../src-tauri/src/command_error.rs)
- [src-tauri/src/commands/archive.rs](../../../src-tauri/src/commands/archive.rs)
- [src-tauri/src/commands/migration.rs](../../../src-tauri/src/commands/migration.rs)
- [src-tauri/src/dev_ipc_bridge/dispatch.rs](../../../src-tauri/src/dev_ipc_bridge/dispatch.rs)
- [src-tauri/src/dev_ipc_bridge/dispatch/tests.rs](../../../src-tauri/src/dev_ipc_bridge/dispatch/tests.rs)
- [src-tauri/src/dev_ipc_bridge/payloads.rs](../../../src-tauri/src/dev_ipc_bridge/payloads.rs)
- [src-tauri/src/lib.rs](../../../src-tauri/src/lib.rs)
- [src-tauri/src/worker_bridge/archive.rs](../../../src-tauri/src/worker_bridge/archive.rs)
- [src-tauri/src/worker_bridge/migration.rs](../../../src-tauri/src/worker_bridge/migration.rs)
- [src-tauri/src/worker_bridge/mod.rs](../../../src-tauri/src/worker_bridge/mod.rs)
- [src-tauri/src/worker_bridge/recovery_tests.rs](../../../src-tauri/src/worker_bridge/recovery_tests.rs)
