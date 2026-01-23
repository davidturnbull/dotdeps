use crate::api;
use crate::deps::DependencyGraph;
use crate::download;
use crate::install::Keg;
use crate::paths;
use crate::system;

pub fn run(args: &[String]) -> Result<(), String> {
    // Parse arguments
    let mut formulae = Vec::new();
    let mut force = false;
    let mut _build_from_source = false;
    let mut ignore_dependencies = false;
    let mut verbose = false;

    let mut i = 0;
    while i < args.len() {
        match args[i].as_str() {
            "-h" | "--help" => {
                print_help();
                return Ok(());
            }
            "-f" | "--force" => force = true,
            "-s" | "--build-from-source" => _build_from_source = true,
            "--ignore-dependencies" => ignore_dependencies = true,
            "-v" | "--verbose" => verbose = true,
            arg if arg.starts_with('-') => {
                return Err(format!("Unknown flag: {}", arg));
            }
            arg => formulae.push(arg.to_string()),
        }
        i += 1;
    }

    if formulae.is_empty() {
        return Err("This command requires at least one formula argument.".to_string());
    }

    // Build dependency graph for all requested formulae
    let mut all_to_install = Vec::new();

    for formula in &formulae {
        let normalized = crate::formula::normalize_name(formula);

        if ignore_dependencies {
            // Just install the requested formula
            all_to_install.push(normalized.to_string());
        } else {
            // Resolve dependencies
            let mut graph = DependencyGraph::new();
            graph.build_for_formula(normalized, false)?;

            // Get install order (dependencies first)
            let install_order = graph.topological_sort()?;

            // Add all to the list (avoiding duplicates)
            for dep in install_order {
                if !all_to_install.contains(&dep) {
                    all_to_install.push(dep);
                }
            }
        }
    }

    // Check which formulae are already installed
    let mut to_install = Vec::new();
    let mut already_installed = Vec::new();

    for formula in &all_to_install {
        // Check if already installed by checking if opt symlink exists
        let opt_path = paths::homebrew_prefix().join("opt").join(formula);
        if opt_path.exists() && !force {
            already_installed.push(formula.clone());
        } else {
            to_install.push(formula.clone());
        }
    }

    // Report already installed formulae
    if !already_installed.is_empty() && !force {
        for formula in &already_installed {
            let info = api::get_formula(formula)?;
            if let Some(version) = &info.versions.stable
                && formulae.contains(formula)
            {
                // Only show warning for explicitly requested formulae
                println!(
                    "Warning: {} {} is already installed and up-to-date.",
                    formula, version
                );
                println!(
                    "To reinstall {}, run:\n  brew reinstall {}",
                    version, formula
                );
            }
        }
    }

    // If nothing to install, we're done
    if to_install.is_empty() {
        return Ok(());
    }

    // Show what will be installed
    println!("==> Installing dependencies: {}", to_install.join(", "));

    // Create HTTP client for downloads
    let client = reqwest::blocking::Client::builder()
        .user_agent("Homebrew Rust/0.1.0")
        .build()
        .map_err(|e| format!("Failed to create HTTP client: {}", e))?;

    // Install each formula
    for formula_name in &to_install {
        install_formula(&client, formula_name, &formulae, verbose)?;
    }

    // Summary
    println!("\n==> Successfully installed {} formulae", to_install.len());

    Ok(())
}

/// Install a single formula
fn install_formula(
    _client: &reqwest::blocking::Client,
    formula_name: &str,
    requested_formulae: &[String],
    verbose: bool,
) -> Result<(), String> {
    println!("\n==> Installing {}", formula_name);

    // Load formula info
    let info = api::get_formula(formula_name)?;

    // Get the stable version
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
        println!("==> Downloading {}", bottle_file.url);

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
        println!("==> Using cached bottle: {}", cache_path.display());
        // Verify cached bottle
        download::verify_sha256(&cache_path, &bottle_file.sha256)?;
    }

    // Create Keg for installation
    let keg = Keg::new(formula_name.to_string(), version.to_string());

    // Check if already exists and remove if forcing
    if keg.exists() {
        if verbose {
            println!(
                "==> Removing existing installation at {}",
                keg.path.display()
            );
        }
        std::fs::remove_dir_all(&keg.path)
            .map_err(|e| format!("Failed to remove existing installation: {}", e))?;
    }

    // Extract bottle to Cellar
    println!("==> Pouring {}", filename);
    download::extract_tar_gz(&cache_path, &paths::homebrew_cellar())?;

    // Verify extraction
    if !keg.exists() {
        return Err(format!(
            "Bottle extraction failed - keg does not exist at {}",
            keg.path.display()
        ));
    }

    // Create opt symlink
    if verbose {
        println!("==> Creating opt symlink for {}", formula_name);
    }
    keg.link_opt()?;

    // Write installation metadata
    let installed_as_dependency = !requested_formulae.contains(&formula_name.to_string());
    keg.write_tab(&info, installed_as_dependency)?;

    println!("==> Installed {} {}", formula_name, version);

    Ok(())
}

#[allow(dead_code)]
pub fn name() -> &'static str {
    "install"
}

#[allow(dead_code)]
pub fn description() -> &'static str {
    "Install a formula"
}

fn print_help() {
    println!(
        "Usage: brew install [options] formula [...]

Install a formula. Additional options specific to a formula may be appended
to the command.

  -f, --force                      Install formulae without checking for
                                   previously installed versions.
  -v, --verbose                    Print the verification and post-install
                                   steps.
  -s, --build-from-source          Compile formula from source even if a
                                   bottle is provided.
      --ignore-dependencies        Skip installing any dependencies of any kind.
  -h, --help                       Show this message."
    );
}
