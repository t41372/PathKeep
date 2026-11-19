//! Authenticated navigation extents. Version columns support planning; this page binds
//! their meaning to the immutable owner, including the legitimate zero-posting case.
use super::*;
use crate::Archive;
use vault_store::OptionalRow;

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub(super) struct Extents {
    pub records: u64,
    pub visible: u64,
    pub blocks: u64,
    pub nonempty: u64,
    pub postings: u64,
    pub lookup: u64,
    pub directories: u64,
    pub ambiguous: u64,
}
impl Extents {
    fn stored(a: &Archive, owner: &str) -> Result<Self> {
        let key = owner.to_owned();
        a.store.read(DB,move|tx| {
            tx.query_one("SELECT records,visible_total,rank_pages,nonempty_blocks,
                postings,lookup_pages,directory_pages,ambiguous_edges FROM ah_reader_version WHERE token=?1",
                [key],|row|Ok(Self{
                    records:row.get::<i64>(0)? as u64,visible:row.get::<i64>(1)? as u64,
                    blocks:row.get::<i64>(2)? as u64,nonempty:row.get::<i64>(3)? as u64,
                    postings:row.get::<i64>(4)? as u64,lookup:row.get::<i64>(5)? as u64,
                    directories:row.get::<i64>(6)? as u64,ambiguous:row.get::<i64>(7)? as u64,
                })).optional()
        })?.ok_or(Error::ReaderRebuildRequired)
    }
    pub fn load(a: &Archive, owner: &str, job: &JobControl) -> Result<Self> {
        let (_, mut rows) =
            pages::load::<Self>(a, owner, EXTENTS, 0, job).map_err(navigation_error)?;
        let sealed = rows.pop().filter(|_| rows.is_empty()).ok_or(Error::ReaderRebuildRequired)?;
        if Self::stored(a, owner)? != sealed {
            return Err(Error::ReaderRebuildRequired);
        }
        Ok(sealed)
    }
    pub fn seal(a: &Archive, owner: &str) -> Result<()> {
        let extent = Self::stored(a, owner)?;
        let mut buffer = pages::Buffer::new(EXTENTS, pages::META_BYTES);
        buffer.push(a, owner, &extent)?;
        buffer.flush(a, owner)?;
        Ok(())
    }
}
