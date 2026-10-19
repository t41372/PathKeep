//! Responsibilities: paginate a globally sorted star set and attach canonical notes/tags.
//! Not responsible for: star mutation, source ingestion or frontend joins.
//! Failure modes: limiting before sorting drops popular stars, tied keys repeat,
//! metadata issues one query per result, domain stars scan pages, grouping sorts all observations, or a cursor is reused under another sort.
//! Tests: pagination_tests and stars::tests.
use super::*;
use crate::models::StarPage;
use serde::{Deserialize, Serialize};
#[derive(Serialize, Deserialize)]
struct Cursor {
    filter: Option<StarEntityKind>,
    sort: StarSort,
    visits: i64,
    at: String,
    kind: StarEntityKind,
    key: String,
}
// Each branch starts with the star set and probes the appropriate page index.
const COUNTS: &str = "WITH counts AS (
 SELECT st.entity_kind,st.entity_key,st.starred_at,
 CASE WHEN st.entity_kind='url' THEN COALESCE((
   SELECT SUM(u.visit_count) FROM pages p CROSS JOIN urls u ON u.page_id=p.id
   WHERE p.canonical_url=st.entity_key
 ),0) ELSE COALESCE((
   SELECT SUM(u.visit_count) FROM pages p CROSS JOIN urls u ON u.page_id=p.id
   WHERE p.registrable_domain IS NOT NULL AND p.registrable_domain=st.entity_key
 ),0) END AS visits
 FROM star st WHERE st.entity_kind IN ('url','domain') AND (?1 IS NULL OR st.entity_kind=?1)
)";

fn list_sql(revisited: bool) -> String {
    let score = if revisited { "visits" } else { "(0+0)" };
    format!(
        "{COUNTS}
 SELECT entity_kind,entity_key,starred_at,visits,
 COALESCE((SELECT u.title FROM pages p JOIN urls u ON u.page_id=p.id
 WHERE counts.entity_kind='url' AND p.canonical_url=counts.entity_key
 ORDER BY u.last_visit_ms DESC,u.id DESC LIMIT 1),'')
 FROM counts WHERE ?2=0 OR {score}<?3 OR ({score}=?3 AND
 (starred_at<?4 OR (starred_at=?4 AND (entity_kind>?5 OR (entity_kind=?5 AND entity_key>?6)))))
 ORDER BY {score} DESC,starred_at DESC,entity_kind,entity_key LIMIT ?7"
    )
}

pub(super) fn page(
    vault: &Vault,
    kind: Option<StarEntityKind>,
    sort: StarSort,
    limit: Option<usize>,
    cursor: Option<String>,
) -> Result<StarPage> {
    let size = limit.unwrap_or(500);
    anyhow::ensure!(
        size > 0 && size < i64::MAX as usize,
        "invalid-page-size: star page size must be positive"
    );
    let cursor: Option<Cursor> = cursor
        .map(|text| serde_json::from_str(&text))
        .transpose()
        .context("invalid-cursor: invalid star cursor")?;
    if let Some(cursor) = &cursor {
        anyhow::ensure!(
            cursor.filter == kind && cursor.sort == sort,
            "invalid-cursor: star sort or kind changed"
        );
    }
    vault.read(Db::Archive, move |s| {
        let revisited = sort == StarSort::MostRevisited;
        let at = cursor.as_ref().map(|c| c.at.as_str()).unwrap_or("");
        let last_kind = cursor.as_ref().map(|c| c.kind.as_str()).unwrap_or("");
        let last_key = cursor.as_ref().map(|c| c.key.as_str()).unwrap_or("");
        let last_score = cursor.as_ref().map(|c| c.visits).unwrap_or(0);
        let sql = list_sql(revisited);
        let mut items = s.query(
            &sql,
            params![
                kind.map(|k| k.as_str()),
                i64::from(cursor.is_some()),
                last_score,
                at,
                last_kind,
                last_key,
                (size + 1) as i64
            ],
            |r| {
                let entity_kind = parse_kind(&r.get::<String>(0)?).expect("stored star kind");
                let entity_key = r.get::<String>(1)?;
                Ok(StarListItem {
                    domain: if entity_kind == StarEntityKind::Domain {
                        entity_key.clone()
                    } else {
                        registrable_domain_for_url(&entity_key).unwrap_or_default()
                    },
                    entity_kind,
                    entity_key,
                    starred_at: r.get(2)?,
                    visit_count: r.get(3)?,
                    title: r.get(4)?,
                    notes: String::new(),
                    tags: vec![],
                })
            },
        )?;
        let more = items.len() > size;
        items.truncate(size);
        let next_cursor = if more {
            items
                .last()
                .map(|item| {
                    serde_json::to_string(&Cursor {
                        filter: kind,
                        sort,
                        visits: if revisited { item.visit_count } else { 0 },
                        at: item.starred_at.clone(),
                        kind: item.entity_kind,
                        key: item.entity_key.clone(),
                    })
                })
                .transpose()?
        } else {
            None
        };
        attach_annotations(s, &mut items)?;
        Ok(StarPage { items, next_cursor })
    })
}

fn attach_annotations(s: &Session<'_>, items: &mut [StarListItem]) -> Result<()> {
    let positions: HashMap<_, _> = items
        .iter()
        .enumerate()
        .filter(|(_, i)| i.entity_kind == StarEntityKind::Url)
        .map(|(index, item)| (item.entity_key.clone(), index))
        .collect();
    let keys = positions.keys().cloned().collect::<Vec<_>>();
    // Fixed-size bulk queries keep parameter counts bounded; never one query per star.
    for keys in keys.chunks(500) {
        let binds = vec!["?"; keys.len()].join(",");
        let notes = s.query(&format!("SELECT canonical_url,notes FROM url_annotations WHERE canonical_url IN ({binds}) ORDER BY updated_at DESC,url"),keys.to_vec(),|r|Ok((r.get::<String>(0)?,r.get::<String>(1)?)))?;
        for (key, note) in notes {
            let item = &mut items[positions[&key]];
            if item.notes.is_empty() {
                item.notes = note;
            }
        }
        let tags = s.query(&format!("SELECT canonical_url,tag FROM url_tags WHERE canonical_url IN ({binds}) ORDER BY created_at,url,tag"),keys.to_vec(),|r|Ok((r.get::<String>(0)?,r.get::<String>(1)?)))?;
        for (key, tag) in tags {
            let item = &mut items[positions[&key]];
            if !item.tags.iter().any(|existing| existing.to_lowercase() == tag.to_lowercase()) {
                item.tags.push(tag);
            }
        }
    }
    Ok(())
}

#[cfg(test)]
mod plan_tests {
    use super::*;
    /// Render enrichment must probe page/URL indexes even for a large corpus.
    #[test]
    fn starred_enrichment_seeks_page_and_url_indexes() {
        for key in [None, Some([81; 32])] {
            let vault = Vault::open_temp_with_key(key).unwrap();
            vault
                .read(Db::Archive, |s| {
                    for sort in [false, true] {
                        let plan = s
                            .query(
                                &format!("EXPLAIN QUERY PLAN {}", list_sql(sort)),
                                params![Option::<String>::None, 0, 0, "", "", "", 20],
                                |r| Ok(r.get::<String>(3)?),
                            )?
                            .join("\n");
                        assert!(plan.contains("idx_urls_page_id"), "{plan}");
                        assert!(
                            plan.contains("idx_pages_canonical_url")
                                && plan.contains("idx_pages_registrable_domain"),
                            "{plan}"
                        );
                        assert!(!plan.contains("SCAN p"), "{plan}");
                        assert!(
                            !plan.contains("SCAN u")
                                && !plan.contains("SCAN urls")
                                && !plan.contains("SCAN visits"),
                            "{plan}"
                        );
                    }
                    Ok(())
                })
                .unwrap();
        }
    }
}
