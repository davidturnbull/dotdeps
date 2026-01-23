use std::env;
use std::process::Command as ProcessCommand;

use crate::commands::{Command, CommandResult};
use crate::paths;
use crate::system;

pub struct Config;

impl Command for Config {
    fn run(&self, _args: &[String]) -> CommandResult {
        // HOMEBREW_VERSION
        let version = get_homebrew_version();
        println!("HOMEBREW_VERSION: {version}");

        // ORIGIN
        if let Some(origin) = get_git_origin() {
            println!("ORIGIN: {origin}");
        }

        // HEAD
        if let Some(head) = get_git_head() {
            println!("HEAD: {head}");
        }

        // Last commit
        if let Some(last_commit) = get_last_commit() {
            println!("Last commit: {last_commit}");
        }

        // Branch
        if let Some(branch) = get_git_branch() {
            println!("Branch: {branch}");
        }

        // Core tap JSON timestamps
        print_tap_json_timestamps();

        // HOMEBREW_PREFIX
        println!("HOMEBREW_PREFIX: {}", paths::homebrew_prefix().display());

        // HOMEBREW_CASK_OPTS
        let cask_opts = env::var("HOMEBREW_CASK_OPTS").unwrap_or_default();
        if cask_opts.is_empty() {
            println!("HOMEBREW_CASK_OPTS: []");
        } else {
            println!("HOMEBREW_CASK_OPTS: {cask_opts}");
        }

        // Environment variables
        print_env_vars();

        // System info
        print_system_info();

        Ok(())
    }
}

fn get_homebrew_version() -> String {
    let repo = paths::homebrew_repository();
    let output = ProcessCommand::new("git")
        .args(["describe", "--tags", "--dirty", "--abbrev=7"])
        .current_dir(&repo)
        .output();

    match output {
        Ok(o) if o.status.success() => String::from_utf8_lossy(&o.stdout).trim().to_string(),
        _ => "unknown".to_string(),
    }
}

fn get_git_origin() -> Option<String> {
    let repo = paths::homebrew_repository();
    let output = ProcessCommand::new("git")
        .args(["remote", "get-url", "origin"])
        .current_dir(&repo)
        .output()
        .ok()?;

    if output.status.success() {
        Some(String::from_utf8_lossy(&output.stdout).trim().to_string())
    } else {
        None
    }
}

fn get_git_head() -> Option<String> {
    let repo = paths::homebrew_repository();
    let output = ProcessCommand::new("git")
        .args(["rev-parse", "HEAD"])
        .current_dir(&repo)
        .output()
        .ok()?;

    if output.status.success() {
        Some(String::from_utf8_lossy(&output.stdout).trim().to_string())
    } else {
        None
    }
}

fn get_last_commit() -> Option<String> {
    let repo = paths::homebrew_repository();
    let output = ProcessCommand::new("git")
        .args(["log", "-1", "--format=%cr"])
        .current_dir(&repo)
        .output()
        .ok()?;

    if output.status.success() {
        Some(String::from_utf8_lossy(&output.stdout).trim().to_string())
    } else {
        None
    }
}

fn get_git_branch() -> Option<String> {
    let repo = paths::homebrew_repository();
    let output = ProcessCommand::new("git")
        .args(["rev-parse", "--abbrev-ref", "HEAD"])
        .current_dir(&repo)
        .output()
        .ok()?;

    if output.status.success() {
        Some(String::from_utf8_lossy(&output.stdout).trim().to_string())
    } else {
        None
    }
}

fn print_tap_json_timestamps() {
    let cache = paths::homebrew_cache();

    // Check formula.jws.json
    let formula_path = cache.join("api/formula.jws.json");
    if let Ok(metadata) = std::fs::metadata(&formula_path)
        && let Ok(modified) = metadata.modified()
    {
        let datetime = format_system_time(modified);
        println!("Core tap JSON: {datetime}");
    }

    // Check cask.jws.json
    let cask_path = cache.join("api/cask.jws.json");
    if let Ok(metadata) = std::fs::metadata(&cask_path)
        && let Ok(modified) = metadata.modified()
    {
        let datetime = format_system_time(modified);
        println!("Core cask tap JSON: {datetime}");
    }
}

fn format_system_time(time: std::time::SystemTime) -> String {
    // Simple UTC formatting
    let duration = time
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default();
    let secs = duration.as_secs();

    // Convert to simple date format
    let days_since_epoch = secs / 86400;
    let time_of_day = secs % 86400;
    let hours = time_of_day / 3600;
    let minutes = (time_of_day % 3600) / 60;

    // Calculate year, month, day (simplified - not accounting for leap years properly)
    let mut days = days_since_epoch;
    let mut year = 1970;

    loop {
        let days_in_year = if is_leap_year(year) { 366 } else { 365 };
        if days < days_in_year {
            break;
        }
        days -= days_in_year;
        year += 1;
    }

    let months_days = if is_leap_year(year) {
        [31, 29, 31, 30, 31, 30, 31, 31, 30, 31, 30, 31]
    } else {
        [31, 28, 31, 30, 31, 30, 31, 31, 30, 31, 30, 31]
    };

    let mut month = 0;
    for (i, &days_in_month) in months_days.iter().enumerate() {
        if days < days_in_month as u64 {
            month = i + 1;
            break;
        }
        days -= days_in_month as u64;
    }

    let day = days + 1;

    let month_names = [
        "Jan", "Feb", "Mar", "Apr", "May", "Jun", "Jul", "Aug", "Sep", "Oct", "Nov", "Dec",
    ];

    format!(
        "{} {} {:02}:{:02} UTC",
        day,
        month_names[month - 1],
        hours,
        minutes
    )
}

fn is_leap_year(year: u64) -> bool {
    (year.is_multiple_of(4) && !year.is_multiple_of(100)) || year.is_multiple_of(400)
}

fn print_env_vars() {
    // Print HOMEBREW_ environment variables that are set
    let env_vars = [
        "HOMEBREW_DOWNLOAD_CONCURRENCY",
        "HOMEBREW_MAKE_JOBS",
        "HOMEBREW_NO_AUTO_UPDATE",
        "HOMEBREW_NO_INSTALL_CLEANUP",
        "HOMEBREW_NO_ENV_HINTS",
    ];

    for var in env_vars {
        if let Ok(value) = env::var(var) {
            println!("{var}: {value}");
        }
    }

    // Special handling for boolean-style vars that are just "set"
    let bool_vars = ["HOMEBREW_FORBID_PACKAGES_FROM_PATHS"];
    for var in bool_vars {
        if env::var(var).is_ok() {
            println!("{var}: set");
        }
    }
}

fn print_system_info() {
    // Homebrew Ruby (we don't use Ruby, but show what's there)
    let ruby_path = paths::homebrew_repository().join("Library/Homebrew/vendor/portable-ruby");
    if ruby_path.exists() {
        // Find the highest version directory (excluding symlinks like "current")
        let mut versions: Vec<String> = Vec::new();
        if let Ok(entries) = std::fs::read_dir(&ruby_path) {
            for entry in entries.flatten() {
                let path = entry.path();
                // Check if it's a directory but not a symlink
                if path.is_dir() && path.read_link().is_err() {
                    let name = entry.file_name().to_string_lossy().to_string();
                    // Only include version-like directories (start with digit)
                    if name.chars().next().is_some_and(|c| c.is_ascii_digit()) {
                        versions.push(name);
                    }
                }
            }
        }
        versions.sort();
        if let Some(version) = versions.last() {
            let ruby_bin = ruby_path.join(version).join("bin/ruby");
            if ruby_bin.exists() {
                println!("Homebrew Ruby: {version} => {}", ruby_bin.display());
            }
        }
    }

    // CPU
    let arch = system::arch();
    let cpu_info = get_cpu_info();
    println!("CPU: {cpu_info} 64-bit {arch}");

    // Clang version
    if let Some(clang_version) = get_clang_version() {
        println!("Clang: {clang_version}");
    }

    // Git version
    if let Some(git_version) = get_git_version() {
        println!("Git: {git_version}");
    }

    // Curl version
    if let Some(curl_version) = get_curl_version() {
        println!("Curl: {curl_version}");
    }

    // macOS version
    if let Some(macos_version) = system::macos_version() {
        println!("macOS: {macos_version}-{arch}");
    }

    // CLT version
    if let Some(clt_version) = get_clt_version() {
        println!("CLT: {clt_version}");
    }

    // Xcode version
    if let Some(xcode_version) = get_xcode_version() {
        println!("Xcode: {xcode_version}");
    }

    // Rosetta 2
    #[cfg(target_arch = "aarch64")]
    println!("Rosetta 2: false");
}

fn get_cpu_info() -> String {
    #[cfg(target_os = "macos")]
    {
        let output = ProcessCommand::new("sysctl")
            .args(["-n", "machdep.cpu.brand_string"])
            .output();

        if let Ok(o) = output
            && o.status.success()
        {
            let brand = String::from_utf8_lossy(&o.stdout).trim().to_string();
            // Simplify: extract core count
            return brand;
        }

        // Fallback: get core count
        let cores = ProcessCommand::new("sysctl")
            .args(["-n", "hw.ncpu"])
            .output()
            .ok()
            .filter(|o| o.status.success())
            .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string())
            .unwrap_or_else(|| "8".to_string());

        format!("{cores}-core")
    }

    #[cfg(not(target_os = "macos"))]
    {
        "unknown".to_string()
    }
}

fn get_clang_version() -> Option<String> {
    let output = ProcessCommand::new("clang")
        .args(["--version"])
        .output()
        .ok()?;

    if output.status.success() {
        let text = String::from_utf8_lossy(&output.stdout);
        // Extract version from first line
        let first_line = text.lines().next()?;
        // Usually: "Apple clang version 17.0.0 (clang-1700.0.13.3)"
        if let Some(version_start) = first_line.find("version ") {
            let version_part = &first_line[version_start + 8..];
            let version = version_part.split_whitespace().next()?;
            // Get build number from parentheses
            if let Some(build_start) = first_line.find("clang-") {
                let build_end = first_line.find(')')?;
                let build = &first_line[build_start + 6..build_end];
                return Some(format!("{version} build {build}"));
            }
            return Some(version.to_string());
        }
    }
    None
}

fn get_git_version() -> Option<String> {
    let output = ProcessCommand::new("git")
        .args(["--version"])
        .output()
        .ok()?;

    if output.status.success() {
        let text = String::from_utf8_lossy(&output.stdout);
        let version = text.trim().strip_prefix("git version ")?;
        // Find git path
        let which = ProcessCommand::new("which").args(["git"]).output().ok()?;
        if which.status.success() {
            let path = String::from_utf8_lossy(&which.stdout).trim().to_string();
            return Some(format!("{version} => {path}"));
        }
        return Some(version.to_string());
    }
    None
}

fn get_curl_version() -> Option<String> {
    let output = ProcessCommand::new("curl")
        .args(["--version"])
        .output()
        .ok()?;

    if output.status.success() {
        let text = String::from_utf8_lossy(&output.stdout);
        let first_line = text.lines().next()?;
        // "curl 8.7.1 (x86_64-apple-darwin23.0) ..."
        let version = first_line.split_whitespace().nth(1)?;
        let which = ProcessCommand::new("which").args(["curl"]).output().ok()?;
        if which.status.success() {
            let path = String::from_utf8_lossy(&which.stdout).trim().to_string();
            return Some(format!("{version} => {path}"));
        }
        return Some(version.to_string());
    }
    None
}

fn get_clt_version() -> Option<String> {
    let output = ProcessCommand::new("pkgutil")
        .args(["--pkg-info", "com.apple.pkg.CLTools_Executables"])
        .output()
        .ok()?;

    if output.status.success() {
        let text = String::from_utf8_lossy(&output.stdout);
        for line in text.lines() {
            if line.starts_with("version: ") {
                return Some(line.strip_prefix("version: ")?.to_string());
            }
        }
    }
    None
}

fn get_xcode_version() -> Option<String> {
    let output = ProcessCommand::new("xcodebuild")
        .args(["-version"])
        .output()
        .ok()?;

    if output.status.success() {
        let text = String::from_utf8_lossy(&output.stdout);
        let first_line = text.lines().next()?;
        // "Xcode 15.4" -> "15.4"
        let version = first_line.strip_prefix("Xcode ")?;
        return Some(version.to_string());
    }
    None
}
