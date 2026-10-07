//! Responsibilities: exercise serialized commands and SSE over a real owner.
//! Not responsible for: browser UI or external listeners.
//! Failure modes: channel events differ from worker results, continuation is lost,
//! cancellation leaves a stream alive, or empty fixtures hide missing hydration.
use super::*;
use axum::{body::HttpBody, response::IntoResponse};

#[test]
fn bridge_commands_and_sse_page_the_same_native_search() {
    let _guard = crate::test_support::lock_env();
    let root = tempfile::tempdir().unwrap();
    let name = crate::test_support::PROJECT_ROOT_OVERRIDE_ENV;
    let old = std::env::var_os(name);
    unsafe {
        std::env::set_var(name, root.path());
    }
    let paths = vault_core::project_paths_with_root(root.path());
    let config = vault_core::AppConfig {
        initialized: true,
        archive_mode: vault_core::ArchiveMode::Plaintext,
        ..Default::default()
    };
    vault_core::save_config(&paths, &config).unwrap();
    let session = crate::session::SessionState::default();
    let vault = session.vault().unwrap();
    vault_core::test_support::search::seed_pages(&vault, 1107);
    let state = DevIpcBridgeState::without_app(session, 43117);
    tokio::runtime::Runtime::new().unwrap().block_on(async {
        let started =
            dispatch(&state, "start_search", json!({"request":{"query":"common","pageSize":400}}))
                .await
                .unwrap();
        let id = started["sessionId"].as_str().unwrap();
        assert!(started["indexer"].is_object());
        let mut body = events(State(state.clone()), Path(id.into()))
            .await
            .unwrap()
            .into_response()
            .into_body();
        let mut ids = Vec::new();
        loop {
            let frame = tokio::time::timeout(
                std::time::Duration::from_secs(30),
                std::future::poll_fn(|cx| Pin::new(&mut body).poll_frame(cx)),
            )
            .await
            .unwrap()
            .unwrap()
            .unwrap();
            let bytes = frame.into_data().unwrap();
            let text = std::str::from_utf8(&bytes).unwrap();
            let data = text.trim().strip_prefix("data: ").unwrap();
            let event: SearchEvent = serde_json::from_str(data).unwrap();
            if let SearchEvent::Step(step) = event {
                if step.provisional || !step.count.exact {
                    continue;
                }
                assert_eq!(step.count.value, 1107);
                ids.extend(step.items.into_iter().map(|hit| hit.page_id));
                if let Some(cursor) = step.cursor {
                    dispatch(&state, "next_search_page", json!({"sessionId":id,"cursor":cursor}))
                        .await
                        .unwrap();
                } else {
                    break;
                }
            }
        }
        ids.sort_unstable();
        assert_eq!(ids, (1..=1107).collect::<Vec<_>>());
        dispatch(&state, "cancel_search", json!({"sessionId":id})).await.unwrap();
        assert!(!state.search_streams.lock().unwrap().contains_key(id));
        drop(body);
    });
    state.session.close_vault().unwrap();
    unsafe {
        match old {
            Some(value) => std::env::set_var(name, value),
            None => std::env::remove_var(name),
        }
    }
}
