//! Core Intelligence schema SQL and legacy table registry.
//!
//! ## Responsibilities
//! - Hold the raw SQLite DDL used to bootstrap the rebuildable intelligence
//!   plane.
//! - Keep migration bootstrap SQL and legacy insight-table names in one owner
//!   so schema logic does not become a giant mixed file again.
//!
//! ## Not responsible for
//! - Executing migrations or opening database connections.
//! - Reporting readiness or clearing derived state.
//! - Serving route-level intelligence reads.
//!
//! ## Dependencies
//! - Consumed by `intelligence_schema.rs`.
//!
//! ## Performance notes
//! - Static constants only; no runtime work happens in this module.

pub(super) const CORE_INTELLIGENCE_SCHEMA_SQL: &str = r#"
CREATE TABLE IF NOT EXISTS visit_derived_facts (
  visit_id           INTEGER PRIMARY KEY,
  profile_id         TEXT NOT NULL,
  session_id         TEXT,
  trail_id           TEXT,
  registrable_domain TEXT NOT NULL,
  canonical_url      TEXT NOT NULL,
  domain_category    TEXT NOT NULL DEFAULT 'unknown',
  page_category      TEXT NOT NULL DEFAULT 'unknown',
  search_engine      TEXT,
  search_query       TEXT,
  is_new_domain      INTEGER NOT NULL DEFAULT 0,
  is_search_event    INTEGER NOT NULL DEFAULT 0,
  evidence_tier      TEXT NOT NULL DEFAULT 'tier-c',
  taxonomy_source    TEXT NOT NULL DEFAULT 'unknown',
  taxonomy_pack      TEXT,
  taxonomy_version   TEXT,
  computed_at        TEXT NOT NULL
);
CREATE INDEX IF NOT EXISTS idx_vdf_profile_session ON visit_derived_facts(profile_id, session_id);
CREATE INDEX IF NOT EXISTS idx_vdf_profile_trail ON visit_derived_facts(profile_id, trail_id);
CREATE INDEX IF NOT EXISTS idx_vdf_profile_domain ON visit_derived_facts(profile_id, registrable_domain);
CREATE INDEX IF NOT EXISTS idx_vdf_profile_search ON visit_derived_facts(profile_id, is_search_event, search_engine);
CREATE INDEX IF NOT EXISTS idx_vdf_profile_visit_id ON visit_derived_facts(profile_id, visit_id);

CREATE TABLE IF NOT EXISTS domain_daily_rollups (
  date_key           TEXT NOT NULL,
  profile_id         TEXT NOT NULL,
  registrable_domain TEXT NOT NULL,
  domain_category    TEXT NOT NULL,
  visit_count        INTEGER NOT NULL,
  search_count       INTEGER NOT NULL,
  new_domain_visits  INTEGER NOT NULL,
  unique_urls        INTEGER NOT NULL,
  PRIMARY KEY(date_key, profile_id, registrable_domain)
);
CREATE TABLE IF NOT EXISTS category_daily_rollups (
  date_key           TEXT NOT NULL,
  profile_id         TEXT NOT NULL,
  domain_category    TEXT NOT NULL,
  visit_count        INTEGER NOT NULL,
  unique_domains     INTEGER NOT NULL,
  PRIMARY KEY(date_key, profile_id, domain_category)
);
CREATE TABLE IF NOT EXISTS engine_daily_rollups (
  date_key           TEXT NOT NULL,
  profile_id         TEXT NOT NULL,
  search_engine      TEXT NOT NULL,
  search_count       INTEGER NOT NULL,
  PRIMARY KEY(date_key, profile_id, search_engine)
);
CREATE TABLE IF NOT EXISTS daily_summary_rollups (
  date_key           TEXT NOT NULL,
  profile_id         TEXT NOT NULL,
  total_visits       INTEGER NOT NULL,
  total_searches     INTEGER NOT NULL,
  new_domains        INTEGER NOT NULL,
  unique_domains     INTEGER NOT NULL,
  hhi_score          REAL,
  discovery_rate     REAL,
  PRIMARY KEY(date_key, profile_id)
);

CREATE TABLE IF NOT EXISTS sessions (
  session_id         TEXT PRIMARY KEY,
  profile_id         TEXT NOT NULL,
  first_visit_ms     INTEGER NOT NULL,
  last_visit_ms      INTEGER NOT NULL,
  visit_count        INTEGER NOT NULL,
  search_count       INTEGER NOT NULL,
  domain_count       INTEGER NOT NULL,
  is_deep_dive       INTEGER NOT NULL DEFAULT 0,
  auto_title         TEXT,
  computed_at        TEXT NOT NULL
);
CREATE INDEX IF NOT EXISTS idx_sessions_profile_time ON sessions(profile_id, first_visit_ms DESC);

CREATE TABLE IF NOT EXISTS search_trails (
  trail_id            TEXT PRIMARY KEY,
  profile_id          TEXT NOT NULL,
  session_id          TEXT,
  initial_query       TEXT NOT NULL,
  search_engine       TEXT NOT NULL,
  reformulation_count INTEGER NOT NULL DEFAULT 0,
  visit_count         INTEGER NOT NULL,
  landing_url         TEXT,
  landing_domain      TEXT,
  first_visit_ms      INTEGER NOT NULL,
  last_visit_ms       INTEGER NOT NULL,
  max_depth           INTEGER NOT NULL DEFAULT 0,
  queries_json        TEXT NOT NULL,
  computed_at         TEXT NOT NULL
);
CREATE INDEX IF NOT EXISTS idx_search_trails_profile_time ON search_trails(profile_id, first_visit_ms DESC);
CREATE INDEX IF NOT EXISTS idx_search_trails_profile_time_trail ON search_trails(profile_id, first_visit_ms ASC, trail_id ASC);

CREATE TABLE IF NOT EXISTS search_trail_members (
  trail_id           TEXT NOT NULL,
  profile_id         TEXT NOT NULL,
  visit_id           INTEGER NOT NULL,
  ordinal            INTEGER NOT NULL,
  role               TEXT NOT NULL,
  PRIMARY KEY(trail_id, visit_id)
);
CREATE INDEX IF NOT EXISTS idx_search_trail_members_profile_visit ON search_trail_members(profile_id, visit_id);

CREATE TABLE IF NOT EXISTS search_events (
  visit_id           INTEGER PRIMARY KEY,
  profile_id         TEXT NOT NULL,
  search_engine      TEXT NOT NULL,
  raw_query          TEXT NOT NULL,
  normalized_query   TEXT NOT NULL,
  query_kind         TEXT NOT NULL DEFAULT 'keyword',
  trail_id           TEXT,
  computed_at        TEXT NOT NULL
);
CREATE INDEX IF NOT EXISTS idx_search_events_profile_engine ON search_events(profile_id, search_engine);
CREATE INDEX IF NOT EXISTS idx_search_events_profile_visit ON search_events(profile_id, visit_id);

CREATE TABLE IF NOT EXISTS search_event_terms (
  visit_id           INTEGER NOT NULL,
  profile_id         TEXT NOT NULL,
  term               TEXT NOT NULL,
  PRIMARY KEY(visit_id, term)
);
CREATE INDEX IF NOT EXISTS idx_search_event_terms_profile_term ON search_event_terms(profile_id, term);

CREATE TABLE IF NOT EXISTS query_families (
  family_id          TEXT PRIMARY KEY,
  profile_id         TEXT NOT NULL,
  anchor_query       TEXT NOT NULL,
  member_count       INTEGER NOT NULL,
  search_engine      TEXT NOT NULL,
  first_seen_ms      INTEGER NOT NULL,
  last_seen_ms       INTEGER NOT NULL,
  queries_json       TEXT NOT NULL,
  computed_at        TEXT NOT NULL
);
CREATE INDEX IF NOT EXISTS idx_query_families_profile_time ON query_families(profile_id, last_seen_ms DESC);

CREATE TABLE IF NOT EXISTS refind_pages (
  profile_id            TEXT NOT NULL,
  canonical_url         TEXT NOT NULL,
  url                   TEXT NOT NULL,
  title                 TEXT,
  registrable_domain    TEXT NOT NULL,
  cross_day_count       INTEGER NOT NULL,
  trail_count           INTEGER NOT NULL,
  search_arrival_count  INTEGER NOT NULL,
  typed_revisit_count   INTEGER NOT NULL,
  refind_score          REAL NOT NULL,
  evidence_json         TEXT NOT NULL,
  first_seen_ms         INTEGER NOT NULL,
  last_seen_ms          INTEGER NOT NULL,
  computed_at           TEXT NOT NULL,
  PRIMARY KEY(profile_id, canonical_url)
);
CREATE INDEX IF NOT EXISTS idx_refind_pages_profile_score ON refind_pages(profile_id, refind_score DESC, last_seen_ms DESC);

CREATE TABLE IF NOT EXISTS source_effectiveness (
  profile_id             TEXT NOT NULL,
  registrable_domain     TEXT NOT NULL,
  source_role            TEXT NOT NULL,
  trail_count            INTEGER NOT NULL,
  stable_landing_count   INTEGER NOT NULL,
  effectiveness_score    REAL NOT NULL,
  evidence_json          TEXT NOT NULL,
  first_seen_ms          INTEGER NOT NULL,
  last_seen_ms           INTEGER NOT NULL,
  computed_at            TEXT NOT NULL,
  PRIMARY KEY(profile_id, registrable_domain)
);
CREATE INDEX IF NOT EXISTS idx_source_effectiveness_profile_score ON source_effectiveness(profile_id, effectiveness_score DESC);

CREATE TABLE IF NOT EXISTS habit_patterns (
  profile_id           TEXT NOT NULL,
  registrable_domain   TEXT NOT NULL,
  habit_type           TEXT NOT NULL,
  mean_interval_days   REAL NOT NULL,
  cv                   REAL NOT NULL,
  visit_count          INTEGER NOT NULL,
  last_visited_ms      INTEGER NOT NULL,
  is_interrupted       INTEGER NOT NULL DEFAULT 0,
  computed_at          TEXT NOT NULL,
  PRIMARY KEY(profile_id, registrable_domain)
);

CREATE TABLE IF NOT EXISTS reopened_investigations (
  investigation_id     TEXT PRIMARY KEY,
  profile_id           TEXT NOT NULL,
  anchor_type          TEXT NOT NULL,
  anchor_id            TEXT NOT NULL,
  anchor_label         TEXT NOT NULL,
  occurrence_count     INTEGER NOT NULL,
  distinct_days        INTEGER NOT NULL,
  first_seen_ms        INTEGER NOT NULL,
  last_seen_ms         INTEGER NOT NULL,
  evidence_json        TEXT NOT NULL,
  computed_at          TEXT NOT NULL
);
CREATE INDEX IF NOT EXISTS idx_reopened_investigations_profile_time ON reopened_investigations(profile_id, last_seen_ms DESC);

CREATE TABLE IF NOT EXISTS path_flows (
  profile_id          TEXT NOT NULL,
  flow_pattern        TEXT NOT NULL,
  step_count          INTEGER NOT NULL,
  occurrence_count    INTEGER NOT NULL,
  last_seen_ms        INTEGER NOT NULL,
  PRIMARY KEY(profile_id, flow_pattern, step_count)
);
"#;

pub(super) const INTELLIGENCE_SCHEMA_MIGRATIONS_SQL: &str = r#"
CREATE TABLE IF NOT EXISTS intelligence_schema_migrations (
  version     INTEGER PRIMARY KEY,
  name        TEXT NOT NULL,
  applied_at  TEXT NOT NULL
);
"#;

pub(super) const LEGACY_INSIGHT_TABLES: &[&str] = &[
    "insight_bursts",
    "insight_query_groups",
    "insight_query_group_members",
    "insight_topics",
    "insight_threads",
    "insight_thread_members",
    "insight_reference_pages",
    "insight_source_effectiveness",
    "insight_cards",
    "insight_snapshot_payloads",
    "insight_runs",
    "visit_insight_features",
];
