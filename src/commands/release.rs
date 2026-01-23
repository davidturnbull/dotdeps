use crate::paths;
use std::process;

pub fn execute(args: &[String]) -> Result<(), Box<dyn std::error::Error>> {
    let brew_path = paths::homebrew_prefix().join("bin/brew");
    let status = process::Command::new(brew_path)
        .arg("release")
        .args(args)
        .status()?;

    process::exit(status.code().unwrap_or(1));
}
