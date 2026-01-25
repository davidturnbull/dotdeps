//! Rust ecosystem support
//!
//! Handles:
//! - Lockfile parsing: Cargo.lock
//! - crates.io repository URL detection via registry API

mod crates_io;
mod lockfile;

pub use crates_io::detect_repo_url;
pub use lockfile::find_version;
