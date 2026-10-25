//! Responsibilities: enforce synchronization and collector admission independently of models.
//! Not responsible for: ranking. Failure cases precede implementation in semantic-index-control.md.
use super::*;
use std::sync::{Barrier, mpsc};
use std::time::Duration;

#[test]
fn waiting_for_a_database_reader_does_not_hold_registry() {
    let vault = Vault::open_temp().unwrap();
    let entered = Arc::new(Barrier::new(3));
    let release = Arc::new(Barrier::new(3));
    let mut readers = Vec::new();
    for _ in 0..2 {
        let (vault, entered, release) = (vault.clone(), entered.clone(), release.clone());
        readers.push(std::thread::spawn(move || {
            vault
                .read(Db::Intelligence, move |_| {
                    entered.wait();
                    release.wait();
                    Ok(())
                })
                .unwrap()
        }));
    }
    entered.wait();
    let (started, started_rx) = mpsc::channel();
    let resolving = vault.clone();
    let resolver = std::thread::spawn(move || {
        started.send(()).unwrap();
        resolve(&resolving, "service", "model").unwrap();
    });
    started_rx.recv().unwrap();
    std::thread::sleep(Duration::from_millis(30));
    let unlocked = vault.vector_generations.registry.try_lock().is_ok();
    release.wait();
    for reader in readers {
        reader.join().unwrap();
    }
    resolver.join().unwrap();
    assert!(unlocked, "a waiting resolve must never prevent lease release");
}

#[test]
fn serving_and_vanished_generations_leave_retired_set() {
    let vault = Vault::open_temp().unwrap();
    let table = provider_table_name("service", "model");
    let relative = format!("{table}/j1/g1");
    let path = vault.paths().vectors_dir.join(&relative);
    std::fs::create_dir_all(&path).unwrap();
    write_identity(
        path.parent().unwrap(),
        &LineageIdentity {
            provider_id: "service".into(),
            model: "model".into(),
            job_id: None,
            source_watermark: 0,
            upper_history_id: 0,
            sources_valid: true,
        },
    )
    .unwrap();
    crate::durable_io::atomic_durable_write(
        &path.join("serving.json"),
        &serde_json::to_vec(&ServingMetadata {
            version: 1,
            provider_id: "service".into(),
            model: "model".into(),
            lineage: "j1".into(),
            generation: "g1".into(),
            ..Default::default()
        })
        .unwrap(),
    )
    .unwrap();
    let pointer = relative.clone();
    vault.write(Db::Intelligence,vault_store::Priority::Batch,move |s| {
        s.execute("INSERT INTO ai_index_ledger(provider_id,model,sidecar_table,index_version,state) VALUES('service','model',?1,'v','ready')",[pointer])?;Ok(())
    }).unwrap();
    let vanished = path.with_file_name("g0");
    {
        let mut registry = vault.vector_generations.registry.lock().unwrap();
        registry.retired.extend([path.clone(), vanished]);
    }
    collect(&vault, false).unwrap();
    assert!(vault.vector_generations.registry.lock().unwrap().retired.is_empty());
    assert!(path.exists());
}

fn empty_generation(vault: &Vault, lineage: &str, run: i64) -> String {
    let relative = format!("{}/{lineage}/g{run}", provider_table_name("service", "model"));
    let path = vault.paths().vectors_dir.join(&relative);
    std::fs::create_dir_all(&path).unwrap();
    write_identity(
        path.parent().unwrap(),
        &LineageIdentity {
            provider_id: "service".into(),
            model: "model".into(),
            job_id: None,
            source_watermark: 0,
            upper_history_id: 0,
            sources_valid: true,
        },
    )
    .unwrap();
    std::fs::write(
        path.join("serving.json"),
        serde_json::to_vec(&ServingMetadata {
            version: 1,
            provider_id: "service".into(),
            model: "model".into(),
            lineage: lineage.into(),
            generation: format!("g{run}"),
            ..Default::default()
        })
        .unwrap(),
    )
    .unwrap();
    relative
}
fn set_pointer(vault: &Vault, relative: &str) {
    let pointer = relative.to_owned();
    vault.write(Db::Intelligence,vault_store::Priority::Batch,move |s| {
        s.execute("INSERT INTO ai_index_ledger(provider_id,model,sidecar_table,index_version,state) VALUES('service','model',?1,'v','ready') ON CONFLICT(provider_id,model) DO UPDATE SET sidecar_table=excluded.sidecar_table,state='ready'",[pointer])?;Ok(())
    }).unwrap();
}
fn hook(vault: &Vault, name: &'static str, action: impl FnOnce() + Send + 'static) {
    vault.vector_generations.hooks.lock().unwrap().insert(name, Box::new(action));
}

#[test]
fn publication_before_gc_reservation_cannot_delete_new_serving_files() {
    let vault = Vault::open_temp().unwrap();
    let old = empty_generation(&vault, "r1", 1);
    set_pointer(&vault, &old);
    let new = empty_generation(&vault, "r2", 2);
    let writer = {
        let mut registry = vault.vector_generations.registry.lock().unwrap();
        pin(&vault, new.clone(), "r2".into(), &mut registry)
    };
    let publishing = vault.clone();
    let published = new.clone();
    hook(&vault, "before_reservation", move || {
        set_pointer(&publishing, &published);
        drop(writer);
    });
    collect(&vault, true).unwrap();
    assert!(vault.paths().vectors_dir.join(&new).exists());
    assert_eq!(resolve(&vault, "service", "model").unwrap().unwrap().relative(), new);
}

#[test]
fn trash_deletion_never_holds_registry_and_leftovers_are_collected() {
    let vault = Vault::open_temp().unwrap();
    let abandoned = empty_generation(&vault, "r1", 1);
    let checking = vault.clone();
    hook(&vault, "before_trash_delete", move || {
        assert!(checking.vector_generations.registry.try_lock().is_ok());
        assert!(checking.paths().vectors_dir.join("trash").read_dir().unwrap().next().is_some());
    });
    collect(&vault, true).unwrap();
    assert!(!vault.paths().vectors_dir.join(abandoned).exists());
    let leftover = vault.paths().vectors_dir.join("trash/interrupted");
    std::fs::create_dir_all(&leftover).unwrap();
    std::fs::write(leftover.join("source"), "retry").unwrap();
    collect(&vault, false).unwrap();
    assert!(!leftover.exists());
}

#[test]
fn close_stops_row_gc_between_batches_and_next_owner_finishes() {
    let vault = Vault::open_temp().unwrap();
    let relative = empty_generation(&vault, "r1", 1);
    vault.write(Db::Intelligence,vault_store::Priority::Batch,|s| {
        for id in 1..=1500_i64 {
            s.execute("INSERT INTO ai_embeddings(history_id,profile_id,url,domain,visited_at,content_hash,content_bytes,provider_id,model,indexed_at,lineage) VALUES(?1,'p','u','d','','h',1,'service','model','','r1')",[id])?;
        }
        s.execute("INSERT INTO ai_embedding_totals VALUES('service','model',1500,1500,0,NULL,'r1')",())?;Ok(())
    }).unwrap();
    let closing = vault.clone();
    hook(&vault, "after_row_batch", move || {
        closing.vector_generations.closed.store(true, std::sync::atomic::Ordering::Release);
    });
    assert!(collect(&vault, true).is_err());
    let lineage = vault.paths().vectors_dir.join(relative).parent().unwrap().to_owned();
    assert!(lineage.join("identity.json").exists());
    assert_eq!(
        vault
            .read(Db::Intelligence, |s| Ok(s.query_one(
                "SELECT COUNT(*) FROM ai_embeddings",
                (),
                |r| Ok(r.get::<i64>(0)?)
            )?))
            .unwrap(),
        988
    );
    // Simulate a later owner with a new admission state, retaining the real database and files.
    let mut reopened = vault.clone();
    reopened.vector_generations = Arc::default();
    collect(&reopened, true).unwrap();
    assert!(!lineage.exists());
    assert_eq!(
        reopened
            .read(Db::Intelligence, |s| Ok(s.query_one(
                "SELECT COUNT(*) FROM ai_embeddings",
                (),
                |r| Ok(r.get::<i64>(0)?)
            )?))
            .unwrap(),
        0
    );
}

#[test]
fn status_discards_file_leases_before_its_database_callback() {
    let vault = Vault::open_temp().unwrap();
    let relative = empty_generation(&vault, "r1", 1);
    set_pointer(&vault, &relative);
    let mut config = vault.config().clone();
    config.ai.enabled = true;
    let provider = crate::AiProviderConfig {
        id: "service".into(),
        default_model: "model".into(),
        ..Default::default()
    };
    config.ai.embedding_provider_id = Some("service".into());
    config.ai.embedding_providers = vec![provider];
    let checking = vault.clone();
    hook(&vault, "status_before_db", move || {
        assert!(
            checking.vector_generations.registry.lock().unwrap().pins.is_empty(),
            "file leases cannot be owned by the status database callback"
        );
    });
    crate::ai::ai_index_status(&vault.with_config(config)).unwrap();
}

#[test]
fn a_gc_reservation_cannot_be_mistaken_for_a_missing_serving_generation() {
    let vault = Vault::open_temp().unwrap();
    let serving = empty_generation(&vault, "r1", 1);
    set_pointer(&vault, &serving);
    let checking = vault.clone();
    let expected = serving.clone();
    hook(&vault, "after_reservation", move || {
        gc::repair_pointers(&checking).unwrap();
        assert_eq!(read_pointer(&checking, "service", "model").unwrap(), Some(expected));
        assert_eq!(
            checking
                .read(Db::Intelligence, |s| Ok(s.query_one(
                    "SELECT state FROM ai_index_ledger",
                    (),
                    |r| Ok(r.get::<String>(0)?)
                )?))
                .unwrap(),
            "ready"
        );
    });
    collect(&vault, false).unwrap();
    assert_eq!(read_pointer(&vault, "service", "model").unwrap(), Some(serving));
}

/// Failure modes: GC reserves the serving lineage while it retires an old generation, and a build
/// or search that resolves in that moment sees no index: the build starts a new lineage and embeds
/// every visit again, and status reports the index as damaged.
#[test]
fn gc_never_hides_the_serving_generation_while_it_retires_old_ones() {
    let vault = Vault::open_temp().unwrap();
    let old = empty_generation(&vault, "r1", 1);
    let serving = empty_generation(&vault, "r1", 2);
    set_pointer(&vault, &serving);
    let (checking, (found, results)) = (vault.clone(), mpsc::channel());
    let expected = serving.clone();
    hook(&vault, "after_reservation", move || {
        // Another thread: if GC held the serving lineage, this would wait for GC itself.
        std::thread::spawn(move || {
            let lease = resolve(&checking, "service", "model").unwrap();
            found.send(lease.map(|lease| lease.relative().to_owned())).unwrap();
        });
        assert_eq!(
            results.recv_timeout(Duration::from_secs(10)).expect("resolve did not wait for GC"),
            Some(expected)
        );
    });
    collect(&vault, true).unwrap();
    assert!(!vault.paths().vectors_dir.join(old).exists());
    assert!(vault.paths().vectors_dir.join(serving).exists());
}

/// Failure modes: GC looks at the pointer, a pass publishes the next generation of the same
/// lineage and drops its writer pin, and GC then reserves that generation; a search arriving in
/// that moment reports no index, or a build starts a new lineage and embeds every visit again.
#[test]
fn a_generation_published_after_gc_looked_is_waited_for_not_missing() {
    let vault = Vault::open_temp().unwrap();
    let old = empty_generation(&vault, "r1", 1);
    set_pointer(&vault, &old);
    let new = empty_generation(&vault, "r1", 2);
    let writer = {
        let mut registry = vault.vector_generations.registry.lock().unwrap();
        pin(&vault, new.clone(), "r1".into(), &mut registry)
    };
    let (publishing, published) = (vault.clone(), new.clone());
    hook(&vault, "before_reservation", move || {
        set_pointer(&publishing, &published);
        drop(writer);
    });
    let (searching, (found, results)) = (vault.clone(), mpsc::channel());
    let (waiting, waited) = mpsc::channel();
    hook(&vault, "resolve_waits_for_gc", move || waiting.send(()).unwrap());
    let reserved = vault.paths().vectors_dir.join(&new);
    hook(&vault, "after_reservation", move || {
        let registry = searching.vector_generations.registry.lock().unwrap();
        assert!(registry.collecting.iter().any(|path| reserved.starts_with(path)));
        drop(registry);
        std::thread::spawn(move || {
            let lease = resolve(&searching, "service", "model").unwrap();
            found.send(lease.map(|lease| lease.relative().to_owned())).unwrap();
        });
        waited.recv_timeout(Duration::from_secs(10)).expect("the search waits for GC");
    });
    collect(&vault, true).unwrap();
    assert_eq!(results.recv().unwrap(), Some(new.clone()));
    assert!(vault.paths().vectors_dir.join(&new).exists());
}
