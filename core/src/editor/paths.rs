//! Paths and ids in a project file come from the file, which may have been made by someone
//! else. Nothing read from it is joined onto the work folder without passing through here.

use std::path::{Component, Path, PathBuf};

/// `work/rel` if `rel` is a plain relative path (no `..`, no root, no drive), else `None`.
pub fn inside(work: &Path, rel: &str) -> Option<PathBuf> {
    let p = Path::new(rel);
    let plain = !rel.is_empty()
        && p.components().all(|c| matches!(c, Component::Normal(_)))
        && !rel.contains('\\');
    plain.then(|| work.join(p))
}

/// A line id as a file name part: kept if it is only letters, digits, `_` and `-`, otherwise
/// replaced by a hash of it, so an id can never name a folder or climb out of one.
pub fn file_part(id: &str) -> String {
    if !id.is_empty()
        && id
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-')
    {
        id.to_string()
    } else {
        format!("id-{}", super::hash::hash8(id))
    }
}
