use crate::paths;
use crate::settings;
use std::error::Error;
use std::fs;
use std::path::Path;

const SHELLS: &[&str] = &["bash", "fish", "zsh"];

pub fn execute(args: &[String]) -> Result<(), Box<dyn Error>> {
    let subcommand = args.first().map(|s| s.as_str());

    match subcommand {
        None | Some("state") => {
            if link_completions()? {
                println!("Completions are linked.");
            } else {
                println!("Completions are not linked.");
            }
        }
        Some("link") => {
            link()?;
            println!("Completions are now linked.");
        }
        Some("unlink") => {
            unlink()?;
            println!("Completions are no longer linked.");
        }
        Some(cmd) => {
            eprintln!("Usage: brew completions [subcommand]");
            eprintln!();
            eprintln!("Control whether Homebrew automatically links external tap shell completion");
            eprintln!("files. Read more at https://docs.brew.sh/Shell-Completion.");
            eprintln!();
            eprintln!("brew completions [state]:");
            eprintln!("    Display the current state of Homebrew's completions.");
            eprintln!();
            eprintln!("brew completions (link|unlink):");
            eprintln!("    Link or unlink Homebrew's completions.");
            eprintln!();
            return Err(format!("Invalid usage: unknown subcommand: {}", cmd).into());
        }
    }

    Ok(())
}

fn link_completions() -> Result<bool, Box<dyn Error>> {
    Ok(settings::read("linkcompletions") == Some("true".to_string()))
}

fn link() -> Result<(), Box<dyn Error>> {
    settings::write("linkcompletions", "true")?;

    // Link completions for all installed taps
    let taps_dir = paths::homebrew_repository().join("Library/Taps");
    if !taps_dir.exists() {
        return Ok(());
    }

    for user_entry in fs::read_dir(&taps_dir)? {
        let user_entry = user_entry?;
        let user_path = user_entry.path();
        if !user_path.is_dir() {
            continue;
        }

        for tap_entry in fs::read_dir(&user_path)? {
            let tap_entry = tap_entry?;
            let tap_path = tap_entry.path();
            if !tap_path.is_dir() {
                continue;
            }

            link_tap_completions(&tap_path)?;
        }
    }

    Ok(())
}

fn unlink() -> Result<(), Box<dyn Error>> {
    settings::write("linkcompletions", "false")?;

    // Unlink completions for all installed taps (except official taps)
    let taps_dir = paths::homebrew_repository().join("Library/Taps");
    if !taps_dir.exists() {
        return Ok(());
    }

    for user_entry in fs::read_dir(&taps_dir)? {
        let user_entry = user_entry?;
        let user_path = user_entry.path();
        if !user_path.is_dir() {
            continue;
        }

        let user_name = user_path.file_name().unwrap().to_str().unwrap();

        for tap_entry in fs::read_dir(&user_path)? {
            let tap_entry = tap_entry?;
            let tap_path = tap_entry.path();
            if !tap_path.is_dir() {
                continue;
            }

            // Skip official taps (homebrew/*)
            if user_name == "homebrew" {
                continue;
            }

            unlink_tap_completions(&tap_path)?;
        }
    }

    Ok(())
}

fn link_tap_completions(tap_path: &Path) -> Result<(), Box<dyn Error>> {
    let completions_dir = tap_path.join("completions");
    if !completions_dir.exists() {
        return Ok(());
    }

    let prefix = paths::homebrew_prefix();

    for shell in SHELLS {
        let shell_completions = completions_dir.join(shell);
        if !shell_completions.exists() {
            continue;
        }

        let target_dir = match *shell {
            "bash" => prefix.join("share/bash-completion/completions"),
            "fish" => prefix.join("share/fish/vendor_completions.d"),
            "zsh" => prefix.join("share/zsh/site-functions"),
            _ => continue,
        };

        // Create target directory if it doesn't exist
        fs::create_dir_all(&target_dir)?;

        // Link all completion files in the shell directory
        for entry in fs::read_dir(&shell_completions)? {
            let entry = entry?;
            let source = entry.path();
            if !source.is_file() {
                continue;
            }

            let filename = source.file_name().unwrap();
            let target = target_dir.join(filename);

            // Skip if symlink already exists and points to the right place
            if target.exists() {
                if let Ok(existing_target) = fs::read_link(&target)
                    && existing_target == source
                {
                    continue;
                }
                // Remove existing symlink/file if it exists
                let _ = fs::remove_file(&target);
            }

            // Create symlink
            #[cfg(unix)]
            {
                use std::os::unix::fs::symlink;
                symlink(&source, &target)?;
            }
        }
    }

    Ok(())
}

fn unlink_tap_completions(tap_path: &Path) -> Result<(), Box<dyn Error>> {
    let completions_dir = tap_path.join("completions");
    if !completions_dir.exists() {
        return Ok(());
    }

    let prefix = paths::homebrew_prefix();

    for shell in SHELLS {
        let shell_completions = completions_dir.join(shell);
        if !shell_completions.exists() {
            continue;
        }

        let target_dir = match *shell {
            "bash" => prefix.join("share/bash-completion/completions"),
            "fish" => prefix.join("share/fish/vendor_completions.d"),
            "zsh" => prefix.join("share/zsh/site-functions"),
            _ => continue,
        };

        if !target_dir.exists() {
            continue;
        }

        // Remove symlinks that point to this tap's completions
        for entry in fs::read_dir(&shell_completions)? {
            let entry = entry?;
            let source = entry.path();
            if !source.is_file() {
                continue;
            }

            let filename = source.file_name().unwrap();
            let target = target_dir.join(filename);

            if target.exists()
                && let Ok(link_target) = fs::read_link(&target)
                && link_target == source
            {
                let _ = fs::remove_file(&target);
            }
        }
    }

    Ok(())
}
