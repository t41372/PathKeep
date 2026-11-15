//! Responsibilities: pin every weak ordering of level and the three valid stamp numbers.
//! Not responsible for: implementing the decision or granting a root admission token.
//! Consumers supply the production evaluator; fixture coverage alone does not prove its decision.
pub fn assert_decisions(decide: impl Fn(u64, u64, u64, u64) -> &'static str) {
    let cases: serde_json::Value =
        serde_json::from_str(include_str!("../fixtures/decision-orderings.json")).unwrap();
    for case in cases.as_array().unwrap() {
        let actual = decide(
            case["level"].as_u64().unwrap(),
            case["revision"].as_u64().unwrap(),
            case["minReader"].as_u64().unwrap(),
            case["minWriter"].as_u64().unwrap(),
        );
        assert_eq!(actual, case["decision"].as_str().unwrap(), "{case}");
    }
}

#[test]
fn vectors_cover_every_valid_weak_ordering() {
    use std::collections::BTreeSet;
    fn ordering(values: [u64; 4]) -> Vec<std::cmp::Ordering> {
        (0..4).flat_map(|a| (a + 1..4).map(move |b| values[a].cmp(&values[b]))).collect()
    }
    let cases: serde_json::Value =
        serde_json::from_str(include_str!("../fixtures/decision-orderings.json")).unwrap();
    let represented: BTreeSet<_> = cases
        .as_array()
        .unwrap()
        .iter()
        .map(|c| {
            ordering([
                c["level"].as_u64().unwrap(),
                c["minReader"].as_u64().unwrap(),
                c["minWriter"].as_u64().unwrap(),
                c["revision"].as_u64().unwrap(),
            ])
        })
        .collect();
    let mut possible = BTreeSet::new();
    for level in 0..=7 {
        for reader in 1..=5 {
            for writer in reader..=5 {
                for revision in writer..=5 {
                    possible.insert(ordering([level, reader, writer, revision]));
                }
            }
        }
    }
    assert_eq!(represented, possible);
    assert_eq!(represented.len(), 20);
}
