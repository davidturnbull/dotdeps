use crate::commands::{Command, CommandResult};
use crate::paths;

pub struct Repository;

impl Command for Repository {
    fn run(&self, args: &[String]) -> CommandResult {
        // No tap arguments - just output the repository
        if args.is_empty() {
            println!("{}", paths::homebrew_repository().display());
            return Ok(());
        }

        // Handle tap arguments - show tap repository paths
        for tap in args {
            let tap_path = resolve_tap_path(tap);
            println!("{}", tap_path.display());
        }

        Ok(())
    }
}

fn resolve_tap_path(tap: &str) -> std::path::PathBuf {
    let taps_dir = paths::homebrew_taps();

    // Handle shorthand: user/repo -> user/homebrew-repo
    let parts: Vec<&str> = tap.split('/').collect();
    if parts.len() == 2 {
        let user = parts[0];
        let repo = parts[1];

        // If repo doesn't start with "homebrew-", add it
        let full_repo = if repo.starts_with("homebrew-") {
            repo.to_string()
        } else {
            format!("homebrew-{repo}")
        };

        taps_dir.join(user).join(full_repo)
    } else {
        // Assume it's already a full path or invalid
        taps_dir.join(tap)
    }
}
