use std::process::Command;

const HOMEBREW_DOCS_WWW: &str = "https://docs.brew.sh";

pub fn execute(_args: &[String]) -> Result<(), Box<dyn std::error::Error>> {
    // Open the Homebrew documentation in the default browser
    let status = Command::new("open").arg(HOMEBREW_DOCS_WWW).status()?;

    if !status.success() {
        return Err(format!("Failed to open browser: {}", HOMEBREW_DOCS_WWW).into());
    }

    Ok(())
}
