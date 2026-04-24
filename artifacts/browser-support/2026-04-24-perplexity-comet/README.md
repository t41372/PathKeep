# Perplexity Comet Browser Direct Validation

Date: 2026-04-24

Scope: one local Perplexity Comet macOS browser-history profile under `~/Library/Application Support/Comet/<profile>`.

Privacy boundary: this artifact intentionally excludes raw URLs, titles, profile directory names, account identifiers, and source filesystem paths. It records only schema coverage, aggregate counts, time ranges, and import-batch outcomes.

## Source Shape

- Browser product: `Perplexity Comet`
- Browser family: `chromium`
- Profile id shape: `comet:<redacted-profile-dir>`
- History file: `History`
- History file size: `983040` bytes
- Favicons sidecar: present
- SQLite sidecars observed during validation: `History-journal`, `Favicons-journal`

## History Schema Coverage

- Table count: `19`
- Tables observed:
  - `cluster_keywords`
  - `cluster_visit_duplicates`
  - `clusters`
  - `clusters_and_visits`
  - `content_annotations`
  - `context_annotations`
  - `downloads`
  - `downloads_slices`
  - `downloads_url_chains`
  - `history_sync_metadata`
  - `keyword_search_terms`
  - `meta`
  - `segment_usage`
  - `segments`
  - `sqlite_sequence`
  - `urls`
  - `visit_source`
  - `visited_links`
  - `visits`

Key column coverage:

| Table                  | Columns                                                                                                                                                                                                                                                                                                                                                                                                        |
| ---------------------- | -------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `urls`                 | `id`, `url`, `title`, `visit_count`, `typed_count`, `last_visit_time`, `hidden`                                                                                                                                                                                                                                                                                                                                |
| `visits`               | `id`, `url`, `visit_time`, `from_visit`, `external_referrer_url`, `transition`, `segment_id`, `visit_duration`, `incremented_omnibox_typed_score`, `opener_visit`, `originator_cache_guid`, `originator_visit_id`, `originator_from_visit`, `originator_opener_visit`, `is_known_to_sync`, `consider_for_ntp_most_visited`, `visited_link_id`, `app_id`                                                        |
| `visit_source`         | `id`, `source`                                                                                                                                                                                                                                                                                                                                                                                                 |
| `keyword_search_terms` | `keyword_id`, `url_id`, `term`, `normalized_term`                                                                                                                                                                                                                                                                                                                                                              |
| `downloads`            | `id`, `guid`, `current_path`, `target_path`, `start_time`, `received_bytes`, `total_bytes`, `state`, `danger_type`, `interrupt_reason`, `hash`, `end_time`, `opened`, `last_access_time`, `transient`, `referrer`, `site_url`, `embedder_download_data`, `tab_url`, `tab_referrer_url`, `http_method`, `by_ext_id`, `by_ext_name`, `by_web_app_id`, `etag`, `last_modified`, `mime_type`, `original_mime_type` |
| `downloads_url_chains` | `id`, `chain_index`, `url`                                                                                                                                                                                                                                                                                                                                                                                     |
| `segments`             | `id`, `name`, `url_id`                                                                                                                                                                                                                                                                                                                                                                                         |
| `segment_usage`        | `id`, `segment_id`, `time_slot`, `visit_count`                                                                                                                                                                                                                                                                                                                                                                 |
| `content_annotations`  | `visit_id`, `visibility_score`, `floc_protected_score`, `categories`, `page_topics_model_version`, `annotation_flags`, `entities`, `related_searches`, `search_normalized_url`, `search_terms`, `alternative_title`, `page_language`, `password_state`, `has_url_keyed_image`                                                                                                                                  |
| `context_annotations`  | `visit_id`, `context_annotation_flags`, `duration_since_last_visit`, `page_end_reason`, `total_foreground_duration`, `browser_type`, `window_id`, `tab_id`, `task_id`, `root_task_id`, `parent_task_id`, `response_code`                                                                                                                                                                                       |
| `clusters`             | `cluster_id`, `should_show_on_prominent_ui_surfaces`, `label`, `raw_label`, `triggerability_calculated`, `originator_cache_guid`, `originator_cluster_id`                                                                                                                                                                                                                                                      |

## Aggregate Counts

| Metric                 | Count |
| ---------------------- | ----: |
| `urls`                 |   279 |
| `visits`               |   587 |
| `keyword_search_terms` |     2 |
| `downloads`            |     2 |
| `downloads_url_chains` |     2 |

Source visit time range:

- Start: `2026-03-26T22:58:42.987+00:00`
- End: `2026-04-23T05:23:02.232+00:00`

## Browser Direct Validation

Validated through the dev IPC bridge against the current plaintext PathKeep archive.

| Step      | Result                                                                                                                                         |
| --------- | ---------------------------------------------------------------------------------------------------------------------------------------------- |
| Preview   | `587` candidate items; recognized kind `chromium-history-db`; preview range `2026-03-26T22:58:42.987+00:00` to `2026-04-23T05:23:02.232+00:00` |
| Import    | Batch `7`; imported `587`; duplicates `0`; visible `587`; status `imported`                                                                    |
| Re-import | Batch `8`; imported `0`; duplicates `587`; visible `0`; status `imported`                                                                      |
| Revert    | batch `7` status `reverted`; visible `0`                                                                                                       |
| Restore   | batch `7` status `imported`; visible `587`                                                                                                     |

Post-import archive checks:

- `source_profiles.browser_family`: `chromium`
- `source_profiles.browser_product`: `Perplexity Comet`
- Source-evidence batches for the Comet profile after import + re-import: `2`
- Source-evidence native entities for the Comet profile: `1740`
- Import-batch audit artifact: present
- Final live archive state: Comet primary import batch restored and visible
