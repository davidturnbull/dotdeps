use crate::paths;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

pub fn execute(args: &[String]) -> Result<(), Box<dyn std::error::Error>> {
    let mut taps_to_show: Vec<String> = Vec::new();
    let mut installed_only = false;
    let mut json_output = false;

    for arg in args {
        match arg.as_str() {
            "--installed" => installed_only = true,
            "--json" => json_output = true,
            _ if arg.starts_with('-') => {
                eprintln!("Error: Unknown option: {}", arg);
                std::process::exit(1);
            }
            _ => taps_to_show.push(arg.clone()),
        }
    }

    // Get all taps
    let taps_dir = paths::homebrew_prefix().join("Library/Taps");
    let mut all_taps = get_all_taps(&taps_dir)?;

    // If no specific taps requested and --installed not specified, show brief stats
    if taps_to_show.is_empty() && !installed_only {
        show_brief_stats(&all_taps)?;
        return Ok(());
    }

    // Filter to installed taps if --installed specified
    // (but keep official taps even if not installed)
    if installed_only {
        all_taps.retain(|t| t.installed || t.official);
    }

    // Filter to requested taps if specified
    if !taps_to_show.is_empty() {
        all_taps.retain(|t| taps_to_show.contains(&t.name));
    }

    // Output as JSON or text
    if json_output {
        output_json(&all_taps)?;
    } else {
        output_text(&all_taps)?;
    }

    // Exit with 1 if showing --installed and homebrew/core or homebrew/cask not installed
    // (matches brew behavior)
    if installed_only {
        let has_core = all_taps.iter().any(|t| t.name == "homebrew/core");
        let has_cask = all_taps.iter().any(|t| t.name == "homebrew/cask");
        if !has_core || !has_cask {
            std::process::exit(1);
        }
    }

    Ok(())
}

#[derive(Debug, Clone)]
struct TapInfo {
    name: String,
    user: String,
    repo: String,
    repository: String,
    path: PathBuf,
    installed: bool,
    official: bool,
    formula_names: Vec<String>,
    cask_tokens: Vec<String>,
    formula_files: Vec<PathBuf>,
    cask_files: Vec<PathBuf>,
    command_files: Vec<PathBuf>,
    remote: String,
    custom_remote: bool,
    private_tap: bool,
    head: String,
    last_commit: String,
    branch: String,
}

fn get_all_taps(taps_dir: &Path) -> Result<Vec<TapInfo>, Box<dyn std::error::Error>> {
    let mut taps = Vec::new();

    // Add official taps (homebrew/core, homebrew/cask) even if not installed
    for official in &["homebrew/core", "homebrew/cask"] {
        let parts: Vec<&str> = official.split('/').collect();
        let user = parts[0];
        let repo = parts[1];
        let tap_path = taps_dir.join(format!("{}/homebrew-{}", user, repo));
        let installed = tap_path.exists();

        if installed {
            if let Ok(tap_info) = get_tap_info(&tap_path, user, repo) {
                taps.push(tap_info);
            }
        } else {
            // Add placeholder for uninstalled official taps
            taps.push(TapInfo {
                name: official.to_string(),
                user: user.to_string(),
                repo: repo.to_string(),
                repository: repo.to_string(),
                path: tap_path,
                installed: false,
                official: true,
                formula_names: vec![],
                cask_tokens: vec![],
                formula_files: vec![],
                cask_files: vec![],
                command_files: vec![],
                remote: String::new(),
                custom_remote: false,
                private_tap: false,
                head: String::new(),
                last_commit: String::new(),
                branch: String::new(),
            });
        }
    }

    // Scan for third-party taps
    if taps_dir.exists() {
        for user_entry in fs::read_dir(taps_dir)? {
            let user_entry = user_entry?;
            let user_name = user_entry.file_name().to_string_lossy().to_string();

            // Skip if not a directory
            if !user_entry.file_type()?.is_dir() {
                continue;
            }

            for repo_entry in fs::read_dir(user_entry.path())? {
                let repo_entry = repo_entry?;
                let repo_name = repo_entry.file_name().to_string_lossy().to_string();

                // Skip if not a directory
                if !repo_entry.file_type()?.is_dir() {
                    continue;
                }

                // Extract repo without homebrew-/linuxbrew- prefix
                let repo = repo_name
                    .strip_prefix("homebrew-")
                    .or_else(|| repo_name.strip_prefix("linuxbrew-"))
                    .unwrap_or(&repo_name)
                    .to_string();

                // Skip official taps (already added)
                let tap_name = format!("{}/{}", user_name, repo);
                if tap_name == "homebrew/core" || tap_name == "homebrew/cask" {
                    continue;
                }

                if let Ok(tap_info) = get_tap_info(&repo_entry.path(), &user_name, &repo) {
                    taps.push(tap_info);
                }
            }
        }
    }

    // Sort by name
    taps.sort_by(|a, b| a.name.cmp(&b.name));

    Ok(taps)
}

fn get_tap_info(
    tap_path: &Path,
    user: &str,
    repo: &str,
) -> Result<TapInfo, Box<dyn std::error::Error>> {
    let name = format!("{}/{}", user, repo);
    let official = user == "homebrew";

    // Get git info
    let head = get_git_head(tap_path)?;
    let last_commit = get_git_last_commit(tap_path)?;
    let branch = get_git_branch(tap_path)?;
    let remote = get_git_remote(tap_path)?;

    // Determine if custom remote
    let expected_remote = format!("https://github.com/{}/homebrew-{}", user, repo);
    let custom_remote = !remote.is_empty() && remote != expected_remote;

    // Check if private (git remote is not github/gitlab/etc)
    let private_tap = !remote.contains("github.com")
        && !remote.contains("gitlab.com")
        && !remote.contains("bitbucket.org");

    // Find formulae
    let mut formula_files: Vec<PathBuf> = Vec::new();
    let mut formula_names: Vec<String> = Vec::new();

    // Check Formula directory and subdirectories
    let formula_dir = tap_path.join("Formula");
    if formula_dir.exists() {
        find_ruby_files(&formula_dir, &mut formula_files)?;
    }

    // Also check root directory for .rb files (some taps have formulae in root)
    for entry in fs::read_dir(tap_path)? {
        let entry = entry?;
        let path = entry.path();
        if path.extension().is_some_and(|e| e == "rb") && path.is_file() {
            // Skip Casks directory
            if !path.to_string_lossy().contains("/Casks/") {
                formula_files.push(path);
            }
        }
    }

    // Generate formula names
    for file in &formula_files {
        if let Some(formula_name) = extract_formula_name(file, tap_path, &name) {
            formula_names.push(formula_name);
        }
    }
    formula_names.sort();

    // Find casks
    let mut cask_files: Vec<PathBuf> = Vec::new();
    let mut cask_tokens: Vec<String> = Vec::new();

    let casks_dir = tap_path.join("Casks");
    if casks_dir.exists() {
        find_ruby_files(&casks_dir, &mut cask_files)?;

        for file in &cask_files {
            if let Some(file_stem) = file.file_stem() {
                let token = file_stem.to_string_lossy().to_string();
                cask_tokens.push(format!("{}/{}", name, token));
            }
        }
        cask_tokens.sort();
    }

    // Find cmd files
    let mut command_files: Vec<PathBuf> = Vec::new();
    let cmd_dir = tap_path.join("cmd");
    if cmd_dir.exists() {
        for entry in fs::read_dir(&cmd_dir)? {
            let entry = entry?;
            let path = entry.path();
            if path.is_file() {
                command_files.push(path);
            }
        }
    }

    Ok(TapInfo {
        name: name.clone(),
        user: user.to_string(),
        repo: repo.to_string(),
        repository: repo.to_string(),
        path: tap_path.to_path_buf(),
        installed: true,
        official,
        formula_names,
        cask_tokens,
        formula_files,
        cask_files,
        command_files,
        remote,
        custom_remote,
        private_tap,
        head,
        last_commit,
        branch,
    })
}

fn find_ruby_files(dir: &Path, files: &mut Vec<PathBuf>) -> Result<(), Box<dyn std::error::Error>> {
    for entry in fs::read_dir(dir)? {
        let entry = entry?;
        let path = entry.path();

        if path.is_dir() {
            // Recurse into subdirectories
            find_ruby_files(&path, files)?;
        } else if path.extension().is_some_and(|e| e == "rb") {
            files.push(path);
        }
    }
    Ok(())
}

fn extract_formula_name(file: &Path, tap_path: &Path, tap_name: &str) -> Option<String> {
    let relative = file.strip_prefix(tap_path).ok()?;
    let relative_str = relative.to_string_lossy();

    // Remove .rb extension
    let without_ext = relative_str.strip_suffix(".rb")?;

    // Remove Formula/ prefix if present
    let without_formula_dir = without_ext.strip_prefix("Formula/").unwrap_or(without_ext);

    // Build full name: tap_name/formula_name
    Some(format!("{}/{}", tap_name, without_formula_dir))
}

fn get_git_head(tap_path: &Path) -> Result<String, Box<dyn std::error::Error>> {
    let path_str = tap_path.to_string_lossy();
    let output = Command::new("git")
        .args(["-C", path_str.as_ref(), "rev-parse", "HEAD"])
        .output()?;

    if output.status.success() {
        Ok(String::from_utf8_lossy(&output.stdout).trim().to_string())
    } else {
        Ok(String::new())
    }
}

fn get_git_last_commit(tap_path: &Path) -> Result<String, Box<dyn std::error::Error>> {
    let path_str = tap_path.to_string_lossy();
    let output = Command::new("git")
        .args(["-C", path_str.as_ref(), "log", "-1", "--format=%cr"])
        .output()?;

    if output.status.success() {
        Ok(String::from_utf8_lossy(&output.stdout).trim().to_string())
    } else {
        Ok(String::new())
    }
}

fn get_git_branch(tap_path: &Path) -> Result<String, Box<dyn std::error::Error>> {
    let path_str = tap_path.to_string_lossy();
    let output = Command::new("git")
        .args(["-C", path_str.as_ref(), "rev-parse", "--abbrev-ref", "HEAD"])
        .output()?;

    if output.status.success() {
        Ok(String::from_utf8_lossy(&output.stdout).trim().to_string())
    } else {
        Ok(String::new())
    }
}

fn get_git_remote(tap_path: &Path) -> Result<String, Box<dyn std::error::Error>> {
    let path_str = tap_path.to_string_lossy();
    let output = Command::new("git")
        .args(["-C", path_str.as_ref(), "remote", "get-url", "origin"])
        .output()?;

    if output.status.success() {
        Ok(String::from_utf8_lossy(&output.stdout).trim().to_string())
    } else {
        Ok(String::new())
    }
}

fn show_brief_stats(taps: &[TapInfo]) -> Result<(), Box<dyn std::error::Error>> {
    let installed_count = taps.iter().filter(|t| t.installed).count();
    let private_count = taps.iter().filter(|t| t.private_tap && t.installed).count();
    let total_formulae: usize = taps
        .iter()
        .filter(|t| t.installed)
        .map(|t| t.formula_names.len())
        .sum();
    let total_commands: usize = taps
        .iter()
        .filter(|t| t.installed)
        .map(|t| t.command_files.len())
        .sum();

    // Count total files
    let mut total_files = 0;
    for tap in taps.iter().filter(|t| t.installed) {
        let path_str = tap.path.to_string_lossy();
        if let Ok(output) = Command::new("find")
            .args([path_str.as_ref(), "-type", "f"])
            .output()
        {
            total_files += String::from_utf8_lossy(&output.stdout).lines().count();
        }
    }

    // Get total size
    let mut total_size_kb = 0u64;
    for tap in taps.iter().filter(|t| t.installed) {
        let path_str = tap.path.to_string_lossy();
        if let Ok(output) = Command::new("du").args(["-sk", path_str.as_ref()]).output()
            && let Some(size_str) = String::from_utf8_lossy(&output.stdout)
                .split_whitespace()
                .next()
            && let Ok(size) = size_str.parse::<u64>()
        {
            total_size_kb += size;
        }
    }

    // Format size
    let size_str = if total_size_kb < 1024 {
        format!("{}KB", total_size_kb)
    } else if total_size_kb < 1024 * 1024 {
        format!("{:.1}MB", total_size_kb as f64 / 1024.0)
    } else {
        format!("{:.1}GB", total_size_kb as f64 / (1024.0 * 1024.0))
    };

    println!(
        "{} taps, {} private, {} formulae, {} commands, {} files, {}",
        installed_count, private_count, total_formulae, total_commands, total_files, size_str
    );

    Ok(())
}

fn output_text(taps: &[TapInfo]) -> Result<(), Box<dyn std::error::Error>> {
    for (i, tap) in taps.iter().enumerate() {
        if !tap.installed {
            println!("{}: Not installed", tap.name);
            println!();
            continue;
        }

        println!("{}: Installed", tap.name);

        // Show formula/cask count
        let formula_count = tap.formula_names.len();
        let cask_count = tap.cask_tokens.len();

        if formula_count > 0 && cask_count > 0 {
            println!(
                "{} casks, {} formula{}",
                cask_count,
                formula_count,
                if formula_count == 1 { "" } else { "e" }
            );
        } else if formula_count > 0 {
            println!(
                "{} formula{}",
                formula_count,
                if formula_count == 1 { "" } else { "e" }
            );
        } else if cask_count > 0 {
            println!("{} casks", cask_count);
        }

        // Get file count and size
        let path_str = tap.path.to_string_lossy();
        if let Ok(output) = Command::new("find")
            .args([path_str.as_ref(), "-type", "f"])
            .output()
        {
            let file_count = String::from_utf8_lossy(&output.stdout).lines().count();

            if let Ok(output) = Command::new("du").args(["-sh", path_str.as_ref()]).output()
                && let Some(size) = String::from_utf8_lossy(&output.stdout)
                    .split_whitespace()
                    .next()
            {
                println!(
                    "{} ({} files, {})",
                    tap.path.display(),
                    file_count,
                    size.trim()
                );
            }
        }

        // Show remote
        println!("From: {}", tap.remote);

        // Show origin if different from remote (felixkratz/formulae case)
        if tap.custom_remote {
            println!("origin: {}", tap.remote);
        }

        // Show HEAD and last commit
        println!("HEAD: {}", tap.head);
        println!("last commit: {}", tap.last_commit);

        // Print blank line between taps (but not after the last one)
        if i < taps.len() - 1 {
            println!();
        }
    }

    Ok(())
}

fn output_json(taps: &[TapInfo]) -> Result<(), Box<dyn std::error::Error>> {
    println!("[");
    for (i, tap) in taps.iter().enumerate() {
        println!("  {{");
        println!("    \"name\": \"{}\",", tap.name);
        println!("    \"user\": \"{}\",", tap.user);
        println!("    \"repo\": \"{}\",", tap.repo);
        println!("    \"repository\": \"{}\",", tap.repository);
        println!("    \"path\": \"{}\",", tap.path.display());
        println!("    \"installed\": {},", tap.installed);
        println!("    \"official\": {},", tap.official);

        // Formula names
        println!("    \"formula_names\": [");
        for (j, name) in tap.formula_names.iter().enumerate() {
            if j < tap.formula_names.len() - 1 {
                println!("      \"{}\",", name);
            } else {
                println!("      \"{}\"", name);
            }
        }
        println!("    ],");

        // Cask tokens
        println!("    \"cask_tokens\": [");
        for (j, token) in tap.cask_tokens.iter().enumerate() {
            if j < tap.cask_tokens.len() - 1 {
                println!("      \"{}\",", token);
            } else {
                println!("      \"{}\"", token);
            }
        }
        println!("    ],");

        // Formula files
        println!("    \"formula_files\": [");
        for (j, file) in tap.formula_files.iter().enumerate() {
            if j < tap.formula_files.len() - 1 {
                println!("      \"{}\",", file.display());
            } else {
                println!("      \"{}\"", file.display());
            }
        }
        println!("    ],");

        // Cask files
        println!("    \"cask_files\": [");
        for (j, file) in tap.cask_files.iter().enumerate() {
            if j < tap.cask_files.len() - 1 {
                println!("      \"{}\",", file.display());
            } else {
                println!("      \"{}\"", file.display());
            }
        }
        println!("    ],");

        // Command files
        println!("    \"command_files\": [");
        for (j, file) in tap.command_files.iter().enumerate() {
            if j < tap.command_files.len() - 1 {
                println!("      \"{}\",", file.display());
            } else {
                println!("      \"{}\"", file.display());
            }
        }
        println!("    ],");

        println!("    \"remote\": \"{}\",", tap.remote);
        println!("    \"custom_remote\": {},", tap.custom_remote);
        println!("    \"private\": {},", tap.private_tap);

        if tap.installed {
            println!("    \"HEAD\": \"{}\",", tap.head);
            println!("    \"last_commit\": \"{}\",", tap.last_commit);
            println!("    \"branch\": \"{}\"", tap.branch);
        } else {
            // For uninstalled taps, omit git info
            println!("    \"HEAD\": null,");
            println!("    \"last_commit\": null,");
            println!("    \"branch\": null");
        }

        if i < taps.len() - 1 {
            println!("  }},");
        } else {
            println!("  }}");
        }
    }
    println!("]");

    Ok(())
}
