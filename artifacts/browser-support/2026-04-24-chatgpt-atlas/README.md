# ChatGPT Atlas Browser Direct Validation

Date: 2026-04-24

Scope: one local ChatGPT Atlas macOS browser-history profile under `~/Library/Application Support/com.openai.atlas/browser-data/host/<profile>`.

Privacy boundary: this artifact intentionally excludes raw URLs, titles, profile directory names, account identifiers, and source filesystem paths. It records only schema coverage, aggregate counts, time ranges, and import-batch outcomes.

## Source Shape

- Browser product: `ChatGPT Atlas`
- Browser family: `chromium`
- Profile id shape: `atlas:<redacted-profile-dir>`
- History file: `History`
- History file size: `196608` bytes
- Favicons sidecar: present
- SQLite sidecars staged during validation: `History-journal`

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
| `urls`                 |    51 |
| `visits`               |    63 |
| `keyword_search_terms` |     0 |
| `downloads`            |     0 |
| `downloads_url_chains` |     0 |

Source visit time range:

- Start: `2026-04-17T05:44:39.085Z`
- End: `2026-04-24T20:58:52.421Z`

## Browser Direct Validation

Validated through the dev IPC bridge against the current plaintext PathKeep archive.

| Step      | Result                                                                                                                                        |
| --------- | --------------------------------------------------------------------------------------------------------------------------------------------- |
| Preview   | `63` candidate items; recognized kind `chromium-history-db`; preview range `2026-04-17T05:44:39.085+00:00` to `2026-04-24T20:58:52.421+00:00` |
| Import    | Batch `4`; imported `63`; duplicates `0`; visible `63`; status `imported`                                                                     |
| Re-import | imported `0`; duplicates `63`                                                                                                                 |
| Revert    | batch status `reverted`; visible `0`                                                                                                          |
| Restore   | batch status `imported`; visible `63`                                                                                                         |

Post-import archive checks:

- `source_profiles.browser_family`: `chromium`
- `source_profiles.browser_product`: `ChatGPT Atlas`
- Source-evidence batches for the Atlas profile after import + re-import: `2`
- Source-evidence native entities for the Atlas profile: `228`
- Import-batch audit artifact: present
- Final live archive state: Atlas batch restored and visible
