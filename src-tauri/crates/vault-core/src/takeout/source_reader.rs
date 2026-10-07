//! Responsibilities: bound memory while consuming local files and ZIP entries.
//! Not responsible for: classification, parser rules or database writes.
//! Failure modes: whole-file allocation, a ZIP entry outlives its archive, I/O
//! failures or ZIP checksum errors after a JSON value disappear. Tests: Takeout import and parser reader_tests.
use super::*;
use std::io::{BufReader, Read};
pub(super) fn with_reader<T>(
    source: &Path,
    file: &TakeoutFile,
    cancel: &vault_store::CancelToken,
    read: impl FnOnce(&mut dyn Read) -> Result<T>,
) -> Result<T> {
    if file.from_zip {
        let mut zip = ZipArchive::new(fs::File::open(source)?)?;
        let entry = zip.by_name(&file.path)?;
        finish(&mut BufReader::new(entry), cancel, read)
    } else {
        finish(&mut BufReader::new(fs::File::open(&file.path)?), cancel, read)
    }
}

fn finish<T>(
    reader: &mut dyn Read,
    cancel: &vault_store::CancelToken,
    read: impl FnOnce(&mut dyn Read) -> Result<T>,
) -> Result<T> {
    let result = read(reader)?;
    // JSON permits trailing bytes. Still drain the file to check transport/ZIP integrity.
    let mut bytes = [0; 8192];
    loop {
        cancel.check()?;
        if reader.read(&mut bytes)? == 0 {
            break;
        }
    }
    Ok(result)
}
