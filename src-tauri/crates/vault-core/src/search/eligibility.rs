//! Responsibilities: canonical same-visit and text constraints, membership and hydration SQL.
//! Not responsible for: posting retrieval, ranking, indexing or opening databases.
//! Failure modes: dates and profiles match different visits; revokes leak from stale
//! index snapshots; wildcard input changes filter meaning; broad filters sort the archive.
//! Tests: search::tests and the plan assertions below.
use super::{SearchHit, SearchRequest};
use crate::archive::search_query::ParsedHistorySearchQuery;
use std::collections::HashMap;
use vault_store::{Db, Result, Session, Store, Value};
pub(super) const PAGE_VISITS: &str = "pages p CROSS JOIN urls u ON u.page_id=p.id CROSS JOIN visits v INDEXED BY idx_visits_visible_url_time ON v.url_id=u.id JOIN source_profiles sp ON sp.id=v.source_profile_id";

#[derive(Clone)]
pub(super) struct Scope {
    pub sql: String,
    pub params: Vec<Value>,
}
impl Scope {
    pub fn new(request: &SearchRequest, parsed: &ParsedHistorySearchQuery) -> Self {
        let mut scope = Self { sql: "v.reverted_at IS NULL".into(), params: Vec::new() };
        let start = request.start_time_ms.into_iter().chain(parsed.after_ms).max();
        let end = request.end_time_ms.into_iter().chain(parsed.before_ms).min();
        if let Some(time) = start {
            scope.bind("v.visit_time_ms>=", Value::Integer(time));
        }
        if let Some(time) = end {
            scope.bind("v.visit_time_ms<=", Value::Integer(time));
        }
        if let Some(id) = &request.profile_id {
            scope.bind("sp.profile_key=", Value::Text(id.clone()));
        }
        if let Some(kind) = &request.browser_kind {
            scope.bind("sp.browser_kind=", Value::Text(kind.clone()));
        }
        if let Some(domain) = &request.domain {
            scope.contains("lower(p.url)", &domain.to_lowercase(), false);
        }
        for (values, negative) in [(&parsed.required_sites, false), (&parsed.excluded_sites, true)]
        {
            for value in values {
                scope.contains("lower(p.url)", value, negative);
            }
        }
        for (values, negative) in [(&parsed.required_tags, false), (&parsed.excluded_tags, true)] {
            for value in values {
                scope.exists(
                    "SELECT 1 FROM url_tags t WHERE t.url=p.url AND lower(t.tag)=",
                    value,
                    negative,
                );
            }
        }
        for (values, negative) in [(&parsed.required_notes, false), (&parsed.excluded_notes, true)]
        {
            for value in values {
                let index = scope.param(Value::Text(value.clone()));
                scope.sql.push_str(&format!(" AND {}EXISTS(SELECT 1 FROM url_annotations a WHERE a.url=p.url AND instr(lower(a.notes),?{index})>0)",if negative{"NOT "}else{""}));
            }
        }
        for (values, negative) in
            [(&parsed.required_filetypes, false), (&parsed.excluded_filetypes, true)]
        {
            for ext in values {
                let index = scope.param(Value::Text(format!(".{ext}")));
                scope.sql.push_str(&format!(" AND {}(lower(p.url) LIKE '%'||?{index} OR instr(lower(p.url),?{index}||'?')>0 OR instr(lower(p.url),?{index}||'#')>0)",if negative{"NOT "}else{""}));
            }
        }
        for (columns, positive, negative) in [
            (
                ["title", "normalized_title"],
                &parsed.required_title_terms,
                &parsed.excluded_title_terms,
            ),
            (["url", "normalized_url"], &parsed.required_url_terms, &parsed.excluded_url_terms),
        ] {
            for value in positive {
                scope.document_text(&columns, value, false);
            }
            for value in negative {
                scope.document_text(&columns, value, true);
            }
        }
        for value in &parsed.excluded_terms {
            scope.document_text(
                &[
                    "url",
                    "title",
                    "normalized_url",
                    "normalized_title",
                    "normalized_search_terms",
                    "compact_text",
                ],
                value,
                true,
            );
        }
        if request.starred {
            scope.sql.push_str(" AND EXISTS(SELECT 1 FROM star st WHERE (st.entity_kind='url' AND st.entity_key=p.canonical_url) OR (st.entity_kind='domain' AND st.entity_key=p.registrable_domain))");
        }
        scope
    }
    fn document_text(&mut self, columns: &[&str], value: &str, negative: bool) {
        let index = self.param(Value::Text(value.into()));
        let terms = columns
            .iter()
            .map(|column| format!("instr(lower(d.{column}),?{index})>0"))
            .collect::<Vec<_>>()
            .join(" OR ");
        self.sql.push_str(&format!(
            " AND {}EXISTS(SELECT 1 FROM search_documents d WHERE d.page_id=p.id AND ({terms}))",
            if negative { "NOT " } else { "" }
        ));
    }
    pub(super) fn pin_visits(&mut self, through: i64) {
        self.bind("v.id<=", Value::Integer(through));
    }
    fn param(&mut self, value: Value) -> usize {
        self.params.push(value);
        self.params.len()
    }
    fn bind(&mut self, left: &str, value: Value) {
        let index = self.param(value);
        self.sql.push_str(&format!(" AND {left}?{index}"));
    }
    fn contains(&mut self, field: &str, value: &str, negative: bool) {
        let index = self.param(Value::Text(value.into()));
        self.sql.push_str(&format!(
            " AND instr({field},?{index}){}0",
            if negative { "=" } else { ">" }
        ));
    }
    fn exists(&mut self, select: &str, value: &str, negative: bool) {
        let index = self.param(Value::Text(value.into()));
        self.sql.push_str(&format!(
            " AND {}EXISTS({select}?{index})",
            if negative { "NOT " } else { "" }
        ));
    }
    pub fn from(&self) -> &'static str {
        if self.sql.contains("sp.profile_key=") || self.sql.contains("sp.browser_kind=") {
            "source_profiles sp CROSS JOIN visits v INDEXED BY idx_visits_visible_profile_time_id ON v.source_profile_id=sp.id CROSS JOIN urls u ON u.id=v.url_id CROSS JOIN pages p ON p.id=u.page_id"
        } else {
            "visits v INDEXED BY idx_visits_visible_time_id CROSS JOIN urls u ON u.id=v.url_id CROSS JOIN pages p ON p.id=u.page_id JOIN source_profiles sp ON sp.id=v.source_profile_id"
        }
    }
}

/// What a session matched against, so hydration can recheck pages written after its snapshot.
pub(super) struct TextCheck {
    pub ast: vault_search::Ast,
    pub fuzzy: Option<(crate::archive::search_lexical::FuzzyQuery, vault_search::Ast)>,
    pub indexed_seq: u64,
}

pub(super) struct Membership {
    bits: Vec<u64>,
    checked: Vec<u64>,
    complete: bool,
    newest: Vec<i64>,
    /// Each page's first visit in scope, kept only for the oldest-first order: another
    /// entry per page of the archive (2.9M at 14.4M visits) that no other order reads.
    first: Option<Vec<i64>>,
    counts: Vec<u64>,
}
impl Membership {
    fn new(max: usize, first: bool) -> Self {
        Self {
            bits: vec![0; max / 64 + 1],
            checked: vec![0; max / 64 + 1],
            complete: false,
            newest: vec![0; max + 1],
            first: first.then(|| vec![0; max + 1]),
            counts: vec![0; max + 1],
        }
    }
    fn insert(&mut self, id: usize, time: i64) {
        self.counts[id] += 1;
        let new = !self.contains(id as u64);
        if new || self.newest[id] < time {
            self.newest[id] = time;
        }
        if let Some(first) = &mut self.first
            && (new || first[id] > time)
        {
            first[id] = time;
        }
        self.bits[id / 64] |= 1u64 << (id % 64);
    }
    pub(super) fn retain_verified_membership(&mut self) {
        self.complete = false;
        self.checked.clone_from(&self.bits);
    }
    pub fn contains(&self, id: u64) -> bool {
        self.bits.get(id as usize / 64).is_some_and(|word| word & (1u64 << (id % 64)) != 0)
    }
    pub fn count(&self, id: u64) -> u64 {
        self.counts.get(id as usize).copied().unwrap_or(0)
    }
    pub fn newest(&self, id: u64) -> i64 {
        self.newest.get(id as usize).copied().unwrap_or(0)
    }
    /// The page's first visit in scope; its newest when first visits are not kept.
    pub fn first(&self, id: u64) -> i64 {
        match &self.first {
            Some(first) => first.get(id as usize).copied().unwrap_or(0),
            None => self.newest(id),
        }
    }
}
fn maximum_page_id(s: &Session<'_>) -> Result<usize> {
    Ok(s.query("SELECT id FROM pages ORDER BY id DESC LIMIT 1", (), |r| {
        Ok(r.get::<i64>(0)? as usize)
    })?
    .pop()
    .unwrap_or(0))
}

pub(super) fn load(
    store: &Store,
    scope: Scope,
    first: bool,
    cancel: vault_store::CancelToken,
) -> Result<Membership> {
    store.read(Db::Archive, move |s| {
        let max = maximum_page_id(s)?;
        let mut membership = Membership::new(max, first);
        s.for_each(
            &format!("SELECT u.page_id,v.visit_time_ms FROM {} WHERE {}", scope.from(), scope.sql),
            scope.params,
            |r| {
                cancel.check()?;
                membership.insert(r.get::<i64>(0)? as usize, r.get(1)?);
                Ok(())
            },
        )?;
        membership.complete = true;
        Ok(membership)
    })
}

/// Pages among `ids` whose current text no longer matches. The index snapshot a
/// session reads can predate a revoke, so its matches are checked against the
/// canonical documents whenever the archive has moved past that snapshot.
fn text_mismatches(
    s: &Session<'_>,
    ids: &[i64],
    ast: &vault_search::Ast,
    fuzzy: Option<&(crate::archive::search_lexical::FuzzyQuery, vault_search::Ast)>,
) -> Result<Vec<u64>> {
    let mut valid = std::collections::HashSet::new();
    s.for_each(
        &format!(
            "SELECT {} FROM search_documents d WHERE d.page_id IN ({}) AND d.newest_visit_ms IS NOT NULL",
            super::documents::COLUMNS,
            vec!["?"; ids.len()].join(",")
        ),
        ids.to_vec(),
        |row| {
            let doc = super::documents::document(row)?;
            let matches = super::query::matches_document(ast, &doc)
                && fuzzy.is_none_or(|(fuzzy, exact)| {
                    super::query::matches_document(exact, &doc)
                        || fuzzy
                            .score_document(&crate::archive::search_lexical::FuzzyDocument {
                                normalized_url: &doc.text[vault_search::TextField::NormalizedUrl as usize],
                                normalized_title: &doc.text
                                    [vault_search::TextField::NormalizedTitle as usize],
                                normalized_search_terms: &doc.text
                                    [vault_search::TextField::NormalizedSearchTerms as usize],
                                compact_text: &crate::archive::search_lexical::normalized_tokens(
                                    &doc.compact_text,
                                ),
                            })
                            .is_some()
                });
            if matches {
                valid.insert(doc.page_id);
            }
            Ok(())
        },
    )?;
    Ok(ids.iter().map(|id| *id as u64).filter(|id| !valid.contains(id)).collect())
}

/// Each page's row shows its newest visit in scope, or its first when `first` (oldest first).
pub(super) fn hydrate(
    s: &Session<'_>,
    scope: &Scope,
    hits: &[vault_search::Hit],
    text: &TextCheck,
    first: bool,
) -> Result<Vec<SearchHit>> {
    if hits.is_empty() {
        return Ok(Vec::new());
    }
    let mut params = scope.params.clone();
    let keys = hits
        .iter()
        .map(|h| {
            params.push(Value::Integer(h.page_id as i64));
            format!("?{}", params.len())
        })
        .collect::<Vec<_>>()
        .join(",");
    let sql = format!(
        "SELECT p.id,v.id,sp.profile_key,p.url,u.title,v.visit_time_ms,v.visit_time_iso,v.visit_duration_ms,v.transition_type,v.source_visit_id,v.app_id,d.enrichment_text FROM {PAGE_VISITS} LEFT JOIN search_documents d ON d.page_id=p.id WHERE p.id IN ({keys}) AND {}",
        scope.sql
    );
    let mut rows: HashMap<i64, crate::HistoryEntry> = HashMap::new();
    s.for_each(&sql, params, |r| {
        let page = r.get::<i64>(0)?;
        let id = r.get::<i64>(1)?;
        let time = r.get::<i64>(5)?;
        let count = rows.get(&page).and_then(|v| v.visit_count).unwrap_or(0) + 1;
        let shown = rows.get(&page).is_none_or(|v| {
            if first {
                (time, id) < (v.visit_time, v.id)
            } else {
                (time, id) > (v.visit_time, v.id)
            }
        });
        if shown {
            let url = r.get::<String>(3)?;
            rows.insert(
                page,
                crate::HistoryEntry {
                    id,
                    profile_id: r.get::<Option<String>>(2)?.unwrap_or_default(),
                    domain: crate::utils::url_domain(&url),
                    url,
                    title: r.get(4)?,
                    visit_time: time,
                    visited_at: chrono::DateTime::from_timestamp_millis(time)
                        .map(|time| time.to_rfc3339())
                        .unwrap_or_default(),
                    duration_ms: r.get(7)?,
                    transition: r.get(8)?,
                    source_visit_id: r
                        .get::<Option<String>>(9)?
                        .and_then(|value| value.parse().ok())
                        .unwrap_or(0),
                    app_id: r.get(10)?,
                    favicon: None,
                    enrichment_excerpt: r
                        .get::<Option<String>>(11)?
                        .and_then(|text| crate::archive::cap_enrichment_excerpt(&text)),
                    visit_count: Some(count),
                },
            );
        } else {
            rows.get_mut(&page).unwrap().visit_count = Some(count);
        }
        Ok(())
    })?;
    if super::documents::watermark(s)? > text.indexed_seq {
        let ids = hits.iter().map(|h| h.page_id as i64).collect::<Vec<_>>();
        for id in text_mismatches(s, &ids, &text.ast, text.fuzzy.as_ref())? {
            rows.remove(&(id as i64));
        }
    }
    Ok(hits
        .iter()
        .filter_map(|h| {
            rows.remove(&(h.page_id as i64)).map(|visit| SearchHit {
                reason: None,
                page_id: h.page_id as i64,
                score: h.score,
                visit,
            })
        })
        .collect())
}

impl Membership {
    pub(super) fn empty(store: &Store, first: bool) -> Result<Self> {
        store.read(Db::Archive, move |s| {
            let max = maximum_page_id(s)?;
            Ok(Self::new(max, first))
        })
    }
    /// Batches bound the SQL request, not the candidate universe. Both ranking and
    /// counting share this session's canonical membership, so a page is read once.
    pub(super) fn ensure(
        &mut self,
        store: &Store,
        scope: &Scope,
        ids: &[u64],
        cancel: &vault_store::CancelToken,
        ast: &vault_search::Ast,
        indexed_seq: u64,
        fuzzy: Option<(&crate::archive::search_lexical::FuzzyQuery, &vault_search::Ast)>,
    ) -> Result<()> {
        let ids =
            ids.iter().filter(|id| !self.resolved(**id)).map(|id| *id as i64).collect::<Vec<_>>();
        if ids.is_empty() {
            return Ok(());
        }
        let requested = ids.clone();
        let ast = ast.clone();
        let fuzzy = fuzzy.map(|(predicate, exact)| (predicate.clone(), exact.clone()));
        let scope = scope.clone();
        let cancel = cancel.clone();
        let found = store.read(Db::Archive, move |s| {
            let mut params = scope.params;
            let keys = ids
                .iter()
                .map(|id| {
                    params.push(Value::Integer(*id));
                    format!("?{}", params.len())
                })
                .collect::<Vec<_>>()
                .join(",");
            let sql = format!(
                "SELECT p.id,v.visit_time_ms FROM {PAGE_VISITS} WHERE p.id IN ({keys}) AND {}",
                scope.sql
            );
            // page -> (visits, newest, first)
            let mut pages: HashMap<usize, (u64, i64, i64)> = HashMap::new();
            s.for_each(&sql, params, |r| {
                cancel.check()?;
                let id = r.get::<i64>(0)? as usize;
                let time = r.get::<i64>(1)?;
                let entry = pages.entry(id).or_insert((0, time, time));
                entry.0 += 1;
                entry.1 = entry.1.max(time);
                entry.2 = entry.2.min(time);
                Ok(())
            })?;
            if fuzzy.is_some() || super::documents::watermark(s)? > indexed_seq {
                for id in text_mismatches(s, &ids, &ast, fuzzy.as_ref())? {
                    pages.remove(&(id as usize));
                }
            }
            Ok(pages)
        })?;
        for id in requested {
            let id = id as usize;
            if id >= self.newest.len() {
                self.newest.resize(id + 1, 0);
                if let Some(first) = &mut self.first {
                    first.resize(id + 1, 0);
                }
                self.counts.resize(id + 1, 0);
                self.bits.resize(id / 64 + 1, 0);
                self.checked.resize(id / 64 + 1, 0);
            }
            self.checked[id / 64] |= 1u64 << (id % 64);
        }
        for (id, (count, newest, earliest)) in found {
            self.bits[id / 64] |= 1u64 << (id % 64);
            self.counts[id] = count;
            self.newest[id] = newest;
            if let Some(first) = &mut self.first {
                first[id] = earliest;
            }
        }
        Ok(())
    }
    fn resolved(&self, id: u64) -> bool {
        self.complete
            || self
                .checked
                .get(id as usize / 64)
                .is_some_and(|word| word & (1u64 << (id % 64)) != 0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn posting_batches_seek_page_urls_and_visible_visits_without_archive_scans() {
        let vault = crate::Vault::open_temp().unwrap();
        let plan=vault.store().read(Db::Archive,|s|s.query(
            &format!("EXPLAIN QUERY PLAN SELECT p.id,v.visit_time_ms FROM {PAGE_VISITS} WHERE p.id IN (1,2) AND v.reverted_at IS NULL"),
            (),|r|Ok(r.get::<String>(3)?))).unwrap().join("\n");
        assert!(plan.contains("idx_urls_page_id"), "{plan}");
        assert!(plan.contains("idx_visits_visible_url_time"), "{plan}");
        assert!(
            !plan.contains("SCAN v") && !plan.contains("SCAN p") && !plan.contains("SCAN u"),
            "{plan}"
        );
        assert!(!plan.to_lowercase().contains("temp b-tree"), "{plan}");
    }
    #[test]
    fn selective_scopes_seek_canonical_time_indexes_without_sorting() {
        let vault = crate::Vault::open_temp().unwrap();
        for request in [
            SearchRequest { start_time_ms: Some(8), end_time_ms: Some(12), ..Default::default() },
            SearchRequest {
                profile_id: Some("chrome".into()),
                start_time_ms: Some(8),
                ..Default::default()
            },
        ] {
            let expected = if request.profile_id.is_some() {
                "idx_visits_visible_profile_time_id"
            } else {
                "idx_visits_visible_time_id"
            };
            let scope = Scope::new(&request, &ParsedHistorySearchQuery::default());
            let sql = format!(
                "EXPLAIN QUERY PLAN SELECT u.page_id,v.visit_time_ms FROM {} WHERE {}",
                scope.from(),
                scope.sql
            );
            let plan = vault
                .store()
                .read(Db::Archive, move |s| {
                    s.query(&sql, scope.params, |r| Ok(r.get::<String>(3)?))
                })
                .unwrap()
                .join("\n");
            assert!(plan.contains(expected), "{plan}");
            assert!(!plan.to_lowercase().contains("temp b-tree"), "{plan}");
        }
    }
}
