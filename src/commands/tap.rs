//! Tap command - clone and install taps from GitHub or custom URLs.

use std::fs;
use std::process::{Command, ExitCode};

use crate::tap;

fn count_formulas(tap_path: &std::path::Path) -> usize {
    // Count .rb files in Formula directory
    let formula_dir = tap_path.join("Formula");
    if !formula_dir.exists() {
        return 0;
    }

    match fs::read_dir(formula_dir) {
        Ok(entries) => entries
            .filter_map(|e| e.ok())
            .filter(|e| e.path().extension().map(|ext| ext == "rb").unwrap_or(false))
            .count(),
        Err(_) => 0,
    }
}

fn print_help() {
    println!(
        "Usage: brew tap [options] [user/repo] [URL]

Tap a formula repository. If no arguments are provided, list all installed taps.

With URL unspecified, tap a formula repository from GitHub using HTTPS. Since
so many taps are hosted on GitHub, this command is a shortcut for brew tap
user/repo https://github.com/user/homebrew-repo.

With URL specified, tap a formula repository from anywhere, using any
transport protocol that git(1) handles. The one-argument form of tap
simplifies but also limits. This two-argument command makes no assumptions, so
taps can be cloned from places other than GitHub and using protocols other than
HTTPS, e.g. SSH, git, HTTP, FTP(S), rsync.

      --custom-remote              Install or change a tap with a custom remote.
                                   Useful for mirrors.
      --repair                     Add missing symlinks to tap manpages and
                                   shell completions. Correct git remote refs
                                   for any taps where upstream HEAD branch has
                                   been renamed.
      --eval-all                   Evaluate all formulae, casks and aliases in
                                   the new tap to check their validity. Enabled
                                   by default if $HOMEBREW_EVAL_ALL is set.
  -f, --force                      Force install core taps even under API mode.
  -d, --debug                      Display any debugging information.
  -q, --quiet                      Make some output more quiet.
  -v, --verbose                    Make some output more verbose.
  -h, --help                       Show this message."
    );
}

pub fn run(args: &[String]) -> ExitCode {
    let mut force = false;
    let mut verbose = false;
    let mut quiet = false;
    let mut custom_remote = false;
    #[allow(unused_variables)]
    let mut repair = false;
    #[allow(unused_variables)]
    let mut eval_all = false;
    let mut tap_name = None;
    let mut url = None;

    let mut i = 0;
    while i < args.len() {
        match args[i].as_str() {
            "-h" | "--help" => {
                print_help();
                return ExitCode::SUCCESS;
            }
            "-f" | "--force" => force = true,
            "-v" | "--verbose" => verbose = true,
            "-q" | "--quiet" => quiet = true,
            "--custom-remote" => custom_remote = true,
            #[allow(unused_assignments)]
            "--repair" => repair = true,
            #[allow(unused_assignments)]
            "--eval-all" => eval_all = true,
            arg => {
                if arg.starts_with('-') {
                    eprintln!("Error: Unknown option: {}", arg);
                    return ExitCode::from(1);
                }
                if tap_name.is_none() {
                    tap_name = Some(arg.to_string());
                } else if url.is_none() {
                    url = Some(arg.to_string());
                } else {
                    eprintln!("Error: Too many arguments");
                    return ExitCode::from(1);
                }
            }
        }
        i += 1;
    }

    // If no tap name provided, list installed taps
    if tap_name.is_none() {
        let taps = tap::list_installed();
        for tap in taps {
            println!("{}", tap.name());
        }
        return ExitCode::SUCCESS;
    }

    let tap_name_str = tap_name.unwrap();

    // Parse tap name
    let tap = match tap::Tap::parse(&tap_name_str) {
        Some(t) => t,
        None => {
            eprintln!("Error: Invalid tap name: {}", tap_name_str);
            eprintln!("Tap names should be in the format 'user/repo'");
            return ExitCode::from(1);
        }
    };

    // Check if already tapped
    if tap.is_installed() && !force && !custom_remote {
        if !quiet {
            println!("Warning: Tap {} already tapped.", tap.name());
        }
        return ExitCode::SUCCESS;
    }

    // Determine the clone URL
    let clone_url = if let Some(custom_url) = url {
        custom_url
    } else {
        // Default to GitHub HTTPS URL
        format!(
            "https://github.com/{}/{}.git",
            tap.user,
            tap.full_repo_name()
        )
    };

    // Create tap directory if needed
    let tap_path = tap.path();
    let tap_parent = tap_path.parent().unwrap();

    if let Err(e) = fs::create_dir_all(tap_parent) {
        eprintln!("Error: Failed to create tap directory: {}", e);
        return ExitCode::from(1);
    }

    // Clone the repository
    if !quiet {
        println!("Tapping {}...", tap.name());
        if verbose {
            println!("Cloning into '{}'...", tap_path.display());
        }
    }

    let mut git_cmd = Command::new("git");
    git_cmd.arg("clone").arg(&clone_url).arg(&tap_path);

    if quiet || !verbose {
        git_cmd.arg("--quiet");
    }

    let status = match git_cmd.status() {
        Ok(s) => s,
        Err(e) => {
            eprintln!("Error: Failed to run git: {}", e);
            return ExitCode::from(1);
        }
    };

    if !status.success() {
        eprintln!("Error: Failed to clone tap repository");
        // Clean up partial clone
        let _ = fs::remove_dir_all(&tap_path);
        return ExitCode::from(1);
    }

    if !quiet {
        // Count formulas in the tap directory
        let formula_count = count_formulas(&tap_path);
        if formula_count > 0 {
            println!(
                "Tapped {} ({} {}).",
                tap.name(),
                formula_count,
                if formula_count == 1 {
                    "formula"
                } else {
                    "formulae"
                }
            );
        } else {
            println!("Tapped {}.", tap.name());
        }
    }

    ExitCode::SUCCESS
}
