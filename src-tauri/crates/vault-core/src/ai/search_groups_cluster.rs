//! Responsibilities: deterministically partition a bounded window with average linkage.
//! Not responsible for: reading history, excluding short searches, or formatting reports.
//! Failure modes: duplicate or missing membership; unstable ties/sample; fewer than two vectors;
//! identical/zero vectors; a low reciprocal pair stops another chain's valid merges; cubic work.
//! Tests: numeric edges and comparison with a direct global-best implementation below.

use anyhow::Result;

/// Rejects directions that cannot take part in cosine comparisons.
pub(super) fn normalize(vector: &mut [f32]) -> bool {
    let norm = dot(vector, vector).sqrt();
    if !norm.is_finite() || norm <= 0. {
        return false;
    }
    for value in vector {
        *value /= norm;
    }
    true
}

/// Independent accumulators keep the quadratic matrix fill vectorisable and repeatable.
pub(super) fn dot(a: &[f32], b: &[f32]) -> f32 {
    let mut sums = [0.; 8];
    let mut left = a.chunks_exact(8);
    let mut right = b.chunks_exact(8);
    for (a, b) in left.by_ref().zip(right.by_ref()) {
        for lane in 0..8 {
            sums[lane] += a[lane] * b[lane];
        }
    }
    sums.into_iter().sum::<f32>()
        + left.remainder().iter().zip(right.remainder()).map(|(a, b)| a * b).sum::<f32>()
}

/// Fixed sampling keeps a repeated read of the same ordered corpus reproducible.
pub(super) fn percentile(
    count: usize,
    percent: usize,
    mut score: impl FnMut(usize, usize) -> f32,
) -> f32 {
    if count < 2 {
        return 1.;
    }
    let mut seed = 0x706174686b656570_u64;
    let mut next = || {
        seed ^= seed << 13;
        seed ^= seed >> 7;
        seed ^= seed << 17;
        seed
    };
    let mut scores = (0..20_000)
        .map(|_| {
            let a = (next() % count as u64) as usize;
            let b = (a + 1 + (next() % (count - 1) as u64) as usize) % count;
            score(a, b)
        })
        .collect::<Vec<_>>();
    *scores.select_nth_unstable_by((20_000 * percent).div_ceil(100) - 1, f32::total_cmp).1
}

/// Average linkage is reducible: reciprocal neighbours may merge out of global score order.
/// Finish the chain tree, retaining only edges at the cut, so a low disjoint pair cannot stop it.
pub(super) fn cluster(
    vectors: &[Vec<f32>],
    cancel: &dyn Fn() -> Result<()>,
) -> Result<Vec<Vec<usize>>> {
    cancel()?;
    let n = vectors.len();
    anyhow::ensure!(n <= 4096, "search grouping is limited to 4,096 vectors");
    if n < 2 {
        return Ok((0..n).map(|i| vec![i]).collect());
    }
    let threshold = percentile(n, 98, |a, b| dot(&vectors[a], &vectors[b]));
    let mut matrix = vec![0.; n * n];
    for a in 0..n {
        cancel()?;
        for b in 0..a {
            let score = dot(&vectors[a], &vectors[b]);
            matrix[a * n + b] = score;
            matrix[b * n + a] = score;
        }
    }
    let mut size = vec![1_usize; n];
    let mut chain = Vec::new();
    let mut parent = (0..n).collect::<Vec<_>>();
    fn root(parent: &mut [usize], mut i: usize) -> usize {
        while parent[i] != i {
            parent[i] = parent[parent[i]];
            i = parent[i];
        }
        i
    }
    let mut remaining = n;
    while remaining > 1 {
        cancel()?;
        if chain.is_empty() {
            chain.push(size.iter().position(|s| *s > 0).unwrap());
        }
        let a = *chain.last().unwrap();
        // Prefer the predecessor on ties to guarantee reciprocal progress on identical vectors.
        let previous = chain.get(chain.len().wrapping_sub(2)).copied();
        let mut b = previous.unwrap_or_else(|| (0..n).find(|&b| b != a && size[b] > 0).unwrap());
        for c in 0..n {
            if c != a && size[c] > 0 && matrix[a * n + c] > matrix[a * n + b] {
                b = c;
            }
        }
        if previous != Some(b) {
            chain.push(b);
            continue;
        }
        chain.truncate(chain.len() - 2);
        let score = matrix[a * n + b];
        if score + 8. * f32::EPSILON >= threshold {
            let left = root(&mut parent, a);
            let right = root(&mut parent, b);
            parent[right] = left;
        }
        let total = size[a] + size[b];
        for c in 0..n {
            if c != a && c != b && size[c] > 0 {
                let average = (matrix[a * n + c] * size[a] as f32
                    + matrix[b * n + c] * size[b] as f32)
                    / total as f32;
                matrix[a * n + c] = average;
                matrix[c * n + a] = average;
            }
        }
        size[a] = total;
        size[b] = 0;
        remaining -= 1;
    }
    let mut groups = std::collections::BTreeMap::<usize, Vec<usize>>::new();
    for i in 0..n {
        let id = root(&mut parent, i);
        groups.entry(id).or_default().push(i);
    }
    let mut groups = groups.into_values().collect::<Vec<_>>();
    groups.sort();
    Ok(groups)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn small_and_identical_windows_are_repeatable_partitions() {
        assert!(cluster(&[], &|| Ok(())).unwrap().is_empty());
        assert_eq!(cluster(&[vec![1., 0.]], &|| Ok(())).unwrap(), vec![vec![0]]);
        assert_eq!(
            cluster(&vec![vec![0.6, 0.8]; 23], &|| Ok(())).unwrap(),
            vec![(0..23).collect::<Vec<_>>()]
        );
        assert!(!normalize(&mut [0., 0.]));
        assert_eq!(percentile(1, 98, |_, _| unreachable!()), 1.);
        assert!(cluster(&[vec![1.], vec![1.]], &|| anyhow::bail!("cancelled")).is_err());
        assert!(super::super::search_vectors::is_short("a1"));
        assert!(super::super::search_vectors::is_short("貓"));
        assert!(!super::super::search_vectors::is_short("貓咪"));
        assert!(!super::super::search_vectors::is_short("usb"));
    }

    #[test]
    fn nearest_neighbour_chain_matches_global_average_linkage() {
        for count in [2, 3, 17, 67] {
            let vectors = (0..count)
                .map(|i| {
                    let mut v = vec![(i * 17 % 31) as f32 - 15., (i * 7 % 23) as f32 - 11., 0.37];
                    normalize(&mut v);
                    v
                })
                .collect::<Vec<_>>();
            let cutoff = percentile(count, 98, |a, b| dot(&vectors[a], &vectors[b]));
            let mut expected = (0..count).map(|i| vec![i]).collect::<Vec<_>>();
            loop {
                let mut best = None;
                let mut score = f32::NEG_INFINITY;
                for a in 0..expected.len() {
                    for b in a + 1..expected.len() {
                        let similarity = expected[a]
                            .iter()
                            .flat_map(|&i| expected[b].iter().map(move |&j| (i, j)))
                            .map(|(i, j)| dot(&vectors[i], &vectors[j]))
                            .sum::<f32>()
                            / (expected[a].len() * expected[b].len()) as f32;
                        if similarity > score {
                            score = similarity;
                            best = Some((a, b));
                        }
                    }
                }
                if score + 8. * f32::EPSILON < cutoff {
                    break;
                }
                let Some((a, b)) = best else { break };
                let members = expected.remove(b);
                expected[a].extend(members);
            }
            for group in &mut expected {
                group.sort_unstable();
            }
            expected.sort();
            let actual = cluster(&vectors, &|| Ok(())).unwrap();
            assert_eq!(actual, expected);
            assert_eq!(actual, cluster(&vectors, &|| Ok(())).unwrap());
            let mut members = actual.into_iter().flatten().collect::<Vec<_>>();
            members.sort_unstable();
            assert_eq!(members, (0..count).collect::<Vec<_>>());
        }
    }
}
