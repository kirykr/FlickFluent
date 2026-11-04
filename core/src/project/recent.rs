//! Recently opened projects, newest first, at most `MAX_RECENT`.

use std::path::Path;

use serde::{Deserialize, Serialize};

use crate::error::AppError;

pub const MAX_RECENT: usize = 20;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct RecentEntry {
    pub path: String,
    pub name: String,
    pub opened_at: String,
    /// Filled in on read: false when the file is gone.
    #[serde(default)]
    pub available: bool,
}

/// A missing or unreadable list file just means an empty list.
pub fn load(file: &Path) -> Vec<RecentEntry> {
    std::fs::read_to_string(file)
        .ok()
        .and_then(|t| serde_json::from_str(&t).ok())
        .unwrap_or_default()
}

pub fn save(file: &Path, list: &[RecentEntry]) -> Result<(), AppError> {
    if let Some(dir) = file.parent() {
        std::fs::create_dir_all(dir).map_err(AppError::io)?;
    }
    let json = serde_json::to_vec_pretty(list).map_err(|e| AppError::io(e.into()))?;
    std::fs::write(file, json).map_err(AppError::io)
}

/// Put an entry first, drop any older entry for the same path, cap the list.
pub fn push(list: &mut Vec<RecentEntry>, entry: RecentEntry) {
    list.retain(|e| e.path != entry.path);
    list.insert(0, entry);
    list.truncate(MAX_RECENT);
}

pub fn remove(list: &mut Vec<RecentEntry>, path: &str) {
    list.retain(|e| e.path != path);
}

pub fn with_availability(mut list: Vec<RecentEntry>) -> Vec<RecentEntry> {
    for e in &mut list {
        e.available = Path::new(&e.path).exists();
    }
    list
}

#[cfg(test)]
mod tests {
    use super::*;

    fn entry(path: &str) -> RecentEntry {
        RecentEntry {
            path: path.into(),
            name: path.into(),
            opened_at: String::new(),
            available: false,
        }
    }

    #[test]
    fn newest_first_without_duplicates() {
        let mut list = vec![];
        push(&mut list, entry("a"));
        push(&mut list, entry("b"));
        push(&mut list, entry("a"));
        let paths: Vec<_> = list.iter().map(|e| e.path.as_str()).collect();
        assert_eq!(paths, ["a", "b"]);
    }

    #[test]
    fn capped_at_twenty() {
        let mut list = vec![];
        for i in 0..30 {
            push(&mut list, entry(&format!("p{i}")));
        }
        assert_eq!(list.len(), MAX_RECENT);
        assert_eq!(list[0].path, "p29");
    }

    #[test]
    fn missing_files_are_marked_unavailable() {
        let dir = tempfile::tempdir().unwrap();
        let real = dir.path().join("x.flick");
        std::fs::write(&real, b"x").unwrap();
        let list = with_availability(vec![entry(real.to_str().unwrap()), entry("/no/such/file")]);
        assert!(list[0].available);
        assert!(!list[1].available);
    }

    #[test]
    fn unreadable_list_file_gives_an_empty_list() {
        let dir = tempfile::tempdir().unwrap();
        let f = dir.path().join("recent.json");
        std::fs::write(&f, b"{{{").unwrap();
        assert!(load(&f).is_empty());
        assert!(load(&dir.path().join("absent.json")).is_empty());
    }
}
