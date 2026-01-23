//! Upgrade command implementation.

use crate::api;
use crate::download;
use crate::install::Keg;
use crate::paths;
use crate::system;
use std::fs;

pub fn run(args: &[String]) -> Result<(), String> {
    let mut dry_run = false;
    let mut force = false;
    let mut verbose = false;
    let mut quiet = false;
    let mut _build_from_source = false;
    let mut specific_formulae = Vec::new();

    // Parse flags
    let mut i = 0;
    while i < args.len() {
        match args[i].as_str() {
            "-n" | "--dry-run" => dry_run = true,
            "-f" | "--force" => force = true,
            "-v" | "--verbose" => verbose = true,
            "-q" | "--quiet" => quiet = true,
            "-s" | "--build-from-source" => _build_from_source = true,
            "-h" | "--help" => {
                print_help();
                return Ok(());
            }
            // Parse and ignore these for now
            "-d"
            | "--debug"
            | "--display-times"
            | "--ask"
            | "-i"
            | "--interactive"
            | "--force-bottle"
            | "--fetch-HEAD"
            | "--keep-tmp"
            | "--debug-symbols"
            | "--overwrite"
            | "-g"
            | "--greedy"
            | "--greedy-latest"
            | "--greedy-auto-updates"
            | "--skip-cask-deps"
            | "--[no-]binaries"
            | "--require-sha"
            | "--appdir"
            | "--keyboard-layoutdir"
            | "--colorpickerdir"
            | "--prefpanedir"
            | "--qlplugindir"
            | "--mdimporterdir"
            | "--dictionarydir"
            | "--fontdir"
            | "--servicedir"
            | "--input-methoddir"
            | "--internet-plugindir"
            | "--audio-unit-plugindir"
            | "--vst-plugindir"
            | "--vst3-plugindir"
            | "--screen-saverdir"
            | "--language" => {
                // Ignore for now
            }
            "--formula" | "--formulae" => {
                // TODO: Filter to formulae only when cask support is added
            }
            "--cask" | "--casks" => {
                // TODO: Filter to casks only when cask support is added
            }
            arg if !arg.starts_with('-') => {
                specific_formulae.push(arg.to_string());
            }
            _ => {}
        }
        i += 1;
    }

    // Load all formulae from API cache
    let formulae = api::load_all_formulae()?;

    // Find outdated formulae
    let mut outdated_formulae = Vec::new();

    for formula in formulae {
        // If specific formulae requested, filter
        if !specific_formulae.is_empty()
            && !specific_formulae.contains(&formula.name)
            && !specific_formulae.contains(&formula.full_name)
        {
            continue;
        }

        // Skip if not installed (check opt symlink exists)
        let opt_path = paths::homebrew_prefix().join("opt").join(&formula.name);
        if !opt_path.exists() {
            continue;
        }

        // Get installed versions from Cellar directory
        let cellar_path = paths::homebrew_cellar().join(&formula.name);
        let mut installed_versions = Vec::new();

        if let Ok(entries) = fs::read_dir(&cellar_path) {
            for entry in entries.flatten() {
                if let Ok(file_type) = entry.file_type()
                    && file_type.is_dir()
                    && let Some(version) = entry.file_name().to_str()
                {
                    installed_versions.push(version.to_string());
                }
            }
        }

        if installed_versions.is_empty() {
            continue;
        }

        // Get current version from API (stable + revision)
        let current_version = match &formula.versions.stable {
            Some(v) => {
                if formula.revision > 0 {
                    format!("{}_{}", v, formula.revision)
                } else {
                    v.clone()
                }
            }
            None => continue,
        };

        // Check if any installed version is outdated
        let mut is_outdated = false;

        for installed in &installed_versions {
            if installed != &current_version {
                is_outdated = true;
                break;
            }
        }

        // If all installed versions match current, not outdated
        if !is_outdated {
            continue;
        }

        // Skip pinned formulae
        if formula.pinned {
            if verbose && !quiet {
                eprintln!("Skipping pinned formula: {}", formula.name);
            }
            continue;
        }

        outdated_formulae.push((
            formula.name.clone(),
            installed_versions.clone(),
            current_version.clone(),
        ));
    }

    // If nothing to upgrade, we're done
    if outdated_formulae.is_empty() {
        if !quiet {
            println!("==> No outdated formulae to upgrade.");
        }
        return Ok(());
    }

    // Show what would be upgraded
    if dry_run || verbose {
        if outdated_formulae.len() == 1 {
            println!("==> Would upgrade 1 outdated package:");
        } else {
            println!(
                "==> Would upgrade {} outdated packages:",
                outdated_formulae.len()
            );
        }

        for (name, installed_versions, current_version) in &outdated_formulae {
            // Show the first installed version (typically the linked one)
            let installed = installed_versions.first().unwrap();
            println!("{} {} -> {}", name, installed, current_version);
        }

        if dry_run {
            return Ok(());
        }
    }

    // Create HTTP client for downloads
    let client = reqwest::blocking::Client::builder()
        .user_agent("Homebrew Rust/0.1.0")
        .build()
        .map_err(|e| format!("Failed to create HTTP client: {}", e))?;

    // Upgrade each formula
    for (formula_name, old_versions, new_version) in &outdated_formulae {
        upgrade_formula(
            &client,
            formula_name,
            old_versions,
            new_version,
            force,
            verbose,
            quiet,
        )?;
    }

    // Summary
    if !quiet {
        if outdated_formulae.len() == 1 {
            println!("\n==> Successfully upgraded 1 formula");
        } else {
            println!(
                "\n==> Successfully upgraded {} formulae",
                outdated_formulae.len()
            );
        }
    }

    Ok(())
}

/// Upgrade a single formula
fn upgrade_formula(
    _client: &reqwest::blocking::Client,
    formula_name: &str,
    old_versions: &[String],
    new_version: &str,
    force: bool,
    verbose: bool,
    quiet: bool,
) -> Result<(), String> {
    if !quiet {
        println!("\n==> Upgrading {}", formula_name);
    }

    // Load formula info
    let info = api::get_formula(formula_name)?;

    // Get the stable version (without revision suffix)
    let version = info
        .versions
        .stable
        .as_ref()
        .ok_or_else(|| format!("No stable version for formula '{}'", formula_name))?;

    // Check if we have a bottle
    let bottle_spec = info
        .bottle
        .as_ref()
        .and_then(|b| b.stable.as_ref())
        .ok_or_else(|| format!("No bottle available for formula '{}'", formula_name))?;

    // Get the bottle for our platform
    let bottle_tag =
        system::bottle_tag().ok_or_else(|| "Could not determine bottle tag".to_string())?;

    let bottle_file = bottle_spec.files.get(&bottle_tag).ok_or_else(|| {
        format!(
            "No bottle available for platform '{}' for formula '{}'",
            bottle_tag, formula_name
        )
    })?;

    // Calculate cache path (matching Homebrew's scheme)
    // The filename format is: {name}--{version}.{tag}.bottle.tar.gz
    let rebuild = if bottle_spec.rebuild > 0 {
        format!(".{}", bottle_spec.rebuild)
    } else {
        String::new()
    };
    let filename = format!(
        "{}--{}.{}.bottle{}.tar.gz",
        formula_name, version, bottle_tag, rebuild
    );

    let url_hash = download::sha256_url(&bottle_file.url);
    let cache_path = paths::homebrew_cache()
        .join("downloads")
        .join(format!("{}--{}", url_hash, filename));

    if verbose {
        println!("==> Downloading {} from {}", formula_name, bottle_file.url);
        println!("    Cache path: {}", cache_path.display());
    }

    // Download bottle if not cached
    if !cache_path.exists() {
        if !quiet {
            println!("==> Downloading {}", bottle_file.url);
        }

        // Use tokio runtime for async download
        let runtime = tokio::runtime::Runtime::new()
            .map_err(|e| format!("Failed to create async runtime: {}", e))?;

        runtime.block_on(async {
            download::download_and_verify(
                &reqwest::Client::new(),
                &bottle_file.url,
                &cache_path,
                &bottle_file.sha256,
            )
            .await
        })?;
    } else {
        if verbose && !quiet {
            println!("==> Using cached bottle: {}", cache_path.display());
        }
        // Verify cached bottle
        download::verify_sha256(&cache_path, &bottle_file.sha256)?;
    }

    // Create Keg for new installation
    let new_keg = Keg::new(formula_name.to_string(), version.to_string());

    // Check if already exists and remove if forcing
    if new_keg.exists() {
        if verbose && !quiet {
            println!(
                "==> Removing existing installation at {}",
                new_keg.path.display()
            );
        }
        std::fs::remove_dir_all(&new_keg.path)
            .map_err(|e| format!("Failed to remove existing installation: {}", e))?;
    }

    // Extract bottle to Cellar
    if !quiet {
        println!("==> Pouring {}", filename);
    }
    download::extract_tar_gz(&cache_path, &paths::homebrew_cellar())?;

    // Verify extraction
    if !new_keg.exists() {
        return Err(format!(
            "Bottle extraction failed - keg does not exist at {}",
            new_keg.path.display()
        ));
    }

    // Create opt symlink
    if verbose && !quiet {
        println!("==> Updating opt symlink for {}", formula_name);
    }
    new_keg.link_opt()?;

    // Write installation metadata
    new_keg.write_tab(&info, false)?;

    // Remove old versions (unless --force specified to keep all)
    if !force {
        for old_version in old_versions {
            if old_version != version {
                let old_keg = Keg::new(formula_name.to_string(), old_version.to_string());
                if old_keg.exists() {
                    if verbose && !quiet {
                        println!("==> Removing old version {}", old_version);
                    }
                    std::fs::remove_dir_all(&old_keg.path).map_err(|e| {
                        format!("Failed to remove old version {}: {}", old_version, e)
                    })?;
                }
            }
        }
    }

    if !quiet {
        println!(
            "==> Upgraded {} {} -> {}",
            formula_name,
            old_versions.first().unwrap(),
            new_version
        );
    }

    Ok(())
}

fn print_help() {
    println!(
        "Usage: brew upgrade [options] [installed_formula ...]

Upgrade outdated formulae. If formula are specified, upgrade only the given
formula kegs (unless they are pinned; see pin, unpin).

  -n, --dry-run                    Show what would be upgraded, but do not
                                   actually upgrade anything.
  -f, --force                      Install formulae without checking for
                                   previously installed versions.
  -v, --verbose                    Print the verification and post-install
                                   steps.
  -q, --quiet                      Make some output more quiet.
  -s, --build-from-source          Compile formula from source even if a
                                   bottle is available.
  -h, --help                       Show this message."
    );
}
