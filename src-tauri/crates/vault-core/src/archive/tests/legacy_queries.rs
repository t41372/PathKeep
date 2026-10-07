//! Responsibilities: supply independent legacy fixtures to maintenance and search-port tests.
//! Not responsible for: exercising legacy search, native reads or schema setup.
//! Failure modes: shared fixture facts lose profiles, visits, search terms or image references.
use super::*;

pub(in crate::archive) fn seed_reader_archive(paths: &ProjectPaths, config: &AppConfig) {
    let db = open_archive_connection(paths, config, None).unwrap();
    db.execute_batch("INSERT INTO runs(id,run_type,trigger,started_at,finished_at,status,stats_json) VALUES(1,'backup','manual','2026-04-05','2026-04-05','success','{}');
        INSERT INTO source_profiles(id,browser_kind,browser_family,browser_product,profile_name,profile_path,discovered_at,enabled,profile_key,updated_at)
        VALUES(1,'chrome','chromium','Google Chrome','Default','/fixture','2026-04-05',1,'chrome:Default','2026-04-05');
        INSERT INTO urls(id,url,title,visit_count,typed_count,first_visit_ms,first_visit_iso,last_visit_ms,last_visit_iso,source_profile_id,created_by_run_id,source_url_id,registrable_domain)
        VALUES(1,'https://example.com/archive','Archive docs',2,0,1775383200000,'2026-04-05T10:00:00+00:00',1775386800000,'2026-04-05T11:00:00+00:00',1,1,1,'example.com');
        INSERT INTO visits(id,source_profile_id,url_id,visit_time_ms,visit_time_iso,transition_type,created_by_run_id,source_visit_id)
        VALUES(1,1,1,1775383200000,'2026-04-05T10:00:00+00:00',805306368,1,'1'),(2,1,1,1775386800000,'2026-04-05T11:00:00+00:00',805306368,1,'2');
        INSERT INTO search_terms(url_id,term,normalized_term,source_profile_id,created_by_run_id) VALUES(1,'deep recall token','deep recall token',1,1);
        INSERT INTO favicon_blobs(blob_hash,image_data,recorded_at) VALUES('fixture-icon',X'89504E470D0A1A0A01','2026-04-05');
        INSERT INTO favicons(page_url,icon_url,width,height,last_updated_ms,last_updated_iso,image_blob_hash,page_host,page_registrable_domain,source_profile_id,created_by_run_id)
        VALUES('https://example.com/archive','https://example.com/favicon.ico',16,16,1775386800000,'2026-04-05T11:00:00+00:00','fixture-icon','example.com','example.com',1,1),
        ('https://example.com/archive','https://example.com/app-icon.ico',16,16,1775386800000,'2026-04-05T11:00:00+00:00','fixture-icon','example.com','example.com',1,1);").unwrap();
}
