//! Responsibilities: preserve the native habit persistence regression.
//! Not responsible for: rebuilding sessions or defining schemas.
//! Failure modes: aggregate writes omit habit labels or interruption flags.
#[cfg(test)]
mod tests {
    use super::super::*;

    #[test]
    fn structural_profile_aggregate_replace_persists_habit_patterns() {
        let vault = crate::Vault::open_temp().unwrap();
        vault
            .write(vault_store::Db::Intelligence, vault_store::Priority::Batch, |connection| {
                connection.execute(
                    "INSERT INTO intelligence_generations(profile_id,aggregates) VALUES('p1',1)",
                    (),
                )?;
                let habits = vec![super::super::HabitPatternRecord {
                    profile_id: "p1".to_string(),
                    registrable_domain: "example.com".to_string(),
                    habit_type: "weekly".to_string(),
                    mean_interval_days: 7.0,
                    cv: 0.15,
                    visit_count: 8,
                    last_visited_ms: 1767225600000,
                    is_interrupted: true,
                }];

                build_rows::persist_aggregates(
                    connection,
                    1,
                    &[],
                    &[],
                    &habits,
                    &[],
                    &[],
                    "2026-01-01T00:00:00Z",
                )?;

                let row: (String, i64) = connection
            .query_one(
                "SELECT habit_type, is_interrupted FROM habit_patterns WHERE profile_id = 'p1'",
                (),
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .expect("habit row");
                assert_eq!(row, ("weekly".to_string(), 1));
                Ok(())
            })
            .unwrap();
    }
}
