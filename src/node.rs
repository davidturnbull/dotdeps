//! Node.js ecosystem support
//!
//! Handles:
//! - Lockfile parsing: pnpm-lock.yaml, yarn.lock, package-lock.json
//! - npm repository URL detection via registry API

mod lockfile;
mod npm;

pub use lockfile::{LockfileError, find_lockfile_path, find_version, list_direct_dependencies};
pub use npm::detect_repo_url;
