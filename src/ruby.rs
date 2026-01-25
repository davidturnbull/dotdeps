//! Ruby ecosystem support
//!
//! Handles:
//! - Lockfile parsing: Gemfile.lock
//! - RubyGems repository URL detection via registry API

mod lockfile;
mod rubygems;

pub use lockfile::{LockfileError, find_lockfile_path, find_version, list_direct_dependencies};
pub use rubygems::detect_repo_url;
