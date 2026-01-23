use crate::api;
use regex::Regex;
use std::collections::BTreeSet;

pub fn run(args: &[String]) -> Result<(), Box<dyn std::error::Error>> {
    let mut search_mode = false;
    let mut name_mode = false;
    let mut description_mode = false;
    let mut _eval_all = false;
    let mut _formula_only = false;
    let mut _cask_only = false;
    let mut query_args = Vec::new();

    let mut i = 0;
    while i < args.len() {
        match args[i].as_str() {
            "-s" | "--search" => search_mode = true,
            "-n" | "--name" => name_mode = true,
            "-d" | "--description" => description_mode = true,
            "--eval-all" => _eval_all = true,
            "--formula" | "--formulae" => _formula_only = true,
            "--cask" | "--casks" => _cask_only = true,
            "-h" | "--help" => {
                print_help();
                return Ok(());
            }
            "-q" | "--quiet" => {}   // TODO: implement quiet mode
            "-v" | "--verbose" => {} // TODO: implement verbose mode
            "--debug" => {}          // TODO: implement debug mode
            arg => {
                if !arg.starts_with('-') {
                    query_args.push(arg.to_string());
                }
            }
        }
        i += 1;
    }

    if query_args.is_empty() {
        eprintln!("Error: No formula or cask specified");
        return Err("No formula or cask specified".into());
    }

    // If no search mode flags specified, just display descriptions for named formulae/casks
    if !search_mode && !name_mode && !description_mode {
        display_descriptions(&query_args)?;
    } else {
        // Search mode
        search_descriptions(&query_args, search_mode, name_mode, description_mode)?;
    }

    Ok(())
}

fn display_descriptions(names: &[String]) -> Result<(), Box<dyn std::error::Error>> {
    // Load all formulae and casks for lookup
    let formulae = api::load_all_formulae()?;
    let casks = api::load_all_casks()?;

    // Collect descriptions in alphabetical order
    let mut descriptions = BTreeSet::new();

    for name in names {
        // Try to find as formula first
        if let Some(formula) = formulae.iter().find(|f| f.name == *name) {
            let desc = formula
                .desc
                .as_deref()
                .unwrap_or("No description available");
            descriptions.insert(format!("{}: {}", formula.name, desc));
        } else if let Some(cask) = casks.iter().find(|c| c.token == *name) {
            let desc = cask.desc.as_deref().unwrap_or("No description available");
            descriptions.insert(format!("{}: {}", cask.token, desc));
        } else {
            eprintln!(
                "Error: No available formula or cask with the name \"{}\".",
                name
            );
        }
    }

    for desc in descriptions {
        println!("{}", desc);
    }

    Ok(())
}

fn search_descriptions(
    queries: &[String],
    search_mode: bool,
    name_mode: bool,
    description_mode: bool,
) -> Result<(), Box<dyn std::error::Error>> {
    // Load all formulae and casks
    let formulae = api::load_all_formulae()?;
    let casks = api::load_all_casks()?;

    let mut formula_results = BTreeSet::new();
    let mut cask_results = BTreeSet::new();

    for query in queries {
        // Check if query is a regex (flanked by slashes)
        let is_regex = query.starts_with('/') && query.ends_with('/') && query.len() > 2;

        if is_regex {
            let pattern = &query[1..query.len() - 1];
            let re = Regex::new(pattern)?;

            // Search formulae
            for formula in &formulae {
                let name_match = re.is_match(&formula.name);
                let desc_match = formula
                    .desc
                    .as_ref()
                    .map(|d| re.is_match(d))
                    .unwrap_or(false);

                let matches = if search_mode || (!name_mode && !description_mode) {
                    name_match || desc_match
                } else if name_mode {
                    name_match
                } else if description_mode {
                    desc_match
                } else {
                    false
                };

                if matches {
                    let desc = formula
                        .desc
                        .as_deref()
                        .unwrap_or("No description available");
                    formula_results.insert(format!("{}: {}", formula.name, desc));
                }
            }

            // Search casks
            for cask in &casks {
                let name_match = re.is_match(&cask.token);
                let desc_match = cask.desc.as_ref().map(|d| re.is_match(d)).unwrap_or(false);

                let matches = if search_mode || (!name_mode && !description_mode) {
                    name_match || desc_match
                } else if name_mode {
                    name_match
                } else if description_mode {
                    desc_match
                } else {
                    false
                };

                if matches {
                    let desc = cask.desc.as_deref().unwrap_or("No description available");
                    cask_results.insert(format!("{}: {}", cask.token, desc));
                }
            }
        } else {
            // Substring search (case-insensitive)
            let query_lower = query.to_lowercase();

            // Search formulae
            for formula in &formulae {
                let name_match = formula.name.to_lowercase().contains(&query_lower);
                let desc_match = formula
                    .desc
                    .as_ref()
                    .map(|d| d.to_lowercase().contains(&query_lower))
                    .unwrap_or(false);

                let matches = if search_mode || (!name_mode && !description_mode) {
                    name_match || desc_match
                } else if name_mode {
                    name_match
                } else if description_mode {
                    desc_match
                } else {
                    false
                };

                if matches {
                    let desc = formula
                        .desc
                        .as_deref()
                        .unwrap_or("No description available");
                    formula_results.insert(format!("{}: {}", formula.name, desc));
                }
            }

            // Search casks
            for cask in &casks {
                let name_match = cask.token.to_lowercase().contains(&query_lower);
                let desc_match = cask
                    .desc
                    .as_ref()
                    .map(|d| d.to_lowercase().contains(&query_lower))
                    .unwrap_or(false);

                let matches = if search_mode || (!name_mode && !description_mode) {
                    name_match || desc_match
                } else if name_mode {
                    name_match
                } else if description_mode {
                    desc_match
                } else {
                    false
                };

                if matches {
                    let desc = cask.desc.as_deref().unwrap_or("No description available");
                    cask_results.insert(format!("{}: {}", cask.token, desc));
                }
            }
        }
    }

    // Display results
    // Always show headers like brew does (even when empty)
    println!("==> Formulae");
    for result in &formula_results {
        println!("{}", result);
    }

    println!();
    println!("==> Casks");
    for result in &cask_results {
        println!("{}", result);
    }

    Ok(())
}

fn print_help() {
    println!(
        r#"Usage: brew desc [options] formula|cask|text|/regex/ [...]

Display formula's name and one-line description. The cache is created on the
first search, making that search slower than subsequent ones.

  -s, --search                     Search both names and descriptions for
                                   text. If text is flanked by slashes, it
                                   is interpreted as a regular expression.
  -n, --name                       Search just names for text. If text is
                                   flanked by slashes, it is interpreted as a
                                   regular expression.
  -d, --description                Search just descriptions for text. If
                                   text is flanked by slashes, it is
                                   interpreted as a regular expression.
      --eval-all                   Evaluate all available formulae and casks,
                                   whether installed or not, to search their
                                   descriptions. Enabled by default if
                                   $HOMEBREW_EVAL_ALL is set.
      --formula, --formulae        Treat all named arguments as formulae.
      --cask, --casks              Treat all named arguments as casks.
      --debug                      Display any debugging information.
  -q, --quiet                      Make some output more quiet.
  -v, --verbose                    Make some output more verbose.
  -h, --help                       Show this message."#
    );
}
