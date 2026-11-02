//! Responsibilities: materialize month/rolling search groups after index and Insights builds;
//! serve kept windows without linkage, and custom/profile windows on the command's worker. A read
//! that finds windows not computed yet says so (`needs_warm`); the worker crate starts the warm.
//! Not responsible for: embeddings, archive schema, titles, or Ask's output format.
//! Failure modes: stale generations publish, two warmers race, undone imports survive, model/store
//! recreation or mapping-only passes leave old groups. Tests: search_group_snapshots_tests (written
//! first), semantic-index.spec.ts (real builds and card); search_groups_bench (complete timings).
use super::{
    AiProviderRuntime, AiRunControl, search_group_signatures, search_groups, search_vectors,
};
use crate::{
    DateRange, ScopedDateRangeRequest, Vault,
    intelligence::InsightsZone,
    models::{
        CoreIntelligenceSectionMeta, CoreIntelligenceSectionWindow, SectionEvidence,
        SectionEvidenceUnit,
    },
};
use anyhow::{Context, Result, ensure};
use chrono::{Datelike, Duration, Months, NaiveDate};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use vault_store::{Db, OptionalRow, Priority, Session, params_owned as params};

static WARMER: super::SemanticWorkGate = super::SemanticWorkGate::new();
static WARM_ERRORS: Mutex<Option<HashMap<PathBuf, String>>> = Mutex::new(None);

/// Only bounded display evidence is persisted; titles remain owned by the archive.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SearchGroup {
    pub wordings: Vec<String>,
    pub distinct_searches: usize,
    pub searches: u64,
    pub trails: usize,
    pub days: Vec<String>,
    pub pages_opened: usize,
    pub top_pages: Vec<i64>,
}
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct MonthGroups {
    pub groups: Vec<SearchGroup>,
    pub searches: u64,
    pub not_indexed: u64,
}
/// Every window keeps its own freshness, so one late month never looks empty.
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SearchGroupWindow {
    pub range: DateRange,
    pub updating: bool,
    pub groups: Vec<SearchGroup>,
}
/// The optional semantic card is separate from the deterministic overview batches.
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SearchGroupsCard {
    pub available: bool,
    pub static_model: bool,
    pub monthly: bool,
    pub data: Vec<SearchGroupWindow>,
    pub meta: Option<CoreIntelligenceSectionMeta>,
    /// Some shown window is not computed yet (a new day, an upgrade): the caller starts the warm
    /// on its background workers, where it is counted and stopped with them.
    #[serde(skip)]
    pub needs_warm: bool,
}
impl SearchGroupsCard {
    pub fn unavailable() -> Self {
        Self {
            available: false,
            static_model: false,
            monthly: false,
            data: Vec::new(),
            meta: None,
            needs_warm: false,
        }
    }
}
#[derive(Debug, Clone)]
pub(super) struct SnapshotScope {
    pub key: String,
    pub range: DateRange,
}
impl SnapshotScope {
    pub(super) fn month(month: &str) -> Result<Self> {
        let start = NaiveDate::parse_from_str(&format!("{month}-01"), "%Y-%m-%d")?;
        let end =
            start.checked_add_months(Months::new(1)).context("month overflow")? - Duration::days(1);
        Ok(Self {
            key: format!("search-groups:{month}"),
            range: DateRange { start: start.to_string(), end: end.to_string() },
        })
    }
    fn rolling(today: NaiveDate, days: i64) -> Self {
        Self {
            key: format!("search-groups:d{days}"),
            range: DateRange {
                start: (today - Duration::days(days - 1)).to_string(),
                end: today.to_string(),
            },
        }
    }
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub(super) struct Fingerprint {
    pub cheap: String,
    pub digest: String,
}
pub(super) struct Kept {
    pub fingerprint: Fingerprint,
    payload: MonthGroups,
    computed_at: String,
}
pub(super) fn needs_read(previous: Option<&Fingerprint>, cheap: &str) -> bool {
    previous.is_none_or(|f| f.cheap != cheap)
}
pub(super) fn needs_group(previous: Option<&Fingerprint>, digest: &str) -> bool {
    previous.is_none_or(|f| f.digest != digest)
}
pub(super) fn load(vault: &Vault, scope: &SnapshotScope) -> Result<Option<Kept>> {
    let key = scope.key.clone();
    vault.read(Db::Intelligence,move |s| {
        let row = s.query_one("SELECT fingerprint,payload_json,computed_at FROM intelligence_overview_snapshots WHERE scope_key=?1 AND band='all'",
            [key],|r| Ok((r.get::<String>(0)?,r.get::<String>(1)?,r.get::<String>(2)?))).optional()?;
        // Derived rows from an older format are cache misses, never archive failures.
        Ok(row.and_then(|(f,p,computed_at)| Some(Kept {fingerprint: serde_json::from_str(&f).ok()?,payload: serde_json::from_str(&p).ok()?,computed_at})))
    })
}
pub(super) fn save(
    s: &Session<'_>,
    scope: &SnapshotScope,
    fingerprint: &Fingerprint,
    payload: &MonthGroups,
) -> Result<()> {
    s.execute("INSERT INTO intelligence_overview_snapshots(scope_key,band,fingerprint,payload_json,computed_at) VALUES(?1,'all',?2,?3,?4)
        ON CONFLICT(scope_key,band) DO UPDATE SET fingerprint=excluded.fingerprint,payload_json=excluded.payload_json,computed_at=excluded.computed_at",
        params![scope.key,serde_json::to_string(fingerprint)?,serde_json::to_string(payload)?,crate::utils::now_rfc3339()])?;
    Ok(())
}
/// Check the serving generations on the writer itself, closing the read→write publication race.
pub(super) fn publish_if_current(
    vault: &Vault,
    scope: &SnapshotScope,
    fingerprint: &Fingerprint,
    payload: &MonthGroups,
    current: impl FnOnce(&Session<'_>) -> Result<String> + Send + 'static,
) -> Result<bool> {
    let (scope, fingerprint, payload) = (scope.clone(), fingerprint.clone(), payload.clone());
    vault.write(Db::Intelligence, Priority::Batch, move |s| {
        if current(s)? != fingerprint.cheap {
            return Ok(false);
        }
        save(s, &scope, &fingerprint, &payload)?;
        Ok(true)
    })
}
fn signature(vault: &Vault, provider: &AiProviderRuntime, scope: &SnapshotScope) -> Result<String> {
    let (paths, provider, range) = (vault.paths().clone(), provider.clone(), scope.range.clone());
    let files = search_group_signatures::files(&paths, &provider)?;
    let expected = vault.generation_lease.as_ref().map(|lease| lease.relative().to_owned());
    vault.read(Db::Intelligence, move |s| {
        ensure!(
            super::generations::pointer(s, &provider.config.id, &provider.config.default_model)?
                == expected,
            "search index changed while reading groups; retry"
        );
        search_group_signatures::cheap(s, &files, &provider, &range)
    })
}
fn compute(
    vault: &Vault,
    provider: &AiProviderRuntime,
    scope: &SnapshotScope,
    profile: Option<&str>,
    control: Option<&Arc<dyn AiRunControl>>,
) -> Result<MonthGroups> {
    let input = prepare(vault, provider, scope, profile, control)?;
    group(vault, provider, input, control)
}
fn prepare(
    vault: &Vault,
    provider: &AiProviderRuntime,
    scope: &SnapshotScope,
    profile: Option<&str>,
    control: Option<&Arc<dyn AiRunControl>>,
) -> Result<search_groups::Prepared> {
    let range = scope.range.clone();
    let (zone, bounds) = vault.read(Db::Intelligence, move |s| {
        let zone = InsightsZone::for_reading(s)?;
        Ok((zone, zone.range_bounds(&range)?))
    })?;
    search_groups::prepare(vault, provider, bounds, zone, profile, control)
}
fn group(
    vault: &Vault,
    provider: &AiProviderRuntime,
    input: search_groups::Prepared,
    control: Option<&Arc<dyn AiRunControl>>,
) -> Result<MonthGroups> {
    let result = search_groups::build(
        vault,
        provider,
        input,
        serde_json::Value::Null,
        None,
        control,
        chrono_tz::UTC,
    )?;
    Ok(MonthGroups {
        groups: result.card.into_iter().take(6).collect(),
        searches: result.report["searches"].as_u64().unwrap_or_default(),
        not_indexed: result.report["notIndexed"]["searches"].as_u64().unwrap_or_default(),
    })
}

fn months(start: NaiveDate, end: NaiveDate) -> Result<Vec<SnapshotScope>> {
    let mut day = start.with_day(1).context("month start")?;
    let mut scopes = Vec::new();
    while day <= end {
        scopes.push(SnapshotScope::month(&day.format("%Y-%m").to_string())?);
        day = day.checked_add_months(Months::new(1)).context("month overflow")?;
    }
    scopes.reverse();
    Ok(scopes)
}
fn archive_start(vault: &Vault, zone: InsightsZone) -> Result<NaiveDate> {
    let first = vault.read(Db::Intelligence,|s| Ok(s.query_one(
        "SELECT e.visit_time_ms FROM search_events_rows e INDEXED BY idx_search_events_kind_time
         CROSS JOIN intelligence_generations g ON g.profile_id=e.profile_id AND g.navigation=e.generation
         CROSS JOIN archive.visits v ON v.id=e.visit_id
         WHERE e.query_kind='keyword' AND v.reverted_at IS NULL ORDER BY e.visit_time_ms LIMIT 1",(),|r| Ok(r.get::<i64>(0)?)).optional()?))?;
    Ok(first.map_or(zone.today(), |ms| zone.datetime(ms).date_naive()))
}
/// Independent end-of-pass step. One warmer at a time; readers never wait for this admission token.
/// A changed cheap signature rereads inputs; an equal digest reuses the stored groups.
pub fn warm_search_group_snapshots(
    vault: &Vault,
    provider: &AiProviderRuntime,
    control: Option<&Arc<dyn AiRunControl>>,
) -> Result<()> {
    let result = warm(vault, provider, control);
    // The card shows a failed warm instead of "Updating" forever.
    let mut errors = WARM_ERRORS.lock().unwrap_or_else(|e| e.into_inner());
    let errors = errors.get_or_insert_with(HashMap::new);
    match &result {
        Ok(()) => {
            errors.remove(&vault.paths().app_root);
        }
        Err(error) => {
            errors.insert(vault.paths().app_root.clone(), format!("{error:#}"));
        }
    }
    result
}

fn warm(
    vault: &Vault,
    provider: &AiProviderRuntime,
    control: Option<&Arc<dyn AiRunControl>>,
) -> Result<()> {
    let _warming = WARMER.enter();
    let Some(pinned) = super::generations::serving_vault(
        vault,
        &provider.config.id,
        &provider.config.default_model,
    )?
    else {
        return Ok(());
    };
    let vault = &pinned;
    let zone = vault.read(Db::Intelligence, InsightsZone::for_reading)?;
    let index = search_vectors::available(vault.paths(), provider)?;
    if search_vectors::open(vault.paths(), provider, index.dim())?.is_empty() {
        return Ok(());
    }
    let mut scopes =
        vec![SnapshotScope::rolling(zone.today(), 7), SnapshotScope::rolling(zone.today(), 30)];
    scopes.extend(months(archive_start(vault, zone)?, zone.today())?);
    warm_scopes(vault, provider, control, scopes)
}

/// The complete recomputation step, kept measurable independently of archive age.
pub(super) fn warm_scopes(
    vault: &Vault,
    provider: &AiProviderRuntime,
    control: Option<&Arc<dyn AiRunControl>>,
    scopes: Vec<SnapshotScope>,
) -> Result<()> {
    let Some(pinned) = super::generations::serving_vault(
        vault,
        &provider.config.id,
        &provider.config.default_model,
    )?
    else {
        return Ok(());
    };
    let vault = &pinned;
    for scope in scopes {
        super::control::checkpoint_ai_run(control, "updating search groups")?;
        let cheap = signature(vault, provider, &scope)?;
        let previous = load(vault, &scope)?;
        if !needs_read(previous.as_ref().map(|r| &r.fingerprint), &cheap) {
            continue;
        }
        let input = prepare(vault, provider, &scope, None, control)?;
        let digest = input.digest.clone();
        let payload = if needs_group(previous.as_ref().map(|r| &r.fingerprint), &digest) {
            group(vault, provider, input, control)?
        } else {
            previous.unwrap().payload
        };
        let (paths, pair, range) = (vault.paths().clone(), provider.clone(), scope.range.clone());
        let files = search_group_signatures::files(&paths, &pair)?;
        publish_if_current(vault, &scope, &Fingerprint { cheap, digest }, &payload, move |s| {
            search_group_signatures::cheap(s, &files, &pair, &range)
        })?;
    }
    Ok(())
}

/// Only unkept short windows do linkage here. Desktop and dev commands run this off the UI thread.
pub fn search_groups_card(
    vault: &Vault,
    provider: Option<&AiProviderRuntime>,
    request: &ScopedDateRangeRequest,
) -> Result<SearchGroupsCard> {
    let Some(provider) = provider else { return Ok(SearchGroupsCard::unavailable()) };
    if !vault.config().ai.enabled || !vault.config().ai.semantic_index_enabled {
        return Ok(SearchGroupsCard::unavailable());
    }
    let Some(pinned) = super::generations::serving_vault(
        vault,
        &provider.config.id,
        &provider.config.default_model,
    )?
    else {
        return Ok(SearchGroupsCard::unavailable());
    };
    let vault = &pinned;
    let Ok(pages) = search_vectors::available(vault.paths(), provider) else {
        return Ok(SearchGroupsCard::unavailable());
    };
    if search_vectors::open(vault.paths(), provider, pages.dim())?.is_empty() {
        return Ok(SearchGroupsCard::unavailable());
    }
    let zone = vault.read(Db::Intelligence, InsightsZone::for_reading)?;
    let start = NaiveDate::parse_from_str(&request.date_range.start, "%Y-%m-%d")?;
    let end = NaiveDate::parse_from_str(&request.date_range.end, "%Y-%m-%d")?;
    ensure!(start <= end, "search groups range is reversed");
    let monthly = (end - start).num_days() >= 31;
    let scopes = if monthly {
        months(start.max(archive_start(vault, zone)?), end.min(zone.today()))?
    } else {
        vec![SnapshotScope { key: String::new(), range: request.date_range.clone() }]
    };
    let mut data = Vec::new();
    let mut searches = 0;
    let mut generated_at: Option<String> = None;
    let mut stale = false;
    for mut scope in scopes {
        if !monthly && request.profile_id.is_none() && end == zone.today() {
            let days = (end - start).num_days() + 1;
            if [7, 30].contains(&days) {
                scope = SnapshotScope::rolling(end, days);
            }
        }
        let kept = !scope.key.is_empty();
        let (payload, updating) = if kept {
            let cheap = signature(vault, provider, &scope)?;
            let row = load(vault, &scope)?;
            match row {
                Some(row) if !needs_read(Some(&row.fingerprint), &cheap) => {
                    if generated_at.as_ref().is_none_or(|at| row.computed_at < *at) {
                        generated_at = Some(row.computed_at);
                    }
                    (row.payload, false)
                }
                _ => (MonthGroups::default(), true),
            }
        } else {
            (compute(vault, provider, &scope, request.profile_id.as_deref(), None)?, false)
        };
        searches += payload.searches as i64;
        stale |= updating;
        data.push(SearchGroupWindow {
            range: scope.range,
            updating,
            groups: payload.groups.into_iter().take(if monthly { 3 } else { 6 }).collect(),
        });
    }
    if stale {
        let previous_error = WARM_ERRORS
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .as_ref()
            .and_then(|errors| errors.get(&vault.paths().app_root))
            .cloned();
        if let Some(error) = previous_error {
            anyhow::bail!("updating search groups: {error}");
        }
    }
    let meta_range = match (data.last(), data.first()) {
        (Some(last), Some(first)) if monthly => {
            DateRange { start: last.range.start.clone(), end: first.range.end.clone() }
        }
        _ => request.date_range.clone(),
    };
    // Keep the same module freshness as the deterministic search cards, even if a read-triggered
    // warm has already verified the still-serving generation while its successor is building.
    let mut meta = crate::intelligence_sections::build_core_intelligence_section_meta(
        vault,
        "search-groups",
        CoreIntelligenceSectionWindow::DateRange { date_range: meta_range },
        data.iter().all(|window| window.groups.is_empty()),
    )?;
    meta.generated_at = generated_at.or_else(|| (!stale).then(crate::utils::now_rfc3339));
    if stale && meta.state != "disabled" {
        meta.state = "stale".into();
    }
    meta.evidence = (!stale)
        .then_some(SectionEvidence { count: searches, unit: SectionEvidenceUnit::Searches });
    Ok(SearchGroupsCard {
        available: true,
        static_model: super::runtime_uses_static(provider),
        monthly,
        data,
        meta: Some(meta),
        needs_warm: stale,
    })
}
