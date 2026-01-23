use crate::api;
use crate::paths;
use regex::Regex;
use std::fs;

pub fn run(args: &[String]) -> Result<(), Box<dyn std::error::Error>> {
    let mut search_formulae = false;
    let mut search_casks = false;
    let mut search_desc = false;
    let mut patterns = Vec::new();

    // Parse arguments
    let mut i = 0;
    while i < args.len() {
        match args[i].as_str() {
            "--formula" | "--formulae" => search_formulae = true,
            "--cask" | "--casks" => search_casks = true,
            "--desc" => search_desc = true,
            arg => patterns.push(arg.to_string()),
        }
        i += 1;
    }

    // If no specific type selected, search both
    if !search_formulae && !search_casks {
        search_formulae = true;
        search_casks = true;
    }

    // If no patterns provided, error out
    if patterns.is_empty() {
        eprintln!("Error: No search term specified.");
        std::process::exit(1);
    }

    let mut found_any = false;

    for pattern in &patterns {
        let is_regex = pattern.starts_with('/') && pattern.ends_with('/');
        let regex = if is_regex {
            let pattern_str = &pattern[1..pattern.len() - 1];
            match Regex::new(pattern_str) {
                Ok(r) => Some(r),
                Err(e) => {
                    eprintln!("Error: Invalid regex: {}", e);
                    std::process::exit(1);
                }
            }
        } else {
            None
        };

        let mut formula_results = Vec::new();
        let mut cask_results = Vec::new();

        if search_formulae {
            if search_desc {
                // Search formula descriptions
                formula_results = search_formula_descriptions(pattern, regex.as_ref())?;
            } else {
                // Search formula names
                formula_results = search_formula_names(pattern, regex.as_ref())?;
            }
        }

        if search_casks {
            if search_desc {
                // Search cask names and descriptions
                cask_results = search_cask_descriptions(pattern, regex.as_ref())?;
            } else {
                // Search cask names
                cask_results = search_cask_names(pattern, regex.as_ref())?;
            }
        }

        if formula_results.is_empty() && cask_results.is_empty() {
            if patterns.len() == 1 {
                eprintln!("Error: No formulae or casks found for \"{}\".", pattern);
                std::process::exit(1);
            }
        } else {
            found_any = true;
        }

        // Display results
        if !formula_results.is_empty() {
            if search_desc {
                println!("==> Formulae");
            }
            for result in &formula_results {
                println!("{}", result);
            }
        }

        if !cask_results.is_empty() {
            if !formula_results.is_empty() {
                println!();
            }
            if search_desc {
                println!("==> Casks");
            }
            for result in &cask_results {
                println!("{}", result);
            }
        }
    }

    if !found_any && patterns.len() > 1 {
        eprintln!("Error: No formulae or casks found for any search term.");
        std::process::exit(1);
    }

    Ok(())
}

fn search_formula_names(
    pattern: &str,
    regex: Option<&Regex>,
) -> Result<Vec<String>, Box<dyn std::error::Error>> {
    let cache_dir = paths::homebrew_cache();
    let formula_names_path = cache_dir.join("api/formula_names.txt");

    if !formula_names_path.exists() {
        return Ok(Vec::new());
    }

    let content = fs::read_to_string(&formula_names_path)?;
    let mut results = Vec::new();

    for line in content.lines() {
        let name = line.trim();
        if name.is_empty() {
            continue;
        }

        if let Some(re) = regex {
            if re.is_match(name) {
                results.push(name.to_string());
            }
        } else {
            // Substring match (case-insensitive)
            if name.to_lowercase().contains(&pattern.to_lowercase()) {
                results.push(name.to_string());
            }
        }
    }

    results.sort();
    Ok(results)
}

fn search_cask_names(
    pattern: &str,
    regex: Option<&Regex>,
) -> Result<Vec<String>, Box<dyn std::error::Error>> {
    let cache_dir = paths::homebrew_cache();
    let cask_names_path = cache_dir.join("api/cask_names.txt");

    if !cask_names_path.exists() {
        return Ok(Vec::new());
    }

    let content = fs::read_to_string(&cask_names_path)?;
    let mut results = Vec::new();

    for line in content.lines() {
        let name = line.trim();
        if name.is_empty() {
            continue;
        }

        if let Some(re) = regex {
            if re.is_match(name) {
                results.push(name.to_string());
            }
        } else {
            // Substring match (case-insensitive)
            if name.to_lowercase().contains(&pattern.to_lowercase()) {
                results.push(name.to_string());
            }
        }
    }

    results.sort();
    Ok(results)
}

fn search_formula_descriptions(
    pattern: &str,
    regex: Option<&Regex>,
) -> Result<Vec<String>, Box<dyn std::error::Error>> {
    let formulae = api::load_all_formulae()?;
    let mut results = Vec::new();

    for formula in formulae {
        let desc = formula.desc.unwrap_or_default();
        let matches = if let Some(re) = regex {
            re.is_match(&formula.name) || re.is_match(&desc)
        } else {
            let pattern_lower = pattern.to_lowercase();
            formula.name.to_lowercase().contains(&pattern_lower)
                || desc.to_lowercase().contains(&pattern_lower)
        };

        if matches {
            let entry = format!("{}: {}", formula.name, desc);
            results.push(entry);
        }
    }

    results.sort();
    Ok(results)
}

fn search_cask_descriptions(
    pattern: &str,
    regex: Option<&Regex>,
) -> Result<Vec<String>, Box<dyn std::error::Error>> {
    let casks = api::load_all_casks()?;
    let mut results = Vec::new();

    for cask in casks {
        let token = &cask.token;
        let desc = cask.desc.as_deref().unwrap_or("");
        let display_name = cask.name.first().map(|s| s.as_str()).unwrap_or(token);

        let matches = if let Some(re) = regex {
            re.is_match(token)
                || re.is_match(desc)
                || cask.name.iter().any(|name| re.is_match(name))
        } else {
            let pattern_lower = pattern.to_lowercase();
            token.to_lowercase().contains(&pattern_lower)
                || desc.to_lowercase().contains(&pattern_lower)
                || cask
                    .name
                    .iter()
                    .any(|name| name.to_lowercase().contains(&pattern_lower))
        };

        if matches {
            let entry = if !desc.is_empty() {
                format!("{}: ({}) {}", token, display_name, desc)
            } else {
                format!("{}: ({})", token, display_name)
            };
            results.push(entry);
        }
    }

    results.sort();
    Ok(results)
}
