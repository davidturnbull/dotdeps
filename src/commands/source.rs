use crate::api::{self, FormulaInfo};
use regex::Regex;
use serde_json::Value;
use std::process::Command;

pub fn execute(args: &[String]) -> Result<(), Box<dyn std::error::Error>> {
    // If no arguments, open Homebrew's own repository
    if args.is_empty() {
        exec_browser(&[String::from("https://github.com/Homebrew/brew")])?;
        return Ok(());
    }

    // Load formula cache
    let formula_cache =
        api::load_formula_cache().map_err(|e| format!("Failed to load formula cache: {}", e))?;

    let mut repo_urls = Vec::new();

    for arg in args {
        if let Some(info) = formula_cache.get(arg) {
            if let Some(repo_url) = extract_repo_url(info) {
                println!("Opening repository for {}", arg);
                repo_urls.push(repo_url);
            } else {
                eprintln!("Warning: Could not determine repository URL for {}", arg);
            }
        } else {
            eprintln!("Error: No available formula with the name \"{}\"", arg);
            std::process::exit(1);
        }
    }

    if !repo_urls.is_empty() {
        exec_browser(&repo_urls)?;
    }

    Ok(())
}

fn extract_repo_url(info: &FormulaInfo) -> Option<String> {
    // Check head URL first
    if let Some(urls) = &info.urls
        && let Some(head) = &urls.head
        && let Some(repo) = url_to_repo(&head.url)
    {
        return Some(repo);
    }

    // Check stable URL
    if let Some(urls) = &info.urls
        && let Some(stable) = &urls.stable
        && let Some(repo) = url_to_repo(&stable.url)
    {
        return Some(repo);
    }

    // Check homepage
    if let Some(homepage) = &info.homepage
        && let Some(repo) = url_to_repo(homepage)
    {
        return Some(repo);
    }

    None
}

fn url_to_repo(url: &str) -> Option<String> {
    github_repo_url(url)
        .or_else(|| gitlab_repo_url(url))
        .or_else(|| bitbucket_repo_url(url))
        .or_else(|| codeberg_repo_url(url))
        .or_else(|| sourcehut_repo_url(url))
        .or_else(|| pypi_repo_url(url))
}

fn github_repo_url(url: &str) -> Option<String> {
    let regex = Regex::new(
        r"(?x)
        https?://github\.com/
        (?P<user>[\w.-]+)/
        (?P<repo>[\w.-]+)
        (?:/.*)?
    ",
    )
    .ok()?;

    let caps = regex.captures(url)?;
    let user = caps.name("user")?.as_str();
    let repo = caps.name("repo")?.as_str().trim_end_matches(".git");

    Some(format!("https://github.com/{}/{}", user, repo))
}

fn gitlab_repo_url(url: &str) -> Option<String> {
    let regex = Regex::new(
        r"(?x)
        https?://gitlab\.com/
        (?P<path>(?:[\w.-]+/)*?[\w.-]+)
        (?:/-/|\.git|/archive/)
    ",
    )
    .ok()?;

    let caps = regex.captures(url)?;
    let path = caps.name("path")?.as_str().trim_end_matches(".git");

    Some(format!("https://gitlab.com/{}", path))
}

fn bitbucket_repo_url(url: &str) -> Option<String> {
    let regex = Regex::new(
        r"(?x)
        https?://bitbucket\.org/
        (?P<user>[\w.-]+)/
        (?P<repo>[\w.-]+)
        (?:/.*)?
    ",
    )
    .ok()?;

    let caps = regex.captures(url)?;
    let user = caps.name("user")?.as_str();
    let repo = caps.name("repo")?.as_str().trim_end_matches(".git");

    Some(format!("https://bitbucket.org/{}/{}", user, repo))
}

fn codeberg_repo_url(url: &str) -> Option<String> {
    let regex = Regex::new(
        r"(?x)
        https?://codeberg\.org/
        (?P<user>[\w.-]+)/
        (?P<repo>[\w.-]+)
        (?:/.*)?
    ",
    )
    .ok()?;

    let caps = regex.captures(url)?;
    let user = caps.name("user")?.as_str();
    let repo = caps.name("repo")?.as_str().trim_end_matches(".git");

    Some(format!("https://codeberg.org/{}/{}", user, repo))
}

fn sourcehut_repo_url(url: &str) -> Option<String> {
    let regex = Regex::new(
        r"(?x)
        https?://(?:git\.)?sr\.ht/
        ~(?P<user>[\w.-]+)/
        (?P<repo>[\w.-]+)
        (?:/.*)?
    ",
    )
    .ok()?;

    let caps = regex.captures(url)?;
    let user = caps.name("user")?.as_str();
    let repo = caps.name("repo")?.as_str().trim_end_matches(".git");

    Some(format!("https://sr.ht/~{}/{}", user, repo))
}

fn pypi_repo_url(url: &str) -> Option<String> {
    let regex = Regex::new(
        r"(?x)
        https?://files\.pythonhosted\.org
        /packages
        (?:/[^/]+)+
        /(?P<package_name>.+)-
        .*?
        (?:\.tar\.[a-z0-9]+|\.[a-z0-9]+)
    ",
    )
    .ok()?;

    let caps = regex.captures(url)?;
    let package_name = caps.name("package_name")?.as_str();

    // Query PyPI API for repository information
    let api_url = format!(
        "https://pypi.org/pypi/{}/json",
        package_name.replace("%20", "-").replace("_", "-")
    );

    // Use curl to fetch the JSON
    let output = Command::new("curl")
        .arg("-sS")
        .arg("--retry")
        .arg("0")
        .arg(&api_url)
        .output()
        .ok()?;

    if !output.status.success() {
        return None;
    }

    let json: Value = serde_json::from_slice(&output.stdout).ok()?;
    let project_urls = json.get("info")?.get("project_urls")?.as_object()?;

    // Look for repository or source URLs (case-insensitive)
    for (key, value) in project_urls {
        let key_lower = key.to_lowercase();
        if (key_lower == "repository" || key_lower == "source")
            && let Some(url_str) = value.as_str()
        {
            return Some(url_str.to_string());
        }
    }

    // Try homepage as fallback
    if let Some(homepage) = project_urls
        .get("homepage")
        .or_else(|| project_urls.get("Homepage"))
        && let Some(url_str) = homepage.as_str()
    {
        return url_to_repo(url_str);
    }

    None
}

fn exec_browser(urls: &[String]) -> Result<(), Box<dyn std::error::Error>> {
    let status = Command::new("open").args(urls).status()?;

    if !status.success() {
        return Err("Failed to open browser".into());
    }

    Ok(())
}
