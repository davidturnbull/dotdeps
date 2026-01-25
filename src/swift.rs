//! Swift ecosystem support
//!
//! Handles:
//! - Lockfile parsing: Package.resolved (v1 and v2 formats)
//! - Repository URL detection from Package.resolved

mod lockfile;

pub use lockfile::{detect_repo_url, find_version};
