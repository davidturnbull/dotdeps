//! Rust ecosystem support
//!
//! Handles:
//! - Lockfile parsing: Cargo.lock
//! - crates.io repository URL detection via registry API

mod crates_io;
mod lockfile;

pub use crates_io::detect_repo_url;
pub use lockfile::{LockfileError, find_lockfile_path, find_version, list_direct_dependencies};
