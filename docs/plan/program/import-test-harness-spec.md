# Import Test Harness Spec

> Companion to [`import-dedup-audit.md`](import-dedup-audit.md).
> The audit answers _what is the current behavior_. This spec answers
> _what tests would prove or disprove that behavior at every supported
> source and edge case_, so the user can be confident that a re-import
> of any combination of browsers will not silently lose, duplicate, or
> corrupt visit records.

Owning work block: `WORK-IMPORT-TEST-HARNESS-A` (queued in `BACKLOG.md`).

---

## 1. Goals & Non-Goals

### Goals

1. Build a **fixture generator** that emits real-format browser history
   payloads — Chromium `History`, Firefox `places.sqlite`, Safari
   `History.db`, Google Takeout JSON / JSONL — from a deterministic
   programmatic scenario description.
2. Build a **scenario library** that covers every documented edge case in
   the audit, including known bugs (B1–B6) and architecturally-correct
   behaviors that future refactors might silently break.
3. Build an **end-to-end test runner** that takes one scenario, drives the
   real `vault-core` ingest pipeline through it, and asserts canonical-DB
   truth (visit counts, URL counts, fingerprint stability, per-profile
   provenance, watermark advancement, revert safety).
4. Guarantee the harness produces **zero false positives**: every failing
   assertion either is a real bug in product code or a real intentional
   change that needs a contract-test update.
5. Keep the harness **self-validating**: the fixture generator itself is
   tested by parser round-trip (write a fixture → parse it → assert the
   parser saw what the generator promised) so a generator bug cannot
   pretend a product bug exists.

### Explicit Non-Goals

1. **No real user data** in fixtures. The user has personal browser data
   on the development machine; the playbook
   ([browser-support-and-adapter-playbook.md:152](../../architecture/browser-support-and-adapter-playbook.md))
   forbids copying private URLs/titles into docs or repo. The fixture
   generator **must not sample from real DBs at any layer** — every URL,
   title, timestamp, and ID is synthesized from a seed.
2. **No product-code bug fixes in this work block.** B1–B6 each get a
   failing test that documents the bug; fixes ship in dedicated follow-up
   blocks so the fix PR can point at the failing test as evidence.
3. **No view-layer cross-browser aggregation work.** That has its own
   pending work block driven by the planning conversation.
4. **No performance optimization.** Harness measures memory bounds as a
   contract assertion (does a 1.44M-record import stay under the agreed
   RSS ceiling?) but does not optimize the ingest pipeline.
5. **No support for non-promised browsers.** Scenarios cover the families
   in [browser-support-and-adapter-playbook.md](../../architecture/browser-support-and-adapter-playbook.md):
   Chromium-family, Firefox-family, Safari, Takeout. Pale Moon, qutebrowser,
   mobile exports are out of scope.

---

## 2. Crate Architecture

### New crate: `browser-history-fixtures`

Location: `src-tauri/crates/browser-history-fixtures/`.

```
browser-history-fixtures/
├── Cargo.toml                    # added to workspace; no Tauri dep
├── src/
│   ├── lib.rs                    # public surface: Scenario, ScenarioBuilder, fixtures::*
│   ├── seed.rs                   # deterministic PRNG (StdRng with explicit seed)
│   ├── catalog.rs                # synthetic URL/title pools (public-domain text only)
│   ├── time.rs                   # epoch conversions (Chrome/Unix/Safari/Firefox)
│   ├── scenario/
│   │   ├── mod.rs                # Scenario / ScenarioBuilder DSL
│   │   ├── browser.rs            # BrowserProfile builder, clone_history, add_visits
│   │   ├── assertions.rs         # CanonicalAssertions: per-profile visit_count, etc.
│   │   └── runner.rs             # drives ingest pipeline, returns CanonicalView
│   ├── chromium_db.rs            # writes real Chromium History sqlite
│   ├── firefox_db.rs             # writes real places.sqlite
│   ├── safari_db.rs              # writes real History.db (CFAbsoluteTime semantics)
│   └── takeout_json.rs           # writes BrowserHistory.json + .jsonl + zip
├── tests/
│   ├── fixture_roundtrip.rs      # self-validation: each generator output parses cleanly
│   ├── chromium_dedup.rs         # scenarios C1–C7
│   ├── firefox_dedup.rs          # scenarios F1–F4
│   ├── safari_dedup.rs           # scenarios S1–S3
│   ├── takeout_dedup.rs          # scenarios T1–T6
│   ├── cross_source.rs           # scenarios X1–X5
│   ├── time_and_url.rs           # scenarios E1–E8
│   ├── corrupt_and_recover.rs    # scenarios R1–R4
│   └── memory_bounds.rs          # scenario M1 (large data, optional `#[ignore]` until --features=big-data)
└── README.md                     # quick-start, how to add a scenario
```

Why a new crate rather than putting it in `vault-core/tests/`:

- `vault-core` already has 31,762 instrumented lines and 1,485+ tests;
  adding a generator crate keeps the test surface focused.
- The generator needs `rusqlite` write access with control over PRAGMAs;
  isolating it makes the dependency story cleaner.
- The fixture generator is itself usable for benchmarks, manual repro
  bundles, and future doctor-tool development — it's a long-lived
  utility, not a one-shot test asset.

### Dependencies

- `rusqlite` with `bundled` feature (matches `vault-core`)
- `serde_json` (Takeout payloads)
- `chrono` (epoch conversions)
- `rand` + `rand_chacha` (deterministic PRNG; explicit seed in every scenario)
- `tempfile` (test sandboxes)
- `zip` (for zipped Takeout fixtures matching the source classifier expectations)
- **No new third-party deps that need supply-chain review** — all four are
  already in the workspace.

---

## 3. Fixture Generator API

### Scenario DSL — declarative, deterministic, readable

```rust
let scenario = Scenario::new("edge_imports_chrome_then_diverges")
    .seed(0xCAFEBABE_DEADBEEF)

    // Chrome profile with 60 days of synthetic browsing
    .add_browser(Chromium("Google Chrome"))
        .profile("Default")
        .with_visits(SyntheticPattern {
            count: 100,
            window: days_ago(60)..days_ago(30),
            url_pool: PublicDomainUrls::news_sites(),
            title_pool: PublicDomainTitles::wikipedia_articles(),
            transition_mix: TransitionMix::typical(),
        })

    // Edge profile that "imported from Chrome" — same visits but
    // different source_visit_ids (Chrome's IDs renumbered by Edge)
    .add_browser(Chromium("Microsoft Edge"))
        .profile("Default")
        .imported_from(Chromium("Google Chrome"), "Default")
            .renumber_visit_ids()   // simulates browser import behavior
            .preserve_visit_times() // visit_time_ms identical to Chrome
        .with_visits(SyntheticPattern {
            count: 50,
            window: days_ago(30)..now(),
            url_pool: PublicDomainUrls::news_sites(),
            transition_mix: TransitionMix::typical(),
        })

    // Chrome also kept browsing for 30 days
    .add_visits_to(Chromium("Google Chrome"), "Default", SyntheticPattern {
        count: 30,
        window: days_ago(30)..now(),
        ..Default::default()
    });

let canonical = scenario.run_in_vault()?;

canonical.assert(|view| {
    // by-design: per-profile dedup keeps Edge + Chrome separate
    view.expect_url_count_for_profile("chrome:Default", 130);
    view.expect_url_count_for_profile("edge:Default", 150);

    // by-design: cross-browser does NOT dedup at storage layer
    view.expect_canonical_url_count_distinct_across_profiles(180);

    // contract: no visit got dropped
    view.expect_visit_count_for_profile("chrome:Default", 130);
    view.expect_visit_count_for_profile("edge:Default", 150);

    // contract: provenance preserved
    view.expect_browser_product("edge:Default", "Microsoft Edge");
    view.expect_browser_product("chrome:Default", "Google Chrome");

    // contract: watermark advanced for both profiles
    view.expect_watermark_visit_id_at_least("chrome:Default", 130);
    view.expect_watermark_visit_id_at_least("edge:Default", 150);
});
```

### `SyntheticPattern`

```rust
pub struct SyntheticPattern {
    pub count: usize,                  // number of visits
    pub window: Range<DateTime<Utc>>,  // time range
    pub url_pool: UrlPool,             // synthetic URLs (public-domain set)
    pub title_pool: TitlePool,         // synthetic titles
    pub transition_mix: TransitionMix, // distribution of Chrome transition types
    pub revisit_rate: f64,             // 0.0 = all unique URLs, 1.0 = all repeats
    pub duration_distribution: DurationDistribution,
}
```

### Synthetic content pools

All URLs and titles are **synthesized from public-domain corpora**:

- **URL hosts**: a small fixed list of obviously-fake hosts
  (`example.com`, `example.org`, `synthetic.test`, `pathkeep-fixture.invalid`)
  plus public Wikipedia / Wikimedia hosts when we need plausible-looking
  long URLs (e.g. `en.wikipedia.org/wiki/<topic>`).
- **Page paths**: deterministic from seed — `/article/<sha8>/<title-slug>`.
- **Titles**: pulled from a checked-in list of public-domain Wikipedia
  article titles (article titles themselves are PD; the corpus file is
  checked in at `browser-history-fixtures/src/catalog/wikipedia_titles.txt`).
- **Search terms**: a fixed set of obviously-non-real queries (`brown
fox jumps`, `lorem ipsum dolor`, etc.).

**No fixture URL or title is ever sampled from a real user DB.** The
catalog is committed once and reused; PRs that touch the catalog must
include an attribution comment for the source.

### Fixture file outputs

Each `Scenario::run_in_vault()` materializes:

- One `History` SQLite per Chromium profile, written with the exact
  schema (`urls`, `visits`, `downloads`, `keyword_search_terms`,
  `meta`) that Chrome ships, populated by the synthetic data and
  indexed the same way Chrome indexes it.
- One `places.sqlite` per Firefox profile with `moz_places`,
  `moz_historyvisits`, and the meta tables Firefox parser inspects.
- One `History.db` per Safari profile with `history_items`,
  `history_visits`, plus the `synthesized` / `load_successful` columns
  the Safari parser may probe.
- Takeout payloads (BrowserHistory.json or JSONL; optionally zipped to
  exercise the zip code path) in a path layout that matches what the
  Takeout source classifier looks for
  ([takeout/source.rs:402-418](../../../src-tauri/crates/browser-history-parser/src/takeout/source.rs)).

### Self-validation: fixture round-trip

`tests/fixture_roundtrip.rs` proves the generator is honest. For every
generator output:

1. Write the fixture.
2. Open it with the **real PathKeep parser** (`browser_history_parser::chromium::parse_history` etc.).
3. Assert the parser saw exactly the records the generator promised.

If a generator bug exists (wrong schema, wrong epoch, missing column),
the round-trip test fails _before_ any scenario can pretend a product
bug exists. **Without this guard, the harness is worse than useless** —
it can give false confidence.

---

## 4. Assertions API

```rust
pub struct CanonicalView<'a> {
    archive: &'a Connection,
}

impl CanonicalView<'_> {
    // ---- counts ----
    pub fn expect_url_count_for_profile(&self, profile_key: &str, expected: usize);
    pub fn expect_visit_count_for_profile(&self, profile_key: &str, expected: usize);
    pub fn expect_total_visit_count(&self, expected: usize);
    pub fn expect_distinct_canonical_url_count_distinct_across_profiles(&self, expected: usize);

    // ---- provenance ----
    pub fn expect_browser_product(&self, profile_key: &str, expected: &str);
    pub fn expect_source_profile_count(&self, expected: usize);

    // ---- dedup behavior ----
    pub fn expect_no_duplicate_visit_keys(&self);
    pub fn expect_no_duplicate_visit_fingerprints(&self);
    pub fn expect_url_visit_count(&self, profile_key: &str, url: &str, expected: i64);
    pub fn expect_url_first_last_visit_within(&self, profile_key: &str, url: &str, range: Range<DateTime<Utc>>);

    // ---- watermark ----
    pub fn expect_watermark_visit_id_at_least(&self, profile_key: &str, min: i64);
    pub fn expect_watermark_url_time_at_least(&self, profile_key: &str, min_ms: i64);

    // ---- import batch behavior ----
    pub fn expect_visits_in_import_batch(&self, batch_id: i64, expected: usize);
    pub fn expect_no_orphan_visits(&self);  // every visit's url_id resolves
    pub fn expect_no_visits_in_reverted_batch(&self);
}
```

The assertion helpers all read directly from the canonical archive
SQLite; no view-model layer is in the path. Assertion failures include
**the SQL query that returned the wrong count** so the developer can
re-run it locally.

### Bug-targeted assertions

For each known bug, the spec defines a named assertion that fails
_now_ and passes after the fix:

- `expect_url_count_monotonic_under_repeated_imports` → catches **B1**
- `expect_firefox_long_tail_revisit_not_dropped` → catches **B2**
- `expect_safari_long_tail_revisit_not_dropped` → catches **B2**
- `expect_takeout_rename_does_not_duplicate` → catches **B3**
- `expect_takeout_then_local_chrome_same_period_dedup` → catches **B4**
- `expect_takeout_url_hash_no_collisions_at_million_scale` → catches **B5**
- `expect_takeout_time_unit_matches_documented_contract` → catches **B6**

These are written first as `#[test] #[should_panic]` (documenting the
current broken behavior), then converted to plain `#[test]` when the
fix lands. The spec is explicit: **landing a fix without flipping the
test invalidates the work block.**

---

## 5. Scenario Library

Each scenario maps to one test function. Priority drives implementation
order in the work block; everything is in scope before the block closes.

### Priority 1 — Highest ROI (lay this in the scaffold commit)

| ID  | Scenario                                  | Targets                                                                                                                                            |
| --- | ----------------------------------------- | -------------------------------------------------------------------------------------------------------------------------------------------------- |
| C1  | `chromium_baseline_import`                | happy path, source_visit_id uniqueness, run ledger correctness                                                                                     |
| C2  | `chromium_incremental_no_new_data`        | watermark works; second import = 0 new rows                                                                                                        |
| C3  | `chromium_incremental_revisit_of_old_url` | regression for the OR clause fix; would fail without [chromium/mod.rs:85-90](../../../src-tauri/crates/browser-history-parser/src/chromium/mod.rs) |
| T1  | `takeout_baseline_import`                 | happy path; no source_visit_id from browser, full fingerprint reliance                                                                             |
| T2  | `takeout_rename_file_reimport`            | **B3 failing test** — same data, different path, expect dedup, assert duplicates appear                                                            |
| X1  | `edge_imports_chrome_then_diverges`       | per-profile contract preserved, no cross-browser dedup                                                                                             |

### Priority 2 — Bug coverage

| ID  | Scenario                                                   | Targets                                             |
| --- | ---------------------------------------------------------- | --------------------------------------------------- |
| C4  | `chromium_reimport_older_snapshot_does_not_regress_counts` | **B1 failing test**                                 |
| F1  | `firefox_baseline_import`                                  | happy path for places.sqlite                        |
| F2  | `firefox_incremental_revisit_of_old_url`                   | **B2 failing test** for Firefox                     |
| S1  | `safari_baseline_import`                                   | happy path for History.db                           |
| S2  | `safari_incremental_revisit_of_old_url`                    | **B2 failing test** for Safari                      |
| T3  | `takeout_then_local_chrome_same_period`                    | **B4 failing test** — assert systematic doubling    |
| T4  | `takeout_million_record_hash_distribution`                 | **B5 failing test** — stress `stable_key_i64`       |
| T5  | `takeout_time_unit_contract`                               | **B6 failing/passing test** — pins format-of-record |

### Priority 3 — Cross-source robustness

| ID  | Scenario                                              | Targets                                                        |
| --- | ----------------------------------------------------- | -------------------------------------------------------------- |
| X2  | `chrome_brave_vivaldi_three_way_overlap`              | three Chromium-family profiles, partial overlap, all preserved |
| X3  | `firefox_places_with_safari_history_overlap`          | mixed family time conversions correct                          |
| X4  | `takeout_and_browser_direct_same_profile_same_period` | end-to-end version of T3 with real ingest commands             |
| X5  | `microsoft_edge_not_collapsed_to_chrome`              | provenance — Edge must not be tagged as Google Chrome          |

### Priority 4 — Time / URL / encoding edge cases

| ID  | Scenario                                      | Targets                                                          |
| --- | --------------------------------------------- | ---------------------------------------------------------------- |
| E1  | `chrome_time_extreme_far_future`              | `unix_micros_to_chrome_time` saturation                          |
| E2  | `safari_cfabsolute_time_pre_2001`             | negative CFAbsoluteTime handling                                 |
| E3  | `firefox_microseconds_vs_chrome_microseconds` | family misrouting test                                           |
| E4  | `dst_transition_visit`                        | hour-boundary visit during DST transition                        |
| E5  | `same_millisecond_two_visits`                 | two visits at literally identical ms, different source_visit_ids |
| E6  | `url_with_fragment_and_trailing_slash`        | document current behavior: separate rows                         |
| E7  | `url_with_idn_punycode_mix`                   | document current behavior                                        |
| E8  | `url_very_long_8kb_plus`                      | SQLite TEXT column accepts; no truncation                        |

### Priority 5 — Corruption / recovery / concurrency

| ID  | Scenario                                                | Targets                                                 |
| --- | ------------------------------------------------------- | ------------------------------------------------------- |
| R1  | `corrupt_history_db_quick_check_fails`                  | preview honestly fails, no partial rows                 |
| R2  | `mid_import_crash_rollback`                             | transaction rolls back, watermark unchanged             |
| R3  | `import_batch_revert_clears_visits_only_for_that_batch` | revert isolation                                        |
| R4  | `staging_lock_contention`                               | History file held by browser, staging snapshot succeeds |
| R5  | `concurrent_import_same_profile_serialization`          | SQLite write lock serializes; no torn state             |

### Priority 6 — Performance / memory bounds (optional `#[ignore]` until opted in)

| ID  | Scenario                                            | Targets                                                                                             |
| --- | --------------------------------------------------- | --------------------------------------------------------------------------------------------------- |
| M1  | `chromium_1_44_million_visits_under_memory_ceiling` | the AGENTS.md design point: 8 GB / 4 core machine, 60 years of moderate use; assert peak RSS < N MB |

---

## 6. How New Bugs Get Added

When a user reports a new dedup / loss / duplication issue:

1. The triage step is to add a scenario to the library that reproduces
   the report from a synthetic fixture. If the synthetic fixture cannot
   reproduce, the report is either operator error or a real-data leak
   (e.g. Chrome version-specific schema we don't generate yet) — the
   audit doc gets updated to widen the fixture surface.
2. Once a failing scenario exists, the bug is in scope for a fix work
   block.
3. The fix block flips the scenario from `#[should_panic]` to plain
   `#[test]` and gets merged. The scenario stays in the library forever
   as a regression guard.

This means **the harness is the bug tracker for ingest correctness**.
The audit doc lists six bugs today; the harness should converge to
zero `should_panic` annotations over time.

---

## 7. Acceptance for `WORK-IMPORT-TEST-HARNESS-A`

The work block is done when:

1. `browser-history-fixtures` crate exists, builds clean, is in the
   Cargo workspace, and is included in `bun run check`.
2. All round-trip self-validation tests pass.
3. All Priority 1 scenarios are implemented and either pass (for
   contract scenarios) or `#[should_panic]` with a doc comment
   referencing the audit bug (for bug scenarios).
4. The work block's CHANGELOG entry lists, by name, which audit bugs
   now have failing tests.
5. The audit doc gets a new section: "Bugs with failing tests" linking
   each to its scenario.

The work block **does not** require Priorities 2–6 to be complete; those
are the natural follow-up blocks once the foundation lands. But the
spec already enumerates them so future work doesn't need to re-derive
the list.

---

## 8. Open Questions to Resolve During Implementation

These are resolvable from code-reading, not user discussion, but
deserve calling out so they aren't forgotten:

1. **Takeout time unit truth.** Does the runtime really receive Chrome
   epoch microseconds in `time_usec`, or Unix epoch microseconds, or
   both depending on file format? Resolve by writing scenario T5 with
   both shapes, observing which one matches the visible Chrome history
   ground truth.
2. **`profile_key` collision under same-name profiles.** If a user has
   two Chrome profiles both named `Default` on the same machine (e.g.
   two macOS user accounts share-mounted), do they collide? Test as
   scenario R6 (added if probe shows this is a real risk).
3. **Are Atlas / Comet adapters fully covered by the chromium
   scenarios?** Probably yes by family membership, but confirm with a
   discovery-side spot test in `vault-core/tests/` if no separate
   parser test exists.
4. **Memory ceiling for M1.** AGENTS.md says 8 GB RAM, 4 core, 1.44M
   records. Pick a sensible RSS bound (likely 800 MB) and document the
   measurement methodology so the test stays deterministic across
   hosts.

---

_Update this doc when scenario coverage expands or when the audit's
bug list changes. Treat it as living source-of-truth alongside
`research-and-decisions.md`._
