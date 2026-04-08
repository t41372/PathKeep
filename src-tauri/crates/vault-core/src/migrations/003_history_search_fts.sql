CREATE VIRTUAL TABLE IF NOT EXISTS history_search USING fts5(
  url,
  title,
  search_terms,
  tokenize = 'unicode61 remove_diacritics 2'
);

DELETE FROM history_search;

INSERT INTO history_search (rowid, url, title, search_terms)
SELECT
  urls.id,
  urls.url,
  COALESCE(urls.title, ''),
  COALESCE(
    (
      SELECT REPLACE(GROUP_CONCAT(DISTINCT search_terms.normalized_term), ',', ' ')
      FROM search_terms
      WHERE search_terms.url_id = urls.id
        AND search_terms.source_profile_id = urls.source_profile_id
        AND search_terms.reverted_at IS NULL
    ),
    ''
  )
FROM urls;

CREATE TRIGGER IF NOT EXISTS history_search_urls_ai
AFTER INSERT ON urls
BEGIN
  INSERT INTO history_search (rowid, url, title, search_terms)
  VALUES (
    NEW.id,
    NEW.url,
    COALESCE(NEW.title, ''),
    COALESCE(
      (
        SELECT REPLACE(GROUP_CONCAT(DISTINCT search_terms.normalized_term), ',', ' ')
        FROM search_terms
        WHERE search_terms.url_id = NEW.id
          AND search_terms.source_profile_id = NEW.source_profile_id
          AND search_terms.reverted_at IS NULL
      ),
      ''
    )
  );
END;

CREATE TRIGGER IF NOT EXISTS history_search_urls_au
AFTER UPDATE OF url, title, source_profile_id ON urls
BEGIN
  DELETE FROM history_search WHERE rowid = OLD.id;
  INSERT INTO history_search (rowid, url, title, search_terms)
  VALUES (
    NEW.id,
    NEW.url,
    COALESCE(NEW.title, ''),
    COALESCE(
      (
        SELECT REPLACE(GROUP_CONCAT(DISTINCT search_terms.normalized_term), ',', ' ')
        FROM search_terms
        WHERE search_terms.url_id = NEW.id
          AND search_terms.source_profile_id = NEW.source_profile_id
          AND search_terms.reverted_at IS NULL
      ),
      ''
    )
  );
END;

CREATE TRIGGER IF NOT EXISTS history_search_urls_ad
AFTER DELETE ON urls
BEGIN
  DELETE FROM history_search WHERE rowid = OLD.id;
END;

CREATE TRIGGER IF NOT EXISTS history_search_terms_ai
AFTER INSERT ON search_terms
BEGIN
  DELETE FROM history_search WHERE rowid = NEW.url_id;
  INSERT INTO history_search (rowid, url, title, search_terms)
  SELECT
    urls.id,
    urls.url,
    COALESCE(urls.title, ''),
    COALESCE(
      (
        SELECT REPLACE(GROUP_CONCAT(DISTINCT search_terms.normalized_term), ',', ' ')
        FROM search_terms
        WHERE search_terms.url_id = urls.id
          AND search_terms.source_profile_id = urls.source_profile_id
          AND search_terms.reverted_at IS NULL
      ),
      ''
    )
  FROM urls
  WHERE urls.id = NEW.url_id;
END;

CREATE TRIGGER IF NOT EXISTS history_search_terms_au
AFTER UPDATE OF normalized_term, term, url_id, source_profile_id, reverted_at ON search_terms
BEGIN
  DELETE FROM history_search WHERE rowid IN (OLD.url_id, NEW.url_id);
  INSERT INTO history_search (rowid, url, title, search_terms)
  SELECT
    urls.id,
    urls.url,
    COALESCE(urls.title, ''),
    COALESCE(
      (
        SELECT REPLACE(GROUP_CONCAT(DISTINCT search_terms.normalized_term), ',', ' ')
        FROM search_terms
        WHERE search_terms.url_id = urls.id
          AND search_terms.source_profile_id = urls.source_profile_id
          AND search_terms.reverted_at IS NULL
      ),
      ''
    )
  FROM urls
  WHERE urls.id IN (OLD.url_id, NEW.url_id);
END;

CREATE TRIGGER IF NOT EXISTS history_search_terms_ad
AFTER DELETE ON search_terms
BEGIN
  DELETE FROM history_search WHERE rowid = OLD.url_id;
  INSERT INTO history_search (rowid, url, title, search_terms)
  SELECT
    urls.id,
    urls.url,
    COALESCE(urls.title, ''),
    COALESCE(
      (
        SELECT REPLACE(GROUP_CONCAT(DISTINCT search_terms.normalized_term), ',', ' ')
        FROM search_terms
        WHERE search_terms.url_id = urls.id
          AND search_terms.source_profile_id = urls.source_profile_id
          AND search_terms.reverted_at IS NULL
      ),
      ''
    )
  FROM urls
  WHERE urls.id = OLD.url_id;
END;
