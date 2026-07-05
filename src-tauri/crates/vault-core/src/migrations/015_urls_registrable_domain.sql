-- Persisted registrable-domain column on `urls`, for an INDEX-SEEK domain-star
-- resolution (Cluster 2a / H-2).
--
-- WHY: `star` resolves a DOMAIN favorite to every page on that source. The
-- registrable domain (`example.com`, `bbc.co.uk`) cannot be derived in pure SQL
-- (it needs the public-suffix list), and a host-anchored `LIKE` on `urls.url`
-- has a LEADING wildcard, so the planner CANNOT use `idx_urls_url` for it — it
-- does a full `SCAN urls` once per starred domain, the O(corpus) cost the 14.4M
-- envelope forbids. A persisted column computed by `registrable_domain_for_url`
-- (the SAME function `StarredMatcher::is_starred` uses) turns the domain pass
-- into `WHERE registrable_domain = :domain` — a true index SEARCH on the partial
-- index below, EXACTLY equivalent to the per-visit matcher (no LIKE over/under-
-- recall to re-check in Rust).
--
-- NULLABLE by design so the ALTER needs no default backfill at migration time:
-- existing rows are filled by the one-time Rust backfill in `create_schema`
-- (`backfill_url_registrable_domains`, bounded, idempotent — only touches NULL
-- rows), and the canonical ingest/import writers compute the value on insert so
-- steady-state rows are never NULL. A row the archive has not classified yet
-- (NULL) is simply absent from the partial index — harmless, since it is also
-- not on any starred domain until the backfill fills it.
ALTER TABLE urls ADD COLUMN registrable_domain TEXT;

-- Partial index so the domain-star seek is an index SEARCH (not a SCAN urls) and
-- the index stays small: only classified rows participate, and the common NULL
-- (pre-backfill / unclassifiable) rows cost no index space.
CREATE INDEX IF NOT EXISTS idx_urls_registrable_domain
  ON urls(registrable_domain)
  WHERE registrable_domain IS NOT NULL;
