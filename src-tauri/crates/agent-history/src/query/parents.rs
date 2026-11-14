//! Responsibilities: resolve native parent/fork links without issuing foreign identifiers.
//! Not responsible for: treating forks as descendants or passing archive-sized SQL parameters.

/// Identifiers are supplied only by the two fixed callers, never by conversation text.
pub(crate) fn native_link(native: &str, path: &str) -> String {
    format!(
        "coalesce(
           (SELECT p.conversation_id FROM ah_sessions p
            WHERE p.harness=s.harness AND p.native_id=s.{native}
              AND p.source_root=s.source_root AND p.relative_path=s.{path}
              AND p.conversation_id<>s.conversation_id LIMIT 1),
           (SELECT min(p.conversation_id) FROM ah_sessions p
            WHERE p.harness=s.harness AND p.native_id=s.{native}
              AND p.source_root=s.source_root AND p.conversation_id<>s.conversation_id
            HAVING count(DISTINCT p.conversation_id)=1),
           (SELECT min(p.conversation_id) FROM ah_sessions p
            WHERE p.harness=s.harness AND p.native_id=s.{native}
              AND p.conversation_id<>s.conversation_id
            HAVING count(DISTINCT p.conversation_id)=1))"
    )
}

/// A single recursive query has a fixed parameter count even for thousands of descendants.
/// UNION also terminates corrupt cycles in native parent evidence.
pub(crate) fn hidden_query() -> String {
    let parent = native_link("parent_session_id", "parent_path");
    format!(
        "WITH RECURSIVE hidden(id) AS (
             SELECT conversation_id FROM ah_user_state WHERE trashed_at IS NOT NULL
             UNION
             SELECT s.conversation_id FROM ah_sessions s
             JOIN hidden h ON h.id={parent}
             WHERE s.conversation_current=1
         ) SELECT id FROM hidden"
    )
}
