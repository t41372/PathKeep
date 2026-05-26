# Import & Dedup Architecture Audit

> Written 2026-05-25 as the foundation for `WORK-IMPORT-TEST-HARNESS-A`.
> Source of truth: the code at the commits referenced below. Scenarios cited
> here are observable behaviors, not speculation — every claim has a file:line.

This audit answers one question: **when a user imports browser history into
PathKeep — once, twice, from multiple browsers, from Takeout, from a re-stage of
the same DB — what does the canonical archive actually end up holding, and
where does that diverge from naive user expectations?**

The audit deliberately keeps product UX out of scope (the cross-browser "looks
duplicated" experience is being addressed by a separate view-layer aggregation
work block). Here we cover only storage-layer truth.

---

## 1. Dedup Keys at a Glance

- **`source_profiles`** — UNIQUE on `profile_key`, computed as
  `browser_kind` + `:` + `profile_name` by
  [002_archive_runtime_foundation.sql:7](../../../src-tauri/crates/vault-core/src/migrations/002_archive_runtime_foundation.sql).
- **`urls`** — UNIQUE on `(source_profile_id, source_url_id)`; upsert at
  [writes.rs:95-157](../../../src-tauri/crates/vault-core/src/archive/ingest/writes.rs).
- **`visits`** — UNIQUE on `(source_profile_id, source_visit_id)` with a
  partial fallback unique index on `(source_profile_id, event_fingerprint)`;
  see [002:28-32](../../../src-tauri/crates/vault-core/src/migrations/002_archive_runtime_foundation.sql)
  and the insert at [writes.rs:160-218](../../../src-tauri/crates/vault-core/src/archive/ingest/writes.rs).
- **`downloads`** — UNIQUE on `(source_profile_id, source_download_id)`
  ([002:38-39](../../../src-tauri/crates/vault-core/src/migrations/002_archive_runtime_foundation.sql)).
- **`search_terms`** — UNIQUE on `(source_profile_id, url_id, normalized_term)`
  ([002:44-45](../../../src-tauri/crates/vault-core/src/migrations/002_archive_runtime_foundation.sql)).
- **`favicons`** — UNIQUE on `(source_profile_id, page_url, icon_url, payload_hash)`
  ([002:49-51](../../../src-tauri/crates/vault-core/src/migrations/002_archive_runtime_foundation.sql)).

`event_fingerprint` = `sha256(json({sourceKind, url, visitTime, title, transition, appId}))`,
where `sourceKind` is **hardcoded to `"chromium-history"`** for every family
([writes.rs:206](../../../src-tauri/crates/vault-core/src/archive/ingest/writes.rs)) and
`visitTime` is converted to Chrome-format (microseconds since 1601) regardless
of source family ([writes.rs:208](../../../src-tauri/crates/vault-core/src/archive/ingest/writes.rs)).
Implementation at [archive/mod.rs:348-365](../../../src-tauri/crates/vault-core/src/archive/mod.rs).

**Architectural invariant**: `source_profile_id` is present in every dedup
key. The schema **cannot** merge two records that come from different
`source_profiles` rows. Cross-browser aggregation must happen at read time
(view layer), not at ingest.

---

## 2. Confirmed Bugs (ranked by likely user impact)

### B1 — URL upsert silently overwrites counts with older data — FIXED

**Fixed in commit 6884c10d.** The URL upsert at
[writes.rs:123-145](../../../src-tauri/crates/vault-core/src/archive/ingest/writes.rs)
now uses:

- `MAX(urls.visit_count, excluded.visit_count)` for `visit_count`
- `MAX(urls.typed_count, excluded.typed_count)` for `typed_count`
- `CASE WHEN excluded.last_visit_ms >= urls.last_visit_ms` for `title` and `hidden`

The same commit also fixed B2 (Firefox long-tail revisit) and B3 (Takeout
path-bound source_visit_id). The C4 scenario
[`c4_chromium_reimport_older_snapshot_regresses_visit_count_demonstrates_b1`](../../src-tauri/crates/vault-core/src/archive/ingest/dedup_scenarios.rs)
is now a plain `#[test]` (no longer `#[should_panic]`) and asserts all four
fields (`visit_count`, `typed_count`, `title`, `hidden`) survive re-import
without regression.

### B2 — Firefox incremental re-import drops long-tail revisits (Safari unaffected) — FIXED

**Fixed in commit 6884c10d** (same commit as B1).

Chromium fixed this via the `OR id IN (SELECT DISTINCT url FROM visits WHERE id > ?2)`
clause at [chromium/mod.rs:74-90](../../../src-tauri/crates/browser-history-parser/src/chromium/mod.rs).
The original audit assumed both Firefox and Safari had the same gap, but the
harness scenarios refined the picture:

- **Firefox** — [firefox/mod.rs:22-33](../../../src-tauri/crates/browser-history-parser/src/firefox/mod.rs):
  `WHERE COALESCE(moz_places.last_visit_date, 0) >= ?1` only. A URL whose
  `last_visit_date` falls before the URL watermark but whose visit id falls
  after the visit watermark gets streamed in the `visits` batch only.
  `ArchiveChunkConsumer::visits()` fails the
  `url_id_map.get(&visit.source_url_id)` lookup
  ([ingest/mod.rs:155-158](../../../src-tauri/crates/vault-core/src/archive/ingest/mod.rs))
  and increments `skipped_visits` silently. The visit is lost forever once
  the next watermark moves past it.
  [`f2_firefox_incremental_revisit_of_old_url_drops_visit_demonstrates_b2`](../../src-tauri/crates/vault-core/src/archive/ingest/dedup_scenarios.rs)
  is `#[should_panic]` until the OR fallback lands.
- **Safari** — turns out NOT to have the bug.
  [safari/mod.rs:42-56](../../../src-tauri/crates/browser-history-parser/src/safari/mod.rs)
  computes `(SELECT MAX(history_visits.visit_time) ...) >= ?1` on the fly
  from the visits table. There is no cached `last_visit_time` column on
  `history_items`, so a new visit row immediately raises the item's
  effective last-visit value and the URL is re-streamed. The
  [`s2_safari_long_tail_revisit_captured_without_or_fallback`](../../src-tauri/crates/vault-core/src/archive/ingest/dedup_scenarios.rs)
  contract scenario pins this; if a future refactor introduces a stored
  cache on `history_items`, the same bug would emerge and this test
  would flip from passing to failing.

The chromium fix exists because it was discovered in real Zhihu-style
long-tail revisit data; the harness now demonstrates Firefox is exposed
to the identical pattern.

### B3 — Takeout `source_visit_id` is bound to file path (degraded defense) — FIXED

**Fixed in commit 6884c10d** (same commit as B1 and B2).

[takeout/browser_history.rs:339](../../../src-tauri/crates/browser-history-parser/src/takeout/browser_history.rs):

```rust
source_visit_id: stable_key_i64(format!("{source_path}:{ordinal}:{url}").as_bytes()),
```

`source_path` is the absolute path to the Takeout JSON file. **Earlier
draft of this audit overstated B3's blast radius** as "renaming the file
produces a full duplicate set"; the harness scenario
[`t2_takeout_rename_file_reimport_dedups_via_fingerprint_partial_index`](../../src-tauri/crates/vault-core/src/archive/ingest/dedup_scenarios.rs)
proved that in the _all-fingerprint-inputs-identical_ case the
`(source_profile_id, event_fingerprint)` partial unique index catches the
duplicates even though every `source_visit_id` changes. So the actual
behaviors are:

- Same file, same path → same hash → primary key dedup → ✅
- Renamed/moved file, **identical record content** → primary key fails to
  dedup, but fingerprint partial index catches it → ✅ in practice
- Renamed/moved file, **fingerprint input drift** (Google captured a new
  page title in the intervening export window, or transition / app_id is
  somehow different) → both indexes miss → ❌ full duplicate set
  ([`t2b_takeout_rename_with_title_change_demonstrates_b3_when_fingerprint_diverges`](../../src-tauri/crates/vault-core/src/archive/ingest/dedup_scenarios.rs)
  reproduces this; the test is `#[should_panic]` until the fix lands)

The design concern stands: the path-bound `source_visit_id` provides
zero useful dedup signal — the system survives only because the
fingerprint partial index is doing double duty. Any change that
narrows the fingerprint inputs (e.g. tightening normalization,
dropping `title` from the hash) would re-expose the user to the full
duplicate set the original B3 claim warned about. Fix shape:
derive `source_visit_id` from `(url, visit_time_micros)` so the
primary key stays stable across re-imports regardless of on-disk path
or downstream fingerprint changes.

### B4 — Takeout × local-Chrome same-period overlap always double-counts

Even with **identical** `(url, visit_time_ms)` pairs, the fingerprint differs
because the inputs differ:

| Field             | Local Chrome           | Takeout                                                                                                                           |
| ----------------- | ---------------------- | --------------------------------------------------------------------------------------------------------------------------------- |
| `app_id`          | real Chrome app id     | hardcoded `"takeout"` ([browser_history.rs:386](../../../src-tauri/crates/browser-history-parser/src/takeout/browser_history.rs)) |
| `transition`      | actual transition int  | `None` ([browser_history.rs:381](../../../src-tauri/crates/browser-history-parser/src/takeout/browser_history.rs))                |
| `from_visit`      | actual from_visit      | `None`                                                                                                                            |
| `source_visit_id` | Chrome visits.id (i64) | path-derived hash                                                                                                                 |

Hash inputs differ → fingerprint differs → both unique indexes pass → two
rows. **Net effect: a user who exports Chrome → Takeout once a month and
also imports their local Chrome will see every visit recorded twice**, even
within the same source_profile.

### B5 — Takeout `stable_key_i64` is collision-prone at scale

[takeout/browser_history.rs:442-445](../../../src-tauri/crates/browser-history-parser/src/takeout/browser_history.rs):

```rust
fn stable_key_i64(bytes: &[u8]) -> i64 {
    let hex = hex::encode(bytes);
    hex.bytes().fold(0_i64, |acc, byte| acc.wrapping_mul(31).wrapping_add(byte as i64)).abs()
}
```

Java-style polynomial hash, folded over hex-encoded bytes, modded by
`abs()`. Theoretical space ≈ 2^63 but the low bits dominate due to
`wrapping_mul(31)` and similar URL prefixes produce similar hash prefixes.
For a 14.4M-record Takeout import (the AGENTS.md design ceiling), birthday
collisions on a degenerate 31-bit-effective hash will hit before
2^15.5 ≈ 47k records.

Collision effects:

- Two distinct URLs map to the same `source_url_id` → the second visit's
  `url_id_map` lookup returns the first URL's canonical id, and its visit
  rows attach to the wrong URL.
- Two distinct visits map to the same `source_visit_id` → second visit
  silently dropped by INSERT OR IGNORE.

### B6 — Takeout time unit ambiguity (potentially silent)

[takeout/browser_history.rs:432-434](../../../src-tauri/crates/browser-history-parser/src/takeout/browser_history.rs):

```rust
fn micros_to_unix_ms(value: i64) -> i64 {
    value.div_euclid(1_000)
}
```

The function name asserts the input is Unix microseconds. Inputs come from:

1. `visitTime` JSON field — provenance unclear; could be either Chrome or Unix.
2. `time_usec` / `timeUsec` — **historically Chrome epoch (microseconds since 1601)** in Google's Takeout dump.
3. `visitedAt` ISO string → `chrono::DateTime::timestamp_micros()` — definitely Unix epoch microseconds.

If the real Takeout files give Chrome-epoch `time_usec`, the resulting
`last_visit_ms` is ~11.6 quadrillion ms in the future. The companion ISO
formatter [chrome_time_to_rfc3339:436](../../../src-tauri/crates/browser-history-parser/src/takeout/browser_history.rs)
calls `DateTime::from_timestamp_micros(value)` which is **Unix-epoch
microseconds**, confirming the code path assumes Unix. Either the runtime
input is in fact Unix (in which case the function names are fine but the
public-facing JSON contract is non-obvious and needs a fixture-pinned
assertion), or the input is Chrome-epoch (in which case all Takeout
timestamps are catastrophically wrong and someone would have noticed). The
audit cannot decide which without a fixture pinned to a real Takeout export
shape — **scenario T-TIME-PIN** in the spec doc resolves this.

---

## 3. Per-Source Behavior Summary

### Chromium (Chrome, Edge, Brave, Vivaldi, Arc, Opera, Opera GX, ChatGPT Atlas, Perplexity Comet, Chromium-proper)

- Time format: microseconds since 1601 → Unix ms via subtract `11_644_473_600_000_000` then `÷ 1000` ([utils.rs:131](../../../src-tauri/crates/vault-core/src/utils.rs)).
- Incremental cursor: `last_visit_id`, `last_url_last_visit_time` (stored as Chrome time).
- URL re-fetch correctness: ✅ has long-tail revisit OR clause ([chromium/mod.rs:85-90](../../../src-tauri/crates/browser-history-parser/src/chromium/mod.rs)).
- Full-import path strips the OR for performance ([chromium/mod.rs:100-103](../../../src-tauri/crates/browser-history-parser/src/chromium/mod.rs)).
- Downloads / search_terms / favicons all supported.

### Firefox (also LibreWolf, Floorp, Waterfox)

- Time format: microseconds since Unix epoch → stored directly as `visit_time_ms` (no conversion — but the field name says `ms`, not `μs`; the actual unit needs fixture verification).
- Incremental cursor: `last_visit_id` (monotonic ✅), `last_url_last_visit_time`.
- URL re-fetch correctness: ❌ **B2** — no long-tail revisit fallback.
- No downloads, no search_terms, no favicons (documented intentional gap per [browser-support-and-adapter-playbook.md:23](../../architecture/browser-support-and-adapter-playbook.md)).

### Safari

- Time format: CFAbsoluteTime (seconds since 2001-01-01 as f64) → Unix ms via `(value - 978_307_200) * 1000` ([safari/mod.rs:59](../../../src-tauri/crates/browser-history-parser/src/safari/mod.rs)).
- URL re-fetch correctness: ❌ **B2** — no long-tail revisit fallback.
- Safari has `synthesized` flag (redirect-generated phantom visits) — currently captured but not de-emphasized in visit_count, may inflate counts vs Chrome's UI numbers.
- No downloads, no search_terms, no favicons.

### Google Takeout

- Goes through a **completely separate ingest path** from Browser Direct ([takeout/mod.rs](../../../src-tauri/crates/browser-history-parser/src/takeout/mod.rs)). The archive `process_profile_snapshot` switch only handles `"chromium" | "firefox" | "safari"` ([ingest/mod.rs:492-493](../../../src-tauri/crates/vault-core/src/archive/ingest/mod.rs)); Takeout-specific Tauri commands wire into different machinery.
- No watermark / cursor support — every re-import replays the whole payload, relying entirely on per-source-profile uniqueness for dedup.
- `source_url_id` = `hash("url::" + url)` — deterministic ✅ from URL alone.
- `source_visit_id` = `hash(path + ordinal + url)` — **B3 path-bound**.
- All Takeout records get `app_id = "takeout"` and `transition = None` → fingerprint can never match local-browser visits.

---

## 4. Areas the Schema Cannot Help With (test-harness must prove behavior)

### URL canonicalization

No URL normalization runs before dedup. From real Chromium exports:

| Surface                                                          | Distinct rows possible?              |
| ---------------------------------------------------------------- | ------------------------------------ |
| `https://example.com` vs `https://example.com/`                  | yes, separate URLs                   |
| `https://Example.com/` vs `https://example.com/`                 | yes if Chrome stored them mixed-case |
| `https://example.com/path` vs `https://example.com/path#section` | yes if Chrome kept fragments         |
| `https://example.com/?a=1&b=2` vs `https://example.com/?b=2&a=1` | yes                                  |
| `https://例子.中国/` vs `https://xn--fsqu00a.xn--fiqs8s/`        | depends on what Chrome wrote         |

The visit_taxonomy/url.rs surface normalizes for search/taxonomy but
**not** for dedup.
[`e6_url_strings_stored_verbatim_no_normalization`](../../src-tauri/crates/vault-core/src/archive/ingest/dedup_scenarios_edge_cases.rs)
pins this contract: trailing slash, fragment, and mixed case are all
stored verbatim as separate URLs.

### Time precision

- Visit times stored at **exact ms** — no fuzzing for "this is probably the
  same visit." Two browsers visiting the same URL within 50ms of each other →
  two rows; same browser firing two navigations at the same ms → second one
  caught by source_visit_id uniqueness ✅.
- DST transitions, system clock changes, and NTP corrections all change
  `visit_time_ms` but not `source_visit_id`, so they're safe at the
  primary index level. Fingerprint fallback would diverge — test required.
- **Sub-millisecond Chrome visit collision (pinned by C_SUB_MS / E5)**: Chrome
  stores visit times at microsecond precision. The ingest pipeline truncates to
  milliseconds (`visit_time_ms`). Two distinct visits to the same URL that land
  within the same millisecond produce **identical fingerprints** (same URL, same
  truncated time, same title, same transition, same app_id). The partial unique
  index on `(source_profile_id, event_fingerprint)` collapses them to one row.
  This is a **known acceptable limitation**: the primary index
  (`source_profile_id, source_visit_id`) still separates them by ID, but
  `INSERT OR IGNORE` stops at the first unique-constraint violation, so the
  fingerprint index fires first and silently drops the second visit.
  [`c_sub_ms_same_millisecond_visits_collapsed_by_fingerprint`](../../src-tauri/crates/vault-core/src/archive/ingest/dedup_scenarios_edge_cases.rs)
  pins this behavior as a contract test.

### Cross-source cannot merge

Already covered in §1. Even the fingerprint partial index is scoped by
`source_profile_id` ([002:30-32](../../../src-tauri/crates/vault-core/src/migrations/002_archive_runtime_foundation.sql)):

```sql
CREATE UNIQUE INDEX IF NOT EXISTS idx_visits_profile_event_fingerprint
  ON visits(source_profile_id, event_fingerprint)
  WHERE event_fingerprint IS NOT NULL AND event_fingerprint != '';
```

### profile_key collisions

`profile_key` = `browser_kind || ':' || profile_name`. Two distinct profiles
with the same name on different paths would collide (e.g. two `Default`
profiles in different OS user accounts on a shared machine). Discovery
should disambiguate via path but is not under audit here.

### Watermark race

[ingest/mod.rs:411-437](../../../src-tauri/crates/vault-core/src/archive/ingest/mod.rs)
saves the watermark inside the same transaction as the canonical writes, so
a crash mid-import rolls everything back together — no torn writes.
However, **concurrent imports of the same profile_id** would both load the
same `last_visit_id` watermark, attempt overlapping writes, and the second
commit would silently re-process records the first already imported. SQLite
prevents simultaneous write transactions on the same DB, but the in-app
queue serialization is not under audit here — flag for harness coverage.

### Visit→URL ordering dependency

[ingest/mod.rs:155-158](../../../src-tauri/crates/vault-core/src/archive/ingest/mod.rs)
silently drops any visit whose `source_url_id` is not already in
`url_id_map`. The parser is expected to emit `urls()` batches before
`visits()` batches for the same URL. Any future refactor that changes
batching order will cause silent data loss — must be pinned by test.

---

## 5. What the Test Harness Must Prove

Maps to scenarios that will be enumerated in
`import-test-harness-spec.md`. Listed here only at the assertion level:

1. **Within one source_profile, no visit is ever stored twice across re-imports**, regardless of which fixture features collide:
   - re-import same file
   - re-import after appending new rows
   - re-import after schema migration in the source DB
   - re-import where some old URLs got revisited but no new URLs added
2. **Cross-source-profile keeps independent rows** (the by-design contract); test must encode this so a future refactor that "tidies it up" gets caught.
3. **No visit is silently dropped**:
   - parser emits visit before URL → must be caught
   - URL last_visit older than watermark but visit newer → must be caught
   - corrupt source DB → revert leaves vault unchanged
4. **B1 / B2 / B3 / B4 / B5 / B6 each have a failing test before the fix lands.**
5. **Time conversions round-trip**:
   - Chromium ms → Chrome time → fingerprint → re-parse same row → same fingerprint
   - Firefox `visit_date` (μs Unix) → ms Unix → ISO → same
   - Safari CFAbsoluteTime → ms Unix → ISO → same
   - Takeout `time_usec` shape pinned by fixture
6. **URL canonicalization contract pinned** — every variant in §4 has a test that documents the _current_ behavior. Changes to URL normalization later require updating the tests, making the change visible in review.
7. **Provenance preserved**:
   - Edge profile imports stay tagged Edge, not collapsed to Chrome (per [browser-support-and-adapter-playbook.md:107](../../architecture/browser-support-and-adapter-playbook.md))
   - ChatGPT Atlas / Perplexity Comet keep their product identity
8. **Memory bounds**: streaming chunks of 10,000 records ([ingest/mod.rs:61](../../../src-tauri/crates/vault-core/src/archive/ingest/mod.rs)) actually limit RAM. A 1.44M-record fixture must import without RSS exceeding a bounded ceiling (the harness target the user gave: 8 GB / 4 core).

---

## 6. Scenarios Now Backed By Tests

> Living section — updated as scenarios land. The expectation is that every
> bug from §2 eventually has a named `#[should_panic]` regression test that
> flips to a plain `#[test]` once the fix ships, and every architectural
> contract from §5 has a contract test that defends it against drift.

### Contract scenarios (pass today, guard against regression)

| Scenario                                           | Location                                                                                                                                                                                 | Asserts                                                                                                                                                                                                                             |
| -------------------------------------------------- | ---------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | ----------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| C1 — Chromium baseline import                      | [`c1_chromium_baseline_import`](../../src-tauri/crates/vault-core/src/archive/ingest/dedup_scenarios.rs)                                                                                 | One profile, one ingest pass produces exactly the fixture URL + visit rows; `source_visit_id` values flow through unmodified.                                                                                                       |
| C2 — Chromium incremental no-new-data              | [`c2_chromium_incremental_no_new_data`](../../src-tauri/crates/vault-core/src/archive/ingest/dedup_scenarios.rs)                                                                         | Re-running the same fixture with `use_watermark = true` returns `new_urls = 0`, `new_visits = 0`, and archive row counts stay constant.                                                                                             |
| C3 — Chromium incremental revisit of an old URL    | [`c3_chromium_incremental_revisit_of_old_url`](../../src-tauri/crates/vault-core/src/archive/ingest/dedup_scenarios.rs)                                                                  | Adversarial pass-2 fixture: visit cursor moves past 10, URL `last_visit_time` deliberately left at the old value. Validates the `OR id IN (SELECT DISTINCT url FROM visits WHERE id > ?2)` fallback in `INGEST_URLS_SQL` is intact. |
| S2 — Safari long-tail revisit (NOT affected by B2) | [`s2_safari_long_tail_revisit_captured_without_or_fallback`](../../src-tauri/crates/vault-core/src/archive/ingest/dedup_scenarios_baselines.rs)                                          | Safari's URL query computes MAX(visit_time) on the fly; no cached `last_visit_time` column to lag behind, so the OR fallback isn't needed. Test flips if a future refactor adds a cache.                                            |
| T1 — Takeout baseline import                       | [`t1_takeout_baseline_import`](../../src-tauri/crates/vault-core/src/archive/ingest/dedup_scenarios_takeout.rs)                                                                          | `crate::takeout::import_takeout` ingests a synthetic `BrowserHistory.json` into `profile_key = "takeout::browser-history"` with `app_id = "takeout"` on every visit.                                                                |
| T2 — Takeout file rename, identical records        | [`t2_takeout_rename_file_reimport_dedups_via_fingerprint_partial_index`](../../src-tauri/crates/vault-core/src/archive/ingest/dedup_scenarios_takeout.rs)                                | Refutes the original B3 framing: the fingerprint partial unique index catches the duplicate set even though every `source_visit_id` differs.                                                                                        |
| T3 — Takeout × local Chrome same-period            | [`t3_takeout_and_local_chrome_same_period_b4_contract`](../../src-tauri/crates/vault-core/src/archive/ingest/dedup_scenarios_takeout.rs)                                                 | B4 contract: per-source-profile dedup truly keeps Chrome and Takeout independent; fingerprint inputs differ (real app_id vs `"takeout"`, real transition vs `None`) so any future cross-source dedup must normalize first.          |
| T5 — Takeout time_usec interpretation              | [`t5_takeout_time_usec_pinned_as_unix_microseconds_b6_contract`](../../src-tauri/crates/vault-core/src/archive/ingest/dedup_scenarios_takeout.rs)                                        | B6 contract: parser interprets `time_usec` as Unix-epoch microseconds. If real Google Takeout disagrees the writer + this test update together; if anyone changes the parser to Chrome epoch this test fails immediately.           |
| X1 — Edge imports Chrome history then diverges     | [`x1_edge_imports_chrome_then_both_diverge`](../../src-tauri/crates/vault-core/src/archive/ingest/dedup_scenarios.rs)                                                                    | Per-source-profile architecture preserved: a URL visited in both browsers keeps two `urls` rows; Edge's `browser_product` stays `"Microsoft Edge"` (playbook §107).                                                                 |
| X2 — Atlas / Comet preserve browser_product        | [`x2_chromium_family_products_preserve_browser_product_identity`](../../src-tauri/crates/vault-core/src/archive/ingest/dedup_scenarios.rs)                                               | ChatGPT Atlas (playbook §156) and Perplexity Comet (playbook §158) stay tagged with their product identity in `source_profiles.browser_product`; do not collapse to "Google Chrome".                                                |
| X3 — Multi-profile per browser independence        | [`x3_multiple_profiles_within_same_browser_stay_independent`](../../src-tauri/crates/vault-core/src/archive/ingest/dedup_scenarios.rs)                                                   | Chrome `Default` and Chrome `Profile 1` produce distinct `source_profiles` rows under same `browser_kind`; identical visits across them do NOT dedup (per-profile fingerprint scope); per-profile watermark isolation preserved. |
| C5 — Chromium incremental append-new-rows          | [`c5_chromium_incremental_append_new_urls_and_visits`](../../src-tauri/crates/vault-core/src/archive/ingest/dedup_scenarios.rs)                                                          | Re-import where second pass adds wholly new URLs + new visits (no overlap with first import) — watermark lets only new rows land while originals stay deduplicated. Pins §5.1 "re-import after appending new rows" contract.        |
| C6 — Chromium source DB schema tolerance           | [`c6_chromium_extra_columns_on_source_db_do_not_break_ingest`](../../src-tauri/crates/vault-core/src/archive/ingest/dedup_scenarios.rs)                                                  | Fixture DB with `ALTER TABLE`-added columns (`favicon_id`, `segment_id`, `opener_visit`, `originator_cache_guid`) imports without error and produces identical canonical rows. Pins §5.1 "re-import after schema migration" contract; catches accidental `SELECT *` regressions. |
| F1 — Firefox baseline import                       | [`f1_firefox_baseline_import`](../../src-tauri/crates/vault-core/src/archive/ingest/dedup_scenarios_baselines.rs)                                                                        | Firefox single-import happy path: 3 URLs, 5 visits all land with correct counts, timestamps, and field values.                                                                                                                      |
| S1 — Safari baseline import                        | [`s1_safari_baseline_import`](../../src-tauri/crates/vault-core/src/archive/ingest/dedup_scenarios_baselines.rs)                                                                         | Safari single-import happy path: 3 URLs, 5 visits all land with correct counts, timestamps, and field values.                                                                                                                       |
| Chromium fingerprint dedup                         | [`chromium_fingerprint_dedup_catches_same_visits_with_different_source_ids`](../../src-tauri/crates/vault-core/src/archive/ingest/dedup_scenarios_baselines.rs)                          | Re-import same visits with different `source_visit_id` values — the `event_fingerprint` partial index catches them as duplicates, no extra rows created.                                                                            |
| F_C2 — Firefox incremental no-new-data             | [`f_c2_firefox_incremental_no_new_data`](../../src-tauri/crates/vault-core/src/archive/ingest/dedup_scenarios_baselines.rs)                                                              | Firefox mirror of C2: re-import with watermark produces `new_urls = 0`, `new_visits = 0`, archive row counts constant.                                                                                                              |
| S_C2 — Safari incremental no-new-data              | [`s_c2_safari_incremental_no_new_data`](../../src-tauri/crates/vault-core/src/archive/ingest/dedup_scenarios_baselines.rs)                                                               | Safari mirror of C2: re-import with watermark produces `new_urls = 0`, `new_visits = 0`, archive row counts constant.                                                                                                               |
| C_SUB_MS (E5) — Sub-ms fingerprint collision       | [`c_sub_ms_same_millisecond_visits_collapsed_by_fingerprint`](../../src-tauri/crates/vault-core/src/archive/ingest/dedup_scenarios_edge_cases.rs)                                        | Two visits to same URL at same ms but different source_visit_ids — fingerprint partial index collapses to 1 row. Pins known precision limitation.                                                                                   |
| E6 — URL canonicalization (no normalization)       | [`e6_url_strings_stored_verbatim_no_normalization`](../../src-tauri/crates/vault-core/src/archive/ingest/dedup_scenarios_edge_cases.rs)                                                  | Trailing slash, fragment, mixed case all stored as separate URLs verbatim. Pins contract so future normalization changes are visible.                                                                                               |
| Empty DB × 3 families                              | `empty_{chromium,firefox,safari}_fixture_imports_without_error` in [`dedup_scenarios_edge_cases.rs`](../../src-tauri/crates/vault-core/src/archive/ingest/dedup_scenarios_edge_cases.rs) | Zero-row fixtures for each family import without error, summary reports 0/0.                                                                                                                                                        |
| R1a — Corrupt random bytes                         | [`r1a_corrupt_random_bytes_returns_error_not_panic`](../../src-tauri/crates/vault-core/src/archive/ingest/dedup_scenarios_edge_cases.rs)                                                 | Random bytes file returns `Err`, not panic — resilience contract.                                                                                                                                                                   |
| R1b — Valid SQLite missing tables                  | [`r1b_valid_sqlite_missing_tables_returns_error_not_panic`](../../src-tauri/crates/vault-core/src/archive/ingest/dedup_scenarios_edge_cases.rs)                                          | Valid SQLite DB without browser tables returns `Err`, not panic — resilience contract.                                                                                                                                              |
| E1 — Epoch timestamp (visit_time_ms = 0)          | [`e1_epoch_timestamp_imports_without_error`](../../src-tauri/crates/vault-core/src/archive/ingest/dedup_scenarios_edge_cases.rs)                                                         | Epoch 0 timestamp stores and round-trips as 0 — pins lower bound of time domain.                                                                                                                                                    |
| E2 — Year-2038 boundary (2^31 seconds)            | [`e2_year_2038_boundary_imports_without_error`](../../src-tauri/crates/vault-core/src/archive/ingest/dedup_scenarios_edge_cases.rs)                                                      | 2038-01-19T03:14:07Z (2,147,483,647,000 ms) round-trips correctly — pins i64 handling above 32-bit overflow.                                                                                                                        |
| E3 — Far-future timestamp (year 9999)             | [`e3_far_future_timestamp_imports_without_error`](../../src-tauri/crates/vault-core/src/archive/ingest/dedup_scenarios_edge_cases.rs)                                                    | Max-range timestamp stores without overflow — pins i64 capacity at the upper extreme.                                                                                                                                                |
| E4 — Negative timestamp (clamped to 0)            | [`e4_negative_timestamp_clamped_to_zero`](../../src-tauri/crates/vault-core/src/archive/ingest/dedup_scenarios_edge_cases.rs)                                                            | All parsers apply `.max(0)` so negative source timestamps import as 0 ms — pins clamping contract.                                                                                                                                  |
| E7 — NULL title handling                          | [`e7_null_title_imports_with_null_archive_title`](../../src-tauri/crates/vault-core/src/archive/ingest/dedup_scenarios_edge_cases.rs)                                                    | URL with NULL source title projects as NULL in archive (not empty string) — pins nullable-column contract. Sibling URL with non-NULL title round-trips normally.                                                                    |
| E8 — Unicode byte-identical round-trip            | [`e8_unicode_urls_and_titles_round_trip_byte_identical`](../../src-tauri/crates/vault-core/src/archive/ingest/dedup_scenarios_edge_cases.rs)                                             | CJK title, percent-encoded path (NOT decoded), and emoji + em-dash all round-trip byte-identical with no NFC/NFD normalization or case folding. Pins international-user contract.                                                    |
| Takeout ptoken evidence round-trip                 | [`takeout_standard_json_round_trips_through_production_parser`](../../src-tauri/crates/browser-history-fixtures/tests/takeout_roundtrip.rs) (ptoken assertion block)                     | `ptoken` field in fixture serializes and parses back as `context.takeout.ptoken` context evidence.                                                                                                                                   |
| Takeout visitedAt ISO-8601 fallback                | [`takeout_visited_at_iso_string_parsed_correctly`](../../src-tauri/crates/browser-history-fixtures/tests/takeout_roundtrip.rs)                                                           | Hand-crafted JSON with `visitedAt` RFC-3339 strings parses to correct millisecond timestamps — covers the parser's ISO fallback path that no fixture writer can exercise.                                                            |
| Takeout missing time field silently skipped        | [`takeout_record_without_time_field_is_skipped`](../../src-tauri/crates/browser-history-fixtures/tests/takeout_roundtrip.rs)                                                             | A record without any time field (`visitTime`, `time_usec`, `timeUsec`, `visitedAt`) is silently dropped; only time-bearing records produce URL + visit rows.                                                                         |

### Bugs with failing tests

| Bug                                                                     | Scenario                                                                                                                                                    | Status                                                                                                                                                                                    |
| ----------------------------------------------------------------------- | ----------------------------------------------------------------------------------------------------------------------------------------------------------- | ----------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| B1 URL upsert regresses counts                                          | [`c4_chromium_reimport_older_snapshot_regresses_visit_count_demonstrates_b1`](../../src-tauri/crates/vault-core/src/archive/ingest/dedup_scenarios.rs)      | **FIXED** (6884c10d) — now a plain `#[test]` asserting `visit_count`, `typed_count`, `title`, and `hidden` all survive re-import without regression                                       |
| B2 Firefox long-tail revisit drop                                       | [`f2_firefox_incremental_revisit_of_old_url_drops_visit_demonstrates_b2`](../../src-tauri/crates/vault-core/src/archive/ingest/dedup_scenarios_baselines.rs) | **FIXED** (6884c10d) — Firefox URL stream now has the OR fallback                                                                                                                        |
| B2 Safari long-tail revisit drop                                        | n/a — refuted                                                                                                                                                | Original audit claim corrected. Safari has no cached last-visit column to lag; see [`s2_...`](../../src-tauri/crates/vault-core/src/archive/ingest/dedup_scenarios_baselines.rs) contract. |
| B3 Takeout path-bound source_visit_id (narrow case — fingerprint drift) | [`t2b_takeout_rename_with_title_change_demonstrates_b3_when_fingerprint_diverges`](../../src-tauri/crates/vault-core/src/archive/ingest/dedup_scenarios_takeout.rs) | **FIXED** (6884c10d) — fix landed in same commit as B1 and B2                                                                                                                      |
| B4 Takeout × local Chrome double-count                                  | [`t3_takeout_and_local_chrome_same_period_b4_contract`](../../src-tauri/crates/vault-core/src/archive/ingest/dedup_scenarios_takeout.rs)                     | Contract test — by-design per-profile storage; reframed from "bug" to "design constraint for any future cross-source dedup proposal"                                                     |
| B5 Takeout hash collision at scale                                      | T4 (deferred to a dedicated scale-test slice)                                                                                                                | needs million-record fixture infrastructure separate from per-scenario harness                                                                                                            |
| B6 Takeout time unit ambiguity                                          | [`t5_takeout_time_usec_pinned_as_unix_microseconds_b6_contract`](../../src-tauri/crates/vault-core/src/archive/ingest/dedup_scenarios_takeout.rs)            | Contract test pins current Unix-microseconds interpretation; the audit's "what does Google really ship" question stays open until a real-world sample lands                               |

---

## 7. Out of Scope For This Audit

- **View-layer cross-browser aggregation** — separate user-flow work, decided
  in the planning conversation but not yet a BACKLOG block.
- **`vault-platform` staging and live-file copy** — concerns file system
  semantics, not dedup correctness.
- **Recall / search projection** — derived from the canonical archive after
  ingest commits; will inherit ingest's truth.
- **Backup vs Browser Direct command-surface differences** — the canonical
  ingest path is the same; differences are in staging and source provenance
  metadata, both of which are validated by separate acceptance tests in the
  m3/m4 milestones.

---

_End of audit. The companion spec doc
(`docs/plan/program/import-test-harness-spec.md`) translates the above bugs
and gaps into concrete scenarios, fixture generator API, and acceptance
criteria for `WORK-IMPORT-TEST-HARNESS-A`. Section 6 above tracks which
scenarios have shipped against the harness._
