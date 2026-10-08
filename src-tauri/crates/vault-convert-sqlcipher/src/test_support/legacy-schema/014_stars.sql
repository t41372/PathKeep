-- star table: user-authored favorites ("加星") keyed by canonical entity.
--
-- A star marks a page (canonical_url) or a source (registrable_domain) as a
-- favorite. Stars are user-authored content keyed by the CANONICAL entity, NOT
-- by visit id — so a star written on one visit survives re-import, dedup, and
-- profile changes, and rides the portable `.pathkeep-bundle` export with the
-- rest of the archive. "Survives re-import" holds BY DESIGN: the canonical key
-- is stable across raw-URL variants (tracking params + host casing collapse via
-- normalize_visit_url), so the same page re-imported under a different raw URL
-- still resolves to the same star.
--
-- Keying difference vs. annotations (migration 011): notes/tags key by the RAW
-- url; stars key by canonical_url. This is deliberate — a star is page identity
-- (collapse the variants), a note is about the exact url (keep them distinct).
--
-- source_profile is captured at write time for audit only and is deliberately
-- NOT part of the primary key: stars are user content and must not be
-- partitioned by the browser they happened to be created from.
--
-- entity_kind is 'url' | 'domain' for the MVP (query_family stars are
-- deferred). The table is tiny by design — users star hundreds of things, not
-- millions — so every read is a primary-key lookup or an index range scan and
-- the surface stays cheap even when the visit archive holds 14.4M rows.

CREATE TABLE IF NOT EXISTS star (
  entity_kind    TEXT NOT NULL,
  entity_key     TEXT NOT NULL,
  starred_at     TEXT NOT NULL,
  source_profile TEXT,
  PRIMARY KEY (entity_kind, entity_key)
);

-- Recently-starred ordering and per-kind listing both ride this index, so the
-- Starred hub never scans the whole table.
CREATE INDEX IF NOT EXISTS idx_star_kind_starred_at
  ON star(entity_kind, starred_at DESC);

-- The Starred hub enriches each (tiny) starred URL set with its visit count
-- and title by matching the canonical star key back to `urls`. Because
-- `urls.url` is the RAW url, enrichment first tries an exact seek on the
-- canonical key, then a prefix RANGE SEEK (scheme://host/path) that
-- canonicalizes only the candidate raw variants in Rust. Both passes ride
-- `idx_urls_url`: the exact pass as an equality probe (`url = ?`), the prefix
-- pass as an explicit byte-range (`url >= :prefix AND url < :prefix_upper`).
-- NOTE — the prefix pass deliberately uses a byte-range, NOT `LIKE 'prefix%'`:
-- this DB runs with the default `case_sensitive_like = OFF`, so a `LIKE` range
-- cannot use the BINARY `idx_urls_url` and EXPLAIN QUERY PLAN shows a full
-- `SCAN urls` (Cluster 2a / H-2). The byte-range plans as
-- `SEARCH urls USING INDEX idx_urls_url`. Without the index the planner does a
-- full `SCAN urls` per lookup — unusable on the 14.4M-row target. og:image
-- prefetch already relies on the equivalent `idx_og_images_page_url`; this
-- mirrors that pattern for the canonical `urls` table and is reused by the
-- deferred AI working-set selector. Domain-star resolution does NOT use this
-- index (a host match needs a leading wildcard); it seeks the persisted
-- `urls.registrable_domain` column added by migration 015 instead.
CREATE INDEX IF NOT EXISTS idx_urls_url ON urls(url);
