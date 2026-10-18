//! Responsibilities: exercise publication races and input invalidation E2E cannot schedule.
//! Not responsible for: semantic quality or card rendering (semantic-index.spec.ts).
//! Failure modes written before implementation:
//! - A generation changes between reading a month and publishing it: reject the write.
//! - Two warmers race: a late writer must not replace a newer month's payload.
//! - Events/trail members change, including undo before Insights catches up: reread inputs.
//! - A model or same-model store rebuild leaves old months: identities invalidate every scope.
//! - A mapping-only pass adds page evidence without adding a vector: reread the month.
//! - Search rules or zone change without visits: never reuse the previous interpretation.
//! - An unrelated generation/pass changes only the cheap signature: retain an equal digest.
//!
//! Tests: real persisted rows, controlled signatures at the publication boundary.
use super::search_group_snapshots::*;
use crate::Vault;
use vault_store::{Db, Priority};

use super::{
    VectorStore, VisitContentMap,
    search_group_fixture::{project, seed},
    search_group_signatures, search_groups,
};
use vault_store::params_owned as params;
fn fingerprint(vault: &Vault, provider: &crate::AiProviderRuntime) -> Fingerprint {
    let scope = SnapshotScope::month("2025-03").unwrap();
    let (paths, pair, range) = (vault.paths().clone(), provider.clone(), scope.range.clone());
    let cheap = vault
        .read(Db::Intelligence, move |s| search_group_signatures::cheap(s, &paths, &pair, &range))
        .unwrap();
    let zone =
        vault.read(Db::Intelligence, crate::intelligence::InsightsZone::for_reading).unwrap();
    let input = search_groups::prepare(
        vault,
        provider,
        zone.range_bounds(&scope.range).unwrap(),
        zone,
        None,
        None,
    )
    .unwrap();
    Fingerprint { cheap, digest: input.digest }
}
#[test]
fn a_late_month_cannot_replace_a_new_generation_or_another_pass() {
    let vault = Vault::open_temp().unwrap();
    let provider = seed(&vault, 3, 3, 4).unwrap();
    let scope = SnapshotScope::month("2025-03").unwrap();
    let old = fingerprint(&vault, &provider);
    vault
        .write(Db::Intelligence, Priority::Batch, |s| {
            s.execute("UPDATE search_events_rows SET generation=2", ())?;
            s.execute("UPDATE search_trail_members_rows SET generation=2", ())?;
            s.execute("UPDATE intelligence_generations SET navigation=2", ())?;
            Ok(())
        })
        .unwrap();
    let new = fingerprint(&vault, &provider);
    assert_ne!(old.cheap, new.cheap);
    assert_eq!(old.digest, new.digest, "an unchanged generation must not rerun linkage");
    let (scope_copy, saved) = (scope.clone(), new.clone());
    vault
        .write(Db::Intelligence, Priority::Batch, move |s| {
            save(s, &scope_copy, &saved, &MonthGroups::default())
        })
        .unwrap();
    let (paths, pair, range) = (vault.paths().clone(), provider, scope.range.clone());
    assert!(
        !publish_if_current(&vault, &scope, &old, &MonthGroups::default(), move |s| {
            search_group_signatures::cheap(s, &paths, &pair, &range)
        })
        .unwrap()
    );
    assert_eq!(load(&vault, &scope).unwrap().unwrap().fingerprint, new);
}
#[test]
fn mapping_undo_and_store_recreation_invalidate_inputs_without_regrouping_unrelated_passes() {
    let vault = Vault::open_temp().unwrap();
    let provider = seed(&vault, 3, 3, 4).unwrap();
    let initial = fingerprint(&vault, &provider);
    project(&vault, &provider).unwrap();
    let projected = fingerprint(&vault, &provider);
    assert!(needs_read(Some(&initial), &projected.cheap));
    assert!(!needs_group(Some(&initial), &projected.digest));
    // March's first opened visit is 14. Remap it to another existing vector: no new vectors.
    VisitContentMap::for_provider(vault.paths(), "group-probe", "model")
        .append(&[(14, 2)])
        .unwrap();
    project(&vault, &provider).unwrap();
    let mapped = fingerprint(&vault, &provider);
    assert_ne!(projected.cheap, mapped.cheap);
    assert_ne!(projected.digest, mapped.digest);
    // Undo while the old navigation generation still serves. The archive delta is the guard.
    vault.write(Db::Archive,Priority::Batch,|s| {
        s.execute("UPDATE visits SET reverted_at='undone' WHERE id=13",())?;
        s.execute("INSERT INTO runs(run_type,trigger,started_at,timezone,status,profile_scope_json,warnings_json,stats_json,due_only) VALUES('undo','test','','UTC','success','[]','[]','{}',0)",())?;
        Ok(())
    }).unwrap();
    let undone = fingerprint(&vault, &provider);
    assert_ne!(mapped.cheap, undone.cheap);
    assert_ne!(mapped.digest, undone.digest);
    let store = VectorStore::for_provider(vault.paths(), "group-probe", "model");
    let header = store.read_header().unwrap().unwrap();
    let records = store.read_all().unwrap();
    let fp = super::EmbeddingFingerprint::from_descriptor(
        &super::embedding_external::external_descriptor(&provider, Some(4)),
    )
    .unwrap();
    let recreated = VectorStore::create_stamped(vault.paths(), &fp).unwrap();
    recreated.append_vectors(&records).unwrap();
    assert_ne!(header.store_id, recreated.read_header().unwrap().unwrap().store_id);
    project(&vault, &provider).unwrap();
    let rebuilt = fingerprint(&vault, &provider);
    assert_ne!(undone.cheap, rebuilt.cheap);
    assert_ne!(undone.digest, rebuilt.digest);
    // A search-rule edit with unchanged events triggers a reread, and keeps the equal digest.
    vault
        .write(Db::Intelligence, Priority::Batch, |s| {
            s.execute("INSERT INTO search_engine_rules(rule_id,engine_id,display_name,host_pattern,query_param_key,created_at,updated_at) VALUES('test','test','Test','example.com','q','',?1)", params!["changed-rule"])?;
            Ok(())
        })
        .unwrap();
    let rules = fingerprint(&vault, &provider);
    assert_ne!(rebuilt.cheap, rules.cheap);
    assert_eq!(rebuilt.digest, rules.digest);
    let search_store = VectorStore::for_provider(
        vault.paths(),
        &super::search_vectors::identity(&provider),
        "model",
    );
    let searches = search_store.read_all().unwrap();
    let recreated_searches =
        search_store.create_stamped_at(vault.paths(), &fp, "normalized-query-v1".into()).unwrap();
    recreated_searches.append_vectors(&searches).unwrap();
    project(&vault, &provider).unwrap();
    let rebuilt_searches = fingerprint(&vault, &provider);
    assert_ne!(rules.cheap, rebuilt_searches.cheap);
    assert_ne!(rules.digest, rebuilt_searches.digest);

    // A different model with the same dimensions, content keys and vector counts cannot inherit
    // any month from the former serving model.
    let mut other = provider.clone();
    other.config.default_model = "other-model".into();
    let other_fp = super::EmbeddingFingerprint::from_descriptor(
        &super::embedding_external::external_descriptor(&other, Some(4)),
    )
    .unwrap();
    VectorStore::create_stamped(vault.paths(), &other_fp)
        .unwrap()
        .append_vectors(&records)
        .unwrap();
    VectorStore::for_provider(
        vault.paths(),
        &super::search_vectors::identity(&other),
        "other-model",
    )
    .create_stamped_at(vault.paths(), &other_fp, "normalized-query-v1".into())
    .unwrap()
    .append_vectors(&searches)
    .unwrap();
    let old_map =
        VisitContentMap::for_provider(vault.paths(), "group-probe", "model").read_all().unwrap();
    let new_map = VisitContentMap::for_provider(vault.paths(), "group-probe", "other-model");
    new_map.ensure_created(vault.paths()).unwrap();
    new_map.append(&old_map.into_iter().collect::<Vec<_>>()).unwrap();
    project(&vault, &other).unwrap();
    let switched = fingerprint(&vault, &other);
    assert_ne!(rebuilt_searches.cheap, switched.cheap);
    assert_ne!(rebuilt_searches.digest, switched.digest);

    // A published search-rule rebuild can replace an old spelling without changing event count
    // or maximum visit id. Generation is therefore part of the cheap guard, not just those totals.
    vault.write(Db::Intelligence,Priority::Batch,|s| {
        s.execute("UPDATE search_events_rows SET generation=2,normalized_query=CASE WHEN visit_id=15 THEN 'changed wording' ELSE normalized_query END",())?;
        s.execute("UPDATE search_trail_members_rows SET generation=2",())?;
        s.execute("UPDATE intelligence_generations SET navigation=2,time_zone='Asia/Taipei'",())?;
        Ok(())
    }).unwrap();
    let changed = fingerprint(&vault, &other);
    assert_ne!(switched.cheap, changed.cheap);
    assert_ne!(switched.digest, changed.digest);
}
