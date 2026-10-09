# vault-core unit-test audit: intelligence, search, storage, enrichment, models, runtime, integration tests

Scope: every `#[test]` and `#[tokio::test]` in `src-tauri/crates/vault-core/` outside `ai/`, `ai_queue`,
`agent_store`, `archive/`, `takeout`, `chrome`, `stars`, `annotations`, `maintenance`, `durable_io.rs`
and `fault_inject.rs`, plus every integration test in `vault-core/tests/` (some of those, such as
`agent_store_paging.rs` and `ai_queue_turso.rs`, sit next to other auditors' areas). That is 591 tests,
including today's uncommitted ones (`visit_taxonomy/tests.rs`, `intelligence/tests/build_needed.rs`, the
tests in `intelligence/intelligence_daily_rollup_state.rs`). Files were read as they are on disk.

Method: read only, nothing built or run. The tests were split into seven slices. For each test we read
the test, the code it calls, and the docs that state the rule. I re-checked every A and A? claim and
the top B claims against the code and docs myself. Paths below are relative to
`src-tauri/crates/vault-core/` unless they start with `docs/`, `src/` (frontend) or `tests/e2e/`.

## Counts

| Category                  | Tests | What it means                                                                                         |
| ------------------------- | ----: | ----------------------------------------------------------------------------------------------------- |
| A, pins wrong behavior    |     5 | The asserted outcome contradicts a doc or common sense, and would fail a correct fix                  |
| A?, probably wrong        |     4 | Same, but a doc conflicts or the behavior can't be reached today                                      |
| B, pins incidental detail |    66 | English strings nobody reads, internal row shapes and counts, hand-copied name lists, magic constants |
| C, mutation-test leftover |   161 | Defaults, serde restatements, branch probes, near-duplicates, tests of test-only code                 |
| D, keep                   |   355 | Rules with a spec, data integrity, crash recovery, races, security, read-plan guards                  |
| Total                     |   591 |                                                                                                       |

About 30 of the D tests should be trimmed: keep the main check and drop one or two incidental
asserts (English text, `cursor.contains(':')`, literal ids, `len() == 8`). The trims are listed in
the Action column.

Per slice:

| Slice                                                          | Tests |   A |  A? |   B |   C |   D |
| -------------------------------------------------------------- | ----: | --: | --: | --: | --: | --: |
| Visit taxonomy and today's tests                               |    28 |   1 |   0 |   4 |   4 |  19 |
| Intelligence, inline tests in `src/intelligence/`              |    84 |   1 |   0 |  23 |  34 |  26 |
| Intelligence, `src/intelligence/tests/` (without build_needed) |    53 |   0 |   0 |   9 |  18 |  26 |
| Intelligence runtime, sections, catalog, `tests/`              |   103 |   1 |   2 |   6 |  19 |  75 |
| Search, store schema, migration, vault                         |   107 |   0 |   1 |   3 |   4 |  99 |
| Enrichment                                                     |    97 |   0 |   1 |   6 |  29 |  61 |
| Models and infrastructure                                      |   119 |   2 |   0 |  15 |  53 |  49 |

Where the keepers are: search (`src/search/**`), vault, migration, and the integration tests in
`tests/` are almost all D. They run scenarios through a real `Vault` in both key modes, check rules
from `docs/architecture/search.md` and `docs/features/recall.md`, and include the read-plan and paging
guards for 14.4M visits. Where the waste is: `models/`, the intelligence inline tests, the
intelligence runtime, enrichment, and the in-crate overview and stage tests.

## Patterns behind most of the B and C

1. **Tests that keep test-only code alive.** Much of the C count comes from tests of code that exists
   only in the test build, so a green test proves nothing about the app:
   - `intelligence/mod.rs:22-46` compiles four whole modules only for tests: `intelligence_core_persist`,
     `intelligence_daily_rollups`, `intelligence_structural_persist`, `intelligence_structural_stage`.
   - The "in-memory vs batched" equivalence tests (`intelligence/tests/batch_equivalence.rs:88`, `:165`)
     compare two `#[cfg(test)]` builders (`intelligence_structural_build.rs:47-203`,
     `intelligence_structural_aggregates.rs:167-384`). Production uses other drivers
     (`build_stages.rs`, `build_refind.rs`). Every change to the refind, path, habit or query-family
     rules has to be made twice, once in production and once in the test copy.
   - The batch-size constants the boundary tests use are `#[cfg(test)]` 8,192 (`intelligence/mod.rs:405-412`).
     Production batches by 4,096 (`intelligence_rebuild.rs:66`).
   - `enqueue_enrichment_job` is `#[cfg(test)]` (`intelligence_runtime/enqueue.rs:31`), so production never
     creates a title-normalization job. The docs say it is on by default (`docs/features/intelligence.md:278-279`).
     Yet seven tests in `enrichment/tests.rs` and `intelligence_runtime/tests_queue.rs` make the feature
     look alive.
   - `drain_one_content_fetch_job_picks_up_a_due_job_when_enabled` (`enrichment/content_fetch_api.rs:311`)
     runs a `#[cfg(test)]` entry with a stub fetcher. In tests the per-redirect SSRF check is allow-all
     (`enrichment/content_fetch.rs:83-86`), which goes against `docs/plan/program/quality-matrix.md:80`.
2. **Tests of commands no screen calls.** `explain_entity`, `get_navigation_path`, `get_hub_pages`,
   embed cards, the widget and public snapshots, `get_compare_set_detail`, the search-rule commands and
   the host-artifact files are registered (`src-tauri/src/lib.rs:293-326`), but nothing under `src/`
   calls them. About a dozen tests pin their internals.
3. **English prose pinned as if it were a contract.** Examples: stale reasons, notes, "noop" text, and
   failure messages. Most never reach a screen, because Settings shows codes. One does reach a screen
   (A2).
4. **Hand-copied lists that only fail with "you forgot to update the list".** These are the table and
   index catalog with Turso-internal names (`store_schema/tests/mod.rs:166`), the clear-preview table
   groups (`intelligence/tests/stage_rebuild.rs:54`, which has already drifted), module counts
   (`len() == 8` in `models/mod.rs`), and overview timing counts (`timings.len() == 11 / 10`).
5. **Serde restatements and version-skew checks for IPC-only types.** The frontend and backend ship in
   one bundle, so "an older payload still deserializes" can't happen for IPC types. This covers 18 tests
   in `models/intelligence.rs` and most of `models/mod.rs`. Compat checks for stored shapes (config.json,
   JSON kept in SQLite) are real, and they are kept.

## A items

### A1. `utils::tests::date_str_to_unix_ms_range_covers_full_day_and_rejects_bad_input` (`src/utils.rs:343`)

- **What it asserts:** `"2026-06-19"` maps to `[1_781_827_200_000, 1_781_913_599_999]`, midnight to
  23:59:59.999 **UTC** (`src/utils.rs:160-166`, `and_utc()`).
- **Who uses it:** its only production caller is `ai/search.rs:393-407` (`resolve_date_filter_ms`). That
  function turns the AI search and agent `startDate` / `endDate` into the `list_history` window.
- **Why it is wrong:** the same tool hands rows back bucketed by the user's local day. The
  `run_code` description (`ai/agent_tools.rs:423`) says `localDate (YYYY-MM-DD in the user's timezone —
GROUP BY THIS for per-day buckets)`. Its own example filters `startDate: "2026-06-15"` to
  `endDate: "2026-06-21"` and then groups by `r.localDate`. `ai/code_mode.rs:536` computes `localDate`
  with `tz_offset`. `docs/architecture/data-model.md:44-47` says data without a timezone uses the user's
  current system timezone. For a user in UTC+8, "June 19" returns 08:00 on the 19th through 07:59 on
  the 20th, so the agent answers "what did I read on June 19" with the wrong day.
- **Right behavior:** a date the user names is a local calendar day. Convert it with the same offset the
  rows are rendered with (`AgentToolContext.tz_offset`).
- **Action:** fix the code. Replace the test with one that passes a non-zero offset and expects local
  midnight to local midnight. The ai-slice fixtures `ai/code_mode/tests.rs:71` and
  `ai/tests_native/fixtures.rs:187` call the helper too.

### A2. `intelligence_sections::tests::module_backed_meta_marks_idle_modules_as_degraded` (`src/intelligence_sections.rs:501`)

The slice rated this A?. I rate it A.

- **What it asserts:** an idle module gives `state = "degraded"` with the exact English `state_reason`
  "No successful deterministic rebuild has been recorded yet."
- **Why it is wrong:** the Insights card renders `meta.stateReason` raw in its "Limited" popover
  (`src/features/insights/parts.tsx:238-239`). There is no code field to translate
  (`intelligence_sections.rs:342-358`). The idle state is reachable: after "Clear Insights data",
  `tests/e2e/background-work.spec.ts:134-137` sees "Not built yet". So a zh-TW or zh-CN user reads an
  English sentence. AGENTS.md principle 5 requires all three languages, and names disabled and empty
  states explicitly. Module stale reasons already moved to codes (`stale_reason_code`, rendered by
  `src/features/settings/background/derived-data-rows.tsx:113`); section meta did not.
- **The doubt, and why it does not hold:** `docs/features/intelligence.md:35` says the card shows
  "部分可用與 `stateReason`". That describes today's behavior. It does not make English acceptable, and
  it needs updating with the fix.
- **Right behavior:** `CoreIntelligenceSectionMeta` carries a `stateReasonCode`, and the frontend
  translates it.
- **Action:** fix the code and update intelligence.md:35. Replace the test with an E2E: "after Clear
  Insights data, an Insights card in zh-TW shows its limited state in zh-TW". `background-work.spec.ts`
  already reaches that state.

### A3. `phase_three::tests::observed_interaction_evidence_merges_metrics_and_context_fallbacks` (`src/intelligence/phase_three.rs:714`)

- **What it asserts:** a Chromium `engagement.visit_duration_ms` metric (7.6, seeded at test line 31)
  comes back as foreground time: `assert_eq!(second.foreground_duration_ms, Some(8))` (line 783).
- **Code:** `phase_three.rs:396-401` folds `engagement.visit_duration_ms` into `foreground_duration_ms`.
  For Chromium the parser stores only `visits.visit_duration` under that key
  (`crates/browser-history-parser/src/chromium/mod.rs:592-600`). That is the time until the tab moved
  on, background time included.
- **Docs it contradicts:** `docs/features/core-intelligence-ultimate-design.md:881-899` (§4.14) names
  `total_foreground_duration` as Chromium's signal and requires the honest label "瀏覽器直接報告的前景停留時間".
  `docs/features/deterministic-intelligence.md:189` removed duration so that no dwell proxy is presented
  as truth.
- **Reach:** the section is still computed in the secondary overview, but no card renders it.
  `docs/design/screens-and-nav.md:69` notes "只有前景時間有值", which is this mislabel at work.
- **Action:** fix the code (take `engagement.visit_duration_ms` out of the foreground arm), or stop
  computing the unused section and delete the test with it.

### A4. `models::core_intelligence::tests::entity_reference_serializes_with_existing_camel_case_tags` (`src/models/core_intelligence/tests.rs:75`)

- **What it asserts:** `InsightEntityReference::RefindPage` serializes as
  `{"kind":"refindPage","canonical_url":...}`, a camelCase tag with a snake_case field.
- **Code:** `models/core_intelligence/shared.rs:147-157` uses `#[serde(tag = "kind", rename_all = "camelCase")]`.
  On an enum that renames variant names only, so `canonical_url`, `family_id`, `session_id`,
  `trail_id` and `compare_set_id` all go out in snake_case.
- **Why it is wrong:** the module header says "Preserve shared serde tags and camelCase field names"
  (`shared.rs:6`). The TypeScript type expects `canonicalUrl`, `familyId` and the rest
  (`src/lib/core-intelligence/types-primitives.ts:110-138`). `models/intelligence.rs:2321` records the same
  bug class as review H-1. The impact is latent: the producers (embed cards, widget, public snapshot)
  have no screen today (`docs/architecture/desktop-command-surface.md:167`).
- **Action:** fix the code (`rename_all_fields = "camelCase"`) and expect `canonicalUrl`.

### A5. `visit_taxonomy::tests::a_host_with_its_own_rule_beats_its_parent`, one assertion (`src/visit_taxonomy/tests.rs:388`, assert at `:399`)

- **What it asserts:** `ads.google.com/home` is a Search site, as the example of "a host without a rule
  of its own takes its parent's".
- **Why it is wrong:** `google.com` is listed as a search engine (`visit_taxonomy/sites.rs:28`). Every
  Google product host the lists name has its real category: mail, docs, drive, calendar and meet are
  Work (`sites.rs:361-408`), cloud is Developer (`:91`), maps is Travel (`:309`). Google Ads is an
  advertising console. It is Search only because nobody listed it. The bug this test was written for
  is the same kind of mistake (`docs/features/deterministic-intelligence.md:194`: "Google 的文件、郵件、Gemini 都算成搜尋").
  `search_engine()` also sets the page kind to search results (`visit_taxonomy/rules.rs:84-86`), so
  every unlisted Google host (ads, accounts, photos, play) counts as search-result pages, which feed
  compare sets (deterministic-intelligence.md:195).
- **Effect:** adding `site("ads.google.com", Work)` is a correct fix, and it fails this test.
- **Action:** demonstrate inheritance with a host whose parent's category is right for it. Consider the
  doc's own portal rule (deterministic-intelligence.md:195: portals whose pages span categories are not
  listed, only their plain hosts) for `google.com`. The rest of the test is right; keep it.

### A? items (each with a stated doubt)

- **`search::ported_queries::unsupported_negative_date_operators_keep_the_parsers_literal_exclusion`** (`src/search/ported_queries.rs:641`)
  - **What it does:** `-after:2024` silently becomes a text exclusion. It returns every visit after 2024
    and hides pages whose text says "after 2024". The negated date falls through to `excluded_terms` in
    `archive/search_query.rs:125-133`.
  - **Doc:** `docs/features/recall.md:107` defines `after:` / `before:` as narrowing by visit time.
  - **Doubt:** no doc says what negation should do, and the test name admits the behavior was kept on
    purpose.
  - **Action:** decide the meaning (`-after:X` as `before:X`, or say "not supported" in the search tips),
    write it into recall.md, and replace the test.
- **`intelligence_sections::tests::direct_meta_uses_query_time_and_capability_gated_empty_reason`** (`src/intelligence_sections.rs:539`)
  - **What it does:** the same English `stateReason` as A2, for observed interactions.
  - **Doubt:** no screen asks for that section today.
  - **Action:** goes with the A2 fix.
- **`enrichment::tests::execute_enrichment_job_by_id_covers_cancellation_and_failure_edges`, lines 502–505** (`src/enrichment/tests.rs:498`)
  - **What it does:** a `readable-content-refetch` job stores a "deferred" row ("remains disabled in
    PathKeep v0.2.0", `enrichment.rs:125-132`) and ends `succeeded`.
  - **Docs against it:** `docs/features/archive.md:382` says content fetch shipped and the deferral is
    superseded. `enrichment/content_fetch.rs:24` says "never a fake success".
  - **Doubt:** `docs/architecture/data-model.md:552` still describes the stub. Only rows carried over from
    an old database reach this branch.
  - **Action:** retire the branch, delete these lines, and fix the stale docs.
- **`intelligence_runtime::tests_queue::next_content_fetch_schedule_eta_reports_soonest_future_then_none`, lines 1171–1183** (`src/intelligence_runtime/tests_queue.rs:1132`)
  - **What it does:** asserts `Some(1)` for an unparseable `scheduled_at`. The due query compares text
    (`intelligence_runtime/claims.rs:397`), so that row never becomes due, and the worker wakes every
    second forever. That contradicts the function's own "shouldn't wedge the lane" doc
    (`claims.rs:434`).
  - **Doubt:** nothing in production writes such a timestamp.
  - **Action:** make the due query and the ETA agree, or quarantine the row; then drop this assert and
    keep the rest of the test.

## The B and C that tax changes most

1. `intelligence/tests/batch_equivalence.rs:165` and `:88` (C) keep about 200 lines of test-only copies
   of the refind, path, habit and query-family rules inside production files. Every rule change is made
   twice, and production is never what gets compared. Delete them together with the `#[cfg(test)]`
   builders. `session_paths.rs:134` shows the right model: compare the production read against a slow
   reference, with `limit: Some(37)` to cross batches on purpose.
2. `store_schema/tests/mod.rs:166` `catalog_and_crud` (B), plus its lists in
   `store_schema/tests/{archive,intelligence,source_evidence,agent}.rs`. Every new table or index, and
   every Turso upgrade, must edit name lists that include Turso's internal names (75 lines of
   `__turso_internal_*` / `sqlite_autoindex_*`). The CRUD half re-tests SQLite. Keep only "no triggers
   or virtual tables" (`docs/architecture/store-schema.md:6`).
3. `intelligence/tests/stage_rebuild.rs:54` `clear_derived_intelligence_state_reports_canonical_group_counts`
   (B) hand-copies the production table groups. The copy has already drifted: it lacks `compare_sets`,
   which production has at `intelligence_schema.rs:144`. It passes only because the fixture has no
   compare sets. `background-work.spec.ts` covers preview, clear, zero and rebuild.
4. `intelligence/tests/schema_overview.rs:145` and `:394` (B) pin `timings.len() == 11` and `10`. Adding
   or removing any Insights card breaks them, which is exactly what today's Insights restore work does.
5. `models/mod.rs:111, 125, 141, 165` and `:57, 78` (B/C) pin `len() == 8`, `len() == 2 / 3` and index
   order. Adding, removing or reordering a module breaks four to six tests. The only real rule is that
   the user's on/off setting survives.
6. `intelligence/intelligence_core_persist.rs:19` (B) asserts exact row counts in 15 internal tables,
   inside a test-only module.
7. `intelligence_runtime/tests_queue.rs:172` `enqueue_runtime_helpers_dedupe_refresh_and_mark_stale_contracts`
   (B) asserts trigger counts, an error substring, a test-only enqueue and stale-row SQL all at once.
   Its one real rule, dedupe, is covered better by `tests/intelligence_turso.rs:388`.
8. `enrichment/tests.rs:231` (B) pins English failure text on the title-normalization path nothing
   reaches. That text lands in `last_error`, which Settings shows untranslated.
9. `intelligence/host_artifacts/tests.rs` (4 tests, B/C) pin artifact paths, version constants and
   zh-TW / zh-CN warning text for a feature whose page was removed.
10. `intelligence/intelligence_overview_snapshot.rs` fingerprint cluster (372, 384, 390, 417, 468, 489,
    570, 618; B/C) pins cache-key strings and two inputs nothing ever changes.
11. `visit_taxonomy/tests.rs:26` (C, written today) restates three `as_str` tables that no production
    code calls. The stored keys come from a second mapping in `site_dictionary/classification.rs:364,385`,
    which writes `search_results` where this test pins `search-results`.
12. `diagnostics.rs:510, 533, 544` (C) probe downcast branches through a process-wide panic hook. Test
    510 flaked in about 3 of 8 runs (`docs/plan/program/quality-matrix.md:84`) and led to about 120 lines
    of hook isolation.

## Today's tests, judged by the same standard

- **`visit_taxonomy/tests.rs` (19 tests):** 12 D. One test is D except for the A5 assertion. There are
  3 B: the tokenizer test and two evidence-tier tests, because the tier is stored and never read, and
  the reason strings are never stored. There are 3 C: the `as_str` restatement, a one-branch probe, and
  `taxonomy_version_names_every_pack`. That last one re-implements the version fold and does not catch
  the failure it names: a list changes but its pack version is not bumped. Hashing each pack's rules
  into its version would catch it.
- **`visit_taxonomy/classification.rs` (2 tests):** 1 B (an internal reason string that is never stored)
  and 1 C (`query_value_contains`, which no built-in rule uses).
- **`intelligence/tests/build_needed.rs` (5 tests):** all D. They test the rule for queueing a build when
  an archive opens (`docs/features/deterministic-intelligence.md:199`). Their fixtures write raw
  `intelligence_jobs` rows and edit `state_json["rule_version"]`, so a job-table refactor will break
  them. That is the price of reaching states E2E can't set up. An E2E "clear Insights, reopen PathKeep,
  the cards come back without Rebuild" would cover the never-built and cleared cases.
- **`intelligence_daily_rollup_state.rs` (2 tests):** both D (deterministic-intelligence.md:198). For the
  tie test, asserting that input order doesn't change the result is the real property. Which name wins
  alphabetically is incidental.

## Code problems found along the way (not test verdicts; no test pins them)

- **Threads counts wordings as days.** `intelligence/intelligence_structural_aggregates.rs:243-246` sets
  `distinct_days` to the number of different wordings in a query family. The Threads card shows it as
  days (`src/features/insights/pattern-cards.tsx:82`). §4.5 of core-intelligence-ultimate-design.md asks
  for recurrence across days.
- **Compare sets check the wrong URL part.** `intelligence/phase_four.rs:250-268` compares only the first
  path segment, so two products under `/products/` never form a compare set (§4.12 says "sibling URL").
- **A 429 from a host is retried after about 1 second.** `enrichment/content_fetch.rs:554, 712` takes
  the delay from PathKeep's own token budget and stores no negative cache. That contradicts
  `docs/architecture/ai-security-posture.md:257`.
- **YouTube video ids get lost.** `enrichment_site_adapters.rs:248` drops the whole query when any pair
  lacks `=`.
- **Lexicon matching is a substring match.** `visit_taxonomy/classification.rs:177` uses
  `title_and_query.contains(token)`, so "api" matches "rapid" and "plan" matches "explanation". After
  today's change this only mislabels the page kind, but it is still wrong.
- **The Clear Insights preview misses tables.** The preview doesn't count `session_paths`,
  `session_domains`, `visit_assignments` or the overview snapshots, which the clear deletes
  (`intelligence/intelligence_schema.rs:72-158`).
- **Ignored tests never run.** The twelve `#[ignore]`d HTTP transport tests in
  `enrichment/content_fetch/tests/transport.rs` use a local mock server and need no internet. Eight carry
  a rule and should be un-ignored. The env-gated LM Studio, candle, static-embedding and S2 tests return
  early and count as passes in `cargo test`; mark them `#[ignore]`.
- **Docs that name tests.** `docs/features/deterministic-intelligence.md:399` (§6.3) asks for a fixture
  for every rule added or changed, and `:196` names the rule-list test as the guarantee. The rule
  fixtures and that test are kept (D), so these lines stay true; update them if those tests are removed.

## Per-file tables

Columns: test, `fn` line, category, reason, E2E coverage, action. "none" in E2E coverage means no spec
in `tests/e2e/` checks it. Where a scenario is named, that is the E2E that would replace the test.

### Visit taxonomy and today's tests

#### src/visit_taxonomy/tests.rs

| Test                                                         | Line | Cat            | Reason                                                                                                                                                                                                                     | E2E coverage                                                                                                | Action                                                                               |
| ------------------------------------------------------------ | ---- | -------------- | -------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | ----------------------------------------------------------------------------------------------------------- | ------------------------------------------------------------------------------------ |
| taxonomy_serialized_identifiers_cover_every_public_variant   | 26   | C              | Restates as_str tables; Domain/Page/InteractionKind::as_str have no production caller (stored keys come from site_dictionary/classification.rs:364,385, with different spellings); also pins the "m5-taxonomy-v2\|" prefix | insights.spec "kinds of sites count every visit by the kind of site it went to" checks the labels users see | delete; delete the unused as_str methods or make site_dictionary use one mapping     |
| normalizes_search_urls_and_strips_tracking_params            | 102  | D              | URL normalizer; doc deterministic-intelligence.md:167-168 (drop utm\_\*, keep query)                                                                                                                                       | partly, via Frequent searches cards                                                                         | keep; the order of dropped params is incidental                                      |
| keeps_semantic_ids_and_extracts_cjk_search_terms             | 118  | D              | Percent-decoded CJK query, tracking param dropped                                                                                                                                                                          | none                                                                                                        | keep                                                                                 |
| registrable_domain_handles_common_multi_label_suffixes       | 129  | D              | Registrable domain for co.uk / com.cn; doc :155,:169                                                                                                                                                                       | none                                                                                                        | keep                                                                                 |
| non_search_urls_keep_semantic_query_params                   | 148  | D              | Issue id kept, utm dropped, no search query on non-search hosts; doc :168                                                                                                                                                  | none                                                                                                        | keep                                                                                 |
| script_aware_tokenization_handles_latin_and_cjk              | 161  | B              | tokenize_text has no caller outside the module; inside, only "does the title have any token" is used (classification.rs:94). The tokens themselves feed nothing                                                            | none needed                                                                                                 | delete (or delete with the export)                                                   |
| normalizer_rejects_hosts_that_trim_to_empty                  | 171  | C              | One-branch probe ("https://./path" → None)                                                                                                                                                                                 | none needed                                                                                                 | delete                                                                               |
| evidence_tier_prefers_canonical_search_and_referrer_chain    | 176  | B              | Tier rule is in doc §3.2, but the stored tier is never read by any query or screen; reason strings are never stored                                                                                                        | none (nothing shows it)                                                                                     | delete with the unused column, or keep only the tier assert once Explain shows tiers |
| evidence_tier_falls_back_to_structural_and_then_time_only    | 195  | B              | Same; also pins `reasons == ["time-adjacency-only"]`                                                                                                                                                                       | none                                                                                                        | same                                                                                 |
| taxonomy_exact_domain_and_host_path_rules_cover_core_sites   | 225  | D              | Rule fixtures doc §6.3 asks for ("每次新增 / 修改規則都要有 fixture"); Issue/PR page kinds feed compare sets                                                                                                               | GitHub host-level category in "kinds of sites"                                                              | keep; drop the interaction_kind assert (not stored or read)                          |
| taxonomy_cn_and_us_packs_cover_regional_sites                | 270  | D              | Same, regional packs                                                                                                                                                                                                       | none                                                                                                        | keep; drop the interaction_kind assert                                               |
| taxonomy_lexicon_and_unknown_fallback_are_honest             | 301  | D              | Doc :197, lexicon sets page kind only                                                                                                                                                                                      | none                                                                                                        | keep (or merge with :494)                                                            |
| user_override_beats_pack_rules                               | 334  | D              | Precedence, doc §5.2 and §6.3                                                                                                                                                                                              | none                                                                                                        | keep                                                                                 |
| a_host_with_its_own_rule_beats_its_parent                    | 388  | A (one assert) | Subhost-before-parent is doc :194 and right; the ads.google.com → Search assert pins a wrong label                                                                                                                         | none                                                                                                        | fix the assertion; see A5                                                            |
| host_walk_stops_at_the_registrable_domain                    | 404  | D              | Never climbs past the registrable domain (no rule can cover co.uk); doc :194                                                                                                                                               | none                                                                                                        | keep                                                                                 |
| every_site_rule_can_fire_and_is_listed_once                  | 418  | D              | Lint over a 1000-line data list; doc :196 cites it                                                                                                                                                                         | none                                                                                                        | keep                                                                                 |
| sites_told_apart_by_path_still_have_a_category_on_every_page | 463  | D              | Doc :196                                                                                                                                                                                                                   | GitHub, YouTube, Reddit pages in "kinds of sites"                                                           | keep                                                                                 |
| taxonomy_version_names_every_pack                            | 485  | C              | Re-implements the fold in rules.rs:104-111; does not catch the failure it names (list changed, version not bumped)                                                                                                         | none                                                                                                        | delete; to catch the real failure, derive each pack version from a hash of its rules |
| a_page_title_does_not_label_the_site                         | 494  | D              | Regression for the "tokio vs smol" bug, doc :197                                                                                                                                                                           | none                                                                                                        | keep; :301 says the same thing, one of them can go                                   |

#### src/visit_taxonomy/classification.rs (inline)

| Test                                               | Line | Cat | Reason                                                                                       | E2E coverage | Action                            |
| -------------------------------------------------- | ---- | --- | -------------------------------------------------------------------------------------------- | ------------ | --------------------------------- |
| taxonomy_fallback_reports_invalid_or_empty_domains | 346  | B   | Pins the reason string "invalid-or-empty-domain"; taxonomy reasons are never stored or shown | none needed  | delete                            |
| host_path_rule_checks_query_keys_and_values        | 364  | C   | Tests `query_value_contains` with a made-up rule; no built-in rule sets that field           | none needed  | delete, and drop the unused field |

#### src/intelligence/tests/build_needed.rs

All five test `insights_build_needed` (intelligence_runtime/mod.rs:368), the rule for queueing a build
when an archive opens; deterministic-intelligence.md:199 and the function's doc comment state it. The
fixtures write raw rows into `intelligence_jobs` and edit `state_json["rule_version"]`, so a job-table
or build-state refactor breaks them; that is the price of reaching states E2E cannot set up.

| Test                                                        | Line | Cat | Reason                                                                 | E2E coverage                                                   | Action                                                                                                        |
| ----------------------------------------------------------- | ---- | --- | ---------------------------------------------------------------------- | -------------------------------------------------------------- | ------------------------------------------------------------------------------------------------------------- |
| an_archive_whose_insights_were_never_built_needs_one_build  | 79   | D   | Never-built archive gets one build, not two                            | first-run.spec covers the first-backup path, not the open path | keep; an E2E "clear Insights, reopen PathKeep, the cards come back without Rebuild" would cover the open path |
| an_empty_archive_needs_no_build                             | 88   | D   | No pointless job for an empty archive (minor)                          | none                                                           | keep                                                                                                          |
| a_build_that_failed_or_was_cancelled_is_not_retried_on_open | 94   | D   | Doc :199 "最近一次 Insights 工作是成功的"; no retry loop on every open | none                                                           | keep                                                                                                          |
| insights_built_with_older_rules_are_built_again             | 108  | D   | Doc :199; E2E cannot fake an older rule version                        | none                                                           | keep                                                                                                          |
| cleared_insights_are_built_again_on_open                    | 121  | D   | Clear dialog promises they come back next open                         | background-work.spec clears but does not reopen                | keep, or replace with the E2E above                                                                           |

#### src/intelligence/intelligence_daily_rollup_state.rs (inline)

| Test                                               | Line | Cat | Reason                                                              | E2E coverage | Action                                                                                                                  |
| -------------------------------------------------- | ---- | --- | ------------------------------------------------------------------- | ------------ | ----------------------------------------------------------------------------------------------------------------------- |
| a_site_takes_the_category_most_of_its_visits_had   | 255  | D   | Doc :198, regression (one docs page made a day of searching "docs") | none         | keep                                                                                                                    |
| ties_go_to_a_known_category_then_the_first_by_name | 261  | D   | Doc :198 tie rule                                                   | none         | keep; the alphabetical winner is incidental, asserting that input order does not change the result is the real property |

### Intelligence, inline tests in src/intelligence/

Reach column: prod = reachable from the product; test-only / orphan / dead input as defined in §1.

#### src/intelligence/compare_sets.rs

| Test                                                  | Line | Cat | Reason                                                                                                                                                                | E2E coverage                                                                                                                              | Action                          |
| ----------------------------------------------------- | ---- | --- | --------------------------------------------------------------------------------------------------------------------------------------------------------------------- | ----------------------------------------------------------------------------------------------------------------------------------------- | ------------------------------- |
| sets_found_while_streaming_are_read_back_newest_first | 286  | D   | Doc §4.12 rule (same trail, back-and-forth, two sites) across a mid-stream `resolve`; newest-first paging; profile scope. Also pins the `compare:t1:product_page` id. | None (insights.spec only checks the command is batched). Scenario: "Pages you compared lists the fixture's back-and-forth product pages". | Keep; drop the id-format assert |

#### src/intelligence/day_insights.rs

| Test                                                            | Line | Cat | Reason                                    | E2E coverage                                                                                                                                                                             | Action |
| --------------------------------------------------------------- | ---- | --- | ----------------------------------------- | ---------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | ------ |
| exact_day_range_rejects_invalid_calendar_days                   | 122  | C   | Private wrapper around `NaiveDate::parse` | Not needed                                                                                                                                                                               | Delete |
| hourly_activity_folds_exact_day_rhythm_into_twenty_four_buckets | 128  | C   | Private 10-line fold, restates the code   | insights-drill "a day from the Home heatmap matches the fixture" covers the day view, not the hours. Scenario: "the day view's hourly bars add up to the fixture's visits by local hour" | Delete |

#### src/intelligence/host_artifacts/tests.rs (Reach: orphan)

| Test                                                                      | Line | Cat | Reason                                                                           | E2E coverage                                | Action                                                                        |
| ------------------------------------------------------------------------- | ---- | --- | -------------------------------------------------------------------------------- | ------------------------------------------- | ----------------------------------------------------------------------------- |
| preview_intelligence_local_host_builds_fixed_generated_files              | 12   | B   | Exact paths, host id / version constants, English title substring                | None; no UI since Integrations page removed | Delete with the orphan commands                                               |
| build_intelligence_local_host_writes_artifacts_and_reports_installed_host | 53   | B   | Writes a file and reads it back; English warning substring                       | None                                        | Delete                                                                        |
| installed_host_loader_reports_missing_corrupt_and_valid_artifacts         | 94   | B   | Pins zh-TW / zh-CN warning substrings and warning counts                         | None                                        | Delete                                                                        |
| local_host_rendering_covers_locales_empty_states_and_html_escaping        | 145  | B   | Localized copy substrings; only the HTML-escaping asserts have a reason to exist | None                                        | Delete with the feature; if the feature stays, keep only the escaping asserts |

#### src/intelligence/incremental.rs

| Test                                                             | Line | Cat | Reason                                                                                      | E2E coverage | Action |
| ---------------------------------------------------------------- | ---- | --- | ------------------------------------------------------------------------------------------- | ------------ | ------ |
| stage_checkpoint_helpers_cover_names_versions_and_scoped_deletes | 299  | C   | Stage-name and version strings; load/delete helpers it calls are `#[cfg(test)]` (test-only) | Not needed   | Delete |

#### src/intelligence/intelligence_core_persist.rs (module is `#[cfg(test)]`)

| Test                                                 | Line | Cat | Reason                                                                                                        | E2E coverage                                                                                                                                    | Action                                                                                            |
| ---------------------------------------------------- | ---- | --- | ------------------------------------------------------------------------------------------------------------- | ----------------------------------------------------------------------------------------------------------------------------------------------- | ------------------------------------------------------------------------------------------------- |
| persist_core_state_replaces_every_full_rebuild_table | 19   | B   | 15 exact internal row counts. The one useful check: the old generation stays visible until the pointer flips. | background-work "…clear and rebuild" covers rebuild end to end. Scenario: "Insights keep the old numbers during a rebuild, then switch at once" | Delete; if the flip is not covered in `intelligence/tests/stage_rebuild.rs`, keep only that check |

#### src/intelligence/intelligence_daily_rollups.rs (module is `#[cfg(test)]`, Reach: test-only)

| Test                                                | Line | Cat | Reason                                                  | E2E coverage | Action                         |
| --------------------------------------------------- | ---- | --- | ------------------------------------------------------- | ------------ | ------------------------------ |
| duplicate_rows_and_empty_dirty_dates_are_guarded    | 258  | C   | Tests helpers no production code calls; error substring | Not needed   | Delete (with the dead helpers) |
| local_result_timestamp_millis_covers_timezone_edges | 283  | C   | Three-arm match restated; empty-table read              | Not needed   | Delete                         |

#### src/intelligence/intelligence_explain_helpers.rs

| Test                                                                    | Line | Cat | Reason                                                                                                                                   | E2E coverage | Action                                             |
| ----------------------------------------------------------------------- | ---- | --- | ---------------------------------------------------------------------------------------------------------------------------------------- | ------------ | -------------------------------------------------- |
| query_family_loader_returns_empty_without_schema_when_queries_are_empty | 254  | C   | Empty-input early return                                                                                                                 | Not needed   | Delete                                             |
| path_flow_match_helper_keeps_only_newest_matching_window                | 265  | C   | `update_latest_path_flow_match` is `#[cfg(test)]`: a test of test-only code                                                              | Not needed   | Delete with the function                           |
| recent_path_flow_loader_keeps_latest_visit_for_repeated_domains         | 301  | D   | Plan guard (`idx_visits_visible_profile_time_id`) plus "last visit of a repeated-domain run" rule. Reach: orphan (`explain_entity` only) | None         | Keep while `explain_entity` exists; delete with it |

#### src/intelligence/intelligence_explain.rs (Reach: orphan, `explain_entity` has no frontend caller)

| Test                                                        | Line | Cat | Reason                                                                                                         | E2E coverage | Action                         |
| ----------------------------------------------------------- | ---- | --- | -------------------------------------------------------------------------------------------------------------- | ------------ | ------------------------------ |
| explanation_builders_cover_all_supported_entity_shapes      | 692  | B   | 150-line seeded fixture read back; English trigger-rule substrings ("Deep dive", "30 minutes", "interruption") | None         | Delete with the orphan command |
| explanation_builders_report_missing_or_malformed_entity_ids | 832  | C   | Seven `is_err()` probes                                                                                        | None         | Delete                         |

#### src/intelligence/intelligence_outputs.rs (Reach: orphan, embed cards)

| Test                                                | Line | Cat | Reason                                      | E2E coverage | Action |
| --------------------------------------------------- | ---- | --- | ------------------------------------------- | ------------ | ------ |
| on_this_day_body_uses_summary_or_aggregate_fallback | 295  | B   | Exact English sentence of an orphan payload | None         | Delete |

#### src/intelligence/intelligence_overview_snapshot.rs

| Test                                                        | Line | Cat | Reason                                                                                                           | E2E coverage                                                                                                                                  | Action                                              |
| ----------------------------------------------------------- | ---- | --- | ---------------------------------------------------------------------------------------------------------------- | --------------------------------------------------------------------------------------------------------------------------------------------- | --------------------------------------------------- |
| detects_all_time_range_by_sentinel_start                    | 372  | C   | String equality restated                                                                                         | insights "Insights opens on all time…"                                                                                                        | Delete                                              |
| scope_key_separates_archive_wide_from_profile               | 384  | B   | Exact cache-key strings                                                                                          | insights "a custom range and one browser profile…"                                                                                            | Delete                                              |
| snapshot_round_trips_and_overwrites                         | 390  | C   | Upsert round trip on an internal cache table                                                                     | Not needed                                                                                                                                    | Delete                                              |
| clear_removes_all_snapshots                                 | 417  | C   | `DELETE` returns 1                                                                                               | background-work "…clear and rebuild"                                                                                                          | Delete                                              |
| resolve_serves_snapshot_until_fingerprint_changes           | 430  | D   | Cache is served on a hit and rebuilt after a rebuild moves the ledger (build-call count is the behavior here)    | Partly by background-work                                                                                                                     | Keep                                                |
| fingerprint_changes_with_checkpoint_ledger                  | 468  | C   | Subset of 430                                                                                                    | —                                                                                                                                             | Delete                                              |
| fingerprint_reacts_to_each_watermark_dimension              | 489  | C   | Its own comment says it exists to catch a field hard-coded to a constant; pins the fingerprint's makeup          | —                                                                                                                                             | Delete                                              |
| resolve_rebuilds_when_archive_gains_un_ingested_visits      | 536  | D   | No stale all-time Insights after a backup adds visits                                                            | backup.spec checks History, not Insights. Scenario: "after a backup, Insights' all-time total includes the new visits"                        | Keep                                                |
| profile_scoped_fingerprint_isolates_its_own_profile         | 570  | B   | Pins that a sibling's checkpoint does not move the key: an optimization, not a promise                           | —                                                                                                                                             | Delete                                              |
| fingerprint_reacts_to_revert_runs_without_new_visits        | 604  | D   | Undoing an import must invalidate the all-time snapshot                                                          | browser-import "…imports, undoes and restores exactly" does not check Insights. Scenario: "Insights totals drop back after undoing an import" | Keep                                                |
| fingerprint_reacts_to_site_dictionary_and_search_rule_edits | 618  | C   | Pins two dead inputs (override table nothing writes; rules no UI edits); stale comment about lazy table creation | None                                                                                                                                          | Delete                                              |
| fingerprint_outlives_the_day_but_not_the_timezone           | 648  | D   | Real past bug (code comment `:131-134`: first open of each day recomputed all history)                           | None                                                                                                                                          | Keep; drop the `day=` / `tz=` string-format asserts |

#### src/intelligence/intelligence_overview.rs

| Test                                                                    | Line | Cat | Reason                                               | E2E coverage | Action |
| ----------------------------------------------------------------------- | ---- | --- | ---------------------------------------------------- | ------------ | ------ |
| search_effectiveness_empty_state_requires_all_signal_groups_to_be_empty | 665  | C   | Private three-`is_empty` predicate, one field probed | Not needed   | Delete |

#### src/intelligence/intelligence_rebuild.rs

| Test                                                           | Line | Cat | Reason                                                                                                                        | E2E coverage                         | Action |
| -------------------------------------------------------------- | ---- | --- | ----------------------------------------------------------------------------------------------------------------------------- | ------------------------------------ | ------ |
| rebuild_helpers_keep_default_notes_and_progress_edges_truthful | 391  | B   | Exact English notes, a `1 of 0 = 100%` edge, struct getters. Misses the one real rule (skip "ready" when the revision moved). | background-work "…clear and rebuild" | Delete |

#### src/intelligence/intelligence_refind.rs

| Test                                                    | Line | Cat | Reason                  | E2E coverage                                                                | Action |
| ------------------------------------------------------- | ---- | --- | ----------------------- | --------------------------------------------------------------------------- | ------ |
| refind_detail_helpers_skip_empty_or_unlinked_visit_sets | 374  | C   | Empty-input probes only | insights-drill "a page you keep reopening shows its visits across browsers" | Delete |

#### src/intelligence/intelligence_sessions.rs

| Test                                                  | Line | Cat | Reason                                                                             | E2E coverage                                                                                             | Action                |
| ----------------------------------------------------- | ---- | --- | ---------------------------------------------------------------------------------- | -------------------------------------------------------------------------------------------------------- | --------------------- |
| session_and_trail_public_reads_return_bounded_details | 385  | B   | Reads back seeded rows; `page_size 0 → 1` clamp; role strings; transition as `"1"` | None. Scenario: "the search view expands one search's trail and lists its pages in order, landing first" | Replace with that E2E |

#### src/intelligence/intelligence_shared.rs

| Test                                                            | Line | Cat | Reason                                                                                 | E2E coverage                                                           | Action                                                 |
| --------------------------------------------------------------- | ---- | --- | -------------------------------------------------------------------------------------- | ---------------------------------------------------------------------- | ------------------------------------------------------ |
| local_datetime_result_helper_covers_ambiguous_and_missing_edges | 296  | B   | Error-message substring; three-arm match                                               | Not needed                                                             | Delete                                                 |
| search_words_leave_out_lone_letters_and_bare_numbers            | 312  | D   | Real past bug ("s", "3" in top words), rule in doc comment and `screens-and-nav.md:69` | Also insights "the cards brought back from 0.3…" (What you search for) | Keep                                                   |
| navigational_query_domain_matching_is_symmetric_for_subdomains  | 319  | D   | URL-like queries stay out of concepts (`core-intelligence-ultimate-design.md:456`)     | Not directly                                                           | Keep; drop the `is_cjk_like` and `"https://./"` probes |

#### src/intelligence/intelligence_structural_aggregates.rs

| Test                                                        | Line | Cat | Reason                                                                                                     | E2E coverage                                           | Action                |
| ----------------------------------------------------------- | ---- | --- | ---------------------------------------------------------------------------------------------------------- | ------------------------------------------------------ | --------------------- |
| habit_records_without_profile_short_circuits                | 714  | C   | Early return                                                                                               | —                                                      | Delete                |
| recent_visit_sample_orders_most_recent_first_and_counts_all | 719  | B   | Internal order of a private sample                                                                         | —                                                      | Delete                |
| recent_visit_sample_breaks_ties_by_visit_id                 | 730  | B   | Tie order of a private sample                                                                              | —                                                      | Delete                |
| recent_visit_sample_caps_to_the_most_recent_window          | 741  | D   | Bounded evidence size at 14.4M (a page visited daily for decades)                                          | —                                                      | Keep                  |
| recent_visit_sample_skips_visits_older_than_the_kept_window | 762  | B   | Pins the fast-path internals                                                                               | —                                                      | Delete                |
| refind_evidence_json_embeds_capped_sample_and_exact_total   | 786  | C   | Duplicate of 741 plus JSON shape                                                                           | —                                                      | Delete                |
| build_refind_pages_caps_visit_ids_but_keeps_full_total      | 803  | C   | `build_refind_pages` is `#[cfg(test)]` (test-only)                                                         | —                                                      | Delete with the twin  |
| streaming_and_in_memory_refind_emit_identical_evidence      | 819  | C   | Both sides are test-only entry points (`add_visit`, `build_refind_pages`); duplicates the score formula    | insights-drill "a page you keep reopening…"            | Delete with the twins |
| bounded_path_flow_tail_matches_all_reference_windows        | 844  | D   | Production `add_summary_visit` keeps ≤4 sites and matches all windows: a bounded-memory differential check | insights "the most frequent path between three sites…" | Keep                  |

#### src/intelligence/intelligence_structural_build.rs

| Test                                                                    | Line | Cat | Reason                                                                                       | E2E coverage | Action |
| ----------------------------------------------------------------------- | ---- | --- | -------------------------------------------------------------------------------------------- | ------------ | ------ |
| query_family_builders_skip_empty_tokens_and_scope_by_profile_and_engine | 290  | C   | Main assert is on the `#[cfg(test)]` reference builder; the production part is an empty case | —            | Delete |
| indexed_family_candidates_preserve_the_reference_first_match            | 306  | D   | The token index (perf) keeps the naive first-match result over 1003 events                   | —            | Keep   |

#### src/intelligence/intelligence_structural_persist.rs (module is `#[cfg(test)]`)

| Test                                                         | Line | Cat | Reason                      | E2E coverage | Action |
| ------------------------------------------------------------ | ---- | --- | --------------------------- | ------------ | ------ |
| structural_profile_aggregate_replace_persists_habit_patterns | 9    | C   | Write one row, read it back | —            | Delete |

#### src/intelligence/intelligence_structural_stage.rs (module is `#[cfg(test)]`, Reach: test-only)

| Test                                                    | Line | Cat | Reason                                                                                                                                    | E2E coverage | Action                 |
| ------------------------------------------------------- | ---- | --- | ----------------------------------------------------------------------------------------------------------------------------------------- | ------------ | ---------------------- |
| structural_fallback_reason_reports_regressed_watermarks | 153  | C   | `structural_fallback_reason` exists only in tests; production uses `watermark_regressed` in `build_pipeline.rs:90,127`; message substring | —            | Delete with the module |

#### src/intelligence/intelligence_structural_state.rs

| Test                                                               | Line | Cat | Reason                                                             | E2E coverage                                                                                             | Action |
| ------------------------------------------------------------------ | ---- | --- | ------------------------------------------------------------------ | -------------------------------------------------------------------------------------------------------- | ------ |
| trail_state_returns_persisted_search_member_and_defensive_fallback | 345  | B   | Role strings, ordinals, a defensive branch for an unknown visit id | insights "where searches end up is counted like the archive" covers the landing rule                     | Delete |
| session_title_summary_covers_empty_and_query_only_inputs           | 379  | B   | Exact title format `"example.com · rust coverage"`                 | None. Scenario: "the day view names each session by its main site and first search", if titles are shown | Delete |

#### src/intelligence/intelligence_structural_stream.rs

| Test                                                                       | Line | Cat | Reason                                                                                     | E2E coverage                                                      | Action                                                                   |
| -------------------------------------------------------------------------- | ---- | --- | ------------------------------------------------------------------------------------------ | ----------------------------------------------------------------- | ------------------------------------------------------------------------ |
| search_event_kind_update_rewrites_keyword_terms                            | 439  | B   | Private writer; internal term rows                                                         | insights "What you search for" (partly)                           | Delete                                                                   |
| stream_state_flushes_trails_on_gap_and_updates_search_event_kind           | 514  | D   | 15-minute trail gap and 30-minute session gap (rules spelled out in insights.spec:487-491) | insights "reworded searches…", "most frequent path…" (indirectly) | Keep; assert on stored rows, not `state.report` counters                 |
| a_reworded_search_continues_its_trail_and_lands_where_the_last_wording_led | 589  | D   | Real past bug ("every search started a trail"), documented rule                            | insights "reworded searches are counted…"                         | Keep; drop the `trail:chrome:Default:N` id and exact `queries_json` text |

#### src/intelligence/intelligence_summary.rs

| Test                                                                | Line | Cat | Reason                                                                                                                 | E2E coverage                                                                                                                           | Action                                                       |
| ------------------------------------------------------------------- | ---- | --- | ---------------------------------------------------------------------------------------------------------------------- | -------------------------------------------------------------------------------------------------------------------------------------- | ------------------------------------------------------------ |
| digest_counts_distinct_domains_and_estimates_active_time_per_window | 565  | D   | Active-time rule from the fn doc (span + 1 min, ≤5 min per visit, window of start); a site counts once across profiles | None asserts active time. Scenario: "the Active time KPI equals the fixture's sessions by the documented rule"                         | Keep; drop the encrypted-vault loop                          |
| friction_signals_cover_reformulation_and_bounce_rows                | 646  | B   | Exact English description no UI shows; signal order                                                                    | insights "reworded searches are counted, and listed as searches that took several tries" (reformulation part); bounce part not covered | Delete; add a bounce row to the fixture if that path matters |

#### src/intelligence/intelligence_visit_records.rs

| Test                                                                   | Line | Cat | Reason                                                                                         | E2E coverage                                                   | Action                      |
| ---------------------------------------------------------------------- | ---- | --- | ---------------------------------------------------------------------------------------------- | -------------------------------------------------------------- | --------------------------- |
| load_visible_visits_unbounded_returns_every_profile_in_ascending_order | 214  | C   | Internal page order; near-duplicate                                                            | insights "every range shows the archive's numbers"             | Delete                      |
| load_visible_visits_scopes_to_one_profile                              | 226  | C   | Near-duplicate                                                                                 | insights "…one browser profile count only their visits"        | Delete                      |
| source_page_size_keeps_all_profile_visits_reachable                    | 236  | D   | Keyset paging across a page boundary loses no visit (page size 2)                              | —                                                              | Keep; fix the stale comment |
| source_pages_cover_every_profile                                       | 245  | C   | Near-duplicate of 236                                                                          | —                                                              | Delete                      |
| zero_rebuild_page_size_still_covers_all_visits                         | 256  | C   | Probes a `limit: 0` clamp no caller sends                                                      | —                                                              | Delete                      |
| oversized_source_page_returns_all_rows                                 | 277  | C   | Near-duplicate (page size 999)                                                                 | —                                                              | Delete                      |
| source_pages_hydrate_search_terms                                      | 285  | C   | Comment claims a bounded window; page size 3 covers the whole profile, so it tests no boundary | insights "Where you search: every fixture search is on Google" | Delete                      |

#### src/intelligence/phase_four.rs

| Test                                                             | Line | Cat | Reason                                                                                                        | E2E coverage                                                                                                                             | Action                                                            |
| ---------------------------------------------------------------- | ---- | --- | ------------------------------------------------------------------------------------------------------------- | ---------------------------------------------------------------------------------------------------------------------------------------- | ----------------------------------------------------------------- |
| path_prefix_extracts_first_segment                               | 614  | C   | Private helper                                                                                                | —                                                                                                                                        | Delete                                                            |
| compare_set_requires_alternation_and_multiple_pages              | 620  | D   | Doc §4.12: several pages, back and forth                                                                      | None                                                                                                                                     | Keep; drop the page-order assert                                  |
| compare_set_ignores_same_domain_without_sibling_path_variation   | 678  | B   | Misnamed: the pages do vary by path; it passes only because of the 10-minute window (a constant with no spec) | None                                                                                                                                     | Delete (or rename to what it tests)                               |
| compare_set_detail_reads_its_trail_and_parses_errors             | 732  | B   | `get_compare_set_detail` is orphan; id parsing; expects an error from an empty vault; unused `_paths` line    | None                                                                                                                                     | Delete with the orphan command                                    |
| browser_diff_read_model_groups_shared_exclusive_and_category_mix | 790  | D   | Doc §4.13: exclusive vs shared sites, category mix                                                            | insights only checks the "Browsers compared" heading. Scenario: "Browsers compared lists sites only one browser visited, with its count" | Keep; drop the `profile_name` / `browser_family` fallback asserts |

#### src/intelligence/phase_three.rs

| Test                                                                 | Line | Cat | Reason                                                                                                     | E2E coverage                                                                                           | Action                                          |
| -------------------------------------------------------------------- | ---- | --- | ---------------------------------------------------------------------------------------------------------- | ------------------------------------------------------------------------------------------------------ | ----------------------------------------------- |
| classify_habit_pattern_requires_five_visits_and_two_weeks            | 521  | D   | Rule in the UI copy (`i18n/messages/insights.ts:228`: 5 days over two weeks)                               | None                                                                                                   | Keep                                            |
| classify_habit_pattern_detects_weekly_habit                          | 527  | D   | Daily / weekly / periodic examples                                                                         | None                                                                                                   | Keep                                            |
| visit_days_match_the_days_they_stand_for                             | 549  | D   | Running gap mean / variance equal the list version; same day from two browsers counts once                 | —                                                                                                      | Keep                                            |
| round_trips_are_not_paths                                            | 568  | D   | Doc rule "不計來回" (`intelligence.md:35`, `screens-and-nav.md:69`); regression                            | insights "the most frequent path between three sites…"                                                 | Keep                                            |
| a_session_counts_every_window_once                                   | 581  | C   | 2-step windows no card shows; step-parsing probes                                                          | insights path test                                                                                     | Delete                                          |
| phase_three_read_models_cover_breadth_habits_interruptions_and_flows | 603  | C   | Grab-bag of weak asserts (`hhi > 0`, `>= 0`, `<= 1`), repeats 527, step-count clamp 8 → 4                  | None for habits or breadth. Scenario: "Habits and Explore-vs-focus show the fixture's sites and score" | Replace with that E2E                           |
| observed_interaction_evidence_merges_metrics_and_context_fallbacks   | 714  | A   | Pins `visit_duration` as foreground time (see A3); the rest is metric-key mapping and an 8-row truth table | None (no card)                                                                                         | Fix the code, or delete with the unused section |

#### src/intelligence/site_dictionary/classification.rs

| Test                                               | Line | Cat | Reason                                                                     | E2E coverage                                                            | Action |
| -------------------------------------------------- | ---- | --- | -------------------------------------------------------------------------- | ----------------------------------------------------------------------- | ------ |
| helper_mappings_cover_rule_and_taxonomy_boundaries | 409  | C   | 29-row enum → string tables and an engine-id table, restated from the code | insights "kinds of sites count every visit…" covers the keys end to end | Delete |

#### src/intelligence/site_dictionary/tests.rs (site_dictionary is live: `build_pipeline.rs:207,246`)

| Test                                                         | Line | Cat | Reason                                                                                                                              | E2E coverage                         | Action                                                         |
| ------------------------------------------------------------ | ---- | --- | ----------------------------------------------------------------------------------------------------------------------------------- | ------------------------------------ | -------------------------------------------------------------- |
| classify_visit_extracts_search_engine_and_query              | 35   | D   | Query extraction and `utm_*` stripping (`deterministic-intelligence.md` §5.1)                                                       | insights "Where you search" (Google) | Keep; drop the encrypted-vault loop                            |
| normalize_query_canonicalizes_whitespace                     | 64   | D   | One-line spec'd normalizer; cheap                                                                                                   | —                                    | Keep                                                           |
| persisted_user_override_beats_builtin_rule_pack              | 69   | D   | Doc §5.2 / §6.3 "使用者 override 永遠優先". Reach: dead input (no production writer); also pins the `"user-override"` source string | None possible                        | Delete with the dead override path, unless overrides will ship |
| persisted_overrides_parse_all_taxonomy_keys_and_target_kinds | 171  | C   | 15-row string → enum table restated. Reach: dead input                                                                              | None                                 | Delete                                                         |
| persisted_overrides_ignore_partial_or_invalid_taxonomy_rows  | 391  | B   | Pins that a partial override is dropped silently (arguably against "override 永遠優先"). Reach: dead input                          | None                                 | Delete                                                         |
| builtin_rules_cover_bilibili_and_github_search               | 436  | D   | Built-in rules for engines outside the generic analysis                                                                             | Fixture has only Google              | Keep; drop the encrypted-vault loop                            |
| custom_search_rules_override_builtin_matching                | 476  | D   | Custom rule beats built-in. Reach: orphan (no UI calls `upsertSearchEngineRule`)                                                    | None                                 | Keep while the command exists                                  |
| list_rules_merges_builtin_and_custom_rules                   | 518  | B   | Pins `builtin:bilibili` and the `custom-search-rule-` id prefix. Reach: orphan                                                      | None                                 | Delete                                                         |

### Intelligence, src/intelligence/tests/

#### src/intelligence/tests/batch_equivalence.rs

| Test                                                            | Line | Cat | Reason                                                                                                                                                                                               | E2E coverage                                                                                                                          | Action                                                                                                                                          |
| --------------------------------------------------------------- | ---- | --- | ---------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | ------------------------------------------------------------------------------------------------------------------------------------- | ----------------------------------------------------------------------------------------------------------------------------------------------- |
| batched_query_family_builder_matches_in_memory_builder          | 88   | C   | Compares a cfg(test) reference builder with a cfg(test) batch driver; production's driver (`build_stages.rs:354`) is not the one compared                                                            | insights.spec "reworded searches are counted…"; insights-drill "a search from Frequent searches…" (family rule)                       | Delete, with the cfg(test) builders in `intelligence_structural_build.rs:47-203`                                                                |
| batched_structural_aggregates_match_in_memory_builders          | 165  | C   | Same: both sides are cfg(test) (`intelligence_structural_aggregates.rs:167-384`, `intelligence_daily_rollups` is a cfg(test) module)                                                                 | Paths: insights.spec "the most frequent path between three sites…"; refind: insights-drill "a page you keep reopening…". Habits: none | Delete with the cfg(test) builders; add E2E "Habits lists a fixture site visited daily for two weeks as daily and one visited weekly as weekly" |
| structural_aggregate_builders_cover_flows_reopened_and_habits   | 318  | C   | "cover" test; flow half runs a cfg(test) builder, reopened half asserts only that two anchor types exist. Habit half checks the documented thresholds (core-intelligence-ultimate-design.md:543-545) | Habits: none                                                                                                                          | Trim: keep only the habit-class cases, called on `habit_records_for_domains`; drop flows and reopened                                           |
| structural_stage_helpers_cover_noop_delta_and_dirty_windows     | 436  | C   | Tests helpers in the cfg(test) module `intelligence_structural_stage` with `!is_empty`/`>= 1`/`<=` asserts, plus `"noop"` and English note text                                                      | none needed                                                                                                                           | Delete (and the cfg(test) module)                                                                                                               |
| visit_derive_fallback_matches_clean_full_rebuild_across_batches | 521  | D   | Undoing a visit then rebuilding gives the same facts as a clean build; data integrity                                                                                                                | none (no E2E checks Insights after an undo)                                                                                           | Keep; use `limit: Some(n)` instead of 8,192 + 17 visits (the constant is cfg(test), mod.rs:410)                                                 |
| daily_rollup_fallback_matches_clean_full_rebuild_across_batches | 620  | D   | Same for daily rollups                                                                                                                                                                               | none                                                                                                                                  | Keep; same trim                                                                                                                                 |

#### src/intelligence/tests/frequent_searches.rs

| Test                                              | Line | Cat | Reason                                                                                                       | E2E coverage                                                                     | Action                                             |
| ------------------------------------------------- | ---- | --- | ------------------------------------------------------------------------------------------------------------ | -------------------------------------------------------------------------------- | -------------------------------------------------- |
| counts_only_the_keyword_searches_inside_the_range | 111  | D   | Range edges to the millisecond, keyword-only, never above the digest total: the bug in ipc-performance.md §8 | insights-drill "a search from Frequent searches…" (all-time count only)          | Keep                                               |
| honours_the_limit_and_the_profile_filter          | 142  | D   | Profile filter and limit on a ranked read                                                                    | partly, insights.spec "a custom range and one browser profile…" (Top sites only) | Keep                                               |
| shows_the_newest_spelling_of_each_query           | 154  | D   | One query across engines and spellings, newest spelling shown (module header states the rule)                | none                                                                             | Keep; note it writes `search_events_rows` directly |
| the_range_read_walks_the_kind_and_time_index      | 183  | D   | Plan guard named in ipc-performance.md §8 (`idx_search_events_kind_time`)                                    | n/a                                                                              | Keep                                               |

#### src/intelligence/tests/new_sites.rs

| Test                                               | Line | Cat | Reason                                                                                            | E2E coverage                                                         | Action |
| -------------------------------------------------- | ---- | --- | ------------------------------------------------------------------------------------------------- | -------------------------------------------------------------------- | ------ |
| a_site_is_new_once_on_its_first_day_in_any_profile | 68   | D   | Real bug (5,244 new sites vs 4,115 sites), range edges, per-profile rule in screens-and-nav.md:69 | insights.spec "the cards brought back from 0.3…" (all profiles only) | Keep   |

#### src/intelligence/tests/rhythm.rs

| Test                                                     | Line | Cat | Reason                                                                                            | E2E coverage                                                   | Action |
| -------------------------------------------------------- | ---- | --- | ------------------------------------------------------------------------------------------------- | -------------------------------------------------------------- | ------ |
| the_rhythm_from_daily_hours_matches_counting_every_visit | 26   | D   | Fast path (stored hour counts) equals the per-visit reference, incl. local time and profile scope | insights.spec "the cards brought back…" (weekday average only) | Keep   |

#### src/intelligence/tests/schema_overview.rs

| Test                                                                       | Line | Cat | Reason                                                                                                                                         | E2E coverage                                                                         | Action                                                                                                                                            |
| -------------------------------------------------------------------------- | ---- | --- | ---------------------------------------------------------------------------------------------------------------------------------------------- | ------------------------------------------------------------------------------------ | ------------------------------------------------------------------------------------------------------------------------------------------------- |
| collapse_date_key_supports_week_and_month                                  | 58   | C   | Restates a private helper; `"not-a-date"` passthrough probe                                                                                    | insights.spec "Insights opens on all time…" (week/month axes)                        | Delete                                                                                                                                            |
| build_kpi_reports_flat_when_values_match                                   | 66   | C   | Restates a 10-line function and its `"up"/"down"/"flat"` strings                                                                               | Partly: all time shows no %; no ranged comparison checked                            | Delete; E2E "30 days: Page visits shows the change against the previous 30 days counted from the fixture"                                         |
| shared_query_and_date_helpers_cover_boundary_inputs                        | 82   | C   | "cover" grab-bag of private helpers (tokenizer, jaccard, previous range, `i64::MAX` fallback)                                                  | Previous-period rule: none                                                           | Delete; the previous-range rule belongs in the E2E above                                                                                          |
| normalize_query_trims_and_lowercases                                       | 106  | C   | One assert; `shows_the_newest_spelling_of_each_query` and frequent_searches cover the grouping                                                 | insights-drill "a search from Frequent searches…"                                    | Delete                                                                                                                                            |
| classifies_url_like_search_queries_as_navigational_noise                   | 112  | D   | Pure classifier with a doc rule (core-intelligence-ultimate-design.md:456), incl. suffix-confusion edge                                        | none                                                                                 | Keep (doc comment above it is wrong: says "schema records versioned migrations")                                                                  |
| primary_overview_reuses_single_connection_and_runtime_snapshot             | 145  | B   | Pins `timings.len() == 11` and section-id strings; the one-runtime-query check is a trivial cost                                               | insights.spec "…read in two batches" (command level)                                 | Delete                                                                                                                                            |
| all_time_primary_overview_serves_persisted_snapshot                        | 199  | D   | All time must come from the snapshot (screens-and-nav.md:69; ipc-performance.md §11: 1.5 s vs over a minute)                                   | none can see it                                                                      | Keep; merge with the secondary twin into one test over both bands; it probes via `total_duration_ms` and pins `scope_key='all-time:archive-wide'` |
| all_time_secondary_overview_serves_persisted_snapshot                      | 284  | D   | Same for the second band                                                                                                                       | none                                                                                 | Merge with the above                                                                                                                              |
| primary_overview_marks_empty_digest_as_flat                                | 364  | C   | `build_kpi(0, 0)` again                                                                                                                        | none for an empty archive                                                            | Delete                                                                                                                                            |
| secondary_overview_reuses_single_connection_and_runtime_snapshot           | 394  | B   | Pins `timings.len() == 10` and section ids                                                                                                     | insights.spec "…read in two batches"                                                 | Delete                                                                                                                                            |
| discovery_trend_reports_available_years_and_respects_profile_scope         | 447  | D   | `available_years` drives Home's year pager (`src/features/home/home-cards.tsx:230`), per profile                                               | insights-drill "a day from the Home heatmap…" (not the year list)                    | Keep                                                                                                                                              |
| day_insights_compose_exact_day_entities_and_drilldown_metadata             | 511  | C   | Smoke test with `any`/`!is_empty` asserts                                                                                                      | insights-drill "a day from the Home heatmap matches the fixture…"                    | Delete                                                                                                                                            |
| domain_deep_dive_keeps_day_scoped_trend_consistent_with_exact_day_insights | 558  | C   | Three counts on a 3-visit fixture; weak cross-check                                                                                            | insights-drill "a site from Top sites shows its 30-day visits"                       | Delete                                                                                                                                            |
| domain_calendar_rhythm_and_flow_helpers_cover_multi_visit_edges            | 616  | C   | "cover" grab-bag: `path_from_url`, deep dive, rhythm `max_count > 0`, on this day, zero-total discovery rate                                   | site drill-in covered by insights-drill; On this day: none                           | Delete; E2E "Home's On this day lists the fixture's sites from a year ago today" if wanted                                                        |
| search_queries_reuse_family_and_trail_identity                             | 766  | B   | Hand-inserts `query_families_rows` to pin which family wins under `family-frequency`; the app only asks for `newest`                           | insights-drill day test (newest only)                                                | Delete                                                                                                                                            |
| keyword_surfaces_filter_navigational_noise_and_support_domain_reads        | 943  | D   | Navigational noise kept out of concepts and per-site search lists (core-intelligence-ultimate-design.md:31, :456)                              | insights.spec "the cards brought back…" (top word, fixture has no URL-like searches) | Keep                                                                                                                                              |
| refind_navigation_and_hub_reads_reuse_persisted_structural_context         | 1042 | C   | Grab-bag; hand-inserts trail rows; most asserts (`get_navigation_path` cycles, `get_hub_pages`, `explain_refind`) hit commands no screen calls | insights-drill "a page you keep reopening…"; insights.spec "Days visited" order      | Delete                                                                                                                                            |

#### src/intelligence/tests/search_families.rs

| Test                                                | Line | Cat | Reason                                                                                | E2E coverage                                                                | Action |
| --------------------------------------------------- | ---- | --- | ------------------------------------------------------------------------------------- | --------------------------------------------------------------------------- | ------ |
| every_family_of_a_search_is_found_however_old_it_is | 64   | D   | Real bug: the search page read only the newest 200 families ("search must be global") | insights-drill "a search from Frequent searches…" (not the old-family case) | Keep   |

#### src/intelligence/tests/session_paths.rs

| Test                                                              | Line | Cat | Reason                                                                                                                        | E2E coverage                                                                | Action                                |
| ----------------------------------------------------------------- | ---- | --- | ----------------------------------------------------------------------------------------------------------------------------- | --------------------------------------------------------------------------- | ------------------------------------- |
| a_range_counts_each_path_once_across_segments_batches_and_appends | 134  | D   | Paths across 256-site segments, local midnight, batch and append edges against a slow reference (data-model.md session_paths) | insights.spec "the most frequent path between three sites…" (top path only) | Keep; model for the other batch tests |

#### src/intelligence/tests/site_days.rs

| Test                                                | Line | Cat | Reason                                                                                                                     | E2E coverage                                 | Action                                                                                             |
| --------------------------------------------------- | ---- | --- | -------------------------------------------------------------------------------------------------------------------------- | -------------------------------------------- | -------------------------------------------------------------------------------------------------- |
| a_kept_range_is_read_again_after_a_build_changes_it | 67   | D   | Cache invalidation of the shared site-day read (data-model.md: "read nothing new until a build or clear changes the data") | none (no E2E checks Insights after a backup) | Keep; E2E "a backup that adds visits updates Top sites without a reload" would cover it end to end |

#### src/intelligence/tests/stage_rebuild.rs

| Test                                                                       | Line | Cat | Reason                                                                                                                                   | E2E coverage                                                                                | Action                                                                                                                |
| -------------------------------------------------------------------------- | ---- | --- | ---------------------------------------------------------------------------------------------------------------------------------------- | ------------------------------------------------------------------------------------------- | --------------------------------------------------------------------------------------------------------------------- |
| clear_derived_intelligence_state_reports_canonical_group_counts            | 54   | B   | Hand-copied table-group lists; already drifted from production (no `compare_sets`, intelligence_schema.rs:144)                           | background-work.spec "background work: … clear and rebuild" (preview, clear, zero, rebuild) | Delete                                                                                                                |
| merge_stage_run_result_sums_stage_timings_across_profiles                  | 204  | C   | Restates addition on a report field no screen shows                                                                                      | none needed                                                                                 | Delete                                                                                                                |
| rebuild_with_progress_reports_noop_when_scope_has_no_profiles              | 235  | B   | Pins `"noop"` and English note text; the useful bit is "modules end ready on an empty archive"                                           | none                                                                                        | Replace with E2E "after Delete all data and onboarding with no browser, Insights shows its empty state, not Updating" |
| scoped_rebuild_uses_batches_and_progress_callbacks                         | 276  | B   | Pins phase strings, the first progress tuple and `"fallback-full"`                                                                       | background-work.spec (job runs to Done)                                                     | Delete                                                                                                                |
| explain_entity_and_provider_snapshots_build_from_core_intelligence_tables  | 337  | C   | Grab-bag over `explain_entity`, embed cards, widget and public snapshots, none called by the app; hand-inserted `*_rows`                 | n/a                                                                                         | Delete; if the public snapshot ever ships, keep only "it holds no URLs or visit ids"                                  |
| visit_derive_stage_processes_only_new_visible_visits                       | 525  | D   | An append derives only the new visit (full rebuild is 5,795 s at 14.4M, ipc-performance.md §11)                                          | none                                                                                        | Keep; drop the "already up to date" text asserts                                                                      |
| visit_derive_stage_handles_version_drift_and_empty_profile_clearing        | 622  | D   | Changed rules force a full rebuild; undoing every visit clears derived facts                                                             | none                                                                                        | Keep; drop reason-text asserts; merge with the two below into one per-stage test                                      |
| structural_stage_handles_noop_version_drift_and_empty_profile_clearing     | 703  | D   | Same for the structural stage                                                                                                            | none                                                                                        | Merge (see above)                                                                                                     |
| daily_rollup_stage_recomputes_only_dirty_days                              | 797  | D   | Incremental daily rollups touch only the new days and stay correct                                                                       | none                                                                                        | Keep                                                                                                                  |
| daily_rollup_stage_clears_when_profile_has_no_visible_visits               | 926  | D   | Undoing every visit clears rollups                                                                                                       | none                                                                                        | Merge with 622/703                                                                                                    |
| daily_rollup_resumes_an_interrupted_fact_batch_without_missing_delta_rows  | 994  | D   | Interrupted batch resumes without losing rows (crash recovery)                                                                           | none                                                                                        | Keep                                                                                                                  |
| daily_rollup_stage_falls_back_after_version_drift                          | 1068 | C   | Near-duplicate of the drift half of 622/703; asserts only mode and English reason                                                        | none                                                                                        | Delete (fold into the merged test)                                                                                    |
| daily_rollup_fallback_collapses_conflicting_categories_into_one_domain_row | 1124 | B   | Injects categories into internal rows to pin the tie-break winner; the rule now has direct tests in `intelligence_daily_rollup_state.rs` | insights.spec "kinds of sites count every visit…"                                           | Delete                                                                                                                |
| visit_derive_stage_falls_back_full_after_visibility_regression             | 1222 | D   | Undoing one visit removes it from derived facts                                                                                          | none                                                                                        | Keep; drop reason text; E2E "undoing an import removes its visits from Insights' total"                               |
| stage_jobs_take_the_manual_full_rebuild_branch_when_forced                 | 1284 | C   | Says it "covers the force_full branch"; asserts only an internal mode string                                                             | background-work.spec (rebuild)                                                              | Delete                                                                                                                |

#### src/intelligence/tests/structural_incremental.rs

| Test                                                              | Line | Cat | Reason                                                                                                                  | E2E coverage                                                | Action                                                                                                                                              |
| ----------------------------------------------------------------- | ---- | --- | ----------------------------------------------------------------------------------------------------------------------- | ----------------------------------------------------------- | --------------------------------------------------------------------------------------------------------------------------------------------------- |
| late_visits_rebuild_tail_assignments_without_duplicates           | 43   | D   | A late visit joins its session and trail, no duplicate (data-model.md:628)                                              | none                                                        | Keep; assert "one shared id" instead of the literal `session:chrome:Default:1`; fix the doc comment ("incrementally" is wrong, it is a full replay) |
| late_visit_chains_keep_assignments_stable_across_batch_boundaries | 135  | D   | Same across a batch edge                                                                                                | none                                                        | Keep; use `limit` instead of the cfg(test) 8,192 constant (header claims it is production's; it is not)                                             |
| late_search_events_are_reclassified_after_their_landing_domain    | 227  | B   | Does not test what its name says (the query is not URL-like); counts internal `search_event_terms` rows                 | none                                                        | Delete                                                                                                                                              |
| structural_range_delete_preserves_unaffected_rows_before_start_ms | 327  | B   | Premise (range delete) is gone, late visits replay everything; pins id strings and internal row counts                  | none                                                        | Delete                                                                                                                                              |
| path_flows_support_four_step_queries_and_explanations             | 443  | C   | Four-step flows and `explain_entity` path-flow factors; the app shows three-site paths and never calls `explain_entity` | insights.spec "the most frequent path between three sites…" | Delete                                                                                                                                              |

#### src/intelligence/tests/top_sites_profiles.rs

| Test                                                                           | Line | Cat | Reason                                                                                                     | E2E coverage                                                               | Action |
| ------------------------------------------------------------------------------ | ---- | --- | ---------------------------------------------------------------------------------------------------------- | -------------------------------------------------------------------------- | ------ |
| several_profiles_are_summed_and_paged_through_every_site_once_without_searches | 70   | D   | Server-side sum over several profiles, paging without repeats, search pages excluded (intelligence.md:290) | insights.spec "a custom range and one browser profile…" (one profile only) | Keep   |

### Intelligence runtime, sections, catalog, and integration tests in tests/

#### src/intelligence_runtime/job_control.rs

| Test                                               | Line | Cat | Reason                                                                                                                                                        | E2E coverage                                                                | Action |
| -------------------------------------------------- | ---- | --- | ------------------------------------------------------------------------------------------------------------------------------------------------------------- | --------------------------------------------------------------------------- | ------ |
| connection_level_retry_and_cancel_cover_race_edges | 207  | C   | Calls private helpers with states the public path never passes. The "vanished job" race cannot happen inside one write transaction. Asserts error substrings. | background-work.spec "queue, pause, cancel, retry…" covers retry and cancel | delete |

#### src/intelligence_runtime/tests_queue.rs

| Test                                                                         | Line | Cat | Reason                                                                                                                                                                                                                                 | E2E coverage                                                                    | Action                                                            |
| ---------------------------------------------------------------------------- | ---- | --- | -------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | ------------------------------------------------------------------------------- | ----------------------------------------------------------------- |
| queue_jobs_can_be_enqueued_and_loaded                                        | 23   | C   | Exercises `#[cfg(test)]` `enqueue_enrichment_job`. Production never enqueues `enrichment-plugin` jobs. The asserts are a trivial count and id.                                                                                         | none (no producer)                                                              | delete with the test-only enqueue                                 |
| deterministic_rebuild_jobs_are_traced_in_runtime_queue                       | 54   | B   | Pins the English title "chrome:Default · full Core Intelligence rebuild". The UI builds its own localized name for core jobs (`job-view.ts:96-98`). Uses `claim_deterministic_rebuild_job_in_session`, which no production code calls. | background-work.spec: the queued rebuild shows as "All browsers · Everything"   | delete                                                            |
| malformed_core_payload_is_quarantined_after_claim                            | 94   | C   | Session-level near-duplicate of `tests/intelligence_turso.rs:281`, which also checks that the quarantine commits                                                                                                                       | not user-visible                                                                | delete (keep the integration test)                                |
| malformed_enrichment_payload_is_quarantined_before_claim                     | 130  | D   | Regression: a malformed queue head was selected forever                                                                                                                                                                                | —                                                                               | keep                                                              |
| enqueue_runtime_helpers_dedupe_refresh_and_mark_stale_contracts              | 172  | B   | Kitchen sink: trigger counts, error substring, test-only enqueue, unknown module ignored, stale-row SQL                                                                                                                                | dedupe is covered by tests/intelligence_turso.rs:388                            | delete                                                            |
| recent_jobs_surface_progress_for_running_deterministic_rebuilds              | 315  | C   | Field-by-field copy of the artifact JSON into the overview. Restates the mapping. The UI uses only current, total and percent.                                                                                                         | none: E2E "a running Insights rebuild shows 'N of M visits' that grows to done" | replace with E2E scenario                                         |
| job_control_state_transitions_persist_success_failure_and_worker_cancel      | 374  | D   | Spec deterministic-intelligence.md:514: success must not overwrite cancel or failed; cooperative cancel                                                                                                                                | —                                                                               | keep                                                              |
| deterministic_rebuild_jobs_run_before_optional_enrichment                    | 491  | C   | `next_queued_intelligence_job` only selects core types (`claims.rs:348`), so the assert is true by construction. Uses the test-only enqueue.                                                                                           | none                                                                            | delete                                                            |
| plugin_enabled_respects_global_toggle                                        | 530  | C   | Restates a config lookup. No UI toggles enrichment.                                                                                                                                                                                    | none                                                                            | delete                                                            |
| running_jobs_can_be_requeued_globally_or_by_run                              | 549  | C   | Tests `#[cfg(test)]`-only `requeue_running_enrichment_jobs*`                                                                                                                                                                           | none                                                                            | delete with the functions                                         |
| claim_enrichment_jobs_scans_past_disallowed_queue_fronts                     | 613  | C   | Tests `#[cfg(test)]`-only `claim_enrichment_jobs`                                                                                                                                                                                      | none                                                                            | delete with the function                                          |
| compare_and_set_claim_skips_jobs_taken_by_another_connection                 | 680  | D   | Real two-thread race on the claim, plaintext and encrypted                                                                                                                                                                             | —                                                                               | keep                                                              |
| retry_and_cancel_require_valid_job_states                                    | 747  | D   | Backend guard for the Retry and Cancel rules. Cancelling a running job sets stop_requested, which E2E does not reach.                                                                                                                  | background-work.spec covers queued→cancel→retry                                 | keep, trim the error-substring asserts                            |
| retry_and_cancel_fail_for_missing_jobs                                       | 836  | C   | Missing id → error containing "404"                                                                                                                                                                                                    | none                                                                            | delete                                                            |
| next_queued_intelligence_job_recovers_expired_leases                         | 861  | D   | Lease recovery, spec deterministic-intelligence.md:514                                                                                                                                                                                 | —                                                                               | keep, trim the `last_error` text assert (not shown unless failed) |
| content_fetch_jobs_dedupe_by_canonical_url_and_claim_by_type                 | 933  | D   | Canonical-URL dedupe and lane isolation                                                                                                                                                                                                | —                                                                               | keep                                                              |
| content_fetch_job_due_respects_version_and_negative_cache                    | 987  | D   | Negative cache and extractor-version refetch rules                                                                                                                                                                                     | —                                                                               | keep                                                              |
| content_fetch_job_due_compares_timestamps_as_instants_at_the_boundary_second | 1054 | D   | Regression CORR-4: text comparison misread a due row                                                                                                                                                                                   | —                                                                               | keep                                                              |
| requeue_content_fetch_job_after_returns_a_running_row_to_queued              | 1093 | D   | Regression SEC-2: a rate-limited job is requeued, not cancelled                                                                                                                                                                        | —                                                                               | keep                                                              |
| next_content_fetch_schedule_eta_reports_soonest_future_then_none             | 1132 | A?  | `Some(1)` for an unparseable stamp is a 1 Hz busy loop (see A? items). The rest is fine.                                                                                                                                               | none (content fetch has no UI)                                                  | fix the code, drop that block                                     |
| job_history_pages_across_equal_update_times_and_seeks_to_the_cursor          | 1190 | D   | Completeness across ties, plus an index-seek plan guard                                                                                                                                                                                | —                                                                               | keep                                                              |

#### src/intelligence_runtime/tests_runtime.rs

| Test                                                                  | Line | Cat | Reason                                                                                                                          | E2E coverage                                                                                  | Action                                                                  |
| --------------------------------------------------------------------- | ---- | --- | ------------------------------------------------------------------------------------------------------------------------------- | --------------------------------------------------------------------------------------------- | ----------------------------------------------------------------------- |
| runtime_snapshot_returns_plugin_defaults_before_archive_exists        | 39   | B   | English `notes` that no UI renders. Plugin count is 2. Misnamed: the archive exists.                                            | none                                                                                          | delete                                                                  |
| load_intelligence_runtime_recovers_interrupted_deterministic_jobs     | 56   | D   | Crash recovery: a running job is requeued on reopen, plaintext and encrypted. Misnamed: the recovery happens at `Vault::open`.  | —                                                                                             | keep (rename)                                                           |
| load_intelligence_runtime_recovers_interrupted_enrichment_jobs        | 60   | C   | Same helper, other job type. Recovery SQL ignores the type (`mod.rs:509`).                                                      | covered by the twin above                                                                     | delete                                                                  |
| load_intelligence_runtime_only_recovers_running_jobs_once_per_archive | 64   | D   | Reads must not reclaim a live job                                                                                               | —                                                                                             | keep (rename: "reads never recover")                                    |
| runtime_snapshot_projects_disabled_stale_and_failed_runtime_rows      | 74   | D   | Version drift → stale with `module-version-changed`, shown in Settings (`derived-data-rows.tsx:42`). Missing timestamp → stale. | none for version drift: E2E "after an app update Settings says the Insights data predates it" | keep, trim the English `stale_reason`/notes and disabled-branch asserts |
| runtime_snapshot_marks_missing_disabled_module_rows                   | 219  | B   | Disabled modules cannot be set from the UI. Pins the English note "Disabled in Settings.".                                      | none                                                                                          | delete                                                                  |

#### src/intelligence_catalog.rs

| Test                                                         | Line | Cat | Reason                                                            | E2E coverage                             | Action                                                     |
| ------------------------------------------------------------ | ---- | --- | ----------------------------------------------------------------- | ---------------------------------------- | ---------------------------------------------------------- |
| structural_rebuild_mode_resolves_modules_in_dependency_order | 377  | B   | Exact five-id list. The real rule is dependency order.            | insights.spec checks full-rebuild output | delete, or replace with "each module after its depends_on" |
| rebuild_modes_expose_stable_queue_job_types                  | 393  | C   | Restates a `match`                                                | —                                        | delete                                                     |
| registry_resolves_entity_owners                              | 401  | C   | Tests the `#[cfg(test)]`-only `module_descriptor_for_entity_type` | —                                        | delete with the function                                   |
| registry_exposes_trait_backed_built_ins                      | 411  | C   | Length is 8 and the first id                                      | —                                        | delete                                                     |
| resolve_intelligence_module_order_rejects_unknown_modules    | 421  | C   | Unreachable: callers pass only built-in ids. Error substring.     | —                                        | delete                                                     |
| module_dependency_cycle_guard_reports_the_current_node       | 427  | C   | Private helper. The registry has no cycles. Error substring.      | —                                        | delete                                                     |

#### src/intelligence_sections.rs

| Test                                                                     | Line | Cat | Reason                                                                                                                | E2E coverage                                                                         | Action                               |
| ------------------------------------------------------------------------ | ---- | --- | --------------------------------------------------------------------------------------------------------------------- | ------------------------------------------------------------------------------------ | ------------------------------------ |
| section_registry_exposes_search_activity_descriptor                      | 449  | C   | Restates a const table                                                                                                | —                                                                                    | delete                               |
| section_registry_exposes_day_insights_descriptor                         | 456  | C   | Restates a const table                                                                                                | —                                                                                    | delete                               |
| module_backed_meta_prefers_stale_over_ready_and_uses_earliest_build_time | 467  | D   | Worst state wins, and "computed" uses the oldest module build. Drives the "updating" spinner and the "Computed" time. | partly: insights.spec                                                                | keep, trim the English reason assert |
| module_backed_meta_marks_idle_modules_as_degraded                        | 501  | A   | Pins English prose as user-visible `stateReason`; slice said A?, report rates A (see A2)                              | none: E2E "after Clear Insights data, a zh-TW card shows its limited state in zh-TW" | fix the code (state reason code)     |
| module_backed_meta_marks_disabled_modules_with_fallback_reason           | 521  | C   | Disabled cannot be set from the UI. English fallback string.                                                          | none                                                                                 | delete                               |
| direct_meta_uses_query_time_and_capability_gated_empty_reason            | 539  | A?  | Same as A2, for observed interactions; no screen reads it today (see A? items)                                        | none (no feature calls `observedInteractions` today)                                 | fix the code, then delete            |
| batch_meta_builder_reuses_the_provided_runtime_snapshot                  | 560  | C   | Name claims reuse, but it asserts no reuse. It restates the table's module_ids.                                       | —                                                                                    | delete                               |

#### tests/agent_store_paging.rs

| Test                                                | Line | Cat | Reason                                                      | E2E coverage | Action |
| --------------------------------------------------- | ---- | --- | ----------------------------------------------------------- | ------------ | ------ |
| a_failed_replacement_preserves_the_saved_transcript | 10   | D   | Atomic replace: a failed save keeps the old transcript      | —            | keep   |
| committed_agent_journal_survives_process_death      | 33   | D   | Kill-after-commit crash test, plaintext and encrypted       | —            | keep   |
| journal_crash_child                                 | 85   | D   | `#[ignore]` subprocess fixture for the test above           | —            | keep   |
| chats_and_messages_reach_every_row_after_reopen     | 126  | D   | Paging completeness across ties (426 chats, 2,501 messages) | —            | keep   |

#### tests/ai_queue_turso.rs

| Test                                                             | Line | Cat | Reason                                   | E2E coverage | Action |
| ---------------------------------------------------------------- | ---- | --- | ---------------------------------------- | ------------ | ------ |
| reopening_recovers_only_abandoned_claims_and_keeps_finished_work | 9    | D   | Crash recovery keeps the resume cursor   | —            | keep   |
| queue_pages_reach_every_job_and_a_cancel_cannot_become_success   | 44   | D   | Completeness, plus a cancel/success race | —            | keep   |
| process_exit_requeues_inflight_claim_and_preserves_completed_job | 70   | D   | Real process exit mid-claim              | —            | keep   |

#### tests/candle_embedding_e2e.rs

Gated by `#![cfg(not(coverage))]` and env vars. Without them the tests return at once and count as passes in `cargo test`. Nothing in `bun run check` or CI sets the env vars.

| Test                             | Line | Cat | Reason                                                                                    | E2E coverage | Action                                                                           |
| -------------------------------- | ---- | --- | ----------------------------------------------------------------------------------------- | ------------ | -------------------------------------------------------------------------------- |
| candle_s1_throughput_benchmark   | 173  | D   | Manual benchmark (`PATHKEEP_CANDLE_S1=1`)                                                 | —            | keep; use `#[ignore]` instead of an early return                                 |
| candle_vs_lmstudio_cosine_parity | 196  | D   | Correctness proof against a reference model (`PATHKEEP_CANDLE_PARITY=1`, needs LM Studio) | —            | keep; use `#[ignore]`; the header mentions a coverage gate that no longer exists |

#### tests/enrichment_turso.rs

| Test                                                           | Line | Cat | Reason                                                                                       | E2E coverage                       | Action |
| -------------------------------------------------------------- | ---- | --- | -------------------------------------------------------------------------------------------- | ---------------------------------- | ------ |
| native_pages_offer_all_candidates_and_reoffer_a_prearchive_job | 11   | D   | A parked job is re-offered with a live visit; every candidate is enqueued past the page size | — (no UI toggle for content fetch) | keep   |
| a_page_without_visits_reoffers_a_parked_candidate              | 73   | D   | Regression: a page with no visits was forgotten                                              | —                                  | keep   |

#### tests/intelligence_agent_pages.rs

| Test                                                       | Line | Cat | Reason                                             | E2E coverage | Action |
| ---------------------------------------------------------- | ---- | --- | -------------------------------------------------- | ------------ | ------ |
| oversized_native_reports_page_through_every_domain         | 12   | D   | 8 KB model budget keeps all 300 domains            | —            | keep   |
| agent_session_detail_forwards_the_independent_trail_cursor | 58   | D   | The cursor is forwarded and every trail is reached | —            | keep   |

#### tests/intelligence_explanation_pages.rs

| Test                                                                | Line | Cat | Reason                                                    | E2E coverage               | Action |
| ------------------------------------------------------------------- | ---- | --- | --------------------------------------------------------- | -------------------------- | ------ |
| entity_evidence_pages_preserve_every_visit_and_whole_entity_factors | 9    | D   | Completeness, factors stable across pages, no visit scans | partly insights-drill.spec | keep   |

#### tests/intelligence_generation_plans.rs

| Test                                            | Line | Cat | Reason                                 | E2E coverage | Action |
| ----------------------------------------------- | ---- | --- | -------------------------------------- | ------------ | ------ |
| generation_views_seek_assignment_keys           | 7    | D   | Plan guard: the join seeks by visit_id | —            | keep   |
| derived_batch_looks_up_only_requested_visit_ids | 17   | D   | Plan guard on the batch lookup         | —            | keep   |

#### tests/intelligence_keyset_plans.rs

| Test                                                               | Line | Cat | Reason                                                      | E2E coverage | Action |
| ------------------------------------------------------------------ | ---- | --- | ----------------------------------------------------------- | ------------ | ------ |
| session_trail_and_family_pages_seek_to_their_cursor                | 76   | D   | Later pages seek to the cursor, not quadratic               | —            | keep   |
| ranked_summary_pages_seek_to_their_cursor                          | 138  | D   | Same, for friction, reopened, topics, refind                | —            | keep   |
| one_profiles_all_time_flows_page_through_their_rows_in_index_order | 209  | D   | Same, for path flows                                        | —            | keep   |
| observed_interaction_pages_seek_the_visit_time_index_to_the_cursor | 233  | D   | Same; the `visit_time_ms<=?` text check is a little brittle | —            | keep   |

#### tests/intelligence_paging.rs

Every test here asserts that every row is reached through its cursor, per "search must be global".

| Test                                                                       | Line | Cat | Reason                                                                                                                             | E2E coverage                                                 | Action                                              |
| -------------------------------------------------------------------------- | ---- | --- | ---------------------------------------------------------------------------------------------------------------------------------- | ------------------------------------------------------------ | --------------------------------------------------- |
| top_sites_page_through_every_domain_in_each_sort_order                     | 8    | D   | All 113 rows in every sort order                                                                                                   | insights-drill.spec covers one site only                     | keep                                                |
| refind_pages_reach_tied_results_and_choose_the_best_profile                | 58   | D   | Ties reached; the merged row picks the best profile                                                                                | —                                                            | keep                                                |
| frequent_queries_and_concepts_continue_beyond_their_former_limits          | 103  | D   | Regression: old caps                                                                                                               | —                                                            | keep                                                |
| sessions_trails_and_families_continue_across_timestamp_ties                | 163  | D   | Ties across pages                                                                                                                  | —                                                            | keep                                                |
| hub_pages_continue_beyond_the_former_ten_page_limit                        | 251  | D   | Regression: old cap. `get_hub_pages` has no frontend caller.                                                                       | —                                                            | keep, or delete with the command if it stays unused |
| summary_lists_keep_every_source_friction_signal_and_reopened_investigation | 298  | D   | Completeness for three lists                                                                                                       | —                                                            | keep                                                |
| materialized_all_time_flows_page_past_twenty_results                       | 360  | D   | Regression: cap of 20, strict order                                                                                                | —                                                            | keep                                                |
| sparse_observed_evidence_has_continuation_even_when_a_page_is_empty        | 408  | D   | Completeness over sparse evidence; but `empty_continuations > 0` requires empty pages                                              | —                                                            | keep, trim the `empty_continuations` assert         |
| search_effectiveness_pages_through_every_hard_topic                        | 452  | D   | Completeness                                                                                                                       | —                                                            | keep                                                |
| domain_details_page_through_all_pages_and_landing_queries                  | 486  | D   | Two independent cursors                                                                                                            | insights-drill.spec "a site from Top sites…" covers part     | keep                                                |
| refind_detail_reaches_all_visits_days_and_related_trails                   | 540  | D   | Completeness                                                                                                                       | insights-drill.spec "a page you keep reopening…" covers part | keep                                                |
| query_family_details_page_through_every_related_trail                      | 595  | D   | Completeness                                                                                                                       | —                                                            | keep                                                |
| recent_queries_have_complete_cursors_for_every_sort                        | 634  | D   | All sorts, exact total                                                                                                             | —                                                            | keep                                                |
| session_and_trail_details_page_in_navigation_order                         | 689  | D   | Order and completeness                                                                                                             | —                                                            | keep                                                |
| anniversary_domains_continue_and_deep_sessions_are_counted_once            | 755  | D   | Regression: a deep session counted twice                                                                                           | —                                                            | keep                                                |
| curated_cards_and_widgets_expose_every_selected_card                       | 800  | B   | Pins the exact card ids `["digest:visits","digest:searches"]` on an empty archive. Embed cards and widget have no frontend caller. | none                                                         | delete (and decide whether the commands stay)       |

#### tests/intelligence_read_plans.rs

| Test                                                   | Line | Cat | Reason                                   | E2E coverage | Action |
| ------------------------------------------------------ | ---- | --- | ---------------------------------------- | ------------ | ------ |
| the_kpi_row_seeks_its_tables_for_every_profile_and_one | 67   | D   | 14.4M perf guard: the KPI row took 126 s | —            | keep   |
| the_slow_range_reads_seek_their_tables                 | 85   | D   | Same, for the slow range reads           | —            | keep   |

#### tests/intelligence_rebuild_bench.rs

| Test                                 | Line | Cat | Reason                                                      | E2E coverage | Action |
| ------------------------------------ | ---- | --- | ----------------------------------------------------------- | ------------ | ------ |
| full_rebuild_of_an_e2e_sized_archive | 49   | D   | `#[ignore]` timing tool; no asserts, prints per-phase times | —            | keep   |

#### tests/intelligence_turso.rs

| Test                                                                           | Line | Cat | Reason                                                    | E2E coverage | Action                                                                  |
| ------------------------------------------------------------------------------ | ---- | --- | --------------------------------------------------------- | ------------ | ----------------------------------------------------------------------- |
| many_batches_equal_one_pass_and_incremental_equals_full                        | 87   | D   | Batch, single-pass and incremental results are equal      | —            | keep                                                                    |
| an_interrupted_build_resumes_and_an_interactive_star_commits_before_completion | 126  | D   | Resume after an error; interactive writes are not starved | —            | keep                                                                    |
| process_exit_mid_rebuild_resumes_without_duplicate_facts                       | 180  | D   | Real process exit, resume from the last commit            | —            | keep                                                                    |
| sessions_that_started_more_than_a_day_earlier_still_overlap_the_requested_day  | 250  | D   | Regression: a long session dropped from its later days    | —            | keep                                                                    |
| malformed_job_claim_commits_its_quarantine                                     | 281  | D   | The quarantine commits through the owner                  | —            | keep                                                                    |
| a_changed_archive_retargets_an_interrupted_generation                          | 306  | D   | Data integrity after archive change and revert            | —            | keep, trim the `execution_mode == "noop"` string                        |
| a_long_session_resumes_from_durable_domain_counts                              | 349  | D   | Incremental perf and correctness (`domain_count`)         | —            | keep, trim: the negative SQL-substring check passes after any rewording |
| work_requested_during_a_claim_is_not_lost_at_completion                        | 388  | D   | Race regression                                           | —            | keep                                                                    |
| a_rebuild_does_not_overwrite_a_newer_invalidation                              | 410  | D   | Race regression                                           | —            | keep                                                                    |

#### tests/intelligence_turso/render_plans.rs

| Test                                                   | Line | Cat | Reason                                     | E2E coverage | Action |
| ------------------------------------------------------ | ---- | --- | ------------------------------------------ | ------------ | ------ |
| render_reads_use_indexed_archive_and_membership_access | 7    | D   | No endpoint scans canonical visits or urls | —            | keep   |

#### tests/lmstudio_agent_e2e.rs, lmstudio_chat_stream_e2e.rs, lmstudio_embedding_e2e.rs

How they are gated:

- The agent tests use `#[ignore]` and `PATHKEEP_AGENT_E2E=1`.
- The chat-stream and embedding tests are not ignored. Without `PATHKEEP_LMSTUDIO_E2E=1` they return at once and count as passes in `cargo test --workspace`.
- Nothing in check or CI sets these variables.

| Test                                                                      | Line | Cat | Reason                                                       | E2E coverage | Action                |
| ------------------------------------------------------------------------- | ---- | --- | ------------------------------------------------------------ | ------------ | --------------------- |
| lmstudio_agent_runs_the_full_tool_loop_and_journals_a_durable_cited_trace | 258  | D   | Real provider; E2E cannot reach it                           | —            | keep (manual)         |
| lmstudio_agent_cancel_mid_loop_emits_a_graceful_done                      | 487  | D   | Same                                                         | —            | keep (manual)         |
| lmstudio_chat_stream_yields_tokens_and_maybe_reasoning                    | 48   | D   | Same                                                         | —            | keep; add `#[ignore]` |
| lmstudio_embedding_returns_consistent_normalized_distinct_vectors         | 47   | D   | Same; exact equality of repeated vectors may be flaky on GPU | —            | keep; add `#[ignore]` |

#### tests/open_file_budget.rs

| Test                                                       | Line | Cat | Reason                                               | E2E coverage | Action |
| ---------------------------------------------------------- | ---- | --- | ---------------------------------------------------- | ------------ | ------ |
| open_files_stay_bounded_as_imports_and_searches_accumulate | 47   | D   | Resource bound: guards against "Too many open files" | —            | keep   |

#### tests/s2_vector_index_bench.rs

| Test                                 | Line | Cat | Reason                                                                      | E2E coverage | Action                |
| ------------------------------------ | ---- | --- | --------------------------------------------------------------------------- | ------------ | --------------------- |
| s2_flat_vector_index_scale_benchmark | 168  | D   | Manual scale benchmark (`PATHKEEP_S2_BENCH=1`); passes as a no-op otherwise | —            | keep; add `#[ignore]` |

#### tests/static_embedding_e2e.rs

| Test                              | Line | Cat | Reason                                                                  | E2E coverage | Action                                                            |
| --------------------------------- | ---- | --- | ----------------------------------------------------------------------- | ------------ | ----------------------------------------------------------------- |
| static_s1_throughput_benchmark    | 99   | D   | Manual benchmark (`PATHKEEP_STATIC_S1=1`)                               | —            | keep; add `#[ignore]`                                             |
| static_vs_reference_cosine_parity | 151  | D   | Parity against committed reference vectors (`PATHKEEP_STATIC_PARITY=1`) | —            | keep; consider running it ungated, since the fixture is committed |

#### tests/turso_write_path.rs

| Test                                                            | Line | Cat | Reason                                                       | E2E coverage                                                        | Action                                   |
| --------------------------------------------------------------- | ---- | --- | ------------------------------------------------------------ | ------------------------------------------------------------------- | ---------------------------------------- |
| imports_annotations_and_visibility_share_the_search_transaction | 11   | D   | Atomicity: a failed notes write leaves the old notes         | history.spec "a starred page keeps its note…" covers the happy path | keep, trim the `search_outbox` seq check |
| cancelled_import_keeps_batch_identity_and_committed_progress    | 93   | D   | Cancel mid-import; revert and restore keep the partial batch | —                                                                   | keep                                     |
| failed_batch_creation_cannot_leave_a_phantom_import_run         | 147  | D   | Atomicity                                                    | —                                                                   | keep                                     |

#### tests/working_set_turso.rs

| Test                                                                    | Line | Cat | Reason                                        | E2E coverage | Action |
| ----------------------------------------------------------------------- | ---- | --- | --------------------------------------------- | ------------ | ------ |
| every_page_is_reachable_past_both_old_pool_limits                       | 10   | D   | Regression: the old 12,500 / 50,000 pool caps | —            | keep   |
| candidate_page_does_not_group_the_entire_archive_for_each_scalar_signal | 69   | D   | Plan guard                                    | —            | keep   |

### Search, store schema, migration, vault

#### src/migration.rs

| Test                                                                                                                                                                                                                                                                                                                                                        | Line                                             | Cat | Reason                                                                                                                   | E2E coverage                                              | Action                                                                    |
| ----------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | ------------------------------------------------ | --- | ------------------------------------------------------------------------------------------------------------------------ | --------------------------------------------------------- | ------------------------------------------------------------------------- |
| import_source_key_error_prefixes_stay_the_literals_the_shell_pins                                                                                                                                                                                                                                                                                           | 349                                              | C   | Restates two constants; the frontend file it claims to mirror no longer uses them                                        | none needed (move.spec.ts covers a wrong export password) | delete                                                                    |
| validate_bundle_relative_path_accepts_a_normal_relative_path, …\_rejects_parent_dir_traversal, …\_rejects_nested_parent_dir_traversal, …\_rejects_current_dir_component, …\_rejects_unix_absolute, …\_rejects_windows_style_absolute, …\_rejects_windows_drive_letter_prefix, …\_collapses_repeated_separators, …\_rejects_empty_input, …\_rejects_nul_byte | 355, 362, 369, 376, 390, 400, 414, 428, 441, 447 | D   | Zip-slip guard on bundle entry paths (security boundary, used by native import, portable artifacts, SQLCipher converter) | n/a                                                       | keep; fold into one table test and assert rejection, not the message text |

#### src/migration/native_tests.rs

| Test                                                                       | Line | Cat | Reason                                                                                                         | E2E coverage                                          | Action |
| -------------------------------------------------------------------------- | ---- | --- | -------------------------------------------------------------------------------------------------------------- | ----------------------------------------------------- | ------ |
| export_then_import_into_an_empty_root_round_trips_native_tables            | 9    | D   | Data integrity: every native table, agent db and saved page text survive a move, both key modes                | move.spec.ts covers the visits only                   | keep   |
| wrong_export_password_and_changed_database_bytes_preserve_the_target       | 83   | D   | Security + integrity: wrong password or tampered file leaves this archive untouched                            | move.spec.ts covers the wrong password, not tampering | keep   |
| native_preview_rejects_an_inventory_missing_a_database                     | 125  | D   | Integrity: a bundle missing a database is refused at preview                                                   | n/a                                                   | keep   |
| export_cannot_replace_the_live_archive_or_its_configuration                | 149  | D   | Data safety: export can't overwrite the live db/config (`export-target-is-managed` is a classified error code) | n/a                                                   | keep   |
| portable_provenance_is_encrypted_relocated_and_replayable_on_an_empty_root | 168  | D   | Raw checkpoints are encrypted in the bundle (plaintext URL never in the file), relocated, and restorable       | n/a                                                   | keep   |
| portable_import_keeps_app_lock_policy_with_its_local_secret                | 251  | D   | Security: App Lock policy follows the machine's secret, never locks you out                                    | n/a                                                   | keep   |

#### src/vault/tests.rs

| Test                                                          | Line | Cat | Reason                                                                                                  | E2E coverage                      | Action |
| ------------------------------------------------------------- | ---- | --- | ------------------------------------------------------------------------------------------------------- | --------------------------------- | ------ |
| temporary_vaults_have_the_complete_schema_in_both_key_modes   | 10   | C   | Smoke check of a file name and one empty table; every other test opens temp vaults in both modes        | first-run.spec.ts                 | delete |
| reopen_interrupts_unfinished_runs_without_losing_progress     | 26   | D   | Crash recovery: a running backup becomes interrupted, progress and watermark kept; second owner refused | n/a                               | keep   |
| encrypted_vault_refuses_another_key                           | 53   | D   | Crypto: wrong or missing key refused                                                                    | n/a                               | keep   |
| vault_uses_the_reserved_owner_without_acquiring_another_lock  | 62   | D   | Concurrency: single owner lock handed in, released only by its holder                                   | n/a                               | keep   |
| database_files_require_their_purpose_specific_keys            | 75   | D   | Crypto: per-database HKDF sub-keys not interchangeable                                                  | n/a                               | keep   |
| a_password_open_beside_an_unconverted_archive_creates_nothing | 99   | D   | Data safety: never starts an empty archive next to an old-format one                                    | upgrade.spec.ts (happy path only) | keep   |
| missing_envelope_never_rekeys_existing_databases              | 124  | D   | Data safety: a missing envelope never rewraps existing databases                                        | n/a                               | keep   |
| new_encrypted_archive_persists_one_envelope_after_open        | 138  | D   | Crypto: one envelope, wrong password leaves it unchanged, same key on reopen                            | n/a                               | keep   |
| checkpoint_restore_takes_the_single_backup_guard              | 167  | D   | Concurrency: restore and backup can't run together (`backup-already-running` is a classified code)      | n/a                               | keep   |
| failed_store_open_does_not_publish_an_envelope                | 179  | D   | Crash safety: failed open leaves no envelope or db                                                      | n/a                               | keep   |
| pending_envelope_recovers_after_a_failure_with_database_files | 189  | D   | Crash recovery of the pending envelope; wrong password changes nothing                                  | n/a                               | keep   |
| pending_envelope_crash_child                                  | 226  | D   | Child-process half of the crash test below                                                              | n/a                               | keep   |
| crash_before_envelope_promotion_is_recovered_on_unlock        | 242  | D   | Kill-at-checkpoint crash recovery (quality-matrix rule 3)                                               | n/a                               | keep   |

#### src/store_schema/tests/mod.rs

| Test                                                 | Line | Cat | Reason                                                                                                                                   | E2E coverage                                                  | Action                                                                                                                                                                                                          |
| ---------------------------------------------------- | ---- | --- | ---------------------------------------------------------------------------------------------------------------------------------------- | ------------------------------------------------------------- | --------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| catalog_and_crud                                     | 166  | B   | Hand mirror of every table/index name incl. Turso-internal `__turso_internal_seq_*` and `sqlite_autoindex_*`; the CRUD half tests SQLite | upgrade.spec.ts / first-run.spec.ts create and use the schema | trim: keep only the doc rule "no triggers or virtual tables" (store-schema.md:6); drop the name lists; keep the row fixtures (maintenance and native tests reuse them); update store-schema.md:483-484, 502-508 |
| foreign_keys                                         | 234  | D   | Integrity: FKs are enforced (real failure mode: FK enforcement off)                                                                      | n/a                                                           | keep; one FK per db is enough, drop the "foreign key" message match                                                                                                                                             |
| agent_cascades                                       | 258  | D   | Deleting a conversation removes its messages/runs; job delete removes triggers (store-schema.md:378, 428-448)                            | n/a                                                           | keep                                                                                                                                                                                                            |
| reopen_preserves_data                                | 293  | D   | Migration safety: reopen does not replay v1, data kept, scratch reset                                                                    | n/a                                                           | keep                                                                                                                                                                                                            |
| newer_versions_are_refused                           | 332  | D   | Migration safety: an older app refuses a newer schema for every db                                                                       | n/a                                                           | keep                                                                                                                                                                                                            |
| consolidated_v1_contains_every_read_and_search_index | 352  | C   | Repeats `assert_catalog` from catalog_and_crud plus a version constant                                                                   | n/a                                                           | delete (move the v1 check into reopen_preserves_data if wanted)                                                                                                                                                 |

#### src/store_schema/tests/search.rs

| Test                 | Line | Cat | Reason                                                                                             | E2E coverage                                                                                      | Action                                          |
| -------------------- | ---- | --- | -------------------------------------------------------------------------------------------------- | ------------------------------------------------------------------------------------------------- | ----------------------------------------------- |
| pages_and_visibility | 26   | C   | Re-implements the visibility writer in test SQL; production `search_documents::refresh` never runs | browser-import.spec.ts "…imports, undoes and restores exactly" (search counts after undo/restore) | delete                                          |
| search_outbox        | 74   | D   | Outbox seq is never reused after delete, so the indexer can't skip a delete (stale or revoked hit) | n/a                                                                                               | keep; drop the rollback half (tests the engine) |
| unique_constraints   | 128  | D   | One page per URL, one profile row per page, CHECKs on counts/change kind                           | history.spec.ts (one row per page)                                                                | keep                                            |
| natural_keys         | 151  | D   | Dedup: a source URL or embedding can't be stored twice                                             | n/a                                                                                               | keep                                            |

#### src/search/tests.rs

| Test                                                                           | Line | Cat | Reason                                                                                                      | E2E coverage                                                    | Action                                                    |
| ------------------------------------------------------------------------------ | ---- | --- | ----------------------------------------------------------------------------------------------------------- | --------------------------------------------------------------- | --------------------------------------------------------- |
| query_forms_and_pages_match_the_generated_corpus                               | 35   | D   | Prefix, accent/OpenCC folding, phrase, exclusion, intitle, OR; full paging with exact count, both key modes | partly history-tools.spec.ts "site: with -word…"                | keep                                                      |
| canonical_filters_use_the_same_visit_and_revokes_do_not_wait_for_indexing      | 73   | D   | Same-visit filters; undone visits leave results before the index catches up (search.md:7, recall.md:17,27)  | browser-import.spec.ts (undo)                                   | keep                                                      |
| annotation_is_searchable_after_one_cycle_and_rebuild_keeps_coverage            | 107  | D   | A note is findable after one indexer cycle; rebuild keeps every result; closed vault ends the session       | history.spec.ts "a starred page keeps its note…" (not searched) | keep; drop the score-by-score equality with the raw index |
| regex_steps_scan_past_empty_chunks_and_page_without_duplicates                 | 173  | D   | Regex walks the whole archive, no repeats (global search)                                                   | search-limits.spec.ts "a regex finds every visit…"              | keep                                                      |
| requested_visit_ids_reach_old_records_and_reject_revoked_ones                  | 201  | D   | Visit lookup reaches old rows and drops undone ones                                                         | n/a                                                             | keep                                                      |
| punctuation_does_not_turn_into_match_all_and_many_exclusions_are_kept          | 220  | D   | `*** !!!` must not match everything; 25+ exclusions all apply                                               | n/a                                                             | keep                                                      |
| closing_a_vault_releases_snapshots_even_if_the_caller_retains_a_session        | 246  | D   | Resource: close drops readers (needed by restore/rekey)                                                     | n/a                                                             | keep                                                      |
| a_serialized_cursor_cannot_be_reused_with_other_filters                        | 262  | D   | Cursor integrity (search.md:127)                                                                            | n/a                                                             | keep; drop the "different query" message match            |
| paging_keeps_its_visit_snapshot_when_a_backup_adds_a_new_visit_to_an_old_match | 281  | D   | Paging stable while a backup writes (search.md:9)                                                           | n/a                                                             | keep                                                      |
| paging_does_not_repeat_a_shared_page_when_its_newest_visit_is_revoked          | 317  | D   | No repeat when undo changes a page's newest visit (search.md:9)                                             | n/a                                                             | keep                                                      |
| fuzzy_mode_preserves_exact_recall_for_short_prefixes_and_cjk                   | 362  | D   | Fuzzy never loses exact matches (search.md:11)                                                              | n/a                                                             | keep                                                      |

#### src/search/ported_grouped.rs

| Test                                                                   | Line | Cat | Reason                                                                            | E2E coverage                                                           | Action |
| ---------------------------------------------------------------------- | ---- | --- | --------------------------------------------------------------------------------- | ---------------------------------------------------------------------- | ------ |
| one_page_visited_from_two_browsers_is_one_row_with_every_visit_counted | 11   | D   | One row per page, every browser's visits counted                                  | history.spec.ts "search lists each matching page once with its visits" | keep   |
| the_row_is_the_newest_visit_even_when_another_copy_ranks_higher        | 30   | D   | Row shows the newest visit's title/profile in every sort                          | partly history.spec.ts                                                 | keep   |
| browser_and_date_filters_apply_to_the_count_and_the_row                | 54   | D   | Filters apply to both counts and the shown visit                                  | n/a                                                                    | keep   |
| the_cursor_reaches_every_page_once_even_when_pages_tie                 | 98   | D   | Walk equals one big page; tie order is the documented one (search.md:11)          | n/a                                                                    | keep   |
| regex_groups_the_same_way                                              | 142  | D   | Regex results grouped like keyword results                                        | n/a                                                                    | keep   |
| a_typo_falls_back_to_fuzzy_matching_and_still_groups                   | 171  | D   | Typo fallback still one row per page                                              | n/a                                                                    | keep   |
| operator_only_searches_group_every_url_the_operators_allow             | 196  | D   | `site:` alone lists every allowed page                                            | history-tools.spec.ts "site: with -word…"                              | keep   |
| tag_searches_list_only_pages_carrying_every_tag                        | 211  | D   | Tag semantics: case-insensitive, AND of tags, quoted tags, combine with operators | history-tools.spec.ts "a tag filters History…" (one tag only)          | keep   |
| three_copies_of_one_page_stay_one_row_on_every_page_of_the_walk        | 243  | D   | Regression for a real duplicate report (comment states it)                        | history.spec.ts, search-loading.spec.ts                                | keep   |
| tag_pages_start_from_tag_postings_not_every_page                       | 278  | D   | Performance guard: tag-only search starts from tag postings (recall.md:40)        | n/a                                                                    | keep   |
| a_tag_filter_does_not_become_an_extra_or_branch                        | 300  | D   | Constraints apply to every OR group (search.md:13-14)                             | n/a                                                                    | keep   |

#### src/search/ported_queries.rs

| Test                                                                          | Line | Cat | Reason                                                                                          | E2E coverage                                                                                                          | Action                                                                                                                                       |
| ----------------------------------------------------------------------------- | ---- | --- | ----------------------------------------------------------------------------------------------- | --------------------------------------------------------------------------------------------------------------------- | -------------------------------------------------------------------------------------------------------------------------------------------- |
| lexical_search_surfaces_capped_enrichment_excerpt_only_for_enriched_results   | 43   | D   | Fetched page text is searchable and its result shows an excerpt; plain pages don't              | none                                                                                                                  | keep; drop the "excerpt contains the topic keyword" assert (pins the index-text layout)                                                      |
| lexical_search_caps_long_enrichment_excerpt_on_a_cjk_char_boundary            | 107  | B   | Pins the unspecified 180-char cap (`== 181`); duplicates archive/history.rs:103-124             | none                                                                                                                  | delete (archive/history.rs:103-124 tests the cap directly, and `cap_enrichment_excerpt` slices by `chars()`, so it cannot split a codepoint) |
| lexical_recall_matches_cjk_script_folding_and_compact_substrings              | 132  | D   | Simplified/traditional folding, CJK substring, compact Latin, NFKC (lexical-recall-v2.md:83-84) | none                                                                                                                  | keep                                                                                                                                         |
| lexical_recall_expands_aliases_and_pages_every_fuzzy_match                    | 208  | D   | gh/yt/pr aliases; fuzzy results page fully                                                      | none                                                                                                                  | keep; drop `cursor.contains(':')` (search.md:127 says opaque) and the BM25 first-hit assert                                                  |
| lexical_recall_defaults_to_relevance_and_accepts_time_sort_override           | 335  | D   | Default sort is relevance; newest overrides; stale cursor rejected                              | none                                                                                                                  | keep; drop `cursor.contains(':')` asserts                                                                                                    |
| history_keyword_query_supports_google_like_local_operators                    | 399  | D   | site/-word/phrase/after/before/intitle/inurl/filetype/OR as recall.md:99-107                    | history-tools.spec.ts "site: with -word…" (one operator pair)                                                         | keep                                                                                                                                         |
| history_keyword_query_supports_tag_and_note_operators_against_annotations     | 522  | D   | `tag:`, `note:`, `-tag:` (recall.md:40)                                                         | history-tools.spec.ts "a tag filters History…" (tag only)                                                             | keep                                                                                                                                         |
| filetype_normalization_does_not_create_sql_wildcards                          | 618  | D   | `%`/`_` in user input stay literal                                                              | n/a                                                                                                                   | keep                                                                                                                                         |
| unsupported_negative_date_operators_keep_the_parsers_literal_exclusion        | 641  | A?  | `-after:2024` silently becomes a text exclusion (see A? items)                                  | none; scenario "searching with -after:<date> leaves out visits after that date, or says the operator isn't supported" | decide semantics, document in recall.md, replace the test                                                                                    |
| advanced_constraints_apply_to_every_keyword_or_group                          | 654  | D   | search.md:13-14                                                                                 | n/a                                                                                                                   | keep                                                                                                                                         |
| field_fragments_keep_the_existing_substring_and_literal_punctuation_semantics | 680  | D   | search.md:14-16, recall.md:18                                                                   | n/a                                                                                                                   | keep                                                                                                                                         |
| phrases_do_not_match_synthetic_gram_adjacency_or_expand_short_aliases         | 702  | D   | search.md:16-18                                                                                 | n/a                                                                                                                   | keep                                                                                                                                         |

#### src/search/ported_regex.rs

| Test                                                                            | Line | Cat | Reason                                                        | E2E coverage                                                                                | Action                                                                           |
| ------------------------------------------------------------------------------- | ---- | --- | ------------------------------------------------------------- | ------------------------------------------------------------------------------------------- | -------------------------------------------------------------------------------- |
| a_match_older_than_the_first_chunk_is_found_by_continuing                       | 28   | D   | Regex reaches the oldest match                                | search-limits.spec.ts "a regex finds every visit of a page, the oldest included"            | keep; drop the "first zero-budget step is empty" asserts (depend on step budget) |
| chunk_edges_neither_repeat_nor_skip_a_page                                      | 44   | D   | Paging at any size equals one page                            | n/a                                                                                         | keep                                                                             |
| a_count_is_exact_only_once_the_scan_covered_everything                          | 62   | D   | Counts are not called exact early (search.md:127)             | search-limits.spec.ts                                                                       | keep                                                                             |
| an_abandoned_scan_leaves_nothing_behind                                         | 73   | D   | Cancel ends the session, next search unaffected               | n/a                                                                                         | keep                                                                             |
| filters_bound_the_scan_before_the_regex                                         | 85   | D   | Canonical filters before regex (recall.md:128)                | n/a                                                                                         | keep                                                                             |
| the_dialect_is_the_rust_regex_crate                                             | 108  | D   | Lookaround/backrefs refused in the backend too                | history-tools.spec.ts "a regex Rust cannot run is explained and never sent" (frontend side) | keep                                                                             |
| regex_keeps_operator_words_and_slashes_literal_and_requires_its_mode_flag       | 131  | D   | search.md:18-19                                               | n/a                                                                                         | keep                                                                             |
| reading_only_the_pages_with_the_fragment_counts_what_reading_every_visit_counts | 168  | D   | Prefilter path equals full walk (strong oracle, search.md:11) | n/a                                                                                         | keep                                                                             |
| a_title_that_starts_matching_while_the_index_is_behind_is_found                 | 187  | D   | Outbox pages are checked while the index lags (search.md:11)  | search-limits.spec.ts "…while the search index is catching up"                              | keep                                                                             |

#### src/search/projection_tests.rs

| Test                                                                              | Line | Cat | Reason                                                                    | E2E coverage                                                              | Action                                                    |
| --------------------------------------------------------------------------------- | ---- | --- | ------------------------------------------------------------------------- | ------------------------------------------------------------------------- | --------------------------------------------------------- |
| every_visible_title_is_searchable_and_revoked_text_never_leaks_from_a_shared_page | 12   | D   | Privacy: undone visit's title never found, even with the index stale      | browser-import.spec.ts (undo, count only)                                 | keep                                                      |
| rebuild_and_clearing_preserve_the_enrichment_and_annotation_contract              | 66   | D   | Notes/tags/fetched text survive rebuild; cleared notes/tags stop matching | n/a                                                                       | keep                                                      |
| regex_hydrates_the_newest_matching_visit_not_another_profiles_title               | 120  | D   | Shown visit is a matching one                                             | n/a                                                                       | keep                                                      |
| compact_recall_does_not_turn_each_word_into_an_independent_substring              | 141  | D   | Precision: words aren't matched as scattered substrings                   | n/a                                                                       | keep                                                      |
| enrichment_text_for_index_concats_summary_and_key_metadata_only                   | 161  | B   | Pins which JSON keys a crate-private helper concatenates; no spec         | none; scenario "a fetched GitHub repo page is found by one of its topics" | delete (or replace with that E2E once enrichment has one) |
| a_live_session_drops_revoked_text_when_hydrating                                  | 185  | D   | Privacy within a running session (search.md:7)                            | n/a                                                                       | keep                                                      |

#### src/search/recovery_tests.rs

| Test                                                                         | Line | Cat | Reason                                                 | E2E coverage                                 | Action |
| ---------------------------------------------------------------------------- | ---- | --- | ------------------------------------------------------ | -------------------------------------------- | ------ |
| owner_replays_a_killed_apply_to_the_same_canonical_result                    | 14   | D   | Kill mid-commit, reopen, no lost note                  | n/a                                          | keep   |
| apply_child                                                                  | 68   | D   | Ignored child half of the crash test                   | n/a                                          | keep   |
| missing_and_corrupt_indexes_rebuild_from_documents_even_after_outbox_pruning | 91   | D   | Derived index lost or corrupt → rebuilt from canonical | n/a                                          | keep   |
| a_rebuild_serves_the_old_generation_without_holding_a_store_reader           | 128  | D   | Concurrency: search keeps working during rebuild       | background-work.spec.ts "…rebuild" (UI side) | keep   |
| an_open_owner_recovers_corrupt_metadata_without_a_read_side_rebuild          | 171  | D   | Corruption recovery while open                         | n/a                                          | keep   |
| cancelling_a_rebuild_preserves_search_and_a_later_rebuild_can_finish         | 194  | D   | Cancellation safety                                    | background-work.spec.ts (cancel)             | keep   |

#### src/search/fuzzy_tests.rs

| Test                                                             | Line | Cat | Reason                                                  | E2E coverage | Action |
| ---------------------------------------------------------------- | ---- | --- | ------------------------------------------------------- | ------------ | ------ |
| fuzzy_prefilters_equal_brute_force_even_without_a_shared_trigram | 12   | D   | Indexed fuzzy equals the brute-force predicate (oracle) | n/a          | keep   |
| the_star_facet_does_not_become_part_of_a_typo_target             | 80   | D   | `is:starred` isn't fuzzed as a word                     | n/a          | keep   |
| fuzzy_mode_keeps_exact_compound_and_cjk_or_branches              | 104  | D   | Exact/CJK OR branches survive fuzzy                     | n/a          | keep   |

#### src/search/coverage_tests.rs, benchmark.rs, eligibility.rs, regex.rs, regex_prefilter.rs, stream.rs

| Test                                                                                 | Line                   | Cat | Reason                                                                                    | E2E coverage                                                                                | Action                 |
| ------------------------------------------------------------------------------------ | ---------------------- | --- | ----------------------------------------------------------------------------------------- | ------------------------------------------------------------------------------------------- | ---------------------- |
| coverage_tests::lexical_fuzzy_regex_palette_and_export_reach_past_old_limits         | coverage_tests.rs:10   | D   | The global-search guard: 25,007 pages reachable by lexical, fuzzy, regex, palette, export | search-limits.spec.ts "a word on every page lists every page with an exact count" (smaller) | keep                   |
| benchmark::encrypted_archive_search                                                  | benchmark.rs:18        | D   | Ignored 14.4M-visit benchmark                                                             | n/a                                                                                         | keep (not in the gate) |
| eligibility::posting_batches_seek_page_urls_and_visible_visits_without_archive_scans | eligibility.rs:436     | D   | Read-plan guard, no full scans                                                            | n/a                                                                                         | keep                   |
| eligibility::selective_scopes_seek_canonical_time_indexes_without_sorting            | eligibility.rs:450     | D   | Read-plan guard, no sorter                                                                | n/a                                                                                         | keep                   |
| regex::each_chunk_seeks_to_the_cursor_instead_of_scanning_from_the_newest_visit      | regex.rs:374           | D   | Read-plan guard for regex chunks                                                          | n/a                                                                                         | keep                   |
| regex::candidate_pages_seek_their_urls_and_visits_without_scanning_the_archive       | regex.rs:387           | D   | Read-plan guard for regex candidates                                                      | n/a                                                                                         | keep                   |
| regex_prefilter::optional_or_alternative_text_is_never_a_required_fragment           | regex_prefilter.rs:127 | D   | Soundness: prefilter never drops a possible match                                         | n/a                                                                                         | keep                   |
| stream::failed_sinks_do_not_accumulate_finished_jobs                                 | stream.rs:199          | D   | Leak guard on the session registry                                                        | n/a                                                                                         | keep                   |
| stream::start_streams_the_same_result_and_close_drops_idle_sessions                  | stream.rs:211          | D   | Streaming result equals the page; close drops sessions                                    | history.spec.ts (streaming path)                                                            | keep                   |

### Enrichment

#### src/enrichment/tests.rs

| Test                                                                       | Line | Cat | Reason                                                                                                                           | E2E coverage                                                                  | Action                                                                                       |
| -------------------------------------------------------------------------- | ---- | --- | -------------------------------------------------------------------------------------------------------------------------------- | ----------------------------------------------------------------------------- | -------------------------------------------------------------------------------------------- |
| percent_encoded_cjk_url_slug_decodes_to_utf8_not_mojibake                  | 26   | D   | Regression for CJK mojibake, with the failure mode stated. Sits on the dead title-normalization path.                            | none                                                                          | Keep only if title normalization is wired; otherwise delete with it                          |
| build_embedding_content_includes_only_non_empty_enrichment_fields          | 36   | B   | Pins exact embedding-text labels ("Readable title: ", "Summary: ", "Title: (untitled)")                                          | none                                                                          | Delete, or trim to "blank fields add nothing"                                                |
| cap_embedding_text_truncates_long_text_to_the_token_budget                 | 66   | D   | Keeps embedding input inside the ~512-token window; loose bound                                                                  | none                                                                          | Keep                                                                                         |
| build_enrichment_summary_caps_and_normalizes                               | 77   | D   | Summary stays bounded for the FTS index (06 §3); loose bound                                                                     | none                                                                          | Keep                                                                                         |
| default_enrichment_refetch_after_for_status_matches_negative_cache_cadence | 89   | D   | Negative-cache rule: which statuses retry (archive.md:385)                                                                       | none                                                                          | Keep; drop the `ends_with('Z')` check (due-check compares instants)                          |
| load_best_enrichment_map_prefers_success_then_source_priority              | 105  | D   | Best-row pick: success first, capture beats refetch (archive.md §7, layer 4); feeds AI indexing                                  | none                                                                          | Keep                                                                                         |
| load_best_enrichment_map_skips_empty_requests                              | 219  | C   | Trivial empty-input early return                                                                                                 | none                                                                          | Delete                                                                                       |
| title_normalization_failure_messages_and_storage_cover_enrichment_edges    | 231  | B   | Exact English failure prose on a path nothing reaches, plus embedding labels; name says "cover edges"                            | none                                                                          | Delete                                                                                       |
| html_enrichment_result_covers_fallback_adapter_and_truncation_paths        | 358  | B   | Pins the 12,000-char cap exactly and the extraction JSON shape; the adapter part duplicates the site-adapter tests               | none                                                                          | Trim to "body fallback when no blocks" and "video title wins"; drop the exact cap            |
| site_adapter_merge_falls_back_to_generic_snippets_and_status               | 411  | C   | Private helper fed an adapter result production cannot produce (`adapt_video_site` returns None with no title or description)    | none                                                                          | Delete                                                                                       |
| execute_enrichment_job_by_id_runs_outside_legacy_insights_module           | 481  | B   | Dead path: runs a job only tests can enqueue (`enqueue_enrichment_job` is `#[cfg(test)]`); name refers to a deleted module       | none; would be "a page with no title shows a readable title from its address" | Decide the feature first: wire it and replace with that E2E, or delete                       |
| execute_enrichment_job_by_id_covers_cancellation_and_failure_edges         | 498  | A?  | Asserts a v0.2 "deferred" readable-content job succeeds (see A? items). The cancel-after-claim race is real but on the dead path | none                                                                          | Delete the deferral asserts with the branch; keep the cancel race only if the executor stays |

#### src/enrichment/content_fetch_api.rs

| Test                                                                    | Line | Cat | Reason                                                                                                   | E2E coverage                                                                                                                      | Action                                                         |
| ----------------------------------------------------------------------- | ---- | --- | -------------------------------------------------------------------------------------------------------- | --------------------------------------------------------------------------------------------------------------------------------- | -------------------------------------------------------------- |
| enqueue_content_fetch_now_is_disabled_when_master_switch_off            | 285  | D   | Hard-default-OFF consent gate (ai-security-posture.md:222)                                               | none                                                                                                                              | Keep                                                           |
| enqueue_content_fetch_now_queues_a_job_when_enabled                     | 292  | C   | Trivial happy path; covered by the drain test                                                            | none                                                                                                                              | Delete                                                         |
| content_fetch_settings_reports_consent_and_zero_jobs_initially          | 299  | C   | Reads back config and defaults; pins extractor count 2                                                   | none                                                                                                                              | Delete                                                         |
| drain_one_content_fetch_job_is_noop_without_consent                     | 307  | D   | No egress while consent is off                                                                           | none                                                                                                                              | Keep                                                           |
| drain_one_content_fetch_job_picks_up_a_due_job_when_enabled             | 311  | B   | Runs the `#[cfg(test)]` stub entry and `OfflineStubFetcher`, not the shipped fetch                       | none; would be "turn on page fetching; with the network blocked a starred page ends with a 'blocked' record and nothing connects" | Replace with that E2E (it runs the real `SharedClientFetcher`) |
| drain_one_content_fetch_job_returns_false_for_a_rate_limited_job        | 323  | D   | Rate-limit back-pressure: the lane idles and the job stays queued (SEC-2/BUG-3); timing E2E cannot reach | none                                                                                                                              | Keep                                                           |
| list_visit_enrichment_is_empty_for_unknown_history_id                   | 336  | C   | Trivial                                                                                                  | none                                                                                                                              | Delete                                                         |
| enqueue_working_set_is_noop_without_consent                             | 340  | D   | Consent gate on the candidate pass                                                                       | none                                                                                                                              | Keep                                                           |
| enqueue_working_set_enqueues_a_starred_due_url                          | 347  | D   | Candidate pass enqueues once; a second pass does not duplicate                                           | none                                                                                                                              | Keep                                                           |
| enqueue_working_set_resolves_a_tracking_param_raw_url                   | 355  | D   | Canonical URL normalizer drops tracking params (data-model.md:312-315)                                   | none                                                                                                                              | Keep                                                           |
| enqueue_working_set_skips_blocked_and_undue_candidates                  | 371  | D   | Per-domain gate plus freshness (no refetch of a fresh row)                                               | none                                                                                                                              | Keep                                                           |
| enqueue_working_set_skips_extractor_disabled_and_unarchived_candidates  | 385  | D   | Per-extractor gate; a star with no page is skipped                                                       | none                                                                                                                              | Keep                                                           |
| list_visit_enrichment_returns_stored_rows                               | 397  | C   | Reads back inserted columns                                                                              | upgrade.spec.ts "a data folder made by PathKeep 0.3.0 is updated with all its data" (checks `list_visit_enrichment` fields)       | Delete                                                         |
| content_fetch_schedule_eta_is_none_without_consent_and_reports_deferral | 409  | D   | Worker wake-up ETA for deferred jobs; timing                                                             | none                                                                                                                              | Keep                                                           |
| candidate_clock_is_in_unix_epoch_frame                                  | 428  | C   | Re-implements the function (`Utc::now` vs `Utc::now`)                                                    | none                                                                                                                              | Delete                                                         |

#### src/enrichment/content_fetch/tests.rs

| Test                                                       | Line | Cat | Reason                                                                                                                                              | E2E coverage | Action                                       |
| ---------------------------------------------------------- | ---- | --- | --------------------------------------------------------------------------------------------------------------------------------------------------- | ------------ | -------------------------------------------- |
| network_switch_blocks_content_fetch_without_connecting     | 85   | D   | Debug network switch blocks before any connect, with a real socket (TESTING.md:27). Keeps E2E and demo hermetic; E2E cannot prove its own isolation | n/a          | Keep                                         |
| read_capped_handles_under_cap_over_cap_and_a_broken_stream | 227  | D   | Body cap and broken stream → error, not a truncated success (bounded memory)                                                                        | none         | Keep; drop the "could not be read" substring |

#### src/enrichment/content_fetch/tests/routing.rs

| Test                                                                   | Line | Cat | Reason                                                                                                                               | E2E coverage | Action                                             |
| ---------------------------------------------------------------------- | ---- | --- | ------------------------------------------------------------------------------------------------------------------------------------ | ------------ | -------------------------------------------------- |
| run_content_fetch_routes_github_url_to_github_extractor                | 7    | D   | GitHub repo routes to the API extractor; summary = description; success → no retry                                                   | none         | Keep                                               |
| run_content_fetch_routes_generic_url_to_readable_extractor             | 30   | C   | Duplicates the registry, generic-extract and storage happy-path tests                                                                | none         | Delete                                             |
| run_content_fetch_records_honest_failure_and_negative_cache            | 47   | D   | Honest failure status plus negative cache (archive.md:385)                                                                           | none         | Keep                                               |
| run_content_fetch_blocks_private_github_api_subresource                | 71   | C   | Misnamed: only checks fetch-error on a missing fixture                                                                               | none         | Delete                                             |
| content_fetch_allowed_honors_master_switch_extractor_and_domain_gates  | 90   | D   | Triple consent gate (ai-security-posture.md:241-243)                                                                                 | none         | Keep                                               |
| guard_then_fetch_json_blocks_private_api_url                           | 119  | D   | SSRF guard on the API sub-resource (ai-security-posture.md:245-250); runs a test stand-in, so only literal-IP classification is real | none         | Keep; remove the cfg(test) stand-in in code        |
| run_content_fetch_blocks_private_github_api_via_guard                  | 128  | C   | Repeats the test above with another IP; its comment admits it                                                                        | none         | Delete                                             |
| fetch_json_api_with_no_api_request_is_fetch_error                      | 145  | C   | Unreachable branch through a test-only extractor; pins "No API request to fetch."                                                    | none         | Delete                                             |
| fetch_json_api_keeps_a_successful_secondary_and_tolerates_a_failed_one | 161  | D   | Failed README is not fatal; egress host is the API host (BUG-1)                                                                      | none         | Keep; the no-secondary arm can go                  |
| html_egress_parks_when_host_bucket_is_empty                            | 196  | D   | Per-host rate limit parks HTML egress (ai-security-posture.md:255-257)                                                               | none         | Keep; drop the "per-host request budget" substring |
| json_egress_parks_when_host_bucket_is_empty                            | 213  | D   | Same rule on the JSON path (SEC-1)                                                                                                   | none         | Keep                                               |
| guard_final_url_blocks_a_private_redirect_landing                      | 228  | D   | Post-redirect final-URL SSRF backstop (ai-security-posture.md:249)                                                                   | none         | Keep                                               |
| run_content_fetch_falls_back_to_generic_for_unresolved_scheme          | 248  | C   | Defensive fallback for an ftp URL the consent gate already refuses                                                                   | none         | Delete                                             |
| content_fetch_allowed_is_false_for_non_http_scheme                     | 257  | D   | Consent gate refuses non-http(s)                                                                                                     | none         | Keep                                               |

#### src/enrichment/content_fetch/tests/storage.rs

| Test                                                                               | Line | Cat | Reason                                                                                                | E2E coverage                  | Action |
| ---------------------------------------------------------------------------------- | ---- | --- | ----------------------------------------------------------------------------------------------------- | ----------------------------- | ------ |
| execute_content_fetch_job_stores_enrichment_and_marks_succeeded                    | 9    | D   | Core path: claim → fetch → stored summary → succeeded                                                 | none (network blocked in E2E) | Keep   |
| execute_content_fetch_job_parks_when_consent_disabled                              | 62   | D   | Consent turned off after enqueue: no fetch                                                            | none                          | Keep   |
| execute_content_fetch_job_blocks_non_https_page_url                                | 99   | D   | https-only gate; the blocked row carries the real extractor, so it is not re-enqueued forever (SEC-4) | none                          | Keep   |
| execute_content_fetch_job_is_noop_for_missing_job                                  | 142  | C   | Trivial missing id                                                                                    | none                          | Delete |
| execute_defers_a_rate_limited_job_for_a_later_drain                                | 160  | D   | Throttle requeues instead of failing (SEC-2)                                                          | none                          | Keep   |
| execute_defers_a_github_job_for_the_egress_api_host_eta                            | 206  | D   | Regression BUG-1 (1 Hz drain thrash), failure mode stated                                             | none                          | Keep   |
| execute_honours_a_concurrent_stop_during_a_rate_limited_fetch                      | 260  | D   | Race regression BUG-2: a cancel is not swallowed by the requeue                                       | none                          | Keep   |
| finish_content_fetch_job_cancels_when_stop_requested_mid_run                       | 302  | D   | Cancel wins before store; nothing stored                                                              | none                          | Keep   |
| finish_marks_cancelled_when_succeed_loses_to_a_concurrent_stop                     | 334  | C   | Cannot build the race it names; asserts only `true` (coverage of one arm)                             | none                          | Delete |
| unarchived_candidate_waits_without_fetching_and_can_be_offered_again               | 375  | D   | `awaiting_archive` without fetching, then reoffered (data-model.md:318-322)                           | none                          | Keep   |
| finish_fans_enrichment_out_to_sibling_visits_and_publishes_outbox                  | 421  | D   | One cache serves every sibling page; one outbox entry per page (data-model.md:323-330)                | none                          | Keep   |
| stored_content_survives_a_crash_and_serves_new_canonical_variants_without_fetching | 536  | D   | Crash replay without refetch; new URL variant published locally (data-model.md:325-328)               | none                          | Keep   |

#### src/enrichment/content_fetch/tests/transport.rs (all `#[ignore]`, never run in the gate)

| Test                                                           | Line | Cat | Reason                                                                                | E2E coverage | Action                                            |
| -------------------------------------------------------------- | ---- | --- | ------------------------------------------------------------------------------------- | ------------ | ------------------------------------------------- |
| shared_fetcher_returns_html_body_for_a_readable_article        | 8    | D   | Real transport happy path (loopback mockito)                                          | none         | Un-ignore                                         |
| shared_fetcher_reports_rate_limited_on_429                     | 29   | D   | 429 → rate-limited                                                                    | none         | Un-ignore                                         |
| shared_fetcher_reports_unsupported_content_for_non_html        | 38   | D   | PDF → unsupported-content (intelligence.md:277)                                       | none         | Un-ignore                                         |
| shared_fetcher_reports_fetch_error_on_4xx_5xx                  | 54   | D   | 4xx → fetch-error with http status                                                    | none         | Un-ignore                                         |
| shared_fetcher_reports_decode_error_when_html_body_exceeds_cap | 65   | D   | 1 MiB HTML cap                                                                        | none         | Un-ignore                                         |
| shared_fetcher_returns_github_repo_json                        | 80   | C   | JSON happy path; duplicates the HTML one                                              | none         | Delete                                            |
| shared_fetcher_treats_github_403_as_rate_limited               | 99   | D   | GitHub quota 403 → rate-limited                                                       | none         | Un-ignore                                         |
| shared_fetcher_caps_json_body                                  | 108  | D   | JSON body cap                                                                         | none         | Un-ignore                                         |
| shared_fetcher_follows_a_redirect_chain_to_the_final_html      | 123  | C   | Proves reqwest follows redirects; the per-hop SSRF guard is allow-all under cfg(test) | none         | Delete, or rewrite against the real per-hop guard |
| shared_fetcher_reports_fetch_error_on_unreachable_host         | 145  | D   | Unreachable host → fetch-error, never a fake success                                  | none         | Un-ignore                                         |
| read_capped_drains_under_cap_and_rejects_over_cap              | 155  | C   | Name lies (under-cap only); duplicates the offline read_capped test                   | none         | Delete                                            |
| shared_fetcher_reports_fetch_error_on_json_5xx                 | 170  | C   | Branch-coverage twin of the 4xx test                                                  | none         | Delete                                            |

#### src/enrichment/extractors.rs

| Test                                                         | Line | Cat | Reason                                                 | E2E coverage | Action |
| ------------------------------------------------------------ | ---- | --- | ------------------------------------------------------ | ------------ | ------ |
| registry_resolves_github_before_generic_fallback             | 144  | D   | First-match-wins routing (archive.md:358)              | none         | Keep   |
| generic_readable_is_the_terminal_fallback_for_any_https_page | 154  | C   | Same as the second half of the test above, another URL | none         | Delete |
| registry_is_first_match_wins_with_fallback_last              | 161  | B   | Pins internal vector order                             | none         | Delete |

#### src/enrichment/extractors/generic_readable.rs

| Test                                                | Line | Cat | Reason                                                                 | E2E coverage | Action |
| --------------------------------------------------- | ---- | --- | ---------------------------------------------------------------------- | ------------ | ------ |
| matches_any_http_or_https_url                       | 75   | C   | Restates `starts_with`                                                 | none         | Delete |
| extract_produces_summary_and_stamps_version         | 83   | D   | Summary prefers the title (06 §3); version stamp drives refetch gating | none         | Keep   |
| extract_recovers_from_invalid_utf8_via_lossy_decode | 95   | D   | A stray invalid byte does not fail extraction                          | none         | Keep   |

#### src/enrichment/extractors/github_repo.rs

| Test                                                            | Line | Cat | Reason                                                              | E2E coverage | Action                                 |
| --------------------------------------------------------------- | ---- | --- | ------------------------------------------------------------------- | ------------ | -------------------------------------- |
| parse_owner_repo_accepts_repo_root_and_rejects_non_repos        | 326  | D   | URL parser with a clear spec (repo roots only)                      | none         | Keep                                   |
| matches_only_repo_roots                                         | 353  | C   | Duplicates the parser test; restates id/version/kind constants      | none         | Delete                                 |
| api_requests_target_the_repos_and_readme_endpoints              | 364  | D   | GitHub REST paths; a typo would fail silently and E2E cannot see it | none         | Keep                                   |
| build_github_enrichment_maps_description_topics_and_stars       | 377  | D   | JSON → summary/title/topics mapping (archive.md:358)                | none         | Keep; drop the "Topics: …" text check  |
| build_github_enrichment_includes_readme_lead                    | 403  | D   | README included; summary still prefers the description              | none         | Keep                                   |
| build_github_enrichment_without_description_or_readme_is_empty  | 419  | D   | Honest `empty` status (summary assert is hedged)                    | none         | Keep; settle the hedged summary assert |
| build_github_enrichment_falls_back_to_url_for_full_name         | 431  | C   | Defensive branch; the API always returns full_name                  | none         | Delete                                 |
| build_github_enrichment_reports_decode_error_on_bad_json        | 441  | D   | Honest decode-error, never a fake success                           | none         | Keep                                   |
| decode_github_readme_handles_base64_and_rejects_other_encodings | 449  | D   | Parser spec                                                         | none         | Keep                                   |
| base64_decode_roundtrips_and_rejects_invalid_chars              | 464  | D   | Hand-rolled base64 decoder (RFC 4648)                               | none         | Keep while the decoder is hand-rolled  |

#### src/enrichment/rate_limit.rs

| Test                                                         | Line | Cat | Reason                                       | E2E coverage | Action |
| ------------------------------------------------------------ | ---- | --- | -------------------------------------------- | ------------ | ------ |
| budget_for_host_uses_github_budget_for_github_hosts          | 201  | D   | api.github.com gets the 60/hr budget (06 §1) | none         | Keep   |
| per_hour_converts_quota_to_refill_rate                       | 209  | C   | Restates `/3600`; probes the floor-to-1      | none         | Delete |
| token_bucket_allows_burst_then_throttles_until_refill        | 218  | D   | Token-bucket math                            | none         | Keep   |
| token_bucket_refill_is_clamped_to_capacity                   | 236  | D   | Bucket never exceeds capacity                | none         | Keep   |
| acquire_host_token_allows_blank_host                         | 253  | C   | Incidental policy for a blank host           | none         | Delete |
| acquire_host_token_consumes_from_a_real_bucket               | 260  | C   | Asserts only that the first acquire succeeds | none         | Delete |
| token_bucket_eta_is_zero_when_full_and_positive_when_drained | 269  | D   | ETA drives the requeue time (SEC-2)          | none         | Keep   |
| token_bucket_eta_is_capped_for_a_zero_refill_budget          | 283  | C   | Impossible budget; pins `u32::MAX`           | none         | Delete |
| next_token_eta_secs_registry_reports_blank_and_drained_hosts | 294  | C   | Thin wrapper; the deferral tests cover it    | none         | Delete |
| reset_host_bucket_for_test_refills_and_ignores_blank         | 306  | C   | Tests a test helper                          | none         | Delete |

#### src/enrichment_site_adapters.rs

| Test                                                      | Line | Cat | Reason                                                                            | E2E coverage | Action                         |
| --------------------------------------------------------- | ---- | --- | --------------------------------------------------------------------------------- | ------------ | ------------------------------ |
| video_adapters_extract_json_ld_and_url_metadata           | 314  | D   | JSON-LD VideoObject parsing, YouTube id (intelligence.md:280)                     | none         | Keep; drop `snippets.len()==3` |
| video_adapters_fall_back_to_meta_tags_and_short_urls      | 350  | D   | og/meta fallback, Vimeo id, youtu.be id                                           | none         | Keep                           |
| adapters_ignore_unsupported_or_unreadable_video_pages     | 375  | D   | Non-video hosts and unreadable JSON-LD yield no adapter                           | none         | Keep                           |
| private_helpers_cover_nested_json_meta_and_duration_edges | 392  | C   | Named as private-helper coverage: invalid selector, `format_duration("PT")`, etc. | none         | Delete                         |

### Models and infrastructure

#### src/utils.rs

| Test                                                            | Line | Cat | Reason                                                               | E2E coverage                          | Action                                                       |
| --------------------------------------------------------------- | ---- | --- | -------------------------------------------------------------------- | ------------------------------------- | ------------------------------------------------------------ |
| escape_html_escapes_all_significant_characters                  | 274  | D   | HTML-injection guard for export report; clear spec                   | export.spec does not inject markup    | keep                                                         |
| sha_helpers_are_stable                                          | 284  | C   | Known SHA-256 vectors re-test the `sha2` crate                       | none needed                           | delete                                                       |
| path_segment_encoding_is_windows_safe_and_reversible            | 300  | D   | Reversible, Windows-safe encoding; reserved names; legacy raw decode | none                                  | keep                                                         |
| chrome_time_roundtrip_is_lossless_for_valid_inputs              | 324  | D   | Timestamp conversion round trip (ADR-002)                            | none                                  | keep                                                         |
| invalid_iso_returns_none_and_invalid_chrome_time_falls_back     | 337  | B   | Pins "corrupt time shows as 1970-01-01", an arbitrary fallback       | none                                  | delete the 1970 assert; keep the `None` check only if wanted |
| date_str_to_unix_ms_range_covers_full_day_and_rejects_bad_input | 343  | A   | Pins UTC day for a user-named local day (see A1)                     | none                                  | fix the code                                                 |
| domain_extraction_handles_urls_and_bare_hosts                   | 362  | D   | Two plain parser cases                                               | none                                  | keep                                                         |
| image_data_to_data_url_detects_common_favicon_formats           | 368  | D   | Magic-byte sniffing + hand-written base64, both standard specs       | link preview / favicon E2E indirectly | keep                                                         |

#### src/config.rs

| Test                                                                 | Line | Cat | Reason                                                                                                                              | E2E coverage                                                                        | Action                                                             |
| -------------------------------------------------------------------- | ---- | --- | ----------------------------------------------------------------------------------------------------------------------------------- | ----------------------------------------------------------------------------------- | ------------------------------------------------------------------ |
| project_paths_honor_override_environment                             | 190  | C   | Restates `project_paths_with_root` joins; the override is used by every E2E run (`playwright.config.ts:58` sets `CHB_PROJECT_ROOT`) | all specs; upgrade.spec checks a 0.3.0 folder layout                                | delete                                                             |
| ensure_paths_and_config_roundtrip                                    | 217  | B   | Restates the `ensure_paths` dir list; pins pretty-printed JSON text `"archiveMode": "Encrypted"`                                    | first-run.spec "onboarding sets up an encrypted archive..." writes and reads config | delete                                                             |
| save_config_overwrite_round_trips_through_the_durable_path           | 250  | B   | Pins temp prefix `.pk-durable-`; does not prove atomicity (bare `fs::write` passes)                                                 | none                                                                                | delete; durability belongs to durable_io / maintenance crash tests |
| load_config_returns_defaults_when_missing_and_errors_on_invalid_json | 291  | D   | Corrupt config must error, not silently reset to onboarding                                                                         | first-run covers the missing case                                                   | keep, drop the "parsing config json" string assert                 |
| load_config_ignores_removed_fields_and_save_drops_them               | 307  | D   | Upgrade regression with stated failure mode (old field looked like a corrupt config)                                                | upgrade.spec loads a real 0.3.0 config                                              | keep                                                               |
| load_config_accepts_legacy_lowercase_archive_mode_values             | 328  | D   | Legacy persisted value compat                                                                                                       | upgrade.spec partly                                                                 | keep                                                               |
| project_root_uses_current_app_directory_when_it_already_exists       | 397  | C   | Re-implements `ProjectDirs::from(...)`; name claims an existence check the code lacks; mutates `HOME`                               | none                                                                                | delete                                                             |
| restore_env_var_sets_and_clears_values                               | 422  | C   | Tests a `#[cfg(test)]` helper                                                                                                       | none                                                                                | delete                                                             |

#### src/app_lock.rs

| Test                                                                      | Line | Cat | Reason                                                                                                                           | E2E coverage                                                        | Action                                                                                                |
| ------------------------------------------------------------------------- | ---- | --- | -------------------------------------------------------------------------------------------------------------------------------- | ------------------------------------------------------------------- | ----------------------------------------------------------------------------------------------------- |
| app_lock_session_round_trips_passcode_and_state                           | 524  | D   | Startup lock and the backend data guard ("currently locked" is a command_error marker) are not E2E-reachable                     | lock.spec test 1 covers set/wrong/right passcode                    | keep                                                                                                  |
| biometric_status_and_unlock_follow_the_supplied_capability                | 589  | D   | Touch ID rules; headless E2E cannot run them (app-lock-panel-tradeoff.md:100)                                                    | none possible                                                       | keep                                                                                                  |
| touch_id_is_checked_when_turned_on_and_never_prompts_while_unavailable    | 674  | D   | Lid-closed regression (tradeoff §7 line 99); never prompts while unavailable                                                     | none possible                                                       | keep                                                                                                  |
| the_recovery_hint_is_readable_while_locked_and_follows_each_passcode_save | 734  | D   | Hint readable while locked; blank hint clears                                                                                    | lock.spec test 2 covers readable-while-locked and change-keeps-hint | keep (blank→None is not in E2E)                                                                       |
| unlock_rejects_no_auth_branch_when_passcode_and_biometric_are_disabled    | 768  | D   | Fail closed when config enables lock with no credential                                                                          | none                                                                | keep                                                                                                  |
| validation_status_and_unlock_cover_unavailable_edges                      | 795  | C   | Branch sweep: restates `biometric_note_for_platform` arms, private helper output, fallback prose ("future platform integration") | none                                                                | trim to: enable needs a credential, passcode ≥ 4, note codes index-aligned; drop prose/helper asserts |
| malformed_app_lock_files_fail_closed_until_passcode_is_cleared            | 915  | D   | Corrupt lock files fail closed; clearing recovers                                                                                | none                                                                | keep                                                                                                  |
| constant_time_hex_eq_matches_string_equality_for_correct_and_wrong_hashes | 967  | D   | Hand-rolled security comparator must equal string equality                                                                       | none                                                                | keep                                                                                                  |
| unlock_rejects_wrong_passcode_with_constant_time_hash_compare             | 998  | C   | Duplicate of the wrong/right passcode steps in test 524                                                                          | lock.spec test 1                                                    | delete                                                                                                |

#### src/diagnostics.rs

| Test                                                                      | Line | Cat | Reason                                                                  | E2E coverage                                                                      | Action                                          |
| ------------------------------------------------------------------------- | ---- | --- | ----------------------------------------------------------------------- | --------------------------------------------------------------------------------- | ----------------------------------------------- |
| diagnostics_report_paths_even_before_crashes                              | 373  | C   | Restates path fields and dir creation                                   | none                                                                              | delete                                          |
| frontend_location_formats_partial_source_positions                        | 385  | B   | Truth table of a private formatter                                      | none                                                                              | delete                                          |
| frontend_report_roundtrips_and_becomes_latest_crash                       | 408  | D   | Public record → latest-crash round trip                                 | none; scenario "a frontend error shows as the latest crash in Settings › Support" | keep (or replace with that E2E)                 |
| record_frontend_error_reports_unwritable_existing_report_path             | 435  | B   | Contrived dir-at-path; pins "replacing crash report" context string     | none                                                                              | delete                                          |
| recover_report_rename_replaces_existing_report_file                       | 460  | B   | Private helper                                                          | none                                                                              | delete                                          |
| recover_report_rename_reports_missing_target_write_failure                | 475  | B   | Private helper + exact context string                                   | none                                                                              | delete                                          |
| rust_panic_reports_capture_payload_location_and_thread                    | 491  | D   | Panic → crash report; E2E cannot panic Rust                             | none                                                                              | keep (the one panic test)                       |
| rust_panic_payloads_keep_owned_strings_and_fallback_text                  | 510  | C   | Probes two downcast branches; historic 3/8 flake (quality-matrix.md:84) | none                                                                              | delete                                          |
| rust_panic_report_uses_unnamed_thread_fallback                            | 533  | C   | Pins the "unnamed" literal                                              | none                                                                              | delete                                          |
| selective_panic_hook_forwards_untagged_thread_panics_to_the_previous_hook | 544  | C   | Tests test-only hook isolation code                                     | none                                                                              | delete (keep the isolation helper for test 491) |
| latest_crash_prefers_most_recent_report                                   | 591  | D   | Newest report wins                                                      | none                                                                              | keep                                            |
| latest_crash_prefers_rust_report_when_timestamps_tie                      | 622  | C   | Off-by-one probe of `>=`                                                | none                                                                              | delete                                          |
| diagnostics_ignore_malformed_crash_reports                                | 653  | D   | A bad report must not break the Support page                            | none                                                                              | keep                                            |
| diagnostics_ignore_unreadable_crash_report_paths                          | 666  | C   | Near-duplicate of 653 via contrived dir-at-path                         | none                                                                              | delete                                          |

#### src/schedule_attempts.rs

| Test                                                                    | Line | Cat | Reason                                                                                    | E2E coverage                                 | Action                                                   |
| ----------------------------------------------------------------------- | ---- | --- | ----------------------------------------------------------------------------------------- | -------------------------------------------- | -------------------------------------------------------- |
| recorder_persists_pre_archive_failure_and_success_transitions           | 371  | D   | Pre-archive failure and success are auditable (ADR-003:24, desktop-command-surface.md:48) | schedule.spec does not run the native worker | keep                                                     |
| next_invocation_durably_finalizes_an_orphaned_running_attempt           | 406  | D   | Crash recovery of a worker that died mid-run                                              | none possible                                | keep                                                     |
| next_invocation_does_not_interrupt_a_recently_started_worker            | 430  | D   | Concurrency: do not close a live worker's attempt                                         | none possible                                | keep                                                     |
| ledger_is_bounded_and_one_malformed_record_does_not_hide_valid_evidence | 446  | D   | Bounded ledger; one bad file does not hide the rest                                       | none                                         | keep                                                     |
| health_derivation_flags_failed_and_stale_or_interrupted_workers         | 475  | D   | Stable issue codes the UI localizes                                                       | none                                         | keep                                                     |
| empty_health_and_invalid_cadence_have_safe_defaults                     | 512  | C   | `stale_after(NaN) == 30 min` magic on a private helper; empty → empty                     | none                                         | delete                                                   |
| malformed_or_unreadable_last_success_is_a_typed_health_issue            | 520  | D   | A bad marker becomes a typed issue, not a crash                                           | none                                         | keep                                                     |
| orphan_finalization_skips_noise_corrupt_and_unreadable_records          | 545  | D   | Bad history must not block recording a new tick                                           | none                                         | keep                                                     |
| invalid_or_missing_ledger_is_safe                                       | 572  | B   | Contrived file-at-dir; pins "reading schedule attempt ledger"                             | none                                         | delete (missing → empty is covered by 371's fresh start) |

#### src/git_audit.rs

| Test                                               | Line | Cat | Reason                                                              | E2E coverage | Action                         |
| -------------------------------------------------- | ---- | --- | ------------------------------------------------------------------- | ------------ | ------------------------------ |
| ensure_repo_bootstraps_git_metadata                | 179  | B   | Pins README.md / .gitignore side files; silently passes without git | none         | delete                         |
| write_and_commit_audit_files                       | 193  | D   | Commit, then no empty commit                                        | none         | keep                           |
| write_audit_file_handles_top_level_paths           | 212  | C   | Probes `ensure_parent_dir` branch; restates side files              | none         | delete                         |
| run_git_reports_failures_with_and_without_stderr   | 224  | B   | Exact error strings of a private runner                             | none         | delete                         |
| optional_commit_reports_warning_instead_of_failing | 240  | D   | Git failure must not fail a backup; coded warning                   | none         | keep, drop the prose substring |

#### src/browser_access.rs

| Test                                                             | Line | Cat | Reason                                                               | E2E coverage  | Action |
| ---------------------------------------------------------------- | ---- | --- | -------------------------------------------------------------------- | ------------- | ------ |
| full_disk_access_applies_matches_the_target_family               | 87   | C   | Restates `cfg!(target_os = "macos")`                                 | none          | delete |
| is_permission_denied_detects_typed_and_stringified_denials       | 92   | D   | Typed and stringified EPERM detection; E2E cannot produce TCC denial | none possible | keep   |
| classify_history_access_core_gates_full_disk_access_to_tcc_hosts | 106  | C   | Four-row truth table of `a && b`                                     | none          | delete |

#### src/browser_retention.rs

| Test                                                                                             | Line   | Cat | Reason                                      | E2E coverage | Action |
| ------------------------------------------------------------------------------------------------ | ------ | --- | ------------------------------------------- | ------------ | ------ |
| marks_safari_as_macos_year_scale_retention, defaults_other_families_to_browser_managed_retention | 27, 34 | C   | Restate the two match arms (365 has no doc) | none         | delete |

#### src/ai_sidecar.rs

| Test                                                              | Line | Cat | Reason                                                                        | E2E coverage | Action                                                 |
| ----------------------------------------------------------------- | ---- | --- | ----------------------------------------------------------------------------- | ------------ | ------------------------------------------------------ |
| deferred_sidecar_accepts_only_noop_syncs                          | 196  | B   | Pins a now-false "tracked for PathKeep v0.3.0" message on a legacy no-op shim | none         | delete with the shim                                   |
| deferred_sidecar_reports_empty_counts_and_searches                | 221  | C   | Asserts stubs return 0 / None                                                 | none         | delete                                                 |
| storage_and_table_helpers_stay_stable_without_vector_dependencies | 244  | C   | Branch sweep (missing / unreadable dir, name prefix)                          | none         | delete; if sidecar bytes stay in Settings, cover there |

#### src/test_support.rs

| Test                                                 | Line | Cat | Reason                                                                                                | E2E coverage | Action       |
| ---------------------------------------------------- | ---- | --- | ----------------------------------------------------------------------------------------------------- | ------------ | ------------ |
| network_override_is_ignored_without_debug_assertions | 59   | C   | In the debug gate it is the "1" case of the next test; the release claim only holds under `--release` | none         | fold into 71 |
| only_exactly_one_blocks_network                      | 71   | D   | Failure modes written first (file header a–d); E2E depends on this switch                             | none         | keep         |

#### src/intelligence_blobs.rs

| Test                                         | Line | Cat | Reason                             | E2E coverage | Action |
| -------------------------------------------- | ---- | --- | ---------------------------------- | ------------ | ------ |
| readable_text_blobs_are_deduplicated_by_hash | 91   | D   | Content-addressed store contract   | none         | keep   |
| clearing_blobs_resets_the_sidecar_directory  | 114  | C   | Tests a `#[cfg(test)]`-only helper | none         | delete |

#### src/pages/tests.rs

| Test                                                 | Line | Cat | Reason                                                                | E2E coverage | Action |
| ---------------------------------------------------- | ---- | --- | --------------------------------------------------------------------- | ------------ | ------ |
| canonical_variants_use_the_production_indexed_lookup | 8    | D   | Canonical equality across URL spellings + EXPLAIN plan uses the index | none         | keep   |

#### src/models/mod.rs

| Test                                                                   | Line | Cat | Reason                                                                                             | E2E coverage                                          | Action                                  |
| ---------------------------------------------------------------------- | ---- | --- | -------------------------------------------------------------------------------------------------- | ----------------------------------------------------- | --------------------------------------- |
| ai_search_request_defaults_to_eight_results                            | 39   | C   | Default value                                                                                      | none                                                  | delete                                  |
| intelligence_status_defaults_to_empty_state                            | 48   | C   | Default value                                                                                      | none                                                  | delete                                  |
| enrichment_preferences_merge_with_defaults                             | 57   | B   | Pins `len() == 2` and index order around a one-line rule                                           | none                                                  | delete or trim to "user's off survives" |
| ai_settings_deserialization_uses_current_enrichment_defaults           | 70   | C   | `{}` → defaults restates `#[serde(default)]`                                                       | upgrade.spec loads a real 0.3.0 config                | delete                                  |
| enrichment_plugin_states_merge_with_defaults                           | 78   | B   | Pins len 3, order, and keeps an unknown "custom-plugin" (no such concept)                          | none                                                  | delete or trim                          |
| enrichment_plugin_state_defaults_include_both_built_ins                | 103  | C   | Restates the default list                                                                          | none                                                  | delete                                  |
| deterministic_module_defaults_include_all_built_ins                    | 111  | C   | Restates registry: count 8 and order                                                               | none                                                  | delete                                  |
| deterministic_module_state_merge_with_defaults                         | 125  | B   | `len() == 8`; breaks on every module change                                                        | none                                                  | delete                                  |
| deterministic_module_state_merge_discards_legacy_module_ids            | 141  | D   | Legacy id migration rule                                                                           | none                                                  | keep, drop `len() == 8`                 |
| normalize_app_config_restores_missing_runtime_defaults                 | 165  | B   | Kitchen sink: counts 2/8/1, prefetch cap 10 (no spec); clamps duplicated in models/intelligence.rs | none                                                  | delete                                  |
| embedding_provider_merge_and_default_honor_user_choice_and_the_builtin | 198  | D   | Built-in provider always present; identity fields forced back; dangling selection falls back       | none                                                  | keep                                    |
| og_image_fetch_mode_default_is_background                              | 256  | C   | Default value                                                                                      | none                                                  | delete                                  |
| og_image_fetch_mode_serializes_as_snake_case                           | 266  | C   | Restates serde rename                                                                              | none                                                  | delete                                  |
| og_image_fetch_mode_rejects_unknown_variants                           | 282  | C   | Tests serde itself                                                                                 | none                                                  | delete                                  |
| og_image_settings_default_matches_doc_contract                         | 288  | C   | Default values; og-images.md has no 50/100 budgets, so "doc contract" is not true                  | none                                                  | delete                                  |
| og_image_settings_effective_mode_truth_table                           | 301  | D   | Kill switch wins (og-images.md:151)                                                                | history-tools.spec preview tests use the default only | keep                                    |
| og_image_settings_deserializes_empty_object_to_defaults                | 327  | C   | Restates `#[serde(default)]`                                                                       | upgrade.spec                                          | delete                                  |
| og_image_settings_round_trips_through_json                             | 339  | C   | serde round trip                                                                                   | none                                                  | delete                                  |
| og_image_settings_partial_json_keeps_explicit_fields                   | 354  | C   | Restates `#[serde(default)]`                                                                       | upgrade.spec                                          | delete                                  |
| extend_notes_keeps_the_legacy_and_coded_note_channels_aligned          | 367  | D   | Invariant between `notes` and `note_details`; stated drift failure                                 | browser-import.spec does not see notes                | keep                                    |

#### src/models/intelligence.rs

| Test                                                                      | Line | Cat | Reason                                                                                       | E2E coverage                                                                                 | Action                                        |
| ------------------------------------------------------------------------- | ---- | --- | -------------------------------------------------------------------------------------------- | -------------------------------------------------------------------------------------------- | --------------------------------------------- |
| ai_chat_stream_event_channel_is_pinned                                    | 2171 | C   | Const equals its own literal; does not check the TS side                                     | none                                                                                         | delete                                        |
| ai_chat_stream_chunk_variants_serialize_with_kind_tag                     | 2177 | D   | Tauri-event wire shape; dev bridge cannot deliver events, so no E2E can; TS is hand-mirrored | none possible                                                                                | keep                                          |
| ai_chat_stream_chunk_w_ai_7_additive_variants_serialize_with_kind_tag     | 2208 | D   | Same contract for toolCall/toolResult/usage/citations                                        | none possible                                                                                | keep; drop "byte-identical to before" framing |
| model_download_progress_event_wires_camel_case_fields                     | 2320 | D   | Regression for review H-1 (fields went snake_case → `undefined` in UI)                       | none possible                                                                                | keep                                          |
| ai_citation_omits_canonical_url_on_the_legacy_path                        | 2367 | D   | Citations are persisted (`ai/read_model.rs:440` parses stored JSON); old rows must load      | none                                                                                         | keep the deserialize half                     |
| ai_chat_role_serializes_kebab_case                                        | 2398 | C   | Restates `rename_all`                                                                        | none                                                                                         | delete                                        |
| ai_chat_send_request_uses_camel_case_keys                                 | 2406 | C   | Restates `rename_all`                                                                        | none                                                                                         | delete                                        |
| ai_chat_send_request_deserializes_legacy_w_ai_1_payload                   | 2424 | C   | IPC-only; frontend and backend ship together                                                 | none                                                                                         | delete                                        |
| ai_chat_send_ack_and_stream_event_use_run_id_camel_case                   | 2440 | C   | One-field camelCase; the `{runId, chunk}` envelope is worth one assert                       | none                                                                                         | fold envelope assert into 2177, delete rest   |
| ai_chat_cancel_result_uses_camel_case_key                                 | 2456 | C   | Single-word field cannot change under camelCase                                              | none                                                                                         | delete                                        |
| llm_provider_capability_report_uses_camel_case_keys                       | 2464 | C   | Restates `rename_all`                                                                        | none                                                                                         | delete                                        |
| content_fetch_domain_blocked_matches_case_insensitively_and_ignores_blank | 2480 | D   | Privacy block list rule                                                                      | none                                                                                         | keep                                          |
| content_extractor_enabled_defaults_on_and_honors_explicit_disable         | 2498 | C   | One-line lookup with default                                                                 | none                                                                                         | delete                                        |
| ai_settings_search_knobs_default_to_conservative_values                   | 2512 | C   | Default values                                                                               | background-work.spec "search ranking values persist in the backend config" asserts 60 / 0.15 | delete                                        |
| ai_settings_search_knobs_serialize_camel_case                             | 2522 | C   | Restates `rename_all`                                                                        | same spec reads `hybridRrfK`, `starredBoost`, `lexicalWeight`                                | delete                                        |
| ai_settings_search_knobs_deserialize_from_older_payload_without_them      | 2535 | C   | `{}` → defaults; normalize already clamps k ≥ 1                                              | upgrade.spec loads a 0.3.0 config                                                            | delete                                        |
| ai_settings_gpu_enabled_round_trips_when_present                          | 2549 | C   | serde round trip of one bool                                                                 | none                                                                                         | delete                                        |
| ai_index_request_scope_defaults_to_incremental_and_round_trips            | 2560 | C   | IPC-only version skew + kebab restatement                                                    | none                                                                                         | delete                                        |
| normalize_search_knobs_clamps_into_valid_ranges                           | 2582 | D   | Hand-edited config sanitizing; zero denominator failure                                      | background-work.spec covers UI range only                                                    | keep                                          |
| normalize_search_knobs_resets_nan_weights_and_boost_to_defaults           | 2606 | D   | NaN guard                                                                                    | none                                                                                         | keep                                          |
| normalize_search_knobs_is_idempotent_for_valid_values                     | 2621 | C   | "covers the in-range branches" — coverage probe                                              | none                                                                                         | delete                                        |
| ai_search_request_starred_only_round_trips_and_defaults_absent            | 2638 | C   | IPC-only skew + restatement                                                                  | none                                                                                         | delete                                        |
| ai_search_request_sort_round_trips_and_defaults_absent                    | 2651 | C   | IPC-only skew + restatement                                                                  | none                                                                                         | delete                                        |
| ai_search_sort_parse_maps_tokens_and_degrades_unknown_to_relevance        | 2665 | D   | Parser for model-supplied tokens; unknown degrades, never errors                             | none                                                                                         | keep                                          |
| ai_search_sort_date_ordered_and_list_history_token                        | 2677 | C   | Restates match arms incl. an unreachable Relevance → "newest"                                | none                                                                                         | delete                                        |
| ensure_ai_capability_refuses_when_master_flag_is_off                      | 2699 | D   | Consent gate                                                                                 | none                                                                                         | keep                                          |
| ensure_ai_capability_refuses_when_sub_flag_is_off                         | 2716 | D   | Consent gate per capability                                                                  | none                                                                                         | keep                                          |
| ensure_ai_capability_passes_only_when_master_and_sub_flag_are_on          | 2737 | D   | Gate does not depend on siblings                                                             | none                                                                                         | keep                                          |
| ensure_ai_capability_refuses_when_both_flags_are_off                      | 2754 | C   | Subsumed by 2699                                                                             | none                                                                                         | delete                                        |

#### src/models/core_intelligence/tests.rs

| Test                                                                  | Line | Cat | Reason                                                                               | E2E coverage                                                       | Action                      |
| --------------------------------------------------------------------- | ---- | --- | ------------------------------------------------------------------------------------ | ------------------------------------------------------------------ | --------------------------- |
| section_window_serializes_with_camel_case_variant_fields              | 26   | D   | Guards manual per-field renames (the H-1 bug class)                                  | insights.spec reads sections, not the window shape                 | keep                        |
| calendar_day_history_window_serializes_with_camel_case_reference_date | 44   | D   | Same, other variant                                                                  | as above                                                           | keep (could merge with 26)  |
| domain_deep_dive_request_accepts_legacy_domain_alias                  | 59   | C   | Alias nobody sends (frontend sends `registrableDomain`, `insights.ts:166`); IPC-only | insights-drill.spec "a site from Top sites..." uses the real field | delete (and drop the alias) |
| entity_reference_serializes_with_existing_camel_case_tags             | 75   | A   | Pins snake_case `canonical_url` against the TS contract (see A4)                     | none                                                               | fix the code                |
