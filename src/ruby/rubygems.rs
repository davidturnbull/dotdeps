//! RubyGems registry integration
//!
//! Fetches gem metadata from RubyGems.org to detect repository URLs.
//!
//! API endpoint: https://rubygems.org/api/v1/gems/{name}.json

use serde::Deserialize;
use thiserror::Error;

#[derive(Error, Debug)]
pub enum RubyGemsError {
    #[error("Failed to fetch gem info from RubyGems: {0}")]
    Fetch(String),

    #[error(
        "Repository URL not found for '{package}'. Add override to ~/.config/dotdeps/config.json"
    )]
    RepoNotFound { package: String },

    #[error("Failed to parse RubyGems response: {0}")]
    Parse(String),
}

/// Detect the repository URL for a Ruby gem via RubyGems.org API
pub fn detect_repo_url(package: &str) -> Result<String, RubyGemsError> {
    let url = format!("https://rubygems.org/api/v1/gems/{}.json", package);

    let response = ureq::get(&url)
        .header("User-Agent", "dotdeps (https://github.com/dotdeps/dotdeps)")
        .call()
        .map_err(|e| RubyGemsError::Fetch(e.to_string()))?;

    let body = response
        .into_body()
        .read_to_string()
        .map_err(|e| RubyGemsError::Parse(e.to_string()))?;

    let metadata: RubyGemsResponse =
        serde_json::from_str(&body).map_err(|e| RubyGemsError::Parse(e.to_string()))?;

    extract_repo_url(&metadata, package)
}

/// RubyGems.org JSON API response structure
#[derive(Deserialize)]
struct RubyGemsResponse {
    source_code_uri: Option<String>,
    homepage_uri: Option<String>,
}

/// Extract repository URL from RubyGems metadata
///
/// Priority:
/// 1. source_code_uri (most reliable for repo URL)
/// 2. homepage_uri (fallback if it looks like a git repo)
fn extract_repo_url(metadata: &RubyGemsResponse, package: &str) -> Result<String, RubyGemsError> {
    // First try source_code_uri - this is specifically for source code
    if let Some(source_uri) = &metadata.source_code_uri
        && !source_uri.is_empty()
        && is_git_repo_url(source_uri)
    {
        return Ok(normalize_git_url(source_uri));
    }

    // Fall back to homepage_uri if it looks like a git repo
    if let Some(homepage) = &metadata.homepage_uri
        && !homepage.is_empty()
        && is_git_repo_url(homepage)
    {
        return Ok(normalize_git_url(homepage));
    }

    Err(RubyGemsError::RepoNotFound {
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
///
/// Handles various URL formats:
/// - Plain repo URLs: https://github.com/rails/rails
/// - Tree URLs: https://github.com/rails/rails/tree/v8.1.2
/// - Blob URLs: https://github.com/rails/rails/blob/main/README.md
fn normalize_git_url(url: &str) -> String {
    let mut url = url.trim().to_string();

    // Remove trailing slashes
    while url.ends_with('/') {
        url.pop();
    }

    // Strip GitHub-specific path suffixes: /tree/..., /blob/..., /releases/...
    // These appear when source_code_uri points to a specific version page
    for pattern in ["/tree/", "/blob/", "/releases/"] {
        if let Some(idx) = url.find(pattern) {
            url.truncate(idx);
            break;
        }
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
        assert!(is_git_repo_url("https://github.com/rails/rails"));
        assert!(is_git_repo_url("https://gitlab.com/user/repo"));
        assert!(is_git_repo_url("https://bitbucket.org/user/repo"));
        assert!(is_git_repo_url("https://example.com/repo.git"));
        assert!(!is_git_repo_url("https://rubygems.org/gems/rails"));
        assert!(!is_git_repo_url("https://example.com"));
    }

    #[test]
    fn test_normalize_git_url() {
        assert_eq!(
            normalize_git_url("https://github.com/rails/rails"),
            "https://github.com/rails/rails.git"
        );
        assert_eq!(
            normalize_git_url("https://github.com/rails/rails/"),
            "https://github.com/rails/rails.git"
        );
        assert_eq!(
            normalize_git_url("https://github.com/rails/rails.git"),
            "https://github.com/rails/rails.git"
        );
    }

    #[test]
    fn test_normalize_git_url_strips_tree_path() {
        // RubyGems sometimes returns URLs like https://github.com/rails/rails/tree/v8.1.2
        assert_eq!(
            normalize_git_url("https://github.com/rails/rails/tree/v8.1.2"),
            "https://github.com/rails/rails.git"
        );
        assert_eq!(
            normalize_git_url("https://github.com/rails/rails/tree/main"),
            "https://github.com/rails/rails.git"
        );
    }

    #[test]
    fn test_normalize_git_url_strips_blob_path() {
        assert_eq!(
            normalize_git_url("https://github.com/rails/rails/blob/main/README.md"),
            "https://github.com/rails/rails.git"
        );
    }

    #[test]
    fn test_normalize_git_url_strips_releases_path() {
        assert_eq!(
            normalize_git_url("https://github.com/rails/rails/releases/tag/v7.1.0"),
            "https://github.com/rails/rails.git"
        );
    }

    #[test]
    fn test_extract_repo_url_from_source_code_uri() {
        let metadata = RubyGemsResponse {
            source_code_uri: Some("https://github.com/rails/rails".to_string()),
            homepage_uri: Some("https://rubyonrails.org".to_string()),
        };

        let result = extract_repo_url(&metadata, "rails").unwrap();
        assert_eq!(result, "https://github.com/rails/rails.git");
    }

    #[test]
    fn test_extract_repo_url_from_homepage_fallback() {
        let metadata = RubyGemsResponse {
            source_code_uri: None,
            homepage_uri: Some("https://github.com/example/gem".to_string()),
        };

        let result = extract_repo_url(&metadata, "gem").unwrap();
        assert_eq!(result, "https://github.com/example/gem.git");
    }

    #[test]
    fn test_extract_repo_url_not_found() {
        let metadata = RubyGemsResponse {
            source_code_uri: None,
            homepage_uri: Some("https://example.com".to_string()),
        };

        let result = extract_repo_url(&metadata, "gem");
        assert!(matches!(result, Err(RubyGemsError::RepoNotFound { .. })));
    }

    #[test]
    fn test_extract_repo_url_empty_source_code_uri() {
        let metadata = RubyGemsResponse {
            source_code_uri: Some("".to_string()),
            homepage_uri: Some("https://github.com/example/gem".to_string()),
        };

        let result = extract_repo_url(&metadata, "gem").unwrap();
        assert_eq!(result, "https://github.com/example/gem.git");
    }
}
