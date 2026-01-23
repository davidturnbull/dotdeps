use crate::paths;
use std::fs;
use std::path::Path;
use std::process::{Command, Stdio};

pub fn run(args: &[String]) -> Result<(), Box<dyn std::error::Error>> {
    let mut verbose = false;
    let mut quiet = false;
    let mut force = false;
    let mut _auto_update = false;
    let mut merge = false;

    // Parse flags
    for arg in args {
        match arg.as_str() {
            "--verbose" | "-v" => verbose = true,
            "--quiet" | "-q" => quiet = true,
            "--force" | "-f" => force = true,
            "--auto-update" => _auto_update = true,
            "--merge" => merge = true,
            "--help" | "-h" => {
                print_help();
                return Ok(());
            }
            _ => {
                eprintln!("Error: Unknown option: {}", arg);
                print_help();
                std::process::exit(1);
            }
        }
    }

    // Check if update is needed (unless --force)
    if !force && !needs_update()? {
        if !quiet {
            println!("Already up-to-date.");
        }
        return Ok(());
    }

    if !quiet {
        println!("==> Updating Homebrew...");
    }

    let mut updated_repos = Vec::new();

    // Update Homebrew repository
    let homebrew_repo = paths::homebrew_repository();
    if homebrew_repo.join(".git").exists() {
        if verbose {
            println!(
                "Checking if we need to fetch {}...",
                homebrew_repo.display()
            );
        }
        if update_git_repo(&homebrew_repo, verbose, quiet, merge)? {
            updated_repos.push("Homebrew/brew".to_string());
        }
    }

    // Update all taps
    let taps_dir = paths::homebrew_prefix().join("Library/Taps");
    if taps_dir.exists() {
        for user_entry in fs::read_dir(&taps_dir)? {
            let user_entry = user_entry?;
            if !user_entry.file_type()?.is_dir() {
                continue;
            }

            for tap_entry in fs::read_dir(user_entry.path())? {
                let tap_entry = tap_entry?;
                let tap_path = tap_entry.path();

                if !tap_path.is_dir() || !tap_path.join(".git").exists() {
                    continue;
                }

                if verbose {
                    println!("Checking if we need to fetch {}...", tap_path.display());
                }

                if update_git_repo(&tap_path, verbose, quiet, merge)? {
                    // Extract user/repo from path
                    let tap_name = format_tap_name(&tap_path);
                    updated_repos.push(tap_name);
                }
            }
        }
    }

    // Update API cache files
    if std::env::var("HOMEBREW_NO_INSTALL_FROM_API").is_err() {
        update_api_cache(verbose, quiet)?;
    }

    // Print summary
    if !quiet {
        if updated_repos.is_empty() {
            println!("Already up-to-date.");
        } else {
            let count = updated_repos.len();
            let tap_word = if count == 1 { "tap" } else { "taps" };
            println!(
                "Updated {} {} ({}).",
                count,
                tap_word,
                updated_repos.join(" and ")
            );

            // Check for outdated formulae
            check_outdated(quiet)?;
        }
    }

    Ok(())
}

fn needs_update() -> Result<bool, Box<dyn std::error::Error>> {
    // Check if we need to update based on last update time
    let cache_dir = paths::homebrew_cache();
    let update_marker = cache_dir.join(".homebrew_update_marker");

    if !update_marker.exists() {
        return Ok(true);
    }

    // Check if last update was more than 5 minutes ago
    let metadata = fs::metadata(&update_marker)?;
    let modified = metadata.modified()?;
    let elapsed = std::time::SystemTime::now().duration_since(modified)?;

    Ok(elapsed.as_secs() > 300) // 5 minutes
}

fn update_git_repo(
    repo: &Path,
    verbose: bool,
    quiet: bool,
    merge: bool,
) -> Result<bool, Box<dyn std::error::Error>> {
    // Change to repo directory
    std::env::set_current_dir(repo)?;

    // Get current revision
    let before_revision = get_current_revision()?;

    // Fetch from remote
    if verbose {
        println!("Fetching {}...", repo.display());
    }

    let mut fetch_cmd = Command::new("git");
    fetch_cmd.arg("fetch").arg("--force").arg("origin");

    if quiet {
        fetch_cmd.stdout(Stdio::null()).stderr(Stdio::null());
    }

    let fetch_status = fetch_cmd.status()?;
    if !fetch_status.success() {
        return Err(format!("Failed to fetch updates for {}", repo.display()).into());
    }

    // Get upstream branch
    let upstream_branch = get_upstream_branch()?;

    // Update to latest
    if verbose {
        println!("Updating {}...", repo.display());
    }

    let update_method = if merge { "merge" } else { "rebase" };
    let mut update_cmd = Command::new("git");

    if merge {
        update_cmd
            .arg("merge")
            .arg(format!("origin/{}", upstream_branch));
    } else {
        update_cmd
            .arg("rebase")
            .arg(format!("origin/{}", upstream_branch));
    }

    if quiet {
        update_cmd.arg("--quiet");
    }

    let update_status = update_cmd.status()?;
    if !update_status.success() {
        eprintln!("Warning: Failed to {} {}", update_method, repo.display());
        return Ok(false);
    }

    // Get new revision
    let after_revision = get_current_revision()?;

    Ok(before_revision != after_revision)
}

fn get_current_revision() -> Result<String, Box<dyn std::error::Error>> {
    let output = Command::new("git")
        .arg("rev-parse")
        .arg("-q")
        .arg("--verify")
        .arg("HEAD")
        .output()?;

    Ok(String::from_utf8_lossy(&output.stdout).trim().to_string())
}

fn get_upstream_branch() -> Result<String, Box<dyn std::error::Error>> {
    let output = Command::new("git")
        .arg("symbolic-ref")
        .arg("refs/remotes/origin/HEAD")
        .output()?;

    if output.status.success() {
        let branch = String::from_utf8_lossy(&output.stdout)
            .trim()
            .strip_prefix("refs/remotes/origin/")
            .unwrap_or("main")
            .to_string();
        Ok(branch)
    } else {
        // Default to main if we can't determine upstream
        Ok("main".to_string())
    }
}

fn format_tap_name(tap_path: &Path) -> String {
    // Extract user/repo from path like /opt/homebrew/Library/Taps/user/homebrew-repo
    let components: Vec<_> = tap_path.components().collect();
    let len = components.len();

    if len >= 2 {
        let user = components[len - 2].as_os_str().to_string_lossy();
        let repo = components[len - 1].as_os_str().to_string_lossy();

        // Strip "homebrew-" prefix from repo name
        let repo_name = repo.strip_prefix("homebrew-").unwrap_or(&repo);

        format!("{}/{}", user, repo_name)
    } else {
        tap_path.display().to_string()
    }
}

fn update_api_cache(verbose: bool, quiet: bool) -> Result<(), Box<dyn std::error::Error>> {
    let api_domain = std::env::var("HOMEBREW_API_DOMAIN")
        .unwrap_or_else(|_| "https://formulae.brew.sh/api".to_string());

    let cache_dir = paths::homebrew_cache().join("api");
    fs::create_dir_all(&cache_dir)?;

    let files = vec![
        "formula.jws.json",
        "cask.jws.json",
        "formula_tap_migrations.jws.json",
        "cask_tap_migrations.jws.json",
    ];

    for file in &files {
        if verbose {
            println!("Checking if we need to fetch {}...", file);
        }

        let url = format!("{}/{}", api_domain, file);
        let target = cache_dir.join(file);

        // Download file
        let mut curl_cmd = Command::new("curl");
        curl_cmd
            .arg("-fsSL")
            .arg("--compressed")
            .arg("-o")
            .arg(&target)
            .arg(&url);

        if quiet {
            curl_cmd.stdout(Stdio::null()).stderr(Stdio::null());
        }

        let status = curl_cmd.status()?;
        if !status.success() {
            eprintln!("Warning: Failed to download {}", file);
        }
    }

    // Update the marker file
    let update_marker = cache_dir.parent().unwrap().join(".homebrew_update_marker");
    fs::write(&update_marker, "")?;

    Ok(())
}

fn check_outdated(quiet: bool) -> Result<(), Box<dyn std::error::Error>> {
    // Run outdated command to check for outdated formulae
    let output = Command::new(std::env::current_exe()?)
        .arg("outdated")
        .arg("--quiet")
        .output()?;

    if output.status.success() {
        let outdated = String::from_utf8_lossy(&output.stdout);
        let lines: Vec<&str> = outdated.lines().collect();

        if !lines.is_empty() && !quiet {
            let count = lines.len();
            let formula_word = if count == 1 { "formula" } else { "formulae" };
            println!("\nYou have {} outdated {}.", count, formula_word);
            println!(
                "You can upgrade {} with brew upgrade",
                if count == 1 { "it" } else { "them" }
            );
            println!(
                "or list {} with brew outdated.",
                if count == 1 { "it" } else { "them" }
            );
        }
    }

    Ok(())
}

fn print_help() {
    println!(
        "Usage: brew update, up [options]

Fetch the newest version of Homebrew and all formulae from GitHub using git(1)
and perform any necessary migrations.

      --merge                      Use git merge to apply updates (rather than
                                   git rebase).
      --auto-update                Run on auto-updates (e.g. before brew
                                   install). Skips some slower steps.
  -f, --force                      Always do a slower, full update check (even
                                   if unnecessary).
  -q, --quiet                      Make some output more quiet.
  -v, --verbose                    Print the directories checked and git
                                   operations performed.
  -h, --help                       Show this message."
    );
}
