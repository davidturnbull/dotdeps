//! Shared lockfile discovery helpers
//!
//! Provides utilities for finding files by walking up the directory tree.

use std::path::{Path, PathBuf};

/// Find the nearest matching file by walking up from the current directory.
///
/// `filenames` are checked in order at each directory level.
pub fn find_nearest_file(filenames: &[&str]) -> Option<PathBuf> {
    let cwd = std::env::current_dir().ok()?;
    find_nearest_file_from(&cwd, filenames)
}

/// Find the nearest matching file by walking up from a start directory.
///
/// `filenames` are checked in order at each directory level.
pub fn find_nearest_file_from(start: &Path, filenames: &[&str]) -> Option<PathBuf> {
    let mut dir = start;
    loop {
        for filename in filenames {
            let path = dir.join(filename);
            if path.exists() {
                return Some(path);
            }
        }

        match dir.parent() {
            Some(parent) => dir = parent,
            None => break,
        }
    }

    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_find_nearest_file_from_missing() {
        let temp = std::env::temp_dir();
        let result = find_nearest_file_from(&temp, &["does-not-exist.txt"]);
        assert!(result.is_none());
    }
}
