use crate::commands::CommandResult;
use crate::paths;
use crate::tap::Tap;
use std::path::PathBuf;
use std::process::Command;

pub struct LogCommand;

impl Default for LogCommand {
    fn default() -> Self {
        Self::new()
    }
}

impl LogCommand {
    pub fn new() -> Self {
        Self
    }

    pub fn run(&self, args: &[String]) -> CommandResult {
        // Parse flags
        let mut patch = false;
        let mut stat = false;
        let mut oneline = false;
        let mut one = false;
        let mut max_count: Option<usize> = None;
        let mut formula_flag = false;
        let mut cask_flag = false;
        let mut items = Vec::new();

        let mut i = 0;
        while i < args.len() {
            match args[i].as_str() {
                "-h" | "--help" => {
                    print_help();
                    return Ok(());
                }
                "-p" | "-u" | "--patch" => patch = true,
                "--stat" => stat = true,
                "--oneline" => oneline = true,
                "-1" => one = true,
                "-n" | "--max-count" => {
                    i += 1;
                    if i < args.len() {
                        max_count = args[i].parse().ok();
                    }
                }
                "--formula" | "--formulae" => formula_flag = true,
                "--cask" | "--casks" => cask_flag = true,
                "-d" | "--debug" => {}   // Ignored
                "-q" | "--quiet" => {}   // Ignored
                "-v" | "--verbose" => {} // Ignored
                arg if !arg.starts_with('-') => items.push(arg.to_string()),
                _ => {}
            }
            i += 1;
        }

        // If no items specified, show Homebrew repository log
        if items.is_empty() {
            return self.show_homebrew_log(patch, stat, oneline, one, max_count);
        }

        // Show log for each formula/cask
        for item in items {
            self.show_formula_log(
                &item,
                formula_flag,
                cask_flag,
                patch,
                stat,
                oneline,
                one,
                max_count,
            )?;
        }

        Ok(())
    }

    fn show_homebrew_log(
        &self,
        patch: bool,
        stat: bool,
        oneline: bool,
        one: bool,
        max_count: Option<usize>,
    ) -> CommandResult {
        let repo = paths::homebrew_repository();
        self.run_git_log(&repo, None, patch, stat, oneline, one, max_count)
    }

    #[allow(clippy::too_many_arguments)]
    fn show_formula_log(
        &self,
        name: &str,
        _formula_flag: bool,
        _cask_flag: bool,
        patch: bool,
        stat: bool,
        oneline: bool,
        one: bool,
        max_count: Option<usize>,
    ) -> CommandResult {
        // Try to find the formula file in taps
        if let Some((tap_path, formula_path)) = self.find_formula_file(name) {
            return self.run_git_log(
                &tap_path,
                Some(&formula_path),
                patch,
                stat,
                oneline,
                one,
                max_count,
            );
        }

        eprintln!("Error: No available formula with the name \"{}\"", name);
        std::process::exit(1);
    }

    fn find_formula_file(&self, name: &str) -> Option<(PathBuf, PathBuf)> {
        let prefix = paths::homebrew_prefix();
        let taps_dir = prefix.join("Library/Taps");

        if !taps_dir.exists() {
            return None;
        }

        // Parse name as potential tap path (e.g., "oven-sh/bun/bun")
        let parts: Vec<&str> = name.split('/').collect();

        // Check for full tap path first
        if parts.len() == 3
            && let Some(tap) = Tap::parse(&format!("{}/{}", parts[0], parts[1]))
        {
            let tap_path = tap.path();
            let formula_name = parts[2];

            if let Some(rel_path) = self.find_formula_in_tap(&tap_path, formula_name) {
                return Some((tap_path, rel_path));
            }
        }

        // Search all taps for the formula
        if let Ok(entries) = std::fs::read_dir(&taps_dir) {
            for user_entry in entries.flatten() {
                if let Ok(repos) = std::fs::read_dir(user_entry.path()) {
                    for repo_entry in repos.flatten() {
                        let tap_path = repo_entry.path();
                        if let Some(rel_path) = self.find_formula_in_tap(&tap_path, name) {
                            return Some((tap_path, rel_path));
                        }
                    }
                }
            }
        }

        None
    }

    fn find_formula_in_tap(&self, tap_path: &std::path::Path, name: &str) -> Option<PathBuf> {
        // Check Formula directory
        let formula_dir = tap_path.join("Formula");
        if formula_dir.exists() {
            // Check direct file
            let direct = formula_dir.join(format!("{}.rb", name));
            if direct.exists() {
                return Some(PathBuf::from("Formula").join(format!("{}.rb", name)));
            }

            // Check subdirectories (e.g., Formula/a/axe.rb)
            if let Ok(entries) = std::fs::read_dir(&formula_dir) {
                for entry in entries.flatten() {
                    if entry.path().is_dir() {
                        let subdir_file = entry.path().join(format!("{}.rb", name));
                        if subdir_file.exists() {
                            let subdir_name = entry.file_name();
                            return Some(
                                PathBuf::from("Formula")
                                    .join(subdir_name)
                                    .join(format!("{}.rb", name)),
                            );
                        }
                    }
                }
            }
        }

        // Check Casks directory
        let casks_dir = tap_path.join("Casks");
        if casks_dir.exists() {
            let cask_file = casks_dir.join(format!("{}.rb", name));
            if cask_file.exists() {
                return Some(PathBuf::from("Casks").join(format!("{}.rb", name)));
            }

            // Check subdirectories
            if let Ok(entries) = std::fs::read_dir(&casks_dir) {
                for entry in entries.flatten() {
                    if entry.path().is_dir() {
                        let subdir_file = entry.path().join(format!("{}.rb", name));
                        if subdir_file.exists() {
                            let subdir_name = entry.file_name();
                            return Some(
                                PathBuf::from("Casks")
                                    .join(subdir_name)
                                    .join(format!("{}.rb", name)),
                            );
                        }
                    }
                }
            }
        }

        None
    }

    #[allow(clippy::too_many_arguments)]
    fn run_git_log(
        &self,
        repo_path: &PathBuf,
        file_path: Option<&PathBuf>,
        patch: bool,
        stat: bool,
        oneline: bool,
        one: bool,
        max_count: Option<usize>,
    ) -> CommandResult {
        let mut cmd = Command::new("git");
        cmd.current_dir(repo_path);
        cmd.arg("log");

        // Add format flags
        if oneline {
            cmd.arg("--oneline");
        }

        // Add count flags
        if one {
            cmd.arg("-1");
        } else if let Some(n) = max_count {
            cmd.arg("-n");
            cmd.arg(n.to_string());
        }

        // Add diff flags
        if patch {
            cmd.arg("--patch");
        }
        if stat {
            cmd.arg("--stat");
        }

        // Add file path if specified
        if let Some(path) = file_path {
            cmd.arg("--");
            cmd.arg(path);
        }

        let status = cmd.status()?;
        if !status.success() {
            std::process::exit(1);
        }

        Ok(())
    }
}

fn print_help() {
    println!("Usage: brew log [options] [formula|cask]");
    println!();
    println!("Show the git log for formula or cask, or show the log for the Homebrew");
    println!("repository if no formula or cask is provided.");
    println!();
    println!("  -p, -u, --patch                  Also print patch from commit.");
    println!("      --stat                       Also print diffstat from commit.");
    println!("      --oneline                    Print only one line per commit.");
    println!("  -1                               Print only one commit.");
    println!("  -n, --max-count                  Print only a specified number of commits.");
    println!("      --formula, --formulae        Treat all named arguments as formulae.");
    println!("      --cask, --casks              Treat all named arguments as casks.");
    println!("  -d, --debug                      Display any debugging information.");
    println!("  -q, --quiet                      Make some output more quiet.");
    println!("  -v, --verbose                    Make some output more verbose.");
    println!("  -h, --help                       Show this message.");
}
