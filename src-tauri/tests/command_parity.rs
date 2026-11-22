//! Failure modes: production-only and dev-only names, duplicate registrations, and
//! delegated arms that exist in source but cannot be reached from the dev router.

use std::collections::BTreeSet;

const HANDLER: &str = include_str!("../src/lib.rs");
const ROUTER: &str = include_str!("../src/dev_ipc_bridge/dispatch.rs");
const SEARCH: &str = include_str!("../src/dev_ipc_bridge/search.rs");

fn section<'a>(source: &'a str, start: &str, end: &str) -> &'a str {
    source.split_once(start).expect(start).1.split_once(end).expect(end).0
}

fn arms<'a>(source: &'a str, start: &str, end: &str) -> impl Iterator<Item = &'a str> {
    section(source, start, end).lines().filter_map(|line| {
        let pattern = line.trim_start().strip_prefix('"')?;
        Some(pattern.split_once("\" =>").expect("literal command arm").0)
    })
}

fn unique<'a>(surface: &str, names: impl Iterator<Item = &'a str>) -> BTreeSet<&'a str> {
    let mut set = BTreeSet::new();
    for name in names {
        assert!(set.insert(name), "duplicate {surface} command {name}");
    }
    assert!(!set.is_empty(), "empty {surface}");
    set
}

fn compare(handler: &str, router: &str, search: &str) {
    // The two dev-only process controls precede the normal dispatch table.
    // A new early return must not become an unreviewed command outside parity.
    let direct = unique(
        "early dev route",
        router.lines().filter_map(|line| {
            line.trim_start()
                .strip_prefix("if command == \"")?
                .split_once('"')
                .map(|(name, _)| name)
        }),
    );
    assert_eq!(direct, BTreeSet::from(["debug_request_quit", "debug_set_search_indexer_paused"]));
    let registered = unique(
        "handler",
        section(handler, "tauri::generate_handler![", "]")
            .split(',')
            .map(str::trim)
            .filter(|name| !name.is_empty()),
    );
    let top = arms(router, "match command {", "other =>");
    let delegated = unique("search", arms(search, "match command.as_str() {", "_ =>"));
    let routes = section(router, "if matches!(", ") {").split_once(',').unwrap().1;
    assert_eq!(
        unique("search route", routes.split('|').map(|s| s.trim().trim_matches('"'))),
        delegated
    );
    let dispatched = unique("dev", top.chain(delegated));
    assert!(direct.is_disjoint(&dispatched), "an early dev route shadows a dispatch arm");
    let missing: Vec<_> = registered.difference(&dispatched).collect();
    let extra: Vec<_> = dispatched.difference(&registered).collect();
    assert!(
        missing.is_empty() && extra.is_empty(),
        "missing dev arms {missing:?}, missing handlers {extra:?}"
    );
}

#[test]
fn desktop_and_dev_commands_match() {
    compare(HANDLER, ROUTER, SEARCH);
}

#[test]
fn removing_or_duplicating_any_registration_breaks_parity() {
    for (handler, router, search) in [
        (HANDLER.replace("            app_snapshot,", ""), ROUTER.into(), SEARCH.into()),
        (HANDLER.into(), ROUTER.replace("\"app_snapshot\" =>", "\"orphan\" =>"), SEARCH.into()),
        (HANDLER.into(), ROUTER.into(), SEARCH.replace("\"start_search\" =>", "\"orphan\" =>")),
        (HANDLER.into(), ROUTER.replace("\"start_search\" | ", ""), SEARCH.into()),
        (
            HANDLER.replace("app_snapshot,", "app_snapshot, app_snapshot,"),
            ROUTER.into(),
            SEARCH.into(),
        ),
        (
            HANDLER.into(),
            ROUTER.replace("\"app_snapshot\" =>", "\"app_build_info\" =>"),
            SEARCH.into(),
        ),
    ] {
        assert!(std::panic::catch_unwind(|| compare(&handler, &router, &search)).is_err());
    }
}

#[test]
fn browsing_index_removal_is_available_on_both_transports() {
    for command in ["preview_free_semantic_index", "free_semantic_index"] {
        assert!(
            section(HANDLER, "tauri::generate_handler![", "]")
                .split(',')
                .any(|s| s.trim() == command),
            "{command}"
        );
        assert!(
            arms(ROUTER, "match command {", "other =>").any(|name| name == command),
            "{command}"
        );
    }
}
