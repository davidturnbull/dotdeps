use crate::paths;
use std::process::Command;

pub fn execute(args: &[String]) -> Result<(), Box<dyn std::error::Error>> {
    // bump-cask-pr requires Ruby DSL and GitHub API interaction
    // Delegate to the original brew implementation
    let brew_bin = paths::homebrew_prefix().join("bin/brew");

    let mut cmd = Command::new(&brew_bin);
    cmd.arg("bump-cask-pr").args(args);

    let status = cmd.status()?;

    if !status.success() {
        std::process::exit(status.code().unwrap_or(1));
    }

    Ok(())
}
