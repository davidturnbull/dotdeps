//! Node.js ecosystem support
//!
//! Handles:
//! - Lockfile parsing: pnpm-lock.yaml, yarn.lock, package-lock.json
//! - npm repository URL detection via registry API

mod lockfile;
mod npm;

pub use lockfile::find_version;
pub use npm::detect_repo_url;
