//! Python ecosystem support
//!
//! Handles:
//! - Lockfile parsing: poetry.lock, uv.lock, requirements.txt, pyproject.toml
//! - PyPI repository URL detection via registry API

mod lockfile;
mod pypi;

pub use lockfile::{LockfileError, find_lockfile_path, find_version, list_direct_dependencies};
pub use pypi::detect_repo_url;
