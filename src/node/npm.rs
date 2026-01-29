//! npm registry integration
//!
//! Fetches package metadata from npm to detect repository URLs.

use serde::Deserialize;
use thiserror::Error;

#[derive(Error, Debug)]
pub enum NpmError {
    #[error("Failed to fetch package info from npm: {0}")]
    Fetch(String),

    #[error(
        "Repository URL not found for '{package}'. Add override to ~/.config/dotdeps/config.json"
    )]
    RepoNotFound { package: String },

    #[error("Failed to parse npm response: {0}")]
    Parse(String),
}

/// Detect the repository URL for a Node.js package via npm registry API
pub fn detect_repo_url(package: &str) -> Result<String, NpmError> {
    // npm registry URL - scoped packages need URL encoding for the slash
    let url = if package.starts_with('@') {
        // Encode the package name: @scope/name -> @scope%2fname
        let encoded = package.replace('/', "%2f");
        format!("https://registry.npmjs.org/{}", encoded)
    } else {
        format!("https://registry.npmjs.org/{}", package)
    };

    let response = ureq::get(&url)
        .call()
        .map_err(|e| NpmError::Fetch(e.to_string()))?;

    let body = response
        .into_body()
        .read_to_string()
        .map_err(|e| NpmError::Parse(e.to_string()))?;

    let metadata: NpmMetadata =
        serde_json::from_str(&body).map_err(|e| NpmError::Parse(e.to_string()))?;

    extract_repo_url(&metadata, package)
}

/// npm registry JSON API response structure
#[derive(Deserialize)]
struct NpmMetadata {
    repository: Option<Repository>,
    homepage: Option<String>,
}

/// Repository field can be a string or an object
#[derive(Deserialize)]
#[serde(untagged)]
enum Repository {
    String(String),
    Object(RepositoryObject),
}

#[derive(Deserialize)]
struct RepositoryObject {
    url: Option<String>,
}

/// Extract repository URL from npm metadata
///
/// Looks for URLs in this order:
/// 1. repository.url (if object) or repository (if string)
/// 2. homepage (if it looks like a git repo)
fn extract_repo_url(metadata: &NpmMetadata, package: &str) -> Result<String, NpmError> {
    // Try repository field first
    if let Some(repo) = &metadata.repository {
        match repo {
            Repository::String(url) => {
                if let Some(normalized) = normalize_repo_url(url) {
                    return Ok(normalized);
                }
            }
            Repository::Object(obj) => {
                if let Some(url) = &obj.url
                    && let Some(normalized) = normalize_repo_url(url)
                {
                    return Ok(normalized);
                }
            }
        }
    }

    // Fall back to homepage if it looks like a repo
    if let Some(homepage) = &metadata.homepage
        && is_known_git_host(homepage)
        && let Some(normalized) = normalize_repo_url(homepage)
    {
        return Ok(normalized);
    }

    Err(NpmError::RepoNotFound {
        package: package.to_string(),
    })
}

/// Check if a URL is hosted on a known git hosting service
fn is_known_git_host(url: &str) -> bool {
    let url_lower = url.to_lowercase();
    url_lower.contains("github.com")
        || url_lower.contains("gitlab.com")
        || url_lower.contains("bitbucket.org")
        || url_lower.contains("codeberg.org")
        || url_lower.contains("sr.ht")
}

/// Normalize a repository URL to HTTPS format suitable for cloning
///
/// Handles various npm repository URL formats:
/// - git+https://github.com/user/repo.git
/// - git://github.com/user/repo.git
/// - git+ssh://git@github.com/user/repo.git
/// - github:user/repo
/// - https://github.com/user/repo
fn normalize_repo_url(url: &str) -> Option<String> {
    let url = url.trim();

    // Handle empty URLs
    if url.is_empty() {
        return None;
    }

    // Handle github shorthand: github:user/repo or user/repo
    if let Some(stripped) = url.strip_prefix("github:") {
        return Some(format!("https://github.com/{}.git", stripped));
    }

    // If it looks like just user/repo (common shorthand)
    if !url.contains("://") && !url.starts_with("git@") && url.contains('/') {
        let parts: Vec<&str> = url.split('/').collect();
        if parts.len() == 2 && !parts[0].is_empty() && !parts[1].is_empty() {
            return Some(format!("https://github.com/{}.git", url));
        }
    }

    // Strip git+ prefix
    let url = url
        .strip_prefix("git+")
        .map(|s| s.to_string())
        .unwrap_or_else(|| url.to_string());

    // Convert git:// to https://
    let url = if url.starts_with("git://") {
        url.replacen("git://", "https://", 1)
    } else {
        url
    };

    // Convert ssh://git@host/path to https://host/path
    let url = if url.starts_with("ssh://git@") {
        url.replacen("ssh://git@", "https://", 1)
    } else if url.starts_with("ssh://") {
        url.replacen("ssh://", "https://", 1)
    } else {
        url
    };

    // Convert git@host:user/repo to https://host/user/repo
    let url = if url.starts_with("git@") {
        let without_prefix = url.strip_prefix("git@").unwrap();
        if let Some(colon_idx) = without_prefix.find(':') {
            let host = &without_prefix[..colon_idx];
            let path = &without_prefix[colon_idx + 1..];
            format!("https://{}/{}", host, path)
        } else {
            url
        }
    } else {
        url
    };

    // Ensure https:// prefix
    let url = if !url.starts_with("https://") && !url.starts_with("http://") {
        format!("https://{}", url)
    } else {
        url
    };

    // Remove trailing slashes
    let mut url = url;
    while url.ends_with('/') {
        url.pop();
    }

    // Only return URLs that are from known git hosts
    if !is_known_git_host(&url) {
        return None;
    }

    // Ensure .git suffix for cloning
    if !url.ends_with(".git") {
        url.push_str(".git");
    }

    Some(url)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_is_known_git_host() {
        assert!(is_known_git_host("https://github.com/lodash/lodash"));
        assert!(is_known_git_host("https://gitlab.com/user/repo"));
        assert!(is_known_git_host("https://bitbucket.org/user/repo"));
        assert!(!is_known_git_host("https://lodash.com"));
        assert!(!is_known_git_host("https://example.com"));
    }

    #[test]
    fn test_normalize_repo_url_https() {
        assert_eq!(
            normalize_repo_url("https://github.com/lodash/lodash"),
            Some("https://github.com/lodash/lodash.git".to_string())
        );
    }

    #[test]
    fn test_normalize_repo_url_with_git_suffix() {
        assert_eq!(
            normalize_repo_url("https://github.com/lodash/lodash.git"),
            Some("https://github.com/lodash/lodash.git".to_string())
        );
    }

    #[test]
    fn test_normalize_repo_url_git_plus_https() {
        assert_eq!(
            normalize_repo_url("git+https://github.com/lodash/lodash.git"),
            Some("https://github.com/lodash/lodash.git".to_string())
        );
    }

    #[test]
    fn test_normalize_repo_url_git_protocol() {
        assert_eq!(
            normalize_repo_url("git://github.com/lodash/lodash.git"),
            Some("https://github.com/lodash/lodash.git".to_string())
        );
    }

    #[test]
    fn test_normalize_repo_url_git_ssh() {
        assert_eq!(
            normalize_repo_url("git@github.com:lodash/lodash.git"),
            Some("https://github.com/lodash/lodash.git".to_string())
        );
    }

    #[test]
    fn test_normalize_repo_url_git_plus_ssh() {
        // git+ssh://git@github.com/user/repo.git format (used by glob and others)
        assert_eq!(
            normalize_repo_url("git+ssh://git@github.com/isaacs/node-glob.git"),
            Some("https://github.com/isaacs/node-glob.git".to_string())
        );
    }

    #[test]
    fn test_normalize_repo_url_ssh_protocol() {
        assert_eq!(
            normalize_repo_url("ssh://git@github.com/user/repo.git"),
            Some("https://github.com/user/repo.git".to_string())
        );
    }

    #[test]
    fn test_normalize_repo_url_github_shorthand() {
        assert_eq!(
            normalize_repo_url("github:lodash/lodash"),
            Some("https://github.com/lodash/lodash.git".to_string())
        );
    }

    #[test]
    fn test_normalize_repo_url_user_repo_shorthand() {
        assert_eq!(
            normalize_repo_url("lodash/lodash"),
            Some("https://github.com/lodash/lodash.git".to_string())
        );
    }

    #[test]
    fn test_normalize_repo_url_trailing_slash() {
        assert_eq!(
            normalize_repo_url("https://github.com/lodash/lodash/"),
            Some("https://github.com/lodash/lodash.git".to_string())
        );
    }

    #[test]
    fn test_normalize_repo_url_non_git() {
        // Non-git URLs should return None
        assert_eq!(normalize_repo_url("https://lodash.com"), None);
        assert_eq!(normalize_repo_url("https://example.com/docs"), None);
    }

    #[test]
    fn test_extract_repo_url_string() {
        let metadata = NpmMetadata {
            repository: Some(Repository::String(
                "https://github.com/lodash/lodash".to_string(),
            )),
            homepage: None,
        };

        let result = extract_repo_url(&metadata, "lodash").unwrap();
        assert_eq!(result, "https://github.com/lodash/lodash.git");
    }

    #[test]
    fn test_extract_repo_url_object() {
        let metadata = NpmMetadata {
            repository: Some(Repository::Object(RepositoryObject {
                url: Some("git+https://github.com/lodash/lodash.git".to_string()),
            })),
            homepage: None,
        };

        let result = extract_repo_url(&metadata, "lodash").unwrap();
        assert_eq!(result, "https://github.com/lodash/lodash.git");
    }

    #[test]
    fn test_extract_repo_url_homepage_fallback() {
        let metadata = NpmMetadata {
            repository: None,
            homepage: Some("https://github.com/lodash/lodash".to_string()),
        };

        let result = extract_repo_url(&metadata, "lodash").unwrap();
        assert_eq!(result, "https://github.com/lodash/lodash.git");
    }

    #[test]
    fn test_extract_repo_url_not_found() {
        let metadata = NpmMetadata {
            repository: None,
            homepage: Some("https://lodash.com".to_string()),
        };

        let result = extract_repo_url(&metadata, "lodash");
        assert!(matches!(result, Err(NpmError::RepoNotFound { .. })));
    }
}
