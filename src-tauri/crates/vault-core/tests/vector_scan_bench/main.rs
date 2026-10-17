//! Responsibilities: measure interactive vector scans, storage and real-corpus recall.
//! Not responsible for: choosing a production index or claiming synthetic recall quality.
//!
//! All byte counts are logical bytes, not hardware counters. File-cache residency is OS-managed; no eviction is requested.
//! Resident bytes count live allocator requests made while building the index (including private
//! library caches/capacity, excluding allocator metadata); exact reports its logical f32 mapping
//! size instead of a heap allocation. Allocation counting is disabled during query timing.
//! Process measurements include the real source mmap, harness and allocations retained by System.

use anyhow::{Context, Result, ensure};
use memmap2::Mmap;
use rayon::prelude::*;
use serde::{Deserialize, Serialize};
use std::cmp::{Ordering, Reverse};
use std::collections::{BTreeMap, BinaryHeap, HashSet};
use std::fs::{self, File, OpenOptions};
use std::io::{BufWriter, Write};
use std::path::{Path, PathBuf};
use std::time::Instant;
use turbovec::IdMapIndex;

mod host;
use host::{Footprint, begin_allocation_count, disk_fits, end_allocation_count, footprint};
use vault_core::{
    EmbeddingDtype, EmbeddingFingerprint, EmbeddingPooling, FlatVectorIndex, VectorIndex,
    VectorStore, build_planes_from_store, project_paths_with_root, quantize_int8,
};

struct Random(u64);
impl Random {
    fn next(&mut self) -> u64 {
        self.0 = self.0.wrapping_add(0x9e37_79b9_7f4a_7c15);
        let mut x = self.0;
        x = (x ^ (x >> 30)).wrapping_mul(0xbf58_476d_1ce4_e5b9);
        x = (x ^ (x >> 27)).wrapping_mul(0x94d0_49bb_1331_11eb);
        x ^ (x >> 31)
    }
    fn vector(&mut self, dim: usize) -> Vec<f32> {
        let mut vector = (0..dim)
            .map(|_| (self.next() >> 40) as f32 / (1_u32 << 23) as f32 - 1.)
            .collect::<Vec<_>>();
        let norm = vector.iter().map(|x| x * x).sum::<f32>().sqrt();
        for value in &mut vector {
            *value /= norm;
        }
        vector
    }
}

#[derive(Deserialize, Serialize)]
struct Meta {
    n: usize,
    dim: usize,
    q: usize,
    model: String,
}
struct Dataset {
    meta: Meta,
    vectors: Option<Mmap>,
    queries: Vec<Vec<f32>>,
    groups: Option<Vec<u32>>,
}
impl Dataset {
    fn open(root: &Path) -> Result<Self> {
        ensure!(cfg!(target_endian = "little"), "source mmap requires a little-endian host");
        let meta: Meta = serde_json::from_reader(File::open(root.join("meta.json"))?)?;
        ensure!(meta.n > 0 && meta.dim > 0 && meta.q > 0, "empty dataset");
        ensure!(meta.dim != 1024 || meta.n <= 4_000_000, "refusing dim=1024 above 4M");
        let file = File::open(root.join("vectors.f32"))?;
        ensure!(file.metadata()?.len() == (meta.n * meta.dim * 4) as u64, "vector size mismatch");
        // SAFETY: benchmark-owned source files are immutable for the mapping's lifetime. Real
        // corpus inputs must likewise remain unchanged until this benchmark finishes.
        let vectors = unsafe { Mmap::map(&file)? };
        let query_bytes = fs::read(root.join("queries.f32"))?;
        ensure!(query_bytes.len() == meta.q * meta.dim * 4, "query size mismatch");
        let queries = query_bytes
            .chunks_exact(meta.dim * 4)
            .map(|row| {
                row.chunks_exact(4).map(|x| f32::from_le_bytes(x.try_into().unwrap())).collect()
            })
            .collect();
        let bytes = fs::read(root.join("groups.u32"))?;
        ensure!(bytes.len() == meta.n * 4, "group size mismatch");
        let groups = Some(
            bytes.chunks_exact(4).map(|x| u32::from_le_bytes(x.try_into().unwrap())).collect(),
        );
        let data = Self { meta, vectors: Some(vectors), queries, groups };
        for row in
            data.rows().chunks_exact(data.meta.dim).chain(data.queries.iter().map(Vec::as_slice))
        {
            let norm = row.iter().map(|x| x * x).sum::<f32>();
            ensure!(norm.is_finite() && (norm - 1.).abs() < 0.002, "input is not L2 normalized");
        }
        Ok(data)
    }
    fn rows(&self) -> &[f32] {
        mapped_rows(self.vectors.as_ref().expect("real f32 source"))
    }
    /// Exact f32 score of the real vector behind a result id (ids are position + 1).
    fn exact_score(&self, query: &[f32], id: u64) -> f32 {
        let start = (id as usize - 1) * self.meta.dim;
        self.rows()[start..start + self.meta.dim].iter().zip(query).map(|(a, b)| a * b).sum()
    }
    fn row(&self, id: usize) -> std::borrow::Cow<'_, [f32]> {
        if self.vectors.is_some() {
            std::borrow::Cow::Borrowed(&self.rows()[id * self.meta.dim..(id + 1) * self.meta.dim])
        } else {
            Random(0x5041_5448_4b45_4550_u64.wrapping_add(id as u64)).vector(self.meta.dim).into()
        }
    }
    // Regenerate synthetic input in bounded batches; no corpus-sized f32 allocation/file.
    // Build times include generating/copying these batches, identically for every candidate.
    fn batches(&self) -> impl Iterator<Item = Vec<f32>> + '_ {
        (0..self.meta.n).step_by(4096).map(|start| {
            let mut rows = Vec::with_capacity((self.meta.n - start).min(4096) * self.meta.dim);
            for id in start..(start + 4096).min(self.meta.n) {
                rows.extend_from_slice(&self.row(id));
            }
            rows
        })
    }
}
fn mapped_rows(map: &Mmap) -> &[f32] {
    const { assert!(cfg!(target_endian = "little")) };
    assert_eq!(map.len() % 4, 0);
    assert_eq!(map.as_ptr().align_offset(std::mem::align_of::<f32>()), 0);
    // SAFETY: immutable mapping, checked alignment and length, and all f32 bit patterns are valid.
    unsafe { std::slice::from_raw_parts(map.as_ptr().cast(), map.len() / 4) }
}

#[derive(Clone, Copy, Debug)]
struct Hit {
    score: f32,
    id: u64,
}
impl PartialEq for Hit {
    fn eq(&self, other: &Self) -> bool {
        self.cmp(other) == Ordering::Equal
    }
}
impl Eq for Hit {}
impl PartialOrd for Hit {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}
impl Ord for Hit {
    fn cmp(&self, other: &Self) -> Ordering {
        self.score.total_cmp(&other.score).then_with(|| other.id.cmp(&self.id))
    }
}
fn retain(heap: &mut BinaryHeap<Reverse<Hit>>, hit: Hit, k: usize) {
    if heap.len() < k {
        heap.push(Reverse(hit));
    } else if heap.peek().is_some_and(|worst| hit > worst.0) {
        *heap.peek_mut().unwrap() = Reverse(hit);
    }
}
fn sorted(heap: BinaryHeap<Reverse<Hit>>) -> Vec<Hit> {
    let mut result: Vec<_> = heap.into_iter().map(|x| x.0).collect();
    result.sort_unstable_by(|a, b| b.cmp(a));
    result
}
fn parallel_top(n: usize, k: usize, score: impl Fn(usize) -> Option<f32> + Sync) -> Vec<Hit> {
    let heaps: Vec<_> = (0..n)
        .into_par_iter()
        .chunks(8192)
        .map(|chunk| {
            let mut heap = BinaryHeap::with_capacity(k + 1);
            for id in chunk {
                if let Some(value) = score(id) {
                    retain(&mut heap, Hit { score: value, id: id as u64 + 1 }, k);
                }
            }
            heap
        })
        .collect();
    let mut merged = BinaryHeap::with_capacity(k + 1);
    for heap in heaps {
        for hit in heap {
            retain(&mut merged, hit.0, k);
        }
    }
    sorted(merged)
}

struct Resident {
    dim: usize,
    codes: Vec<i8>,
    scales: Vec<f32>,
    bits: Vec<u64>,
}
impl Resident {
    fn build(data: &Dataset, binary: bool) -> Self {
        let dim = data.meta.dim;
        let mut result = Self {
            dim,
            codes: Vec::with_capacity(data.meta.n * dim),
            scales: Vec::with_capacity(data.meta.n),
            bits: Vec::with_capacity(if binary { data.meta.n * dim.div_ceil(64) } else { 0 }),
        };
        for batch in data.batches() {
            for row in batch.chunks_exact(dim) {
                let vector = quantize_int8(row);
                result.codes.extend_from_slice(&vector.codes);
                result.scales.push(vector.scale);
                if binary {
                    for chunk in row.chunks(64) {
                        result.bits.push(
                            chunk
                                .iter()
                                .enumerate()
                                .fold(0, |bits, (i, x)| bits | (u64::from(*x >= 0.) << i)),
                        );
                    }
                }
            }
        }
        result
    }
    fn search(
        &self,
        query: &[f32],
        k: usize,
        allow: Option<&HashSet<u64>>,
        binary: bool,
    ) -> Vec<Hit> {
        let q = quantize_int8(query);
        let score = |id: usize| {
            let dot: i64 = self.codes[id * self.dim..(id + 1) * self.dim]
                .iter()
                .zip(&q.codes)
                .map(|(&a, &b)| i64::from(a) * i64::from(b))
                .sum();
            dot as f32 * self.scales[id] * q.scale
        };
        let eligible = |id: usize| allow.is_none_or(|set| set.contains(&(id as u64 + 1)));
        if !binary {
            return parallel_top(self.scales.len(), k, |id| eligible(id).then(|| score(id)));
        }
        let words = self.dim.div_ceil(64);
        let query_bits: Vec<_> = query
            .chunks(64)
            .map(|chunk| {
                chunk.iter().enumerate().fold(0, |bits, (i, x)| bits | (u64::from(*x >= 0.) << i))
            })
            .collect();
        let shortlist = parallel_top(self.scales.len(), 2000.max(20 * k), |id| {
            eligible(id).then(|| {
                -(self.bits[id * words..(id + 1) * words]
                    .iter()
                    .zip(&query_bits)
                    .map(|(a, b)| (a ^ b).count_ones())
                    .sum::<u32>() as f32)
            })
        });
        let mut heap = BinaryHeap::with_capacity(k + 1);
        for hit in shortlist {
            retain(&mut heap, Hit { id: hit.id, score: score(hit.id as usize - 1) }, k);
        }
        sorted(heap)
    }
    fn write(&self, path: &Path) -> Result<()> {
        let mut writer = BufWriter::new(File::create(path)?);
        writer.write_all(&(self.dim as u64).to_le_bytes())?;
        writer.write_all(&(self.scales.len() as u64).to_le_bytes())?;
        for code in &self.codes {
            writer.write_all(&code.to_le_bytes())?;
        }
        for scale in &self.scales {
            writer.write_all(&scale.to_le_bytes())?;
        }
        for bits in &self.bits {
            writer.write_all(&bits.to_le_bytes())?;
        }
        writer.flush()?;
        Ok(())
    }
}

struct Filter {
    name: &'static str,
    ids: Option<Vec<u64>>,
    set: Option<HashSet<u64>>,
}
fn filters(data: &Dataset) -> Vec<Filter> {
    let mut filters = vec![Filter { name: "all", ids: None, set: None }];
    if let Some(groups) = &data.groups {
        let mut counts = BTreeMap::<u32, usize>::new();
        for group in groups {
            *counts.entry(*group).or_default() += 1;
        }
        for (name, fraction) in [("sites-1pct", 0.01), ("sites-10pct", 0.1)] {
            // Ascending group ids make selection independent of hash iteration and file ordering.
            let mut selected = HashSet::new();
            let target = (data.meta.n as f64 * fraction).round() as usize;
            let mut count = 0;
            for (group, size) in &counts {
                if count + size <= target {
                    selected.insert(*group);
                    count += size;
                }
            }
            // A dominant site must not turn the 1% case into a 50% allowlist. Fill with
            // whole groups below target first, then take an overshoot only if closer.
            if let Some((&group, &size)) = counts
                .iter()
                .filter(|(id, _)| !selected.contains(*id))
                .min_by_key(|(id, size)| ((count + **size).abs_diff(target), **id))
            {
                if selected.is_empty() || (count + size).abs_diff(target) < target - count {
                    selected.insert(group);
                }
            }
            let ids: Vec<_> = groups
                .iter()
                .enumerate()
                .filter_map(|(i, group)| selected.contains(group).then_some(i as u64 + 1))
                .collect();
            println!(
                "Allowlist {name}: {}/{} vectors ({:.3}%), whole sites",
                ids.len(),
                data.meta.n,
                ids.len() as f64 / data.meta.n as f64 * 100.
            );
            filters.push(Filter { name, set: Some(ids.iter().copied().collect()), ids: Some(ids) });
        }
    }
    filters
}

#[derive(Serialize)]
struct Record<'a> {
    #[serde(flatten)]
    build: &'a Built,
    candidate: &'a str,
    dataset: &'a Meta,
    real: bool,
    threads: usize,
    k: usize,
    filter: &'a str,
    eligible: usize,
    queries: usize,
    read_bytes_per_query: u64,
    read_bytes_note: &'a str,
    staged: bool,
    open_ms: [f64; 3],
    search_ms: [f64; 3],
    interactive_ms: [f64; 3],
    recall: Option<f64>,
}
fn distribution(values: &mut [f64]) -> [f64; 3] {
    values.sort_by(f64::total_cmp);
    let at = |q: f64| values[((values.len() - 1) as f64 * q).ceil() as usize];
    [at(0.5), at(0.95), *values.last().unwrap()]
}
fn append(path: &Path, value: &impl Serialize) -> Result<()> {
    let mut file = OpenOptions::new().create(true).append(true).open(path)?;
    serde_json::to_writer(&mut file, value)?;
    writeln!(file)?;
    Ok(())
}
fn skip(out: &Path, candidate: &str, meta: &Meta, reason: &str) -> Result<()> {
    eprintln!("SKIP {candidate} n={} dim={}: {reason}", meta.n, meta.dim);
    append(
        out,
        &serde_json::json!({"status":"skipped", "candidate":candidate, "dataset":meta, "reason":reason}),
    )
}
fn tree_bytes(root: &Path) -> Result<u64> {
    let mut size = 0;
    for item in fs::read_dir(root)? {
        let path = item?.path();
        size += if path.is_dir() { tree_bytes(&path)? } else { path.metadata()?.len() };
    }
    Ok(size)
}
// Mirrors turbovec 1.1.2's private planes_wanted dispatch, including its opt-out knobs.
fn staged(n: usize, dim: usize, bits: usize) -> bool {
    let off = |key| std::env::var(key).is_ok_and(|v| v != "0");
    let disabled = std::env::var(format!("TURBOVEC_{bits}BIT_PLANES")).as_deref() == Ok("0");
    if n < 32768 || dim % 32 != 0 || disabled {
        return false;
    }
    let vector_major_off = off("TURBOVEC_NO_VECTOR_MAJOR") || off("TURBOVEC_NO_VNNI");
    #[cfg(target_arch = "x86_64")]
    {
        !vector_major_off
            && std::is_x86_feature_detected!("avx512vbmi")
            && std::is_x86_feature_detected!("avx512vnni")
            && std::is_x86_feature_detected!("avx512f")
            && std::is_x86_feature_detected!("avx512bw")
    }
    #[cfg(target_arch = "aarch64")]
    {
        if bits == 2 {
            std::env::var("TURBOVEC_2BIT_VM8").as_deref() != Ok("1")
        } else {
            !vector_major_off && std::arch::is_aarch64_feature_detected!("dotprod")
        }
    }
    #[cfg(not(any(target_arch = "x86_64", target_arch = "aarch64")))]
    {
        let _ = vector_major_off;
        false
    }
}

#[derive(Serialize)]
struct Built {
    build_ms: f64,
    resident_bytes: usize,
    before: Footprint,
    after: Footprint,
    disk_bytes: u64,
    write_ms: f64,
    load_ms: f64,
}
// Ground truths are saved once at k=100 per filter/query, outside every candidate's timed region.
type Truth = Vec<Vec<Vec<Hit>>>;
fn truths(data: &Dataset, filters: &[Filter]) -> Truth {
    filters
        .iter()
        .map(|filter| {
            data.queries
                .iter()
                .map(|q| {
                    parallel_top(data.meta.n, 100, |id| {
                        let id = id as u64 + 1;
                        filter
                            .set
                            .as_ref()
                            .is_none_or(|set| set.contains(&id))
                            .then(|| data.exact_score(q, id))
                    })
                })
                .collect()
        })
        .collect()
}

struct Measurement<'a> {
    data: &'a Dataset,
    filters: &'a [Filter],
    truth: Option<&'a Truth>,
    out: &'a Path,
    name: &'a str,
    threads: usize,
    built: &'a Built,
}
impl Measurement<'_> {
    fn run(
        &self,
        mut search: impl FnMut(&[f32], usize, &Filter) -> Result<(f64, Option<f64>, Vec<Hit>)>,
    ) -> Result<()> {
        for (fi, filter) in self.filters.iter().enumerate() {
            for k in [10, 100] {
                let mut opens = Vec::new();
                let mut scans = Vec::new();
                let mut totals = Vec::new();
                let mut hits = 0;
                let mut expected = 0;
                for (qi, query) in self.data.queries.iter().enumerate() {
                    let begin = Instant::now();
                    let (open_ms, search_ms, result) = search(query, k, filter)?;
                    let elapsed = begin.elapsed().as_secs_f64() * 1000.;
                    ensure!(
                        result.len()
                            == k.min(filter.ids.as_ref().map_or(self.data.meta.n, Vec::len)),
                        "wrong result count"
                    );
                    ensure!(
                        result.iter().all(|hit| hit.score.is_finite()
                            && filter.set.as_ref().is_none_or(|set| set.contains(&hit.id))),
                        "invalid/filtered hit"
                    );
                    opens.push(open_ms);
                    scans.push(search_ms.unwrap_or(elapsed - open_ms));
                    totals.push(elapsed);
                    if let Some(truth) = self.truth {
                        // Identical pages tie, so any result whose exact score reaches the
                        // exact k-th score is a hit, whichever copy the scan returned.
                        let exact = &truth[fi][qi][..k.min(truth[fi][qi].len())];
                        let floor = exact.last().map_or(f32::NEG_INFINITY, |hit| hit.score) - 1e-5;
                        expected += exact.len();
                        hits += result
                            .iter()
                            .filter(|hit| self.data.exact_score(query, hit.id) >= floor)
                            .count()
                            .min(exact.len());
                    }
                }
                let n = self.data.meta.n;
                let dim = self.data.meta.dim;
                let eligible = filter.ids.as_ref().map_or(n, Vec::len);
                let (read_bytes, note) = match self.name {
                    "shipped" => (
                        n * (8 + dim.div_ceil(8) + 12 + dim),
                        "disk layout payload incl. per-query binary open; all int8 scored before filter; excludes JSON headers",
                    ),
                    "int8" => (
                        n * usize::from(filter.set.is_some()) + eligible * (dim + 4),
                        "resident payload; +n conservative filter-probe bytes, excludes hash table cache amplification",
                    ),
                    "binary-int8" => (
                        eligible * dim.div_ceil(64) * 8
                            + eligible.min(2000.max(20 * k)) * (dim + 4),
                        "resident eligible sign bits + shortlist codes/scales; excludes filter hash probes",
                    ),
                    "exact" => {
                        (eligible * dim * 4, "f32 source payload; excludes filter hash probes")
                    }
                    _ => (
                        n * (dim * if self.name == "turbovec-4" { 4 } else { 2 } / 8 + 4) + k * 8,
                        "dense compressed payload upper bound + output ids; staged search is query-dependent; excludes calibration/mask/scratch passes",
                    ),
                };
                let b = self.built;
                let record = Record {
                    build: b,
                    candidate: self.name,
                    dataset: &self.data.meta,
                    real: self.truth.is_some(),
                    threads: self.threads,
                    k,
                    filter: filter.name,
                    eligible,
                    queries: self.data.queries.len(),
                    read_bytes_per_query: read_bytes as u64,
                    read_bytes_note: note,
                    staged: self.name.starts_with("turbovec")
                        && staged(n, dim, if self.name == "turbovec-4" { 4 } else { 2 }),
                    open_ms: distribution(&mut opens),
                    search_ms: distribution(&mut scans),
                    interactive_ms: distribution(&mut totals),
                    recall: self.truth.map(|_| hits as f64 / expected.max(1) as f64),
                };
                println!(
                    "| {} | {} | {} | {} | {} | {} | {:.1} | {:.2} | {:.1}/{:.1} | {:.2} | {:.2} | {:.2}/{:.2} | {:.2}/{:.2}/{:.2} | {:.2}/{:.2}/{:.2} | {} | {} |",
                    self.name,
                    n,
                    dim,
                    self.threads,
                    k,
                    filter.name,
                    b.build_ms,
                    b.resident_bytes as f64 / 1e6,
                    b.before.current as f64 / 1e6,
                    b.after.current as f64 / 1e6,
                    b.disk_bytes as f64 / 1e6,
                    read_bytes as f64 / 1e6,
                    b.write_ms,
                    b.load_ms,
                    record.open_ms[0],
                    record.open_ms[1],
                    record.open_ms[2],
                    record.search_ms[0],
                    record.search_ms[1],
                    record.search_ms[2],
                    record.recall.map_or_else(|| "—".into(), |r| format!("{r:.4}")),
                    record.staged
                );
                append(self.out, &record)?;
            }
        }
        Ok(())
    }
}

fn run_dataset(data: &Dataset, tmp: &Path, threads: &[usize], out: &Path) -> Result<()> {
    let filters = filters(data);
    println!(
        "\n| candidate | n | dim | threads | k | filter | build ms | own MB | phys before/after MB | disk MB | read MB/query | write/load ms | open p50/p95/max ms | search p50/p95/max ms | recall@k | staged |\n|---|---:|---:|---:|---:|---|---:|---:|---|---:|---:|---|---|---|---:|---|"
    );
    let truth = if data.groups.is_some() {
        let pool = rayon::ThreadPoolBuilder::new().num_threads(*threads.last().unwrap()).build()?;
        Some(pool.install(|| truths(data, &filters)))
    } else {
        None
    };
    let n = data.meta.n;
    let dim = data.meta.dim;
    // There is no public plane writer; don't bypass the production format with a homemade writer.
    if n > 4_000_000 {
        skip(
            out,
            "shipped",
            &data.meta,
            "no public direct plane writer; refusing .pkvec above 4M",
        )?;
    } else if !disk_fits(tmp, (n * (8 + 4 * dim + 8 + dim.div_ceil(8) + 12 + dim)) as u64 + 4096)? {
        skip(out, "shipped", &data.meta, "persisting source and planes would leave <10 GB free")?;
    } else {
        let dir = tempfile::tempdir_in(tmp)?;
        let paths = project_paths_with_root(dir.path());
        let before = footprint();
        let begin = Instant::now();
        let fingerprint = EmbeddingFingerprint::new(
            "scan-bench",
            "unit",
            dim,
            EmbeddingDtype::Float32,
            true,
            EmbeddingPooling::Mean,
            None,
        );
        let store = VectorStore::create_stamped(&paths, &fingerprint)?;
        for (batch, rows) in data.batches().enumerate() {
            let items = rows
                .chunks_exact(dim)
                .enumerate()
                .map(|(i, row)| ((batch * 4096 + i + 1) as u64, row.to_vec()))
                .collect::<Vec<_>>();
            store.append_vectors(&items)?;
        }
        build_planes_from_store(&paths, "scan-bench", "unit")?;
        let build_ms = begin.elapsed().as_secs_f64() * 1000.;
        begin_allocation_count();
        let index = FlatVectorIndex::open(&paths, "scan-bench", "unit")?;
        let resident = end_allocation_count();
        let after = footprint();
        drop(index);
        let built = Built {
            build_ms,
            resident_bytes: resident,
            before,
            after,
            disk_bytes: tree_bytes(dir.path())?,
            write_ms: 0.,
            load_ms: 0.,
        };
        Measurement {
            data,
            filters: &filters,
            truth: truth.as_ref(),
            out,
            name: "shipped",
            threads: 1,
            built: &built,
        }
        .run(|q, k, filter| {
            let begin = Instant::now();
            let index = FlatVectorIndex::open(&paths, "scan-bench", "unit")?;
            let open = begin.elapsed().as_secs_f64() * 1000.;
            let start = Instant::now();
            let result = index.search(q, k, filter.ids.as_deref())?;
            let search_ms = start.elapsed().as_secs_f64() * 1000.;
            let result = result.into_iter().map(|(id, score)| Hit { id, score }).collect();
            Ok((open, Some(search_ms), result))
        })?;
    }
    for &count in threads {
        let pool = rayon::ThreadPoolBuilder::new().num_threads(count).build()?;
        pool.broadcast(|_| {
            std::hint::black_box(Vec::<u8>::with_capacity(64));
        });
        for (name, binary) in [("int8", false), ("binary-int8", true)] {
            let bytes = n * (dim + 4 + if binary { dim.div_ceil(64) * 8 } else { 0 }) + 16;
            if !disk_fits(tmp, bytes as u64)? {
                skip(out, name, &data.meta, "persisting index would leave <10 GB free")?;
                continue;
            }
            let before = footprint();
            begin_allocation_count();
            let begin = Instant::now();
            let index = Resident::build(data, binary);
            let build_ms = begin.elapsed().as_secs_f64() * 1000.;
            let resident = end_allocation_count();
            let after = footprint();
            let dir = tempfile::tempdir_in(tmp)?;
            let path = dir.path().join("resident.bin");
            let begin = Instant::now();
            index.write(&path)?;
            let write_ms = begin.elapsed().as_secs_f64() * 1000.;
            let built = Built {
                build_ms,
                resident_bytes: resident,
                before,
                after,
                disk_bytes: path.metadata()?.len(),
                write_ms,
                load_ms: 0.,
            };
            pool.install(|| {
                Measurement {
                    data,
                    filters: &filters,
                    truth: truth.as_ref(),
                    out,
                    name,
                    threads: count,
                    built: &built,
                }
                .run(|q, k, filter| Ok((0., None, index.search(q, k, filter.set.as_ref(), binary))))
            })?;
        }
        for (name, bits) in [("turbovec-4", 4), ("turbovec-2", 2)] {
            if dim % 8 != 0 {
                skip(out, name, &data.meta, "turbovec requires dim a positive multiple of 8")?;
                continue;
            }
            if !disk_fits(tmp, (n * (dim * bits / 8 + 32) + 4096) as u64)? {
                skip(out, name, &data.meta, "persisting index would leave <10 GB free")?;
                continue;
            }
            let before = footprint();
            begin_allocation_count();
            let begin = Instant::now();
            let index = pool.install(|| -> Result<_> {
                let mut index = IdMapIndex::new(dim, bits)?;
                let mut random = Random(0x4341_4c49_4252_4154);
                let mut chosen = HashSet::new();
                while chosen.len() < 2048.min(n) {
                    chosen.insert(random.next() as usize % n);
                }
                let mut chosen = chosen.into_iter().collect::<Vec<_>>();
                chosen.sort_unstable();
                let sample =
                    chosen.iter().flat_map(|&i| data.row(i).into_owned()).collect::<Vec<_>>();
                index.calibrate(&sample)?;
                for (batch, rows) in data.batches().enumerate() {
                    let ids = (batch * 4096 + 1..batch * 4096 + rows.len() / dim + 1)
                        .map(|i| i as u64)
                        .collect::<Vec<_>>();
                    index.add_with_ids(&rows, &ids)?;
                }
                index.prepare();
                let _ = index.try_search(&data.queries[0], 100)?;
                Ok(index)
            })?;
            let build_ms = begin.elapsed().as_secs_f64() * 1000.;
            let resident = end_allocation_count();
            let after = footprint();
            let dir = tempfile::tempdir_in(tmp)?;
            let path = dir.path().join("index.tvim");
            let begin = Instant::now();
            index.write(&path)?;
            let write_ms = begin.elapsed().as_secs_f64() * 1000.;
            let begin = Instant::now();
            let loaded = IdMapIndex::load(&path)?;
            let load_ms = begin.elapsed().as_secs_f64() * 1000.;
            let probe = &data.queries[0];
            ensure!(
                pool.install(|| index.search(probe, 10).1 == loaded.search(probe, 10).1),
                "turbovec persistence changed results"
            );
            drop(loaded);
            let built = Built {
                build_ms,
                resident_bytes: resident,
                before,
                after,
                disk_bytes: path.metadata()?.len(),
                write_ms,
                load_ms,
            };
            pool.install(|| {
                Measurement {
                    data,
                    filters: &filters,
                    truth: truth.as_ref(),
                    out,
                    name,
                    threads: count,
                    built: &built,
                }
                .run(|q, k, filter| {
                    let result = index.try_search_with_allowlist(q, k, filter.ids.as_deref())?;
                    Ok((
                        0.,
                        None,
                        result
                            .ids
                            .into_iter()
                            .zip(result.scores)
                            .map(|(id, score)| Hit { id, score })
                            .collect(),
                    ))
                })
            })?;
        }
    }
    run_exact(data, &filters, truth.as_ref(), tmp, threads, out)
}

fn run_exact(
    data: &Dataset,
    filters: &[Filter],
    truth: Option<&Truth>,
    tmp: &Path,
    threads: &[usize],
    out: &Path,
) -> Result<()> {
    let n = data.meta.n;
    let dim = data.meta.dim;
    let bytes = (n * dim * 4) as u64;
    let dir = tempfile::tempdir_in(tmp)?;
    let before = footprint();
    let begin = Instant::now();
    let mut write_ms = 0.;
    let mut load_ms = 0.;
    let mapping = if data.vectors.is_none() {
        if !disk_fits(tmp, bytes)? {
            return skip(
                out,
                "exact",
                &data.meta,
                "synthetic f32 exact index would leave <10 GB free",
            );
        }
        let path = dir.path().join("exact.f32");
        let mut file = BufWriter::new(File::create(&path)?);
        for batch in data.batches() {
            for value in batch {
                file.write_all(&value.to_le_bytes())?;
            }
        }
        file.flush()?;
        drop(file);
        write_ms = begin.elapsed().as_secs_f64() * 1000.;
        let start = Instant::now();
        // SAFETY: this owned file remains immutable until after the mapping is dropped.
        let map = unsafe { Mmap::map(&File::open(&path)?)? };
        load_ms = start.elapsed().as_secs_f64() * 1000.;
        Some(map)
    } else {
        None
    };
    let rows = mapping.as_ref().map_or_else(|| data.rows(), mapped_rows);
    let built = Built {
        build_ms: if mapping.is_some() { begin.elapsed().as_secs_f64() * 1000. } else { 0. },
        resident_bytes: bytes as usize,
        before,
        after: footprint(),
        disk_bytes: bytes,
        write_ms,
        load_ms,
    };
    for &count in threads {
        let pool = rayon::ThreadPoolBuilder::new().num_threads(count).build()?;
        pool.install(|| {
            Measurement { data, filters, truth, out, name: "exact", threads: count, built: &built }
                .run(|q, k, filter| {
                    Ok((
                        0.,
                        None,
                        parallel_top(n, k, |id| {
                            filter
                                .set
                                .as_ref()
                                .is_none_or(|set| set.contains(&(id as u64 + 1)))
                                .then(|| {
                                    rows[id * dim..(id + 1) * dim]
                                        .iter()
                                        .zip(q)
                                        .map(|(a, b)| a * b)
                                        .sum()
                                })
                        }),
                    ))
                })
        })?;
    }
    Ok(())
}

fn env_list(name: &str, default: &str) -> Result<Vec<usize>> {
    let values = std::env::var(name)
        .unwrap_or_else(|_| default.into())
        .split(',')
        .map(|v| v.trim().parse::<usize>())
        .collect::<std::result::Result<Vec<_>, _>>()?;
    ensure!(
        !values.is_empty() && values.iter().all(|x| *x > 0),
        "{name} must contain positive integers"
    );
    Ok(values)
}

#[test]
fn vector_scan_benchmark() -> Result<()> {
    if std::env::var("PATHKEEP_SCAN_BENCH").as_deref() != Ok("1") {
        eprintln!("SKIP: set PATHKEEP_SCAN_BENCH=1");
        return Ok(());
    }
    let out = PathBuf::from(
        std::env::var_os("PATHKEEP_SCAN_OUT").context("set PATHKEEP_SCAN_OUT to a JSONL file")?,
    );
    let tmp = PathBuf::from(
        std::env::var_os("PATHKEEP_SCAN_TMP")
            .unwrap_or_else(|| std::env::temp_dir().into_os_string()),
    );
    fs::create_dir_all(&tmp)?;
    let threads = env_list("PATHKEEP_SCAN_THREADS", "1,4")?;
    println!(
        "\nSource: real immutable f32 mmap or batch-generated synthetic vectors; {}. No synthetic recall. Times in ms; sizes in decimal MB. Real exact reuses the source (build/write/load N/A=0); synthetic exact writes one f32 file. Build includes batch generation/copying. Exact own MB is logical mmap size, not heap. Shipped open/search are timed separately; interactive samples also include index destruction. N/A times use 0 in JSONL.",
        footprint().method
    );

    if let Some(real) = std::env::var_os("PATHKEEP_SCAN_REAL") {
        let mut data = Dataset::open(Path::new(&real))?;
        // Latency settles long before thousands of queries; recall over every query is
        // measured elsewhere. An even spread keeps the sample representative.
        if let Ok(limit) = std::env::var("PATHKEEP_SCAN_QUERIES") {
            let limit: usize = limit.trim().parse()?;
            ensure!(limit > 0, "PATHKEEP_SCAN_QUERIES must be positive");
            let step = data.queries.len().div_ceil(limit);
            data.queries = data.queries.into_iter().step_by(step).collect();
            data.meta.q = data.queries.len();
        }
        run_dataset(&data, &tmp, &threads, &out)?;
    } else {
        let q = env_list("PATHKEEP_SCAN_QUERIES", "200")?;
        ensure!(q.len() == 1, "PATHKEEP_SCAN_QUERIES must be one integer");
        for n in env_list("PATHKEEP_SCAN_SCALES", "1000000,4000000,14400000")? {
            for dim in env_list("PATHKEEP_SCAN_DIMS", "256")? {
                ensure!(dim != 1024 || n <= 4_000_000, "refusing dim=1024 above 4M");
                let meta = Meta { n, dim, q: q[0], model: "synthetic-unit-splitmix64".into() };
                let mut random = Random(0x5155_4552_4945_5300);
                let queries = (0..meta.q).map(|_| random.vector(dim)).collect();
                let data = Dataset { meta, vectors: None, queries, groups: None };
                let dir = tempfile::tempdir_in(&tmp)?;
                run_dataset(&data, dir.path(), &threads, &out)?;
            }
        }
    }
    Ok(())
}
