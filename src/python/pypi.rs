//! PyPI registry integration
//!
//! Fetches package metadata from PyPI to detect repository URLs.

use serde::Deserialize;
use std::collections::HashMap;
use thiserror::Error;

#[derive(Error, Debug)]
pub enum PyPiError {
    #[error("Failed to fetch package info from PyPI: {0}")]
    Fetch(String),

    #[error(
        "Repository URL not found for '{package}'. Add override to ~/.config/dotdeps/config.json"
    )]
    RepoNotFound { package: String },

    #[error("Failed to parse PyPI response: {0}")]
    Parse(String),
}

/// Detect the repository URL for a Python package via PyPI API
pub fn detect_repo_url(package: &str) -> Result<String, PyPiError> {
    let url = format!("https://pypi.org/pypi/{}/json", package);

    let response = ureq::get(&url)
        .call()
        .map_err(|e| PyPiError::Fetch(e.to_string()))?;

    let body = response
        .into_body()
        .read_to_string()
        .map_err(|e| PyPiError::Parse(e.to_string()))?;

    let metadata: PyPiMetadata =
        serde_json::from_str(&body).map_err(|e| PyPiError::Parse(e.to_string()))?;

    extract_repo_url(&metadata, package)
}

/// PyPI JSON API response structure
#[derive(Deserialize)]
struct PyPiMetadata {
    info: PackageInfo,
}

#[derive(Deserialize)]
struct PackageInfo {
    project_urls: Option<HashMap<String, String>>,
    home_page: Option<String>,
}

/// Extract repository URL from PyPI metadata
///
/// Looks for URLs in this order:
/// 1. project_urls["Source"] or project_urls["source"]
/// 2. project_urls["Repository"] or project_urls["repository"]
/// 3. project_urls["Source Code"] or project_urls["source code"]
/// 4. project_urls["Code"] or project_urls["code"]
/// 5. project_urls["GitHub"]
/// 6. home_page (if it looks like a git repo)
fn extract_repo_url(metadata: &PyPiMetadata, package: &str) -> Result<String, PyPiError> {
    // Priority list of keys to check in project_urls
    let source_keys = [
        "Source",
        "source",
        "Repository",
        "repository",
        "Source Code",
        "source code",
        "Code",
        "code",
        "GitHub",
        "github",
        "Homepage",
        "homepage",
    ];

    if let Some(project_urls) = &metadata.info.project_urls {
        for key in source_keys {
            if let Some(url) = project_urls.get(key)
                && is_git_repo_url(url)
            {
                return Ok(normalize_git_url(url));
            }
        }

        // If no explicit source key found, try any URL that looks like a repo
        for url in project_urls.values() {
            if is_git_repo_url(url) {
                return Ok(normalize_git_url(url));
            }
        }
    }

    // Fall back to home_page if it looks like a repo
    if let Some(home_page) = &metadata.info.home_page
        && is_git_repo_url(home_page)
    {
        return Ok(normalize_git_url(home_page));
    }

    Err(PyPiError::RepoNotFound {
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

    // Add .git suffix if not present (needed for some git hosts)
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
        assert!(is_git_repo_url("https://github.com/psf/requests"));
        assert!(is_git_repo_url("https://gitlab.com/user/repo"));
        assert!(is_git_repo_url("https://bitbucket.org/user/repo"));
        assert!(is_git_repo_url("https://example.com/repo.git"));
        assert!(!is_git_repo_url("https://requests.readthedocs.io"));
        assert!(!is_git_repo_url("https://example.com"));
    }

    #[test]
    fn test_normalize_git_url() {
        assert_eq!(
            normalize_git_url("https://github.com/psf/requests"),
            "https://github.com/psf/requests.git"
        );
        assert_eq!(
            normalize_git_url("https://github.com/psf/requests/"),
            "https://github.com/psf/requests.git"
        );
        assert_eq!(
            normalize_git_url("https://github.com/psf/requests.git"),
            "https://github.com/psf/requests.git"
        );
    }

    #[test]
    fn test_extract_repo_url_from_source() {
        let metadata = PyPiMetadata {
            info: PackageInfo {
                project_urls: Some(HashMap::from([
                    (
                        "Documentation".to_string(),
                        "https://docs.example.com".to_string(),
                    ),
                    (
                        "Source".to_string(),
                        "https://github.com/example/pkg".to_string(),
                    ),
                ])),
                home_page: None,
            },
        };

        let result = extract_repo_url(&metadata, "pkg").unwrap();
        assert_eq!(result, "https://github.com/example/pkg.git");
    }

    #[test]
    fn test_extract_repo_url_from_homepage_fallback() {
        let metadata = PyPiMetadata {
            info: PackageInfo {
                project_urls: Some(HashMap::from([(
                    "Documentation".to_string(),
                    "https://docs.example.com".to_string(),
                )])),
                home_page: Some("https://github.com/example/pkg".to_string()),
            },
        };

        let result = extract_repo_url(&metadata, "pkg").unwrap();
        assert_eq!(result, "https://github.com/example/pkg.git");
    }

    #[test]
    fn test_extract_repo_url_not_found() {
        let metadata = PyPiMetadata {
            info: PackageInfo {
                project_urls: Some(HashMap::from([(
                    "Documentation".to_string(),
                    "https://docs.example.com".to_string(),
                )])),
                home_page: Some("https://example.com".to_string()),
            },
        };

        let result = extract_repo_url(&metadata, "pkg");
        assert!(matches!(result, Err(PyPiError::RepoNotFound { .. })));
    }
}
