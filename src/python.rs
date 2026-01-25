//! Python ecosystem support
//!
//! Handles:
//! - Lockfile parsing: poetry.lock, uv.lock, requirements.txt, pyproject.toml
//! - PyPI repository URL detection via registry API

mod lockfile;
mod pypi;

pub use lockfile::find_version;
pub use pypi::detect_repo_url;
