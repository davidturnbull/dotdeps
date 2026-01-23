use std::path::Path;
use std::process::Command as ProcessCommand;

use crate::commands::{Command, CommandResult};
use crate::paths;

pub struct Version;

impl Command for Version {
    fn run(&self, _args: &[String]) -> CommandResult {
        let version = get_homebrew_version()?;
        println!("Homebrew {version}");

        // Check for homebrew-core tap
        let taps_dir = paths::homebrew_taps();
        let core_repo = taps_dir.join("homebrew/homebrew-core");
        if core_repo.is_dir() {
            let core_version = get_repo_version_string(&core_repo);
            println!("Homebrew/homebrew-core {core_version}");
        }

        // Check for homebrew-cask tap
        let cask_repo = taps_dir.join("homebrew/homebrew-cask");
        if cask_repo.is_dir() {
            let cask_version = get_repo_version_string(&cask_repo);
            println!("Homebrew/homebrew-cask {cask_version}");
        }

        Ok(())
    }
}

/// Get the Homebrew version using git describe.
fn get_homebrew_version() -> Result<String, Box<dyn std::error::Error>> {
    let repo = paths::homebrew_repository();

    // Try to get version from git describe
    let output = ProcessCommand::new("git")
        .args([
            "-C",
            repo.to_str().unwrap_or("."),
            "describe",
            "--tags",
            "--dirty",
            "--abbrev=7",
        ])
        .output();

    match output {
        Ok(output) if output.status.success() => {
            let version = String::from_utf8_lossy(&output.stdout).trim().to_string();
            Ok(version)
        }
        _ => Ok(">=4.3.0 (shallow or no git repository)".to_string()),
    }
}

/// Get a version string for a git repository.
/// Returns: (git revision <short-hash>; last commit <date>)
fn get_repo_version_string(repo: &Path) -> String {
    if !repo.is_dir() {
        return "N/A".to_string();
    }

    // Get short revision
    let rev_output = ProcessCommand::new("git")
        .args([
            "-C",
            repo.to_str().unwrap_or("."),
            "rev-parse",
            "--short",
            "--verify",
            "--quiet",
            "HEAD",
        ])
        .output();

    let revision = match rev_output {
        Ok(output) if output.status.success() => {
            String::from_utf8_lossy(&output.stdout).trim().to_string()
        }
        _ => return "(no Git repository)".to_string(),
    };

    if revision.is_empty() {
        return "(no Git repository)".to_string();
    }

    // Get last commit date
    let date_output = ProcessCommand::new("git")
        .args([
            "-C",
            repo.to_str().unwrap_or("."),
            "show",
            "-s",
            "--format=%cd",
            "--date=short",
            "HEAD",
        ])
        .output();

    let date = match date_output {
        Ok(output) if output.status.success() => {
            String::from_utf8_lossy(&output.stdout).trim().to_string()
        }
        _ => "unknown".to_string(),
    };

    format!("(git revision {revision}; last commit {date})")
}
