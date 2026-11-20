//! Keep unchanged passage vectors across publications. Only content present in the new
//! projection is copied. A page cache avoids loading all vector values into RAM.
use super::{Generation, SemanticFiles};
use anyhow::{Result, ensure};
use std::collections::HashMap;
use vault_store::CancelToken;
#[derive(Default)]
pub(super) struct Previous {
    generation: String,
    stride: usize,
    slots: HashMap<[u8; 32], (u32, usize)>,
    cached: Option<(u32, Vec<u8>)>,
    pub copied: u64,
}
impl Previous {
    pub fn load(
        files: &SemanticFiles,
        previous: Option<&Generation>,
        cancel: &CancelToken,
    ) -> Result<Self> {
        let Some(g) = previous else {
            return Ok(Self::default());
        };
        let stride = g.dimension + 36;
        let mut slots = HashMap::new();
        for page in 0..g.vector_pages {
            cancel.check()?;
            let bytes = files.read(Some(&g.generation), &format!("vectors-{page}"))?;
            ensure!(bytes.len() % stride == 0, "incomplete serving vector page");
            for (i, record) in bytes.chunks_exact(stride).enumerate() {
                slots.insert(record[..32].try_into()?, (page, i * stride));
            }
        }
        Ok(Self { generation: g.generation.clone(), stride, slots, ..Default::default() })
    }
    pub fn contains(&self, hash: &[u8; 32]) -> bool {
        self.slots.contains_key(hash)
    }
    pub fn append(
        &mut self,
        files: &SemanticFiles,
        hash: &[u8; 32],
        out: &mut Vec<u8>,
    ) -> Result<bool> {
        let Some(&(page, offset)) = self.slots.get(hash) else {
            return Ok(false);
        };
        if self.cached.as_ref().is_none_or(|(p, _)| *p != page) {
            self.cached =
                Some((page, files.read(Some(&self.generation), &format!("vectors-{page}"))?));
        }
        let bytes = &self.cached.as_ref().unwrap().1;
        out.extend_from_slice(&bytes[offset..offset + self.stride]);
        self.copied += 1;
        Ok(true)
    }
}
