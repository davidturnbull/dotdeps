use std::fs;
use std::path::PathBuf;

use crate::paths;

/// Normalize a formula name by stripping any tap prefix.
/// e.g., "homebrew/core/wget" -> "wget"
pub fn normalize_name(name: &str) -> &str {
    // If it contains a slash, it might be a full tap path
    // homebrew/core/wget -> wget
    // user/tap/formula -> formula
    if let Some(pos) = name.rfind('/') {
        &name[pos + 1..]
    } else {
        name
    }
}

/// Check if a formula exists by looking up the formula names cache.
pub fn exists(name: &str) -> bool {
    let normalized = normalize_name(name);

    // Check the API cache for formula names
    if let Some(cache_path) = get_formula_names_cache_path()
        && let Ok(contents) = fs::read_to_string(&cache_path)
    {
        return contents.lines().any(|line| line == normalized);
    }

    // Fallback: if cache doesn't exist, check if the formula is installed (opt symlink exists)
    let opt_path = paths::homebrew_prefix().join("opt").join(normalized);
    opt_path.exists()
}

/// Get the path to the formula names cache file.
fn get_formula_names_cache_path() -> Option<PathBuf> {
    let cache = paths::homebrew_cache();
    let path = cache.join("api/formula_names.txt");
    if path.exists() { Some(path) } else { None }
}
