use crate::paths;
use crate::tap::Tap;
use std::fs;
use std::path::Path;
use walkdir::WalkDir;

pub fn run(args: &[String]) -> Result<(), String> {
    let mut force = false;
    let mut verbose = false;
    let mut quiet = false;
    let mut tap_args = Vec::new();

    for arg in args {
        match arg.as_str() {
            "-f" | "--force" => force = true,
            "-v" | "--verbose" => verbose = true,
            "-q" | "--quiet" => quiet = true,
            "-d" | "--debug" => {
                // Ignore debug flag
            }
            "-h" | "--help" => {
                print_help();
                return Ok(());
            }
            _ => {
                if arg.starts_with('-') {
                    return Err(format!("invalid option: {}", arg));
                }
                tap_args.push(arg.clone());
            }
        }
    }

    if tap_args.is_empty() {
        return Err("This command requires at least one tap argument.".to_string());
    }

    for tap_name in tap_args {
        untap_repository(&tap_name, force, verbose, quiet)?;
    }

    Ok(())
}

fn untap_repository(tap_name: &str, force: bool, verbose: bool, quiet: bool) -> Result<(), String> {
    let tap = Tap::parse(tap_name).ok_or_else(|| format!("Invalid tap name: {}", tap_name))?;
    let tap_path = tap.path();

    if !tap_path.exists() {
        return Err(format!("No available tap {}", tap_name));
    }

    // Check for installed formulae/casks from this tap
    let installed = get_installed_from_tap(&tap_path)?;

    if !installed.is_empty() && !force {
        return Err(format!(
            "Refusing to untap {} because it contains the following installed formulae or casks:\n{}",
            tap_name,
            installed.join("\n")
        ));
    }

    if !installed.is_empty() && force {
        eprintln!(
            "Warning: Untapping {} even though it contains the following installed formulae or casks:",
            tap_name
        );
        for formula in &installed {
            eprintln!("{}", formula);
        }
    }

    if !quiet {
        println!("Untapping {}...", tap_name);
    }

    // Count formulae and get directory size before removing
    let formula_count = count_formulae(&tap_path);
    let (file_count, total_size) = count_files_and_size(&tap_path);

    // Remove the tap directory
    if let Err(e) = fs::remove_dir_all(&tap_path) {
        return Err(format!("Failed to remove tap: {}", e));
    }

    if verbose {
        println!("Removed tap directory: {}", tap_path.display());
    }

    if !quiet {
        println!(
            "Untapped {} ({} files, {}).",
            if formula_count > 0 {
                format!("{} formulae", formula_count)
            } else {
                "0 formulae".to_string()
            },
            file_count,
            format_size(total_size)
        );
    }

    Ok(())
}

fn get_installed_from_tap(tap_path: &Path) -> Result<Vec<String>, String> {
    let mut installed = Vec::new();
    let prefix = paths::homebrew_prefix();
    let opt_dir = prefix.join("opt");

    // Check if opt directory exists
    if !opt_dir.exists() {
        return Ok(installed);
    }

    // Get all formula files in the tap
    let tap_formulae = get_tap_formulae(tap_path)?;

    // Check which ones are installed by looking in opt
    for entry in fs::read_dir(&opt_dir).map_err(|e| e.to_string())? {
        let entry = entry.map_err(|e| e.to_string())?;
        let formula_name = entry.file_name().to_string_lossy().to_string();

        if tap_formulae.contains(&formula_name) {
            installed.push(formula_name);
        }
    }

    Ok(installed)
}

fn get_tap_formulae(tap_path: &Path) -> Result<Vec<String>, String> {
    let mut formulae = Vec::new();

    // Check Formula directory (old-style tap)
    let formula_dir = tap_path.join("Formula");
    if formula_dir.exists() {
        for entry in fs::read_dir(&formula_dir).map_err(|e| e.to_string())? {
            let entry = entry.map_err(|e| e.to_string())?;
            let path = entry.path();
            if path.extension().and_then(|s| s.to_str()) == Some("rb")
                && let Some(name) = path.file_stem().and_then(|s| s.to_str())
            {
                formulae.push(name.to_string());
            }
        }
    }

    // Check root directory for formula files
    for entry in fs::read_dir(tap_path).map_err(|e| e.to_string())? {
        let entry = entry.map_err(|e| e.to_string())?;
        let path = entry.path();
        if path.is_file()
            && path.extension().and_then(|s| s.to_str()) == Some("rb")
            && let Some(name) = path.file_stem().and_then(|s| s.to_str())
        {
            formulae.push(name.to_string());
        }
    }

    Ok(formulae)
}

fn count_formulae(tap_path: &Path) -> usize {
    match get_tap_formulae(tap_path) {
        Ok(formulae) => formulae.len(),
        Err(_) => 0,
    }
}

fn count_files_and_size(path: &Path) -> (usize, u64) {
    let mut file_count = 0;
    let mut total_size = 0;

    for entry in WalkDir::new(path).into_iter().filter_map(|e| e.ok()) {
        if entry.file_type().is_file() {
            file_count += 1;
            if let Ok(metadata) = entry.metadata() {
                total_size += metadata.len();
            }
        }
    }

    (file_count, total_size)
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

fn print_help() {
    println!("Usage: brew untap [--force] tap [...]");
    println!();
    println!("Remove a tapped formula repository.");
    println!();
    println!("  -f, --force                      Untap even if formulae or casks from this tap");
    println!("                                   are currently installed.");
    println!("  -d, --debug                      Display any debugging information.");
    println!("  -q, --quiet                      Make some output more quiet.");
    println!("  -v, --verbose                    Make some output more verbose.");
    println!("  -h, --help                       Show this message.");
}
