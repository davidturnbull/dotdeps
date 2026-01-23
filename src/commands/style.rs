use crate::paths;
use std::process::Command;

pub fn execute(args: &[String]) -> Result<(), Box<dyn std::error::Error>> {
    // style command requires Ruby DSL
    // Delegate to the original brew implementation
    let brew_bin = paths::homebrew_prefix().join("bin/brew");

    let mut cmd = Command::new(&brew_bin);
    cmd.arg("style").args(args);

    let status = cmd.status()?;

    if !status.success() {
        std::process::exit(status.code().unwrap_or(1));
    }

    Ok(())
}
