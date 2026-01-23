use crate::paths;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

pub fn run(args: &[String]) -> Result<(), Box<dyn std::error::Error>> {
    let mut repos: Vec<PathBuf> = Vec::new();

    // Parse arguments
    for arg in args {
        if arg == "--help" || arg == "-h" || arg == "-?" {
            print_help();
            return Ok(());
        } else if arg == "--debug" || arg == "-d" {
            // Debug flag - ignored for now
            continue;
        } else if arg == "--quiet" || arg == "-q" {
            // Quiet flag - ignored for now
            continue;
        } else if arg == "--verbose" || arg == "-v" {
            // Verbose flag - ignored for now
            continue;
        } else if arg.starts_with('-') {
            // Handle combined flags like -dqv
            continue;
        } else {
            // It's a repository path
            let path = PathBuf::from(arg);
            if !path.join(".git").exists() {
                eprintln!("Error: {} is not a Git repository!", arg);
                print_help();
                std::process::exit(1);
            }
            repos.push(path);
        }
    }

    // If no repos specified, default to HOMEBREW_REPOSITORY and all taps
    if repos.is_empty() {
        repos.push(paths::homebrew_repository());

        // Add all taps from Library/Taps/*/*
        let taps_dir = paths::homebrew_library().join("Taps");
        if taps_dir.exists()
            && let Ok(entries) = std::fs::read_dir(&taps_dir)
        {
            for user_entry in entries.flatten() {
                if user_entry.path().is_dir()
                    && let Ok(tap_entries) = std::fs::read_dir(user_entry.path())
                {
                    for tap_entry in tap_entries.flatten() {
                        let tap_path = tap_entry.path();
                        if tap_path.is_dir() && tap_path.join(".git").exists() {
                            repos.push(tap_path);
                        }
                    }
                }
            }
        }
    }

    // Process each repository
    for repo in repos {
        if !repo.join(".git").exists() {
            continue;
        }

        // Check if remote 'origin' exists
        if !has_remote_origin(&repo) {
            eprintln!(
                "Warning: No remote 'origin' in {}, skipping update and reset!",
                repo.display()
            );
            continue;
        }

        // Configure git settings
        git_config(&repo, "core.autocrlf", "false")?;
        git_config(&repo, "core.symlinks", "true")?;

        // Fetch from origin
        println!("==> Fetching {}...", repo.display());
        git_fetch(&repo)?;
        git_set_remote_head(&repo)?;
        println!();

        // Reset to origin/HEAD
        println!("==> Resetting {}...", repo.display());
        git_reset(&repo)?;

        // Remove describe cache
        let describe_cache = repo.join(".git/describe-cache");
        if describe_cache.exists() {
            std::fs::remove_dir_all(&describe_cache).ok();
        }
        println!();
    }

    Ok(())
}

fn has_remote_origin(repo: &Path) -> bool {
    Command::new("git")
        .args([
            "-C",
            &repo.to_string_lossy(),
            "config",
            "--local",
            "--get",
            "remote.origin.url",
        ])
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .map(|s| s.success())
        .unwrap_or(false)
}

fn git_config(repo: &Path, key: &str, value: &str) -> Result<(), Box<dyn std::error::Error>> {
    Command::new("git")
        .args([
            "-C",
            &repo.to_string_lossy(),
            "config",
            "--bool",
            key,
            value,
        ])
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()?;
    Ok(())
}

fn git_fetch(repo: &Path) -> Result<(), Box<dyn std::error::Error>> {
    let status = Command::new("git")
        .args([
            "-C",
            &repo.to_string_lossy(),
            "fetch",
            "--force",
            "--tags",
            "origin",
        ])
        .status()?;

    if !status.success() {
        return Err("Git fetch failed".into());
    }
    Ok(())
}

fn git_set_remote_head(repo: &Path) -> Result<(), Box<dyn std::error::Error>> {
    Command::new("git")
        .args([
            "-C",
            &repo.to_string_lossy(),
            "remote",
            "set-head",
            "origin",
            "--auto",
        ])
        .stdout(Stdio::null())
        .status()?;
    Ok(())
}

fn git_reset(repo: &Path) -> Result<(), Box<dyn std::error::Error>> {
    let homebrew_repo = paths::homebrew_repository();

    // For Homebrew repository, check if we should use tags
    if repo == homebrew_repo {
        let should_use_tag = std::env::var("HOMEBREW_UPDATE_TO_TAG").is_ok()
            || (std::env::var("HOMEBREW_DEVELOPER").is_err()
                && std::env::var("HOMEBREW_DEV_CMD_RUN").is_err());

        if should_use_tag {
            // Get latest git tag
            let output = Command::new("git")
                .args([
                    "-C",
                    &repo.to_string_lossy(),
                    "tag",
                    "--list",
                    "--sort=-version:refname",
                ])
                .output()?;

            if output.status.success() {
                let tags = String::from_utf8_lossy(&output.stdout);
                if let Some(latest_tag) = tags.lines().next() {
                    // Checkout the tag
                    let status = Command::new("git")
                        .args([
                            "-C",
                            &repo.to_string_lossy(),
                            "checkout",
                            "--force",
                            "-B",
                            "stable",
                            &format!("refs/tags/{}", latest_tag),
                        ])
                        .status()?;

                    if !status.success() {
                        return Err("Git checkout failed".into());
                    }
                    return Ok(());
                }
            }
        }
    }

    // Get the default branch from remote HEAD
    let output = Command::new("git")
        .args([
            "-C",
            &repo.to_string_lossy(),
            "symbolic-ref",
            "refs/remotes/origin/HEAD",
        ])
        .output()?;

    if !output.status.success() {
        return Err("Failed to get remote HEAD".into());
    }

    let head_ref = String::from_utf8_lossy(&output.stdout).trim().to_string();
    let branch = head_ref
        .strip_prefix("refs/remotes/origin/")
        .unwrap_or("master");

    // Checkout and reset to origin/HEAD
    let status = Command::new("git")
        .args([
            "-C",
            &repo.to_string_lossy(),
            "checkout",
            "--force",
            "-B",
            branch,
            "origin/HEAD",
        ])
        .status()?;

    if !status.success() {
        return Err("Git checkout failed".into());
    }

    Ok(())
}

fn print_help() {
    println!("Usage: brew update-reset [repository ...]");
    println!();
    println!("Fetch and reset Homebrew and all tap repositories (or any specified");
    println!("repository) using git(1) to their latest origin/HEAD.");
    println!();
    println!("Note: this will destroy all your uncommitted or committed changes.");
    println!();
    println!("  -d, --debug                      Display any debugging information.");
    println!("  -q, --quiet                      Make some output more quiet.");
    println!("  -v, --verbose                    Make some output more verbose.");
    println!("  -h, --help                       Show this message.");
}
