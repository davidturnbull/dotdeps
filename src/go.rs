//! Go ecosystem support
//!
//! Handles:
//! - Lockfile parsing: go.sum (and go.mod for require statements)
//! - Repository URL detection is handled in main.rs since Go module paths are URLs

mod lockfile;

pub use lockfile::{LockfileError, find_lockfile_path, find_version, list_direct_dependencies};
