use sha2::{Digest, Sha256};

use crate::api;
use crate::commands::{Command, CommandResult};
use crate::paths;
use crate::system;

pub struct Cache;

impl Command for Cache {
    fn run(&self, args: &[String]) -> CommandResult {
        let mut formula_args: Vec<String> = Vec::new();
        let mut build_from_source = false;
        let mut force_bottle = false;
        let mut head = false;
        let mut formula_only = false;
        let mut cask_only = false;
        let mut bottle_tag: Option<String> = None;

        // Parse flags
        let mut i = 0;
        while i < args.len() {
            let arg = &args[i];
            match arg.as_str() {
                "-s" | "--build-from-source" => build_from_source = true,
                "--force-bottle" => force_bottle = true,
                "--HEAD" => head = true,
                "--formula" | "--formulae" => formula_only = true,
                "--cask" | "--casks" => cask_only = true,
                "--bottle-tag" => {
                    i += 1;
                    if i < args.len() {
                        bottle_tag = Some(args[i].clone());
                    }
                }
                s if s.starts_with("--bottle-tag=") => {
                    bottle_tag = Some(s.trim_start_matches("--bottle-tag=").to_string());
                }
                "--os" | "--arch" => {
                    // Skip the value
                    i += 1;
                }
                s if s.starts_with("--os=") || s.starts_with("--arch=") => {
                    // Ignore for now
                }
                s if s.starts_with('-') => {
                    // Skip other flags
                }
                _ => {
                    formula_args.push(arg.clone());
                }
            }
            i += 1;
        }

        // No formula/cask arguments - just output the cache
        if formula_args.is_empty() {
            println!("{}", paths::homebrew_cache().display());
            return Ok(());
        }

        // Process each formula/cask argument
        for name in &formula_args {
            // Try as formula first (unless --cask only)
            if !cask_only
                && let Ok(true) = print_formula_cache(
                    name,
                    build_from_source,
                    force_bottle,
                    head,
                    bottle_tag.as_deref(),
                )
            {
                continue;
            }

            // Try as cask (unless --formula only)
            if !formula_only && let Ok(true) = print_cask_cache(name) {
                continue;
            }

            // Not found
            eprintln!("Error: No available formula or cask with the name \"{name}\".");
            return Err(format!("No available formula or cask with the name \"{name}\".").into());
        }

        Ok(())
    }
}

/// Print the cache path for a formula. Returns Ok(true) if found.
fn print_formula_cache(
    name: &str,
    build_from_source: bool,
    _force_bottle: bool,
    head: bool,
    bottle_tag: Option<&str>,
) -> Result<bool, String> {
    let formula = api::get_formula(name)?;

    if head {
        // HEAD builds are just directories with the formula name
        let cache_dir = paths::homebrew_cache().join(format!("{}--git", formula.name));
        println!("{}", cache_dir.display());
        return Ok(true);
    }

    // Get the version
    let version = formula
        .versions
        .stable
        .as_ref()
        .ok_or("Formula has no stable version")?;

    if build_from_source {
        // Source download
        if let Some(ref urls) = formula.urls
            && let Some(ref stable) = urls.stable
        {
            let cache_path = compute_cache_path(&stable.url, &formula.name, version, None);
            println!("{}", cache_path.display());
            return Ok(true);
        }
        return Err("Formula has no source URL".into());
    }

    // Get bottle info
    let bottle = formula
        .bottle
        .as_ref()
        .and_then(|b| b.stable.as_ref())
        .ok_or("Formula has no bottle")?;

    // Determine the bottle tag to use
    let tag = if let Some(explicit_tag) = bottle_tag {
        explicit_tag.to_string()
    } else {
        system::bottle_tag().ok_or("Could not determine bottle tag for this system")?
    };

    // Find the bottle file for this tag
    let bottle_file = bottle.files.get(&tag).ok_or_else(|| {
        // Try to find a fallback tag (e.g., if arm64_tahoe doesn't exist, try arm64_sequoia)
        format!("Bottle for tag {tag:?} is unavailable.")
    })?;

    // Compute the cache path
    // The filename format is: {name}--{version}.{tag}.bottle.tar.gz
    let rebuild = if bottle.rebuild > 0 {
        format!(".{}", bottle.rebuild)
    } else {
        String::new()
    };
    let filename = format!(
        "{}--{}.{}.bottle{}.tar.gz",
        formula.name, version, tag, rebuild
    );

    let cache_path = compute_cache_path(&bottle_file.url, &formula.name, version, Some(&filename));
    println!("{}", cache_path.display());

    Ok(true)
}

/// Print the cache path for a cask. Returns Ok(true) if found.
fn print_cask_cache(name: &str) -> Result<bool, String> {
    let cask = api::get_cask(name)?;

    // Get the URL - the default URL in the API is for the current platform (e.g., arm64)
    // Variations are for OTHER platforms (e.g., x86_64 Intel Macs)
    //
    // The variation keys are:
    // - codename only (e.g., "tahoe") = x86_64 for that macOS version
    // - arch_codename (e.g., "arm64_big_sur") = arm64 for that macOS version
    //
    // On arm64, we should check for arm64_codename first, then fall back to default
    // On x86_64, we should check for codename (no prefix)
    let url = if let Some(ref variations) = cask.variations {
        let codename = system::macos_codename().unwrap_or("tahoe");
        let arch = system::arch();

        // On arm64, first check for arm64_codename variation
        // If not found, use the default URL (which is usually arm64)
        if arch == "arm64" {
            let arm64_key = format!("arm64_{codename}");
            if let Some(var) = variations.get(&arm64_key) {
                var.url
                    .clone()
                    .unwrap_or_else(|| cask.url.clone().unwrap_or_default())
            } else {
                // Default URL is typically for arm64
                cask.url.clone().ok_or("Cask has no URL")?
            }
        } else {
            // On x86_64, check for codename variation
            if let Some(var) = variations.get(codename) {
                var.url
                    .clone()
                    .unwrap_or_else(|| cask.url.clone().unwrap_or_default())
            } else {
                cask.url.clone().ok_or("Cask has no URL")?
            }
        }
    } else {
        cask.url.clone().ok_or("Cask has no URL")?
    };

    // Extract filename from URL
    let filename = url
        .rsplit('/')
        .next()
        .unwrap_or(&cask.token)
        .split('?')
        .next()
        .unwrap_or(&cask.token);

    let cache_path = compute_cask_cache_path(&url, filename);
    println!("{}", cache_path.display());

    Ok(true)
}

/// Compute the cache path for a download.
/// Format: HOMEBREW_CACHE/downloads/{SHA256_of_URL}--{filename}
fn compute_cache_path(
    url: &str,
    _name: &str,
    _version: &str,
    explicit_filename: Option<&str>,
) -> std::path::PathBuf {
    let url_hash = sha256_hex(url);

    let filename = if let Some(f) = explicit_filename {
        f.to_string()
    } else {
        // Extract filename from URL
        // The basename is the last path component, stripped of query strings
        url.rsplit('/')
            .next()
            .unwrap_or("download")
            .split('?')
            .next()
            .unwrap_or("download")
            .to_string()
    };

    paths::homebrew_cache()
        .join("downloads")
        .join(format!("{url_hash}--{filename}"))
}

/// Compute the cache path for a cask download.
fn compute_cask_cache_path(url: &str, filename: &str) -> std::path::PathBuf {
    let url_hash = sha256_hex(url);

    paths::homebrew_cache()
        .join("downloads")
        .join(format!("{url_hash}--{filename}"))
}

/// Compute SHA256 hash of a string and return as hex.
fn sha256_hex(input: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(input.as_bytes());
    let result = hasher.finalize();
    hex_encode(&result)
}

/// Encode bytes as lowercase hex string.
fn hex_encode(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}
