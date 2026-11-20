//! Passage embeddings share the browsing model selector, batch validation and int8 scorer.
//! The address map and vectors publish together in the Agent archive's keyed search directory.
mod reuse;
use super::{EmbeddingProvider, EmbeddingRole, quantize_int8, vector_index::scaled_dot};
use crate::AiProviderRuntime;
use agent_history::{Archive, search::SemanticFiles};
use anyhow::{Context, Result, ensure};
use serde::{Deserialize, Serialize};
use std::{collections::HashMap, fs, sync::RwLock};
use vault_store::CancelToken;

const CHUNKS: i64 = 1 << 20;
const PAGE_BYTES: usize = 1024 * 1024;
static SWITCH: RwLock<()> = RwLock::new(());

#[derive(Clone, Serialize, Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct Generation {
    pub generation: String,
    pub provider: String,
    pub model: String,
    endpoint: String,
    pub projection: i64,
    pub marks: i64,
    pub passages: u64,
    pub vectors: u64,
    #[serde(default)]
    pub reused_vectors: u64,
    dimension: usize,
    vector_pages: u32,
    address_pages: u32,
    pub built_at: i64,
}
fn manifest(files: &SemanticFiles) -> Result<Option<Generation>> {
    if !files.path().join("current").try_exists()? {
        return Ok(None);
    }
    let g: Generation = serde_json::from_slice(&files.read(None, "current")?)?;
    ensure!(
        g.generation.starts_with("generation-")
            && g.generation.bytes().all(|b| b == b'-' || b.is_ascii_alphanumeric()),
        "invalid Agent generation name"
    );
    Ok(Some(g))
}
pub fn current(a: &Archive) -> Result<Option<Generation>> {
    let _lock = SWITCH.read().unwrap();
    manifest(&a.semantic_files(&Default::default())?)
}
fn endpoint(config: &crate::AiProviderConfig) -> String {
    crate::utils::sha256_hex(
        &serde_json::to_vec(&(&config.base_url, config.dimensions))
            .expect("scalar provider metadata"),
    )
}
impl Generation {
    pub fn matches(&self, r: &AiProviderRuntime) -> bool {
        self.matches_config(&r.config)
    }
    pub fn matches_config(&self, config: &crate::AiProviderConfig) -> bool {
        self.provider == config.id
            && self.model == config.default_model
            && self.endpoint == endpoint(config)
    }
}
fn key(text: &str) -> [u8; 32] {
    use sha2::{Digest, Sha256};
    Sha256::digest(text.as_bytes()).into()
}
pub struct RankedPassage {
    pub event: i64,
    pub chunk: u32,
    pub score: f32,
    index: u32,
}
pub struct Ranking {
    pub generation: Generation,
    pub hits: Vec<RankedPassage>,
    keys: Vec<[u8; 32]>,
}
impl Ranking {
    pub fn matches(&self, hit: &RankedPassage, text: &str) -> bool {
        key(text) == self.keys[hit.index as usize]
    }
}
struct Pages<'a> {
    files: &'a SemanticFiles,
    generation: &'a str,
    family: &'static str,
    number: u32,
    buffer: Vec<u8>,
}
impl<'a> Pages<'a> {
    fn new(files: &'a SemanticFiles, generation: &'a str, family: &'static str) -> Self {
        Self { files, generation, family, number: 0, buffer: Vec::with_capacity(PAGE_BYTES) }
    }
    fn flush(&mut self) -> Result<()> {
        if !self.buffer.is_empty() {
            self.files.write(
                Some(self.generation),
                &format!("{}-{}", self.family, self.number),
                &self.buffer,
            )?;
            self.buffer.clear();
            self.number += 1;
        }
        Ok(())
    }
    fn boundary(&mut self) -> Result<()> {
        if self.buffer.len() >= PAGE_BYTES {
            self.flush()?;
        }
        Ok(())
    }
}
struct EmbeddingCall(CancelToken);
impl super::AiRunControl for EmbeddingCall {
    fn checkpoint(&self, detail: &str) -> Result<()> {
        self.0.check().map_err(|_| super::AiRunCancelled::new(detail).into())
    }
}
async fn request(
    engine: &impl EmbeddingProvider,
    texts: &[String],
    role: EmbeddingRole,
    cancel: &CancelToken,
) -> Result<Vec<Vec<f32>>> {
    let control: std::sync::Arc<dyn super::AiRunControl> =
        std::sync::Arc::new(EmbeddingCall(cancel.clone()));
    super::control::await_with_ai_cancellation(
        Some(&control),
        "Agent embedding cancelled",
        engine.embed(texts, role),
    )
    .await
}
pub async fn build(
    archive: &Archive,
    r: &AiProviderRuntime,
    engine: &impl EmbeddingProvider,
    cancel: &CancelToken,
    checkpoint: impl Fn() -> Result<()>,
    mut progress: impl FnMut(u64, u64),
) -> Result<Generation> {
    checkpoint()?;
    let files = archive.semantic_files(cancel)?;
    fs::create_dir_all(files.path())?;
    let temp = tempfile::Builder::new().prefix("generation-").tempdir_in(files.path())?;
    let name = temp.path().file_name().unwrap().to_string_lossy().into_owned();
    let mut vectors = Pages::new(&files, &name, "vectors");
    let mut addresses = Pages::new(&files, &name, "addresses");
    let projection = archive.projection_generation()?;
    let marks = archive.search_marks_generation()?;
    let mut known = HashMap::new();
    let mut cursor = 0;
    let mut passages = 0;
    let serving = manifest(&files)?.filter(|g| g.matches(r));
    let mut dimension = serving.as_ref().map_or(0, |g| g.dimension);
    let mut previous = reuse::Previous::load(&files, serving.as_ref(), cancel)?;
    loop {
        cancel.check()?;
        checkpoint()?;
        let page = archive.embedding_rows(cursor)?;
        if page.is_empty() {
            break;
        }
        let mut pending = Vec::new();
        for row in page {
            cursor = row.id;
            ensure!(
                row.chunks.len() <= CHUNKS as usize,
                "record exceeds Agent passage address space"
            );
            for (chunk, passage) in row.chunks.into_iter().enumerate() {
                let text = passage.text;
                let hash = key(&text);
                let next = u32::try_from(known.len())?;
                let index = *known.entry(hash).or_insert_with(|| {
                    pending.push(text);
                    next
                });
                let address = row
                    .id
                    .checked_mul(CHUNKS)
                    .and_then(|n| n.checked_add(chunk as i64))
                    .context("Agent address overflow")?;
                addresses.buffer.extend_from_slice(&address.to_le_bytes());
                addresses.buffer.extend_from_slice(&index.to_le_bytes());
                addresses.boundary()?;
                passages += 1;
                if pending.len() == 64 {
                    checkpoint()?;
                    cancel.check()?;
                    embed(engine, &mut vectors, &pending, &mut dimension, cancel, &mut previous)
                        .await?;
                    pending.clear();
                }
            }
        }
        if !pending.is_empty() {
            checkpoint()?;
            cancel.check()?;
            embed(engine, &mut vectors, &pending, &mut dimension, cancel, &mut previous).await?;
        }
        progress(passages, known.len() as u64);
    }
    vectors.flush()?;
    addresses.flush()?;
    let g = Generation {
        generation: name.clone(),
        provider: r.config.id.clone(),
        model: r.config.default_model.clone(),
        endpoint: endpoint(&r.config),
        projection,
        marks,
        passages,
        vectors: known.len() as u64,
        dimension,
        reused_vectors: previous.copied,
        vector_pages: vectors.number,
        address_pages: addresses.number,
        built_at: chrono::Utc::now().timestamp_millis(),
    };
    checkpoint()?;
    cancel.check()?;
    let _lock = SWITCH.write().unwrap();
    let old = manifest(&files)?;
    files.write(None, "current", &serde_json::to_vec(&g)?)?;
    let _published = temp.keep();
    if let Some(old) = old
        && let Err(e) = fs::remove_dir_all(files.path().join(old.generation))
    {
        eprintln!("Retiring Agent semantic generation: {e}");
    }
    Ok(g)
}
async fn embed(
    engine: &impl EmbeddingProvider,
    pages: &mut Pages<'_>,
    texts: &[String],
    dim: &mut usize,
    cancel: &CancelToken,
    previous: &mut reuse::Previous,
) -> Result<()> {
    let hashes: Vec<_> = texts.iter().map(|text| key(text)).collect();
    let missing: Vec<_> = texts
        .iter()
        .zip(&hashes)
        .filter(|(_, hash)| !previous.contains(hash))
        .map(|(text, _)| text.clone())
        .collect();
    let vectors = if missing.is_empty() {
        Vec::new()
    } else {
        request(engine, &missing, EmbeddingRole::Document, cancel).await?
    };
    ensure!(vectors.len() == missing.len(), "embedding service returned a different batch size");
    let mut returned = vectors.into_iter();
    for hash in hashes {
        if previous.append(pages.files, &hash, &mut pages.buffer)? {
            pages.boundary()?;
            continue;
        }
        let v = returned.next().expect("one embedding per missing key");
        if *dim == 0 {
            *dim = v.len();
        }
        ensure!(
            *dim > 0 && v.len() == *dim && v.iter().all(|n| n.is_finite()),
            "invalid passage embedding"
        );
        let norm = v.iter().map(|n| n * n).sum::<f32>().sqrt();
        let unit = if norm > 0. { v.into_iter().map(|n| n / norm).collect() } else { v };
        let vector = quantize_int8(&unit);
        pages.buffer.extend_from_slice(&hash);
        pages.buffer.extend_from_slice(&vector.scale.to_le_bytes());
        pages.buffer.extend(vector.codes.into_iter().map(|v| v as u8));
        pages.boundary()?;
    }
    Ok(())
}
pub async fn rank(
    archive: &Archive,
    r: &AiProviderRuntime,
    engine: &impl EmbeddingProvider,
    query: &str,
    cancel: &CancelToken,
    check: impl Fn() -> Result<()>,
) -> Result<Ranking> {
    check()?;
    cancel.check()?;
    let files = archive.semantic_files(cancel)?;
    let initial = current(archive)?.context("agent-semantic-index-not-ready")?;
    ensure!(initial.matches(r), "agent-semantic-index-not-ready");
    if initial.vectors == 0 {
        return Ok(Ranking { generation: initial, hits: vec![], keys: vec![] });
    }
    let mut query = request(engine, &[query.into()], EmbeddingRole::Query, cancel)
        .await?
        .pop()
        .context("missing query vector")?;
    check()?;
    cancel.check()?;
    let _lock = SWITCH.read().unwrap();
    let g = manifest(&files)?.context("agent-semantic-index-not-ready")?;
    ensure!(g.generation == initial.generation, "agent-semantic-index-changed");
    ensure!(
        g.dimension == query.len() && query.iter().all(|n| n.is_finite()),
        "query vector does not match serving index"
    );
    let norm = query.iter().map(|v| v * v).sum::<f32>().sqrt();
    if norm > 0. {
        for n in &mut query {
            *n /= norm;
        }
    }
    let mut keys = vec![];
    let mut scores = vec![];
    for page in 0..g.vector_pages {
        cancel.check()?;
        let data = files.read(Some(&g.generation), &format!("vectors-{page}"))?;
        let stride = g.dimension + 36;
        ensure!(data.len() % stride == 0, "incomplete vector page");
        for record in data.chunks_exact(stride) {
            keys.push(record[..32].try_into()?);
            scores.push(scaled_dot(
                &query,
                &record[36..],
                f32::from_le_bytes(record[32..36].try_into()?),
            ));
        }
    }
    let mut hits = vec![];
    for page in 0..g.address_pages {
        cancel.check()?;
        let data = files.read(Some(&g.generation), &format!("addresses-{page}"))?;
        ensure!(data.len() % 12 == 0, "incomplete passage map");
        for record in data.chunks_exact(12) {
            let address = i64::from_le_bytes(record[..8].try_into()?);
            let index = u32::from_le_bytes(record[8..].try_into()?);
            let score = *scores.get(index as usize).context("missing passage vector")?;
            if score > 0. {
                hits.push(RankedPassage {
                    event: address / CHUNKS,
                    chunk: (address % CHUNKS) as u32,
                    index,
                    score,
                });
            }
        }
    }
    drop(_lock);
    hits.sort_unstable_by(|a, b| {
        b.score.total_cmp(&a.score).then_with(|| (a.event, a.chunk).cmp(&(b.event, b.chunk)))
    });
    cancel.check()?;
    Ok(Ranking { generation: g, hits, keys })
}
