use crate::api;
use crate::commands::{Command, CommandResult};
use crate::formula;
use crate::paths;
use std::fs;
use std::path::PathBuf;

pub struct InfoCommand;

impl Command for InfoCommand {
    fn run(&self, args: &[String]) -> CommandResult {
        let mut json_mode = false;
        let mut formulae = Vec::new();

        for arg in args {
            if arg == "--json" || arg.starts_with("--json=") {
                json_mode = true;
            } else if !arg.starts_with("--") {
                formulae.push(arg.clone());
            }
        }

        if formulae.is_empty() {
            // Show summary statistics
            show_summary()?;
        } else if json_mode {
            // JSON output
            show_json(&formulae)?;
        } else {
            // Text output for each formula
            for (i, formula_name) in formulae.iter().enumerate() {
                if i > 0 {
                    println!();
                }
                show_formula_info(formula_name)?;
            }
        }

        Ok(())
    }
}

fn show_summary() -> CommandResult {
    // TODO: Implement summary statistics
    println!("Summary statistics not yet implemented");
    Ok(())
}

fn show_json(formulae: &[String]) -> CommandResult {
    let mut results = Vec::new();

    for formula_name in formulae {
        let normalized = formula::normalize_name(formula_name);
        match api::get_formula(normalized) {
            Ok(info) => results.push(info),
            Err(e) => {
                eprintln!("Error: {}", e);
                std::process::exit(1);
            }
        }
    }

    // Output JSON in v2 format
    let output = serde_json::json!({
        "formulae": results,
        "casks": []
    });

    println!("{}", serde_json::to_string_pretty(&output)?);
    Ok(())
}

fn show_formula_info(formula_name: &str) -> CommandResult {
    let normalized = formula::normalize_name(formula_name);

    let info = match api::get_formula(normalized) {
        Ok(info) => info,
        Err(_) => {
            eprintln!(
                "Error: No available formula with the name \"{}\"",
                formula_name
            );
            std::process::exit(1);
        }
    };

    // First line: ==> name: stable version (bottled), HEAD
    print!("==> {}: ", info.name);
    if let Some(stable) = &info.versions.stable {
        print!("stable {} ", stable);
        if info.bottle.is_some() {
            print!("(bottled)");
        }
    }
    if info.versions.head.is_some() {
        if info.versions.stable.is_some() {
            print!(", ");
        }
        print!("HEAD");
    }
    println!();

    // Description
    if let Some(desc) = &info.desc {
        println!("{}", desc);
    }

    // Homepage
    if let Some(homepage) = &info.homepage {
        println!("{}", homepage);
    }

    // Installation status
    if info.installed.is_empty() {
        println!("Not installed");
    } else {
        println!("Installed");
        for installed_version in &info.installed {
            let cellar_path = get_cellar_path(&info.name, &installed_version.version);
            let file_count = count_files(&cellar_path);
            let size = get_directory_size(&cellar_path);

            // Check if this is the linked version
            let linked_marker = if Some(&installed_version.version) == info.linked_keg.as_ref() {
                " *"
            } else {
                ""
            };

            // Determine if poured or built
            let install_method = if installed_version.poured_from_bottle {
                "Poured from bottle using the formulae.brew.sh API"
            } else {
                "Built from source"
            };

            // Format time
            let time_str = if let Some(time) = installed_version.time {
                use chrono::{Local, TimeZone};
                let dt = Local.timestamp_opt(time, 0).unwrap();
                dt.format(" on %Y-%m-%d at %H:%M:%S").to_string()
            } else {
                String::new()
            };

            println!(
                "{} ({} files, {}){}",
                cellar_path.display(),
                format_number(file_count),
                format_size(size),
                linked_marker
            );
            println!("  {}{}", install_method, time_str);
        }
    }

    // From
    let tap_url = if info.tap == "homebrew/core" {
        format!(
            "https://github.com/Homebrew/homebrew-core/blob/HEAD/Formula/{}/{}.rb",
            info.name.chars().next().unwrap().to_lowercase(),
            info.name
        )
    } else {
        format!("homebrew/{}", info.tap)
    };
    println!("From: {}", tap_url);

    // License
    if let Some(license) = &info.license {
        println!("License: {}", license);
    }

    // Dependencies
    if !info.build_dependencies.is_empty() {
        println!("==> Dependencies");
        println!("Build: {}", info.build_dependencies.join(", "));
    }
    if !info.dependencies.is_empty() {
        if info.build_dependencies.is_empty() {
            println!("==> Dependencies");
        }
        println!("Required: {}", info.dependencies.join(", "));
    }

    // Options
    if info.versions.head.is_some() {
        println!("==> Options");
        println!("--HEAD");
        println!("\tInstall HEAD version");
    }

    // Caveats
    if let Some(caveats) = &info.caveats {
        println!("==> Caveats");
        println!("{}", caveats);
    }

    // Analytics
    println!("==> Analytics");
    println!("install: 0 (30 days), 0 (90 days), 0 (365 days)");
    println!("install-on-request: 0 (30 days), 0 (90 days), 0 (365 days)");
    println!("build-error: 0 (30 days)");

    Ok(())
}

fn get_cellar_path(name: &str, version: &str) -> PathBuf {
    paths::homebrew_cellar().join(name).join(version)
}

fn count_files(path: &PathBuf) -> usize {
    if !path.exists() {
        return 0;
    }

    let mut count = 0;
    if let Ok(entries) = fs::read_dir(path) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_file() {
                count += 1;
            } else if path.is_dir() {
                count += count_files(&path);
            }
        }
    }
    count
}

fn get_directory_size(path: &PathBuf) -> u64 {
    if !path.exists() {
        return 0;
    }

    let mut size = 0;
    if let Ok(entries) = fs::read_dir(path) {
        for entry in entries.flatten() {
            let path = entry.path();
            // Use symlink_metadata to not follow symlinks
            if let Ok(metadata) = fs::symlink_metadata(&path) {
                if metadata.is_file() {
                    size += metadata.len();
                } else if metadata.is_dir() {
                    size += get_directory_size(&path);
                }
            }
        }
    }
    size
}

fn format_size(bytes: u64) -> String {
    const KB: u64 = 1024;
    const MB: u64 = KB * 1024;
    const GB: u64 = MB * 1024;

    if bytes >= GB {
        format!("{:.1}GB", bytes as f64 / GB as f64)
    } else if bytes >= MB {
        format!("{:.1}MB", bytes as f64 / MB as f64)
    } else if bytes >= KB {
        format!("{:.1}KB", bytes as f64 / KB as f64)
    } else {
        format!("{}B", bytes)
    }
}

fn format_number(num: usize) -> String {
    let s = num.to_string();
    let mut result = String::new();
    let chars: Vec<char> = s.chars().collect();

    for (i, c) in chars.iter().enumerate() {
        if i > 0 && (chars.len() - i).is_multiple_of(3) {
            result.push(',');
        }
        result.push(*c);
    }

    result
}
