//! Responsibilities: bounded Browser Direct previews and indexed duplicate probes.
//! Not responsible for: canonical writes or staging source files.
//! Failure modes: preview overflows, duplicate counts use another profile, a visit the
//! import skips counted as new or the reverse. Tests: browser_history suites.
use super::*;
use crate::archive::ingest::identity::Previewed;
const DUPLICATE_PROBE_CHUNK: usize = 500;

/// Finds which parsed visits an import would skip because the archive already
/// has them for this source profile.
///
/// Exists so the Browser Direct preview can show "N new, M already in your
/// archive" before anything is written. It asks what the import does
/// (`archive::ingest::identity`): a visit is its address and browser time, not
/// its row id, which the browser gives to another visit after it numbers its
/// rows again. Rows of one visit anywhere in the file are one visit, and a
/// version 1 visit is matched once, as the import keeps them.
pub(super) struct ArchiveDuplicateProbe {
    store: std::sync::Arc<vault_store::Store>,
    /// `None` while the archive has never seen this profile.
    source_profile_id: Option<i64>,
    source_kind: &'static str,
    family: &'static str,
    seen: Option<crate::archive::ingest::identity::Preview>,
}

impl ArchiveDuplicateProbe {
    /// A profile the archive has never seen has none of the file's visits, yet the file's own
    /// rows of one visit (a redirect's hops) are one visit there too, as the import writes them.
    pub(super) fn open(vault: &Vault, staged: &StagedBrowserHistorySource) -> Result<Self> {
        let profile = staged.profile_id.clone();
        let id = vault.store().read(Db::Archive, move |s| {
            Ok(s.query("SELECT id FROM source_profiles WHERE profile_key=?1", [profile], |r| {
                Ok(r.get::<i64>(0)?)
            })?
            .pop())
        })?;
        Ok(Self {
            store: vault.store().clone(),
            source_profile_id: id,
            source_kind: staged.family.source_kind(),
            family: staged.family.as_str(),
            seen: Some(Default::default()),
        })
    }

    /// What the import would do with each visit.
    fn existing(&mut self, batch: &[ParsedVisit]) -> Result<Vec<Previewed>> {
        let keyed = batch.iter().all(|visit| visit.source_visit_time.is_some());
        if keyed {
            let visits = batch.to_vec();
            let (profile, family) = (self.source_profile_id, self.family);
            let mut seen = self.seen.take().unwrap_or_default();
            let (flags, seen) = self.store.read(Db::Archive, move |s| {
                let mut flags = Vec::with_capacity(visits.len());
                for visit in &visits {
                    let key = visit.source_visit_time.clone().unwrap_or_default();
                    flags.push(seen.classify(s, profile, family, visit, &key)?);
                }
                Ok((flags, seen))
            })?;
            self.seen = Some(seen);
            return Ok(flags);
        }
        // Rows without a browser time (Takeout's) are matched by row id or fingerprint, of which a
        // profile the archive has never seen has none.
        let Some(profile) = self.source_profile_id else {
            return Ok(vec![Previewed::New; batch.len()]);
        };
        let mut flags = Vec::with_capacity(batch.len());
        for chunk in batch.chunks(DUPLICATE_PROBE_CHUNK) {
            let ids =
                chunk.iter().map(|visit| visit.source_visit_id.to_string()).collect::<Vec<_>>();
            let found_ids = self.matching(profile, "source_visit_id", &ids)?;
            let mut chunk_flags = ids.iter().map(|id| found_ids.contains(id)).collect::<Vec<_>>();
            let missing =
                (0..chunk_flags.len()).filter(|&index| !chunk_flags[index]).collect::<Vec<_>>();
            if !missing.is_empty() {
                let fingerprints = missing
                    .iter()
                    .map(|&index| {
                        let visit = &chunk[index];
                        visit_event_fingerprint(
                            self.source_kind,
                            &visit.url,
                            visit.visit_time_ms,
                            visit.title.as_deref(),
                            visit.transition,
                            visit.app_id.as_deref(),
                        )
                    })
                    .collect::<Vec<_>>();
                let found_fingerprints =
                    self.matching(profile, "event_fingerprint", &fingerprints)?;
                for (index, fingerprint) in missing.into_iter().zip(&fingerprints) {
                    chunk_flags[index] = found_fingerprints.contains(fingerprint);
                }
            }
            flags.extend(
                chunk_flags
                    .into_iter()
                    .map(|found| if found { Previewed::Archived } else { Previewed::New }),
            );
        }
        Ok(flags)
    }

    /// Values of `column` among `values` that this profile's visits already use.
    fn matching(
        &self,
        profile: i64,
        column: &'static str,
        values: &[String],
    ) -> Result<std::collections::HashSet<String>> {
        let placeholders = vec!["?"; values.len()].join(",");
        let sql = format!(
            "SELECT {column} FROM visits WHERE source_profile_id = ? AND {column} IN ({placeholders})"
        );
        let parameters = std::iter::once(vault_store::Value::Integer(profile))
            .chain(values.iter().cloned().map(vault_store::Value::Text))
            .collect::<Vec<_>>();
        Ok(self.store.read(Db::Archive, move |s| {
            Ok(s.query(&sql, parameters, |r| Ok(r.get::<String>(0)?))?.into_iter().collect())
        })?)
    }
}

pub(super) struct BrowserPreviewCollector {
    pub(super) preview_entries: Vec<TakeoutPreviewEntry>,
    pub(super) candidate_items: usize,
    pub(super) duplicate_items: usize,
    /// Rows the import writes hidden behind a visit and counts with it (Firefox Sync copies):
    /// neither new nor duplicates, as the import reports them.
    pub(super) same_visit_items: usize,
    pub(super) url_count: usize,
    pub(super) preview_range: PreviewRangeSummary,
    pub(super) source_path: String,
    pub(super) duplicates: ArchiveDuplicateProbe,
}

impl BrowserPreviewCollector {
    pub(super) fn new(source_path: String, duplicates: ArchiveDuplicateProbe) -> Self {
        Self {
            preview_entries: Vec::new(),
            candidate_items: 0,
            duplicate_items: 0,
            same_visit_items: 0,
            url_count: 0,
            preview_range: PreviewRangeSummary::default(),
            source_path,
            duplicates,
        }
    }
}

impl HistoryBatchConsumer for BrowserPreviewCollector {
    type Error = anyhow::Error;

    fn urls(&mut self, batch: Vec<ParsedUrl>) -> Result<(), Self::Error> {
        self.url_count += batch.len();
        Ok(())
    }

    fn visits(&mut self, batch: Vec<ParsedVisit>) -> Result<(), Self::Error> {
        self.candidate_items += batch.len();
        let existing = self.duplicates.existing(&batch)?;
        let count = |kind| existing.iter().filter(|found| **found == kind).count();
        self.duplicate_items += count(Previewed::Archived);
        self.same_visit_items += count(Previewed::Copy);
        for (visit, previewed) in batch.into_iter().zip(existing) {
            merge_preview_range(
                &mut self.preview_range,
                Some(&visit.visit_time_iso),
                Some(&visit.visit_time_iso),
            );
            if self.preview_entries.len() >= PREVIEW_LIMIT {
                continue;
            }
            self.preview_entries.push(TakeoutPreviewEntry {
                source_path: self.source_path.clone(),
                url: visit.url,
                title: visit.title,
                visited_at: visit.visit_time_iso,
                source_visit_id: visit.source_visit_id,
                status: match previewed {
                    Previewed::Archived => "duplicate",
                    Previewed::New => "candidate",
                    Previewed::Copy => "same-visit",
                }
                .to_string(),
            });
        }
        Ok(())
    }

    fn retain_source_evidence_in_report(&self) -> bool {
        false
    }
}
