use crate::paths;
use std::process::Command;

pub fn execute(args: &[String]) -> Result<(), Box<dyn std::error::Error>> {
    // bump-revision command requires Ruby DSL
    // Delegate to the original brew implementation
    let brew_bin = paths::homebrew_prefix().join("bin/brew");

    let mut cmd = Command::new(&brew_bin);
    cmd.arg("bump-revision").args(args);

    let status = cmd.status()?;

    if !status.success() {
        std::process::exit(status.code().unwrap_or(1));
    }

    Ok(())
}
