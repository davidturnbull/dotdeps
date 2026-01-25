//! crates.io registry integration
//!
//! Fetches crate metadata from crates.io to detect repository URLs.

use serde::Deserialize;
use thiserror::Error;

#[derive(Error, Debug)]
pub enum CratesIoError {
    #[error("Failed to fetch crate info from crates.io: {0}")]
    Fetch(String),

    #[error(
        "Repository URL not found for '{package}'. Add override to ~/.config/dotdeps/config.json"
    )]
    RepoNotFound { package: String },

    #[error("Failed to parse crates.io response: {0}")]
    Parse(String),
}

/// Detect the repository URL for a Rust crate via crates.io API
pub fn detect_repo_url(package: &str) -> Result<String, CratesIoError> {
    let url = format!("https://crates.io/api/v1/crates/{}", package);

    let response = ureq::get(&url)
        .header("User-Agent", "dotdeps (https://github.com/dotdeps/dotdeps)")
        .call()
        .map_err(|e| CratesIoError::Fetch(e.to_string()))?;

    let body = response
        .into_body()
        .read_to_string()
        .map_err(|e| CratesIoError::Parse(e.to_string()))?;

    let metadata: CratesIoResponse =
        serde_json::from_str(&body).map_err(|e| CratesIoError::Parse(e.to_string()))?;

    extract_repo_url(&metadata, package)
}

/// crates.io JSON API response structure
#[derive(Deserialize)]
struct CratesIoResponse {
    #[serde(rename = "crate")]
    crate_info: CrateInfo,
}

#[derive(Deserialize)]
struct CrateInfo {
    repository: Option<String>,
    homepage: Option<String>,
}

/// Extract repository URL from crates.io metadata
fn extract_repo_url(metadata: &CratesIoResponse, package: &str) -> Result<String, CratesIoError> {
    // First try the repository field
    if let Some(repo) = &metadata.crate_info.repository
        && is_git_repo_url(repo)
    {
        return Ok(normalize_git_url(repo));
    }

    // Fall back to homepage if it looks like a repo
    if let Some(homepage) = &metadata.crate_info.homepage
        && is_git_repo_url(homepage)
    {
        return Ok(normalize_git_url(homepage));
    }

    Err(CratesIoError::RepoNotFound {
        package: package.to_string(),
    })
}

/// Check if a URL looks like a git repository
fn is_git_repo_url(url: &str) -> bool {
    let url_lower = url.to_lowercase();
    url_lower.contains("github.com")
        || url_lower.contains("gitlab.com")
        || url_lower.contains("bitbucket.org")
        || url_lower.contains("codeberg.org")
        || url_lower.contains("sr.ht")
        || url_lower.ends_with(".git")
}

/// Normalize a git URL to HTTPS format suitable for cloning
fn normalize_git_url(url: &str) -> String {
    let mut url = url.trim().to_string();

    // Remove trailing slashes
    while url.ends_with('/') {
        url.pop();
    }

    // Add .git suffix if not present
    if !url.ends_with(".git") {
        url.push_str(".git");
    }

    url
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_is_git_repo_url() {
        assert!(is_git_repo_url("https://github.com/serde-rs/serde"));
        assert!(is_git_repo_url("https://gitlab.com/user/repo"));
        assert!(is_git_repo_url("https://bitbucket.org/user/repo"));
        assert!(is_git_repo_url("https://example.com/repo.git"));
        assert!(!is_git_repo_url("https://docs.rs/serde"));
        assert!(!is_git_repo_url("https://example.com"));
    }

    #[test]
    fn test_normalize_git_url() {
        assert_eq!(
            normalize_git_url("https://github.com/serde-rs/serde"),
            "https://github.com/serde-rs/serde.git"
        );
        assert_eq!(
            normalize_git_url("https://github.com/serde-rs/serde/"),
            "https://github.com/serde-rs/serde.git"
        );
        assert_eq!(
            normalize_git_url("https://github.com/serde-rs/serde.git"),
            "https://github.com/serde-rs/serde.git"
        );
    }

    #[test]
    fn test_extract_repo_url_from_repository() {
        let metadata = CratesIoResponse {
            crate_info: CrateInfo {
                repository: Some("https://github.com/serde-rs/serde".to_string()),
                homepage: Some("https://serde.rs".to_string()),
            },
        };

        let result = extract_repo_url(&metadata, "serde").unwrap();
        assert_eq!(result, "https://github.com/serde-rs/serde.git");
    }

    #[test]
    fn test_extract_repo_url_from_homepage_fallback() {
        let metadata = CratesIoResponse {
            crate_info: CrateInfo {
                repository: None,
                homepage: Some("https://github.com/example/pkg".to_string()),
            },
        };

        let result = extract_repo_url(&metadata, "pkg").unwrap();
        assert_eq!(result, "https://github.com/example/pkg.git");
    }

    #[test]
    fn test_extract_repo_url_not_found() {
        let metadata = CratesIoResponse {
            crate_info: CrateInfo {
                repository: None,
                homepage: Some("https://example.com".to_string()),
            },
        };

        let result = extract_repo_url(&metadata, "pkg");
        assert!(matches!(result, Err(CratesIoError::RepoNotFound { .. })));
    }
}
