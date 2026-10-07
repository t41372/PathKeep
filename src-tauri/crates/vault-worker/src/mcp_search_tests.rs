//! Responsibilities: prove MCP cursor coverage through the real owner search engine.
//! Not responsible for: external MCP clients or provider network calls.
//! Failure modes: tool row limits hide later pages, cursors lose scope, totals are sampled.
use super::*;
#[test]
fn mcp_cursor_enumerates_beyond_the_old_candidate_and_date_limits() {
    let _guard = lock_env();
    let root = tempdir().unwrap();
    let old = std::env::var_os(PROJECT_ROOT_OVERRIDE_ENV);
    unsafe {
        std::env::set_var(PROJECT_ROOT_OVERRIDE_ENV, root.path());
    }
    let mut config = configured_ai_config();
    config.ai.embedding_provider_id = None;
    config.ai.job_queue_paused = true;
    save_user_config(&config, None).unwrap();
    for key in [None, Some([95; 32])] {
        let vault = vault_core::Vault::open_temp_with_key(key).unwrap();
        vault_core::test_support::search::seed_pages(&vault, 1107);
        let mut cursor = None;
        let mut ids = Vec::new();
        loop {
            let response = mcp_search_result(
                &vault,
                None,
                McpSearchRequest {
                    query: "common".into(),
                    profile_id: None,
                    domain: None,
                    limit: Some(50),
                    cursor,
                },
            )
            .unwrap();
            assert_eq!(response.total, 1107);
            assert!(response.search_state.unwrap().coverage_complete);
            ids.extend(response.items.into_iter().map(|item| item.history_id));
            cursor = response.next_cursor;
            if cursor.is_none() {
                break;
            }
            assert!(ids.len() < 1107);
        }
        ids.sort_unstable();
        assert_eq!(ids, (1..=1107).collect::<Vec<_>>());
    }
    restore_env_var(PROJECT_ROOT_OVERRIDE_ENV, old.as_deref());
}
