//! Responsibilities: share a bounded, fully accounted search partition between Ask and Insights.
//! Not responsible for: naming subjects, changing lexical families, or embedding on reads.
//! Failure modes: searches in two groups or none; missing vectors vanish; short-only groups;
//! unstable cap/ties; the same trail/page is counted twice through different wordings; pages
//! outside trails leak; a generation or position changes while reading.
//! Tests: cap and shared-trail accounting below; cluster edges in search_groups_cluster;
//! semantic-index.spec.ts covers real index/build/report integration and the 31-day boundary.

use super::{
    agent_tools::AgentToolContext,
    search_groups_cluster::{cluster, normalize},
    search_vectors,
};
use crate::{AiProviderRuntime, AiRunControl, Vault};
use anyhow::{Context, Result, ensure};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::sync::Arc;
use vault_store::{Db, params_owned as params};

type Trail = (String, i64, String);
#[derive(Default)]
struct Search {
    query: String,
    wording: String,
    searches: u64,
    first: i64,
    last: i64,
    days: BTreeSet<String>,
    trails: BTreeSet<Trail>,
}
#[derive(Clone)]
struct Opened {
    id: i64,
    page: i64,
    time: i64,
    title: Option<String>,
    url: String,
    site: String,
}

fn limit(rows: &mut Vec<Search>) -> Vec<Search> {
    rows.sort_by(|a, b| {
        b.trails.len().cmp(&a.trails.len()).then(a.first.cmp(&b.first)).then(a.query.cmp(&b.query))
    });
    rows.split_off(rows.len().min(4096))
}
fn group_trails(rows: &[Search], members: &[usize]) -> BTreeSet<Trail> {
    members.iter().flat_map(|&i| rows[i].trails.iter().cloned()).collect()
}
fn group_pages(
    trails: &BTreeSet<Trail>,
    opened: &HashMap<Trail, Vec<Opened>>,
) -> Vec<(Opened, u64)> {
    let mut seen = BTreeSet::new();
    let mut pages = BTreeMap::<i64, (Opened, u64)>::new();
    for visit in trails.iter().filter_map(|t| opened.get(t)).flatten() {
        if !seen.insert(visit.id) {
            continue;
        }
        let entry = pages.entry(visit.page).or_insert_with(|| (visit.clone(), 0));
        entry.1 += 1;
        if (visit.time, visit.id) > (entry.0.time, entry.0.id) {
            entry.0 = visit.clone();
        }
    }
    let mut pages = pages.into_values().collect::<Vec<_>>();
    pages.sort_by(|a, b| b.1.cmp(&a.1).then(b.0.time.cmp(&a.0.time)).then(a.0.page.cmp(&b.0.page)));
    pages
}

/// Requires an explicit month-sized window so the quadratic linkage has a visible boundary.
pub(super) async fn report(
    context: &AgentToolContext,
    start: &str,
    end: &str,
    group: Option<&str>,
) -> Result<Value> {
    let day = |s: &str| -> Result<chrono::NaiveDate> {
        let date = chrono::NaiveDate::parse_from_str(s, "%Y-%m-%d")
            .context("searches requires dates in YYYY-MM-DD")?;
        ensure!(date.format("%Y-%m-%d").to_string() == s, "searches dates must be YYYY-MM-DD");
        Ok(date)
    };
    let (first, last) = (day(start)?, day(end)?);
    ensure!(last >= first, "searches end_date must be on or after start_date");
    ensure!((last - first).num_days() < 31, "searches covers at most 31 days; ask month by month");
    let range = crate::DateRange { start: start.into(), end: end.into() };
    let (bounds, zone) = context.vault.read(Db::Intelligence, move |session| {
        let built = session.query_one(
            "SELECT COUNT(*) FROM intelligence_generations WHERE navigation > 0",
            (),
            |row| Ok(row.get::<i64>(0)?),
        )?;
        ensure!(built > 0,
            "searches unavailable: the Insights build has not run yet. Build Insights, or use intelligence_report report=\"search_trails\".");
        let zone = crate::intelligence::InsightsZone::for_reading(session)?;
        Ok((zone.range_bounds(&range)?, zone))
    })?;
    let window = json!({"startDate":start,"endDate":end,"timeZone":zone.name()});
    let context = context.clone();
    let group = group.map(str::to_owned);
    tokio::task::spawn_blocking(move || {
        let provider = context.embedding_provider.as_ref().context(
            "searches unavailable: no embedding provider is configured. Use intelligence_report report=\"search_trails\".",
        )?;
        let input = prepare(&context.vault, provider, bounds, zone, None, context.run_control.as_ref())?;
        Ok(build(&context.vault, provider, input, window, group.as_deref(), context.run_control.as_ref(), context.time_zone)?.report)
    })
        .await?
}

/// Pins all inputs before quadratic linkage; snapshots can digest them and skip linkage entirely.
pub(super) struct Prepared {
    page_index: super::FlatVectorIndex,
    search_index: super::FlatVectorIndex,
    positions: HashMap<u64, usize>,
    page_positions: super::visit_position_map::PointLookup,
    generations: Vec<(String, i64)>,
    rows: Vec<Search>,
    opened: HashMap<Trail, Vec<Opened>>,
    bounds: (i64, i64),
    pub digest: String,
}

pub(super) fn prepare(
    vault: &Vault,
    provider: &AiProviderRuntime,
    bounds: (i64, i64),
    zone: crate::intelligence::InsightsZone,
    profile: Option<&str>,
    control: Option<&Arc<dyn AiRunControl>>,
) -> Result<Prepared> {
    let page_index = search_vectors::available(vault.paths(), provider).map_err(|e| {
        anyhow::anyhow!(
            "searches unavailable: {e}. Use intelligence_report report=\"search_trails\"."
        )
    })?;
    let search_index = search_vectors::open(vault.paths(), provider, page_index.dim())?;
    let positions =
        (0..search_index.len()).map(|i| (search_index.key(i), i)).collect::<HashMap<_, _>>();
    let generations = generations(vault)?;
    let row_control = control.cloned();
    let profile = profile.map(str::to_owned);
    let (rows, events_digest) = vault.read(Db::Intelligence, move |session| {
        let mut events = Sha256::new();
        let mut rows = BTreeMap::<String, Search>::new();
        session.for_each(
            "SELECT e.normalized_query,e.raw_query,e.visit_time_ms,e.profile_id,e.generation,e.trail_id,e.visit_id
             FROM search_events_rows e INDEXED BY idx_search_events_kind_time
             CROSS JOIN intelligence_generations g ON g.profile_id=e.profile_id AND g.navigation=e.generation
             CROSS JOIN archive.visits v ON v.id=e.visit_id
             WHERE e.query_kind='keyword' AND e.visit_time_ms>=?1 AND e.visit_time_ms<?2 AND v.reverted_at IS NULL
               AND (?3 IS NULL OR e.profile_id=?3)
             ORDER BY e.visit_time_ms,e.visit_id",
            params![bounds.0, bounds.1, profile],
            |record| {
                if let Some(control) = &row_control {
                    control.checkpoint("reading period searches")
                        .map_err(|error| vault_store::Error::Operation(error.into_boxed_dyn_error()))?;
                }
                let query = record.get::<String>(0)?;
                let time = record.get::<i64>(2)?;
                let row = rows.entry(query.clone()).or_insert_with(|| Search {
                    query,
                    first: time,
                    ..Default::default()
                });
                row.wording = record.get(1)?;
                row.last = time;
                row.searches += 1;
                // Stream event evidence into a fixed-size digest: a busy month must not retain
                // millions of event copies just to detect a changed search or undo.
                events.update(record.get::<i64>(6)?.to_le_bytes());
                events.update(time.to_le_bytes());
                events.update(search_vectors::key(&row.query).to_le_bytes());
                for text in [record.get::<String>(1)?,record.get::<String>(3)?] {
                    events.update((text.len() as u64).to_le_bytes());
                    events.update(text.as_bytes());
                }
                row.days.insert(zone.date_key(time));
                if let Some(trail) = record.get::<Option<String>>(5)? {
                    row.trails.insert((record.get(3)?, record.get(4)?, trail));
                }
                Ok(())
            },
        )?;
        Ok((rows.into_values().collect::<Vec<_>>(),hex::encode(events.finalize())))
    })?;
    let mut candidates = rows
        .iter()
        .filter(|row| {
            !search_vectors::is_short(&row.query)
                && positions
                    .get(&search_vectors::key(&row.query))
                    .is_some_and(|&p| search_index.has_vector(p))
        })
        .collect::<Vec<_>>();
    candidates.sort_by(|a, b| {
        b.trails.len().cmp(&a.trails.len()).then(a.first.cmp(&b.first)).then(a.query.cmp(&b.query))
    });
    let trails = candidates.into_iter().take(4096).flat_map(|r| r.trails.iter().cloned()).collect();
    let opened = load_opened(vault, &trails, control)?;
    let mut page_positions = super::visit_position_map::PointLookup::open(
        vault.paths(), &provider.config.id, &provider.config.default_model, &page_index,
    )?.context("searches unavailable: the semantic index is being published; retry, or use intelligence_report report=\"search_trails\".")?;
    // No titles in the digest or persisted card. A title change matters only through the content
    // key selected by the published visit map. Sort hash-map entries for a deterministic digest.
    let mut evidence = opened
        .iter()
        .map(|(trail, visits)| {
            let mut pages = visits
                .iter()
                .map(|v| {
                    let position = page_positions.get(v.id)?;
                    let vector = position.map(|p| (page_index.key(p), page_index.has_vector(p)));
                    Ok((v.id, v.page, v.time, vector))
                })
                .collect::<Result<Vec<_>>>()?;
            pages.sort();
            Ok(((trail.0.clone(), trail.2.clone()), pages))
        })
        .collect::<Result<Vec<_>>>()?;
    evidence.sort();
    let indexed = rows
        .iter()
        .map(|row| {
            positions
                .get(&search_vectors::key(&row.query))
                .map(|&p| search_index.has_vector(p))
                .unwrap_or(false)
        })
        .collect::<Vec<_>>();
    let identity = super::search_group_signatures::identity(vault.paths(), provider, zone.name())?;
    // Navigation generations are publication guards, not grouping evidence. A rebuild of an
    // unchanged month must update its cheap signature without repeating linkage.
    let searches = rows
        .iter()
        .map(|r| {
            (
                &r.query,
                &r.wording,
                r.searches,
                r.first,
                r.last,
                &r.days,
                r.trails.iter().map(|(profile, _, id)| (profile, id)).collect::<Vec<_>>(),
            )
        })
        .collect::<Vec<_>>();
    let digest = crate::utils::sha256_hex(&serde_json::to_vec(&(
        identity,
        searches,
        events_digest,
        evidence,
        indexed,
    ))?);
    Ok(Prepared {
        page_index,
        search_index,
        positions,
        page_positions,
        generations,
        rows,
        opened,
        bounds,
        digest,
    })
}

pub(super) struct Grouped {
    pub report: Value,
    pub card: Vec<super::search_group_snapshots::SearchGroup>,
}

/// The same linkage and ordering feed both surfaces; only Ask hydrates its model-facing evidence.
#[allow(clippy::too_many_arguments)]
pub(super) fn build(
    vault: &Vault,
    provider: &AiProviderRuntime,
    input: Prepared,
    window: Value,
    requested: Option<&str>,
    control: Option<&Arc<dyn AiRunControl>>,
    time_zone: chrono_tz::Tz,
) -> Result<Grouped> {
    let Prepared {
        page_index,
        search_index,
        positions,
        mut page_positions,
        generations,
        rows,
        opened,
        bounds,
        ..
    } = input;
    let total = rows.len();
    let searches = rows.iter().map(|r| r.searches).sum::<u64>();
    let mut short = Vec::new();
    let mut not_indexed = Vec::new();
    let mut eligible = Vec::new();
    for row in rows {
        if search_vectors::is_short(&row.query) {
            short.push(row);
        } else if positions
            .get(&search_vectors::key(&row.query))
            .is_none_or(|&p| !search_index.has_vector(p))
        {
            not_indexed.push(row);
        } else {
            eligible.push(row);
        }
    }
    let omitted = limit(&mut eligible);
    let mut vectors = Vec::with_capacity(eligible.len());
    for row in &eligible {
        super::control::checkpoint_ai_run(control, "combining search and opened-page vectors")?;
        let mut vector = search_index.vector_at(positions[&search_vectors::key(&row.query)]);
        normalize(&mut vector);
        let top = group_pages(&row.trails, &opened);
        let mut page_mean = vec![0.; page_index.dim()];
        let mut count = 0;
        for (page, _) in top.iter().take(3) {
            // Different visits can carry different titles; take the newest indexed trail visit.
            let mut newest = None;
            for visit in row
                .trails
                .iter()
                .filter_map(|t| opened.get(t))
                .flatten()
                .filter(|visit| visit.page == page.page)
            {
                if let Some(position) = page_positions.get(visit.id)?
                    && newest.is_none_or(|(time, id, _)| (visit.time, visit.id) > (time, id))
                {
                    newest = Some((visit.time, visit.id, position));
                }
            }
            let position = newest.map(|(_, _, position)| position);
            if let Some(p) = position {
                let mut page_vector = page_index.vector_at(p);
                if normalize(&mut page_vector) {
                    for (a, b) in page_mean.iter_mut().zip(page_vector) {
                        *a += b;
                    }
                    count += 1;
                }
            }
        }
        if count > 0 {
            for (a, b) in vector.iter_mut().zip(page_mean) {
                *a += b / count as f32;
            }
            if !normalize(&mut vector) {
                vector = search_index.vector_at(positions[&search_vectors::key(&row.query)]);
                normalize(&mut vector);
            }
        }
        vectors.push(vector);
    }
    let mut groups = cluster(&vectors, &|| {
        super::control::checkpoint_ai_run(control, "grouping period searches")
    })?;
    let days = |members: &[usize]| {
        members.iter().flat_map(|&i| eligible[i].days.iter().cloned()).collect::<BTreeSet<_>>()
    };
    groups.sort_by(|a, b| {
        days(b)
            .len()
            .cmp(&days(a).len())
            .then(group_trails(&eligible, b).len().cmp(&group_trails(&eligible, a).len()))
            .then(
                a.iter()
                    .map(|&i| eligible[i].first)
                    .min()
                    .cmp(&b.iter().map(|&i| eligible[i].first).min()),
            )
            .then(a.cmp(b))
    });
    let requested = requested
        .map(|id| {
            id.strip_prefix('s')
                .and_then(|n| n.parse::<usize>().ok())
                .filter(|&n| n > 0 && n <= groups.len() && id == format!("s{n}"))
                .map(|n| n - 1)
                .ok_or_else(|| {
                    anyhow::anyhow!("unknown group {id}; list searches for the same dates first")
                })
        })
        .transpose()?;
    let excluded = |rows: &[Search]| {
        json!({
            "count": rows.len(),
            "searches": rows.iter().map(|row| row.searches).sum::<u64>(),
            "examples": rows.iter().take(5).map(|row| &row.wording).collect::<Vec<_>>(),
        })
    };
    let mut result = json!({
        "window": window,
        "model": provider.config.default_model,
        "distinctSearches": total,
        "searches": searches,
        "short": excluded(&short),
        "notIndexed": excluded(&not_indexed),
        "notGrouped": excluded(&omitted),
    });
    result["notGrouped"]["reason"] = json!(
        "A window groups at most 4,096 indexed searches; those with the fewest trails are left out."
    );
    if super::runtime_uses_static(provider) {
        result["note"] = json!(
            "The in-app static model groups searches by shared words more than meaning. Say so when it matters."
        );
    }
    let mut output = Vec::new();
    let mut card = Vec::new();
    for (number, members) in
        groups.iter().enumerate().filter(|(n, _)| requested.is_none_or(|r| r == *n))
    {
        let trails = group_trails(&eligible, members);
        let pages = group_pages(&trails, &opened);
        let dates = days(members);
        let mut wordings = members.clone();
        wordings.sort_by(|&a, &b| {
            eligible[b]
                .trails
                .len()
                .cmp(&eligible[a].trails.len())
                .then(eligible[b].last.cmp(&eligible[a].last))
                .then(eligible[a].query.cmp(&eligible[b].query))
        });
        if !pages.is_empty() || dates.len() > 1 {
            let mut card_wordings = wordings.clone();
            card_wordings.sort_by(|&a, &b| {
                eligible[b]
                    .searches
                    .cmp(&eligible[a].searches)
                    .then(eligible[b].trails.len().cmp(&eligible[a].trails.len()))
                    .then(eligible[b].last.cmp(&eligible[a].last))
                    .then(eligible[a].query.cmp(&eligible[b].query))
            });
            card.push(super::search_group_snapshots::SearchGroup {
                wordings: card_wordings
                    .iter()
                    .take(3)
                    .map(|&i| eligible[i].wording.clone())
                    .collect(),
                distinct_searches: wordings.len(),
                searches: members.iter().map(|&i| eligible[i].searches).sum(),
                trails: trails.len(),
                days: dates.iter().cloned().collect(),
                pages_opened: pages.len(),
                top_pages: pages.iter().take(5).map(|(page, _)| page.id).collect(),
            });
        }
        let mut sites = BTreeMap::<&str, u64>::new();
        for (page, visits) in &pages {
            *sites.entry(&page.site).or_default() += visits;
        }
        let mut sites = sites.into_iter().collect::<Vec<_>>();
        sites.sort_by(|a, b| b.1.cmp(&a.1).then(a.0.cmp(b.0)));
        let wording_rows = wordings
            .iter()
            .map(|&i| {
                let row = &eligible[i];
                json!({
                    "query": row.wording,
                    "normalizedQuery": row.query,
                    "trails": row.trails.len(),
                    "searches": row.searches,
                    "lastSearchLocal": super::agent_tools::format_local_ms(row.last, time_zone),
                })
            })
            .collect::<Vec<_>>();
        let page_limit = if requested.is_some() { usize::MAX } else { 5 };
        let page_rows = pages
            .iter()
            .take(page_limit)
            .map(|(page, visits)| {
                json!({
                    "historyId": page.id,
                    "title": page.title,
                    "url": page.url,
                    "visits": visits,
                })
            })
            .collect::<Vec<_>>();
        let mut group = json!({
            "id": format!("s{}", number + 1),
            "wordings": wording_rows,
            "trails": trails.len(),
            "searches": members.iter().map(|&i| eligible[i].searches).sum::<u64>(),
            "days": {"count": dates.len(), "first": dates.first(), "last": dates.last()},
            "sites": sites.into_iter().take(3)
                .map(|(site, visits)| json!({"site": site, "visits": visits})).collect::<Vec<_>>(),
            "pages": page_rows,
        });
        if requested.is_some() {
            group["searchEvents"] = load_events(
                vault,
                &wordings.iter().map(|&i| eligible[i].query.clone()).collect::<Vec<_>>(),
                bounds,
                time_zone,
            )?;
        }
        output.push(group);
    }
    result["groups"] = json!(output);
    // A changed position projection must not publish vectors combined from two page indexes.
    ensure!(
        page_positions.is_current(
            vault.paths(),
            &provider.config.id,
            &provider.config.default_model,
            &page_index
        )?,
        "search index changed while grouping; retry the same dates"
    );
    ensure!(
        generations == self::generations(vault)?,
        "Insights changed while grouping searches; retry the same dates"
    );
    Ok(Grouped { report: result, card })
}

pub(super) fn generations(vault: &Vault) -> Result<Vec<(String, i64)>> {
    vault.read(Db::Intelligence, |session| {
        Ok(session.query(
            "SELECT profile_id,navigation FROM intelligence_generations ORDER BY profile_id",
            (),
            |row| Ok((row.get(0)?, row.get(1)?)),
        )?)
    })
}

fn load_opened(
    vault: &Vault,
    trails: &BTreeSet<Trail>,
    control: Option<&Arc<dyn AiRunControl>>,
) -> Result<HashMap<Trail, Vec<Opened>>> {
    let mut opened = HashMap::<Trail, Vec<Opened>>::new();
    let trails = trails.iter().cloned().collect::<Vec<_>>();
    for chunk in trails.chunks(256) {
        super::control::checkpoint_ai_run(control, "reading search trail pages")?;
        let chunk = chunk.to_vec();
        // One primary-key seek per trail: a join against a JSON list of trails is not planned as
        // seeks and would read the whole member table for each one.
        let rows = vault.read(Db::Intelligence, move |session| {
            let mut rows = Vec::new();
            for trail in chunk {
                let (profile, generation, id) = trail.clone();
                for page in session.query(
                    "SELECT v.id,u.page_id,v.visit_time_ms,u.title,u.url,COALESCE(p.registrable_domain,'')
                     FROM search_trail_members_rows m
                     CROSS JOIN archive.visits v ON v.id=m.visit_id
                     CROSS JOIN archive.urls u ON u.id=v.url_id
                     CROSS JOIN archive.pages p ON p.id=u.page_id
                     WHERE m.generation=?1 AND m.trail_id=?2 AND m.profile_id=?3
                       AND m.role IN ('landing','click') AND v.reverted_at IS NULL",
                    params![generation, id, profile],
                    |row| Ok(Opened {
                        id: row.get(0)?, page: row.get(1)?, time: row.get(2)?,
                        title: row.get(3)?, url: row.get(4)?, site: row.get(5)?,
                    }),
                )? {
                    rows.push((trail.clone(), page));
                }
            }
            Ok(rows)
        })?;
        for (trail, page) in rows {
            opened.entry(trail).or_default().push(page);
        }
    }
    Ok(opened)
}
fn load_events(
    vault: &Vault,
    queries: &[String],
    bounds: (i64, i64),
    time_zone: chrono_tz::Tz,
) -> Result<Value> {
    let queries = queries.to_vec();
    // One index range per wording, for the same reason as `load_opened`.
    let mut rows = vault.read(Db::Intelligence, move |session| {
        let mut rows = Vec::new();
        for query in queries {
            rows.extend(session.query(
                "SELECT e.visit_id,e.raw_query,e.visit_time_ms,e.trail_id
                 FROM search_events_rows e INDEXED BY idx_search_events_query_time
                 CROSS JOIN intelligence_generations g ON g.profile_id=e.profile_id AND g.navigation=e.generation
                 CROSS JOIN archive.visits v ON v.id=e.visit_id
                 WHERE e.query_kind='keyword' AND e.normalized_query=?1
                   AND e.visit_time_ms>=?2 AND e.visit_time_ms<?3 AND v.reverted_at IS NULL",
                params![query, bounds.0, bounds.1],
                |row| Ok((row.get::<i64>(0)?, row.get::<String>(1)?, row.get::<i64>(2)?, row.get::<Option<String>>(3)?)),
            )?);
        }
        Ok(rows)
    })?;
    rows.sort_by_key(|(id, _, time, _)| (*time, *id));
    Ok(json!(
        rows.into_iter()
            .map(|(id, query, time, trail)| json!({
                "historyId": id,
                "query": query,
                "searchedAtLocal": super::agent_tools::format_local_ms(time, time_zone),
                "trailId": trail,
            }))
            .collect::<Vec<_>>()
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn cap_prefers_trails_and_reports_each_omission() {
        let mut rows = (0..4100)
            .map(|i| Search { query: format!("query {i:04}"), first: i, ..Default::default() })
            .collect::<Vec<_>>();
        rows[4099].trails.insert(("profile".into(), 1, "important".into()));
        let omitted = limit(&mut rows);
        assert_eq!(rows.len(), 4096);
        assert_eq!(omitted.len(), 4);
        assert!(rows.iter().any(|r| r.query == "query 4099"));
        let keys = rows.iter().chain(&omitted).map(|r| &r.query).collect::<BTreeSet<_>>();
        assert_eq!(keys.len(), 4100);
    }
    #[test]
    fn shared_trails_and_visits_count_once_within_each_group() {
        let trail = ("profile".into(), 1, "trail".into());
        let searches = vec![
            Search { trails: BTreeSet::from([trail.clone()]), ..Default::default() },
            Search { trails: BTreeSet::from([trail.clone()]), ..Default::default() },
        ];
        let trails = group_trails(&searches, &[0, 1]);
        assert_eq!(trails.len(), 1);
        assert_eq!(group_trails(&searches, &[0]).len(), 1);
        assert_eq!(group_trails(&searches, &[1]).len(), 1);
        let pages = HashMap::from([(
            trail,
            vec![Opened {
                id: 7,
                page: 2,
                time: 1,
                title: None,
                url: "https://example.com/".into(),
                site: "example.com".into(),
            }],
        )]);
        let opened = group_pages(&trails, &pages);
        assert_eq!(opened.len(), 1);
        assert_eq!(opened[0].1, 1);
    }
}
