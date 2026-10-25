//! Responsibilities: group a bounded period's indexed pages for Ask, preserving visit weights.
//! Not responsible for: naming subjects with a model, building indexes, or persisting topics.
//! Failure modes: a page belongs to two topics or none; hash/thread order changes results;
//! missing vectors disappear from counts; empty, single-page or identical-vector windows fail;
//! zero vectors or rounding at the threshold split identical pages; empty centres survive a pass.
//! Tests: edge cases of the clustering below; `semantic-index.spec.ts` for archive coverage,
//! missing vectors, labels, ordering, drill-down and the 31-day limit. Measurements:
//! `docs/architecture/research/2026-10-topics-of-a-period.md`.

use super::{FlatVectorIndex, agent_tools::AgentToolContext};
use anyhow::{Context, Result, ensure};
use serde_json::{Value, json};
use std::collections::{BTreeMap, HashMap};
use vault_store::{CancelToken, Db, params_owned as params};

#[derive(Debug)]
struct Page {
    id: i64,
    visits: u64,
    first: i64,
    last: i64,
    newest: i64,
    domain: String,
    position: Option<(i64, i64, usize)>,
}

/// Keeps topics independent of the Insights build and on the same serving model as Ask search.
pub(super) async fn report(
    context: &AgentToolContext,
    start: &str,
    end: &str,
    topic: Option<&str>,
) -> Result<Value> {
    let day = |value: &str| -> Result<chrono::NaiveDate> {
        let date = chrono::NaiveDate::parse_from_str(value, "%Y-%m-%d")
            .context("topics requires start_date and end_date in YYYY-MM-DD")?;
        ensure!(date.format("%Y-%m-%d").to_string() == value, "topics dates must be YYYY-MM-DD");
        Ok(date)
    };
    let (first, last) = (day(start)?, day(end)?);
    ensure!(last >= first, "topics end_date must be on or after start_date");
    ensure!((last - first).num_days() < 31, "topics covers at most 31 days; ask month by month");
    let range = crate::DateRange { start: start.into(), end: end.into() };
    let (bounds, zone) = context.vault.read(Db::Intelligence, move |s| {
        let zone = crate::intelligence::InsightsZone::for_reading(s)?;
        Ok((zone.range_bounds(&range)?, zone.name()))
    })?;
    let context = context.clone();
    let topic = topic.map(str::to_owned);
    let window = json!({"startDate":start,"endDate":end,"timeZone":zone});
    tokio::task::spawn_blocking(move || build(&context, bounds, window, topic.as_deref())).await?
}

fn unavailable(reason: &str) -> anyhow::Error {
    anyhow::anyhow!(
        "topics unavailable: {reason}. Use search_history or intelligence_report report=\"sessions\" instead."
    )
}

fn build(
    context: &AgentToolContext,
    (start, end): (i64, i64),
    window: Value,
    topic: Option<&str>,
) -> Result<Value> {
    let provider = context
        .embedding_provider
        .as_ref()
        .ok_or_else(|| unavailable("no embedding provider is configured"))?;
    super::validate_provider(provider, crate::AiProviderPurpose::Embedding)
        .map_err(|error| unavailable(&error.to_string()))?;
    let lease = super::generations::resolve(
        &context.vault,
        &provider.config.id,
        &provider.config.default_model,
    )?
    .ok_or_else(|| unavailable("the semantic index is empty"))?;
    let paths = lease.paths();
    let index =
        FlatVectorIndex::open(paths, &provider.config.id, &provider.config.default_model)
            .map_err(|error| unavailable(&format!("the semantic index cannot be read: {error}")))?;
    ensure!(
        !index.is_empty(),
        unavailable("the semantic index is empty; build it in Settings → AI")
    );
    let engine = super::embedding_engine::select_embedding_provider(&context.paths, provider)
        .map_err(|error| unavailable(&error.to_string()))?;
    if super::search::semantic_config_drift_reason(paths, provider, &engine, &index, index.dim())?
        .is_some()
    {
        return Err(unavailable(
            "the semantic index has drifted from this model; rebuild it in Settings → AI",
        ));
    }
    let cancel = CancelToken::new();
    let positions = super::visit_position_map::load(
        paths,
        &provider.config.id,
        &provider.config.default_model,
        &index,
        &cancel,
    )?;
    let pages = load_pages(context, (start, end), positions)?;
    let total_pages = pages.len();
    let total_visits = pages.iter().map(|p| p.visits).sum::<u64>();
    let mut pages = pages;
    pages.sort_unstable_by(|a, b| b.visits.cmp(&a.visits).then(a.id.cmp(&b.id)));
    let mut vectors = Vec::new();
    pages.retain(|page| {
        let Some((_, _, position)) = page.position else { return false };
        let mut vector = index.vector_at(position);
        if !normalize(&mut vector) {
            return false;
        }
        vectors.push(vector);
        true
    });
    let not_indexed = total_pages - pages.len();
    let weights = pages.iter().map(|p| p.visits).collect::<Vec<_>>();
    // The release month probe stays below two seconds on one worker; see the research note.
    let assignments = cluster(&vectors, &weights, 1, &|| {
        super::control::checkpoint_ai_run(context.run_control.as_ref(), "clustering period topics")
    })?;
    super::control::checkpoint_ai_run(context.run_control.as_ref(), "finishing period topics")?;
    let topics = summarize(&pages, &vectors, &weights, &assignments);
    let requested = topic
        .map(|id| {
            id.strip_prefix('t')
                .and_then(|n| n.parse::<usize>().ok())
                .filter(|&n| n > 0 && n <= topics.len() && id == format!("t{n}"))
                .map(|n| n - 1)
                .ok_or_else(|| {
                    anyhow::anyhow!("unknown topic {id}; list topics for the same dates first")
                })
        })
        .transpose()?;
    let mut shown = std::collections::BTreeSet::new();
    for (number, topic) in topics.iter().enumerate() {
        let members = &topic.members;
        if requested.is_none() {
            shown.insert(topic.label);
            shown.extend(members.iter().take(5).copied());
        } else if requested == Some(number) {
            shown.extend(members.iter().copied());
        }
    }
    let ids = shown.iter().map(|&i| pages[i].newest).collect::<Vec<_>>();
    let hydrated = crate::search::visits(&context.vault, &ids)?
        .into_iter()
        .map(|visit| (visit.id, visit))
        .collect::<HashMap<_, _>>();
    let page_json = |i: usize| -> Result<Value> {
        let page = &pages[i];
        let visit = hydrated
            .get(&page.newest)
            .context("period topics changed while reading; retry for the same dates")?;
        Ok(json!({
            "historyId": page.newest,
            "title": visit.title,
            "url": visit.url,
            "visits": page.visits,
            "lastVisitLocal": super::agent_tools::format_local_ms(page.last, context.time_zone),
        }))
    };
    let mut result = json!({
        "window": window,
        "model": provider.config.default_model,
        "pages": total_pages,
        "visits": total_visits,
        "notIndexed": not_indexed,
    });
    // The static model's vectors weigh the site and address words heavily (measured in the topics
    // research note), so the model should not present a site-wide group as one subject.
    if super::runtime_uses_static(provider) {
        result["note"] = json!(
            "Grouped by the in-app static embedding model: pages of one site tend to fall into one topic even when their subjects differ. Say so when it matters, and suggest a contextual model (Settings → AI) for topics by subject."
        );
    }
    if let Some(number) = requested {
        result["topic"] = json!(format!("t{}", number + 1));
        result["topicPages"] =
            topics[number].members.iter().map(|&i| page_json(i)).collect::<Result<Value>>()?;
    } else {
        result["topics"] = topics.iter().enumerate().map(|(number, topic)| {
            let mut sites = BTreeMap::<&str, u64>::new();
            for &i in &topic.members {
                *sites.entry(&pages[i].domain).or_default() += pages[i].visits;
            }
            let mut sites = sites.into_iter().collect::<Vec<_>>();
            sites.sort_unstable_by(|a, b| b.1.cmp(&a.1).then(a.0.cmp(b.0)));
            let label = page_json(topic.label)?;
            Ok(json!({
                "id": format!("t{}", number + 1),
                "label": label["title"],
                "pages": topic.members.len(),
                "visits": topic.visits,
                "firstVisitLocal": super::agent_tools::format_local_ms(topic.first, context.time_zone),
                "lastVisitLocal": super::agent_tools::format_local_ms(topic.last, context.time_zone),
                "sites": sites.into_iter().take(3)
                    .map(|(site, visits)| json!({"site": site, "visits": visits})).collect::<Vec<_>>(),
                "examples": topic.members.iter().take(5)
                    .map(|&i| page_json(i)).collect::<Result<Vec<_>>>()?,
            }))
        }).collect::<Result<Value>>()?;
    }
    Ok(result)
}

struct Topic {
    members: Vec<usize>,
    label: usize,
    visits: u64,
    first: i64,
    last: i64,
    smallest_page: i64,
}

fn summarize(
    pages: &[Page],
    vectors: &[Vec<f32>],
    weights: &[u64],
    assignments: &[usize],
) -> Vec<Topic> {
    let groups = groups(assignments);
    let centres = centres(vectors, weights, &groups);
    let mut topics = groups
        .into_iter()
        .zip(centres)
        .map(|(mut members, centre)| {
            let label = *members
                .iter()
                .min_by(|&&a, &&b| {
                    dot(&vectors[b], &centre)
                        .total_cmp(&dot(&vectors[a], &centre))
                        .then(pages[b].visits.cmp(&pages[a].visits))
                        .then(pages[a].id.cmp(&pages[b].id))
                })
                .unwrap();
            members.sort_unstable_by(|&a, &b| {
                pages[b].visits.cmp(&pages[a].visits).then(pages[a].id.cmp(&pages[b].id))
            });
            Topic {
                label,
                visits: members.iter().map(|&i| pages[i].visits).sum(),
                first: members.iter().map(|&i| pages[i].first).min().unwrap(),
                last: members.iter().map(|&i| pages[i].last).max().unwrap(),
                smallest_page: members.iter().map(|&i| pages[i].id).min().unwrap(),
                members,
            }
        })
        .collect::<Vec<_>>();
    topics.sort_unstable_by(|a, b| {
        b.visits
            .cmp(&a.visits)
            .then(b.members.len().cmp(&a.members.len()))
            .then(a.smallest_page.cmp(&b.smallest_page))
    });
    topics
}

fn load_pages(
    context: &AgentToolContext,
    (start, end): (i64, i64),
    positions: Vec<u32>,
) -> Result<Vec<Page>> {
    let control = context.run_control.clone();
    context.vault.read(Db::Archive, move |s| {
        let mut pages = HashMap::<i64, Page>::new();
        s.for_each(
            "SELECT v.id,u.page_id,v.visit_time_ms,COALESCE(p.registrable_domain,'')
             FROM visits v INDEXED BY idx_visits_visible_time_id
             CROSS JOIN urls u ON u.id=v.url_id CROSS JOIN pages p ON p.id=u.page_id
             WHERE v.reverted_at IS NULL AND v.visit_time_ms>=?1 AND v.visit_time_ms<?2",
            params![start, end],
            |r| {
                if let Some(control) = &control {
                    control
                        .checkpoint("reading period topics")
                        .map_err(|e| vault_store::Error::Operation(e.into_boxed_dyn_error()))?;
                }
                let (id, page, time) = (r.get::<i64>(0)?, r.get::<i64>(1)?, r.get::<i64>(2)?);
                let page = match pages.entry(page) {
                    std::collections::hash_map::Entry::Occupied(entry) => entry.into_mut(),
                    std::collections::hash_map::Entry::Vacant(entry) => entry.insert(Page {
                        id: page,
                        visits: 0,
                        first: time,
                        last: time,
                        newest: id,
                        domain: r.get(3)?,
                        position: None,
                    }),
                };
                page.visits += 1;
                page.first = page.first.min(time);
                if (time, id) > (page.last, page.newest) {
                    page.last = time;
                    page.newest = id;
                }
                if let Some(&position) = positions.get(id as usize)
                    && position != super::visit_position_map::ABSENT
                    && page.position.is_none_or(|(at, visit, _)| (time, id) > (at, visit))
                {
                    page.position = Some((time, id, position as usize));
                }
                Ok(())
            },
        )?;
        Ok(pages.into_values().collect::<Vec<_>>())
    })
}

fn normalize(vector: &mut [f32]) -> bool {
    let norm = dot(vector, vector).sqrt();
    if !norm.is_finite() || norm <= 0. {
        return false;
    }
    for value in vector {
        *value /= norm;
    }
    true
}

fn dot(a: &[f32], b: &[f32]) -> f32 {
    // Independent accumulators allow vectorisation without changing the thread-dependent order.
    let mut sums = [0.; 8];
    let mut left = a.chunks_exact(8);
    let mut right = b.chunks_exact(8);
    for (a, b) in left.by_ref().zip(right.by_ref()) {
        for lane in 0..8 {
            sums[lane] += a[lane] * b[lane];
        }
    }
    let mut sum = sums.into_iter().sum::<f32>();
    for (a, b) in left.remainder().iter().zip(right.remainder()) {
        sum += a * b;
    }
    sum
}

fn threshold(vectors: &[Vec<f32>]) -> f32 {
    let mut seed = 0x706174686b656570_u64;
    let mut next = || {
        seed ^= seed << 13;
        seed ^= seed >> 7;
        seed ^= seed << 17;
        seed
    };
    let mut similarities = (0..20_000)
        .map(|_| {
            let a = (next() % vectors.len() as u64) as usize;
            let b = (a + 1 + (next() % (vectors.len() - 1) as u64) as usize) % vectors.len();
            dot(&vectors[a], &vectors[b])
        })
        .collect::<Vec<_>>();
    let percentile = (similarities.len() * 95).div_ceil(100) - 1;
    *similarities.select_nth_unstable_by(percentile, f32::total_cmp).1
}

fn nearest(vector: &[f32], centres: &[Vec<f32>], threshold: f32) -> Option<usize> {
    let mut best = None;
    let mut similarity = f32::NEG_INFINITY;
    for (i, centre) in centres.iter().enumerate() {
        let score = dot(vector, centre);
        if score > similarity {
            similarity = score;
            best = Some(i);
        }
    }
    // Renormalising the same vector can move its cosine by a few ulps at 1.
    (similarity + 8. * f32::EPSILON >= threshold).then_some(best).flatten()
}

fn groups(assignments: &[usize]) -> Vec<Vec<usize>> {
    let mut groups = vec![Vec::new(); assignments.iter().max().map_or(0, |id| id + 1)];
    for (i, &id) in assignments.iter().enumerate() {
        groups[id].push(i);
    }
    groups.into_iter().filter(|members| !members.is_empty()).collect()
}

fn centres(vectors: &[Vec<f32>], weights: &[u64], groups: &[Vec<usize>]) -> Vec<Vec<f32>> {
    groups
        .iter()
        .map(|members| {
            let mut centre = vec![0.; vectors[0].len()];
            for &i in members {
                for (c, v) in centre.iter_mut().zip(&vectors[i]) {
                    *c += v * weights[i] as f32;
                }
            }
            // Antipodal members can cancel; the first member is a deterministic direction then.
            if !normalize(&mut centre) {
                centre.clone_from(&vectors[members[0]]);
            }
            centre
        })
        .collect()
}

fn cluster(
    vectors: &[Vec<f32>],
    weights: &[u64],
    threads: usize,
    cancel: &(dyn Fn() -> Result<()> + Sync),
) -> Result<Vec<usize>> {
    cancel()?;
    if vectors.len() < 2 {
        return Ok(vec![0; vectors.len()]);
    }
    let threshold = threshold(vectors);
    let mut centres_now = Vec::new();
    let mut assignments = Vec::with_capacity(vectors.len());
    for vector in vectors {
        cancel()?;
        assignments.push(nearest(vector, &centres_now, threshold).unwrap_or_else(|| {
            centres_now.push(vector.clone());
            centres_now.len() - 1
        }));
    }
    for _ in 0..3 {
        cancel()?;
        centres_now = centres(vectors, weights, &groups(&assignments));
        let chunk = vectors.len().div_ceil(threads.clamp(1, 4));
        std::thread::scope(|scope| -> Result<()> {
            let handles = assignments
                .chunks_mut(chunk)
                .zip(vectors.chunks(chunk))
                .map(|(out, vectors)| {
                    let centres = &centres_now;
                    scope.spawn(move || -> Result<()> {
                        for (id, vector) in out.iter_mut().zip(vectors) {
                            cancel()?;
                            *id = nearest(vector, centres, threshold).unwrap_or(usize::MAX);
                        }
                        Ok(())
                    })
                })
                .collect::<Vec<_>>();
            for handle in handles {
                handle.join().expect("topic worker panicked")?;
            }
            Ok(())
        })?;
        let mut next = centres_now.len();
        for id in &mut assignments {
            if *id == usize::MAX {
                *id = next;
                next += 1;
            }
        }
        // First encounter order makes empty-centre removal and singleton IDs independent of workers.
        let mut ids = HashMap::new();
        for id in &mut assignments {
            let next = ids.len();
            *id = *ids.entry(*id).or_insert(next);
        }
    }
    Ok(assignments)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_single_and_identical_pages_keep_their_membership() {
        let cancel = vault_store::CancelToken::new();
        assert!(cluster(&[], &[], 1, &|| Ok(cancel.check()?)).unwrap().is_empty());
        assert_eq!(cluster(&[vec![1., 0.]], &[7], 1, &|| Ok(cancel.check()?)).unwrap(), [0]);
        // Non-axis-aligned unit vectors expose rounding at cosine 1.
        let mut vector = vec![1., 2., 3., 4., 5.];
        normalize(&mut vector);
        let vectors = vec![vector; 31];
        assert_eq!(cluster(&vectors, &[3; 31], 1, &|| Ok(cancel.check()?)).unwrap(), vec![0; 31]);
    }

    #[test]
    fn refinement_is_a_repeatable_partition_even_with_empty_centres() {
        let cancel = vault_store::CancelToken::new();
        let vectors = (0..137)
            .map(|i| {
                let mut vector = vec![(i % 7) as f32 - 3., (i % 11) as f32 - 5., 1.];
                normalize(&mut vector);
                vector
            })
            .collect::<Vec<_>>();
        let weights = (0..137).map(|i| 138 - i).collect::<Vec<_>>();
        let single = cluster(&vectors, &weights, 1, &|| Ok(cancel.check()?)).unwrap();
        assert_eq!(single, cluster(&vectors, &weights, 4, &|| Ok(cancel.check()?)).unwrap());
        assert_eq!(single, cluster(&vectors, &weights, 1, &|| Ok(cancel.check()?)).unwrap());
        assert_eq!(single.len(), vectors.len());
        let topics = single.iter().copied().collect::<std::collections::BTreeSet<_>>();
        assert_eq!(
            topics.into_iter().collect::<Vec<_>>(),
            (0..=*single.iter().max().unwrap()).collect::<Vec<_>>()
        );
        let mut zero = vec![0.; 3];
        assert!(!normalize(&mut zero), "zero vectors are counted as not indexed");
        cancel.cancel();
        assert!(cluster(&vectors, &weights, 4, &|| Ok(cancel.check()?)).is_err());
    }

    #[test]
    #[ignore = "release measurement; PATHKEEP_TOPICS_BENCH=1"]
    fn month_clustering_measurement() {
        if std::env::var("PATHKEEP_TOPICS_BENCH").as_deref() != Ok("1") {
            return;
        }
        if cfg!(debug_assertions) {
            panic!("measure with --release");
        }
        let mut seed = 73_u64;
        let mut random = || {
            seed ^= seed << 13;
            seed ^= seed >> 7;
            seed ^= seed << 17;
            (seed >> 40) as f32 / (1_u32 << 24) as f32 - 0.5
        };
        for dim in [256, 1024] {
            // 240 recurring subjects, each with independent page variation, and repeat visits.
            let directions = (0..240)
                .map(|_| {
                    let mut v = (0..dim).map(|_| random()).collect::<Vec<_>>();
                    normalize(&mut v);
                    v
                })
                .collect::<Vec<_>>();
            let vectors = (0..60_000)
                .map(|i| {
                    let mut v = directions[i % directions.len()]
                        .iter()
                        .map(|x| x + random() / (dim as f32).sqrt())
                        .collect::<Vec<_>>();
                    normalize(&mut v);
                    v
                })
                .collect::<Vec<_>>();
            let weights = (0..60_000).map(|i| 1 + (60_000 - i) / 1000).collect::<Vec<_>>();
            let mut reference = None;
            for threads in [1, 4] {
                let start = std::time::Instant::now();
                let assignments = cluster(&vectors, &weights, threads, &|| Ok(())).unwrap();
                eprintln!(
                    "topics: pages=60000 dim={dim} threads={threads} topics={} seconds={:.3}",
                    assignments.iter().max().unwrap() + 1,
                    start.elapsed().as_secs_f64()
                );
                if let Some(reference) = &reference {
                    assert_eq!(&assignments, reference);
                } else {
                    reference = Some(assignments);
                }
            }
        }
    }
}
