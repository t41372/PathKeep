//! Responsibilities: read owned or mapped matrix rows and add their weighted components without
//! copying rows. Not responsible for file verification, shape, token counts or normalization.

use std::ops::Range;
#[cfg(not(test))]
use std::sync::Arc;

#[derive(Debug, Clone)]
pub(super) enum MatrixStorage {
    Owned(Vec<f32>),
    #[cfg(not(test))]
    Mapped {
        mapping: Arc<memmap2::Mmap>,
        data: Range<usize>,
    },
}

impl MatrixStorage {
    pub(super) fn len(&self) -> usize {
        match self {
            Self::Owned(values) => values.len(),
            #[cfg(not(test))]
            Self::Mapped { data, .. } => data.len() / size_of::<f32>(),
        }
    }

    /// Dispatch once per row to keep storage selection out of the component loop.
    pub(super) fn add_scaled_row(&self, range: Range<usize>, scale: f32, sum: &mut [f32]) {
        match self {
            Self::Owned(values) => add_scaled_values(sum, values[range].iter().copied(), scale),
            #[cfg(not(test))]
            Self::Mapped { mapping, data } => {
                let start = data.start + range.start * size_of::<f32>();
                let end = data.start + range.end * size_of::<f32>();
                add_scaled_values(sum, mapping[start..end].chunks_exact(4).map(decode_f32), scale);
            }
        }
    }

    fn component(&self, index: usize) -> f32 {
        match self {
            Self::Owned(values) => values[index],
            #[cfg(not(test))]
            Self::Mapped { mapping, data } => {
                let start = data.start + index * size_of::<f32>();
                decode_f32(&mapping[start..start + size_of::<f32>()])
            }
        }
    }
}

impl PartialEq for MatrixStorage {
    fn eq(&self, other: &Self) -> bool {
        self.len() == other.len()
            && (0..self.len()).all(|index| self.component(index) == other.component(index))
    }
}

/// Both backings use the same arithmetic and component order for model2vec parity.
fn add_scaled_values(sum: &mut [f32], values: impl Iterator<Item = f32>, scale: f32) {
    for (accumulator, component) in sum.iter_mut().zip(values) {
        *accumulator += component * scale;
    }
}

#[cfg(not(test))]
fn decode_f32(bytes: &[u8]) -> f32 {
    f32::from_le_bytes([bytes[0], bytes[1], bytes[2], bytes[3]])
}
