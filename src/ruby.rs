//! Ruby ecosystem support
//!
//! Handles:
//! - Lockfile parsing: Gemfile.lock
//! - RubyGems repository URL detection via registry API

mod lockfile;
mod rubygems;

pub use lockfile::find_version;
pub use rubygems::detect_repo_url;
