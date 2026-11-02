//! Responsibilities: sort visit projections with bounded shutdown checkpoints.
//! Not responsible for: file formats or index publication.
use anyhow::Result;
use vault_store::CancelToken;

/// Limit each uninterruptible sort to one block, then checkpoint during merge passes.
pub(super) fn sort(
    pairs: &mut Vec<(i64, u64)>,
    key: impl Fn(&(i64, u64)) -> (u64, u64),
    cancel: &CancelToken,
) -> Result<()> {
    const BLOCK: usize = 32_768;
    for chunk in pairs.chunks_mut(BLOCK) {
        cancel.check()?;
        chunk.sort_unstable_by_key(&key);
    }
    let mut width = BLOCK;
    let mut merged = Vec::with_capacity(pairs.len());
    while width < pairs.len() {
        for start in (0..pairs.len()).step_by(width * 2) {
            let middle = (start + width).min(pairs.len());
            let end = (middle + width).min(pairs.len());
            let (mut left, mut right) = (start, middle);
            while left < middle || right < end {
                if merged.len() % 1024 == 0 {
                    cancel.check()?;
                }
                if left < middle && (right == end || key(&pairs[left]) <= key(&pairs[right])) {
                    merged.push(pairs[left]);
                    left += 1;
                } else {
                    merged.push(pairs[right]);
                    right += 1;
                }
            }
        }
        std::mem::swap(pairs, &mut merged);
        merged.clear();
        width *= 2;
    }
    cancel.check()?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn projection_sort_crosses_blocks_and_honors_shutdown() {
        // Failure cases: block boundaries lose/duplicate pairs, signed ids sort differently than
        // the on-disk u64 lookup, or an already closing archive sorts an entire corpus.
        let mut pairs =
            (0..100_003_i64).rev().map(|id| (id - 3, (id % 97) as u64)).collect::<Vec<_>>();
        let mut expected = pairs.clone();
        expected.sort_unstable_by_key(|p| (p.0 as u64, p.1));
        sort(&mut pairs, |p| (p.0 as u64, p.1), &CancelToken::new()).unwrap();
        assert_eq!(pairs, expected);
        let cancel = CancelToken::new();
        cancel.cancel();
        assert!(sort(&mut pairs, |p| (p.1, p.0 as u64), &cancel).is_err());
    }
}
