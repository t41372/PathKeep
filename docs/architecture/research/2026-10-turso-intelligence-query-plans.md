# Intelligence query plans on Turso 0.8.1

Native plan tests found three query shapes whose work grew with unrelated archive rows.
These are local reproductions and application workarounds. No dependency source was changed.

## Assignment joins

A published-generation view originally joined assignments using both `visit_id` and
`generation`. Turso selected `idx_assignments_generation_trail (generation=?)`, visiting
the generation's assignments for each fact. The generated-archive equivalence test made
this visible even with 11,313 visits.

Minimal shape:

```sql
SELECT r.visit_id, a.session_id
FROM visit_derived_facts_rows r
LEFT JOIN intelligence_generations g ON g.profile_id = r.profile_id
LEFT JOIN visit_assignments a
  ON a.visit_id = r.visit_id AND a.generation = COALESCE(g.navigation, 0)
WHERE r.generation = COALESCE(g.facts, 0);
```

The application now selects published generations with a small `IN` subquery and uses
an explicit `(visit_id, generation)` assignment index. The native plan must seek
`a (visit_id=?)`. The regression is `intelligence_generation_plans.rs`.

## Correlated counts

The optimizer's `translate/optimizer/unnest.rs` rewrites eligible scalar aggregates into
a grouped inner relation. For a domain detail, this changed an indexed count of that
domain's visits into a scan of the entire visible-visit index:

```sql
SELECT u.url,
  (SELECT COUNT(*) FROM visits v
   WHERE v.url_id = u.id AND v.reverted_at IS NULL
     AND v.visit_time_ms >= ?2 AND v.visit_time_ms < ?3)
FROM pages p CROSS JOIN urls u ON u.page_id = p.id
WHERE p.registrable_domain = ?1;
```

The observed plan contained a materialized scalar-subquery relation and
`SCAN visits USING COVERING INDEX idx_visits_visible_url_time`. The rewrite groups keys
that the outer domain query never requests. This is valid SQL execution but unsuitable
for this render path.

`LIMIT 1 OFFSET 0` on the scalar aggregate preserves its single-row result and excludes
it from that rewrite. Domain counts and working-set scalar signals use this shape;
last-visit reads use an ordered index seek. Tests capture the actual application SQL
and bindings on its Store reader, then reject full canonical scans. The capture facility
is disabled by default.

## Batched IN lookups

A secondary-index `IN` lookup did not become a point seek in the actual derived-row batch.
Even `INDEXED BY` forced an index scan rather than constraining all requested visit IDs.
The full-scale diagnostic reached fact completion, then spent its daily phase repeatedly
walking a generation for small batches. This was reproduced with plan capture on the real
rebuild path, not a separately invented query.

Minimal contrast (with `facts(visit_id, generation)` indexed in that order):

```sql
SELECT * FROM facts INDEXED BY fact_visit_generation
WHERE generation = ?1 AND visit_id IN (?2, ?3);

WITH requested(visit_id) AS (VALUES (?2), (?3))
SELECT f.* FROM requested
CROSS JOIN facts f INDEXED BY fact_visit_generation
  ON f.visit_id = requested.visit_id
WHERE f.generation = ?1;
```

The second form drives one equality lookup per requested ID. The captured shipping plan
contains `SCAN requested` followed by `SEARCH f ... (visit_id=? AND generation=?)` and the
same two-column assignment seek. It replaces the IN shape in derived facts, source search
terms, embedding-hash hydration and SourceEvidence lookups. `intelligence_generation_plans`
keeps the actual rebuild-plan regression; sparse evidence and metadata tests cover the
other callers. Dependency code is unchanged.
