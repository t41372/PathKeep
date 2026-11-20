//! Exercise Records pagination through a real captured transcript and provider transport.
use super::*;

fn query(f: &support::Fixture, text: &str, cursor: &Value) -> kinds::preview::Frames {
    kinds::chats::search(
        &f.session,
        from_value(json!({"query":text,"mode":"semantic","roles":["agent"],"cursor":cursor}))
            .unwrap(),
    )
    .unwrap()
}
fn frames(mut stream: kinds::preview::Frames) -> Vec<Value> {
    tokio::runtime::Builder::new_current_thread().enable_all().build().unwrap().block_on(async {
        let mut frames = vec![];
        while let Some(f) = stream.next_frame().await {
            frames.push(f);
        }
        frames
    })
}
#[test]
fn every_eligible_passage_is_reached_once_across_ties_and_score_signs() {
    let f = support::Fixture::new(true);
    let p = provider::Provider::new();
    configure(&f, &p);
    let root = f.temp.path().join("pi");
    let mut rows =
        vec![json!({"type":"session","version":3,"id":"many","timestamp":"2026-11-10T12:00:00Z"})];
    const COUNT: usize = 2049;
    for n in 0..COUNT {
        let prefix = ["kitten", "network", "opposite"][n % 3];
        rows.push(fixtures::pi_message(
            &format!("m{n}"),
            n.checked_sub(1).map(|i| format!("m{i}")).as_deref(),
            "assistant",
            &format!("{prefix} record {n}"),
        ));
    }
    write(&root.join("sessions/many.jsonl"), &fixtures::jsonl(&rows));
    f.vault()
        .agent_history()
        .unwrap()
        .backup(&[SourceRoot { harness: Harness::Pi, root }], &Default::default())
        .unwrap();
    prefs(&f, json!({"buildSemanticIndex":true}));
    let now = Instant::now();
    loop {
        let status = semantic::status(&f.session, from_value(json!({})).unwrap()).unwrap();
        if status["job"]["state"] == "ready" {
            break;
        }
        assert_ne!(status["job"]["state"], "failed", "{status}");
        assert!(now.elapsed() < Duration::from_secs(90), "{status}");
        std::thread::sleep(Duration::from_millis(25));
    }
    p.requests.lock().unwrap().clear();
    let mut cursor = Value::Null;
    let mut actual = vec![];
    let mut pages = 0;
    loop {
        let result = frames(query(&f, "kitten", &cursor));
        let last = result.last().unwrap();
        assert_eq!(last["type"], "finished", "{last}");
        assert_eq!(result[0]["payload"]["pageSize"], 100);
        let items = result[1]["payload"]["items"].as_array().unwrap();
        let count = items.len();
        assert_eq!(count, (COUNT - actual.len()).min(100));
        assert_eq!(last["payload"]["hits"], count);
        for hit in items {
            let i = hit["address"]["ordinal"].as_u64().unwrap() as usize - 1;
            let score = hit["score"].as_f64().unwrap();
            assert!((score - [1., 0., -1.][i % 3]).abs() < 0.001);
            actual.push(i);
        }
        if pages == 1 {
            let retry = frames(query(&f, "kitten", &cursor));
            assert_eq!(retry[1]["payload"], result[1]["payload"]);
        }
        pages += 1;
        cursor = last["payload"]["nextCursor"].clone();
        if cursor.is_null() {
            break;
        }
    }
    let expected: Vec<_> = (0..3).flat_map(|bucket| (bucket..COUNT).step_by(3)).collect();
    assert_eq!(actual, expected);
    assert_eq!(pages, 21);
    assert_eq!(p.requests.lock().unwrap().len(), 1, "scroll uses one query embedding");
    let stream = query(&f, "kitten", &Value::Null);
    kinds::preview::cancel(
        from_value(json!({"requestIdOrStreamId":stream.started.stream_id})).unwrap(),
    )
    .unwrap();
    assert_eq!(frames(stream).last().unwrap()["type"], "cancelled");

    // The endpoint moves after admission, while the query embedding is in flight.
    let paths = f.paths();
    *p.on_embed.lock().unwrap() = Some(Box::new(move || {
        let gate = ControlGate::acquire(&paths);
        let mut cfg = load_config(&paths).unwrap();
        cfg.ai.embedding_providers.iter_mut().find(|p| p.id == "embed").unwrap().default_model =
            "replacement".into();
        save_config(&gate, &cfg).unwrap();
    }));
    let changed = frames(query(&f, "changed provider probe", &Value::Null));
    assert_eq!(changed.last().unwrap()["payload"]["code"], "embedding-provider-changed");
    let paths = f.paths();
    let mut cfg = load_config(&paths).unwrap();
    cfg.ai.embedding_providers.iter_mut().find(|p| p.id == "embed").unwrap().default_model =
        "fixture".into();
    save_config(&ControlGate::acquire(&paths), &cfg).unwrap();
    drop(p);
    let down = frames(query(&f, "unreachable provider probe", &Value::Null));
    assert_eq!(down.last().unwrap()["payload"]["code"], "embedding-service-unavailable");
}
