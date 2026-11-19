//! Responsibilities: bound no-follow git reads through capture's descriptor traversal.
//! Not responsible for: resolving the user's folder alias or interpreting git contents.
use super::{AuthorizedPath, FileMark, changed, open_regular, reject_link, validate_after};
use crate::{Error, Result};
use std::{fs, io::Read, path::Path};

/// Gives the bounded git observer the same descriptor-relative no-follow read
/// boundary as capture, without making a transcript path a general file reader.
pub(crate) fn read_git_file(path: &Path, limit: usize) -> Result<Vec<u8>> {
    let root = path
        .ancestors()
        .last()
        .ok_or_else(|| Error::InvalidInput("git path must be absolute".into()))?;
    // Preserve the original pathname for descriptor traversal. A canonicalized
    // replacement could follow a directory swapped after component validation.
    let metadata = fs::symlink_metadata(path)?;
    reject_link(&metadata)?;
    let authorized = AuthorizedPath {
        path: path.to_owned(),
        root: root.to_owned(),
        mark: FileMark::from(&metadata),
    };
    let mut input = open_regular(&authorized, None).map_err(|error| {
        #[cfg(unix)]
        if matches!(&error, Error::Io(cause)
            if matches!(cause.raw_os_error(), Some(libc::ELOOP | libc::ENOTDIR)))
        {
            return Error::InvalidInput("git path component became a symlink".into());
        }
        error
    })?;
    if !input.metadata()?.is_file() || FileMark::from(&input.metadata()?) != authorized.mark {
        return Err(changed("git file changed before open"));
    }
    if authorized.mark.len > limit as u64 {
        return Err(Error::LimitExceeded("git file size".into()));
    }
    let mut bytes = Vec::new();
    (&mut input).take((limit + 1) as u64).read_to_end(&mut bytes)?;
    if bytes.len() > limit {
        return Err(Error::LimitExceeded("git file size".into()));
    }
    validate_after(&input, path, root, &authorized)?;
    Ok(bytes)
}
