//! Options command - show install options for formulae.
//!
//! Modern Homebrew has deprecated build options, so most formulae return empty results.
//! This command matches brew's behavior for backwards compatibility.

use crate::api;
use crate::paths;
use std::io::{self, Write};

pub fn run(args: &[String]) -> Result<(), String> {
    let mut compact = false;
    let mut installed = false;
    let mut eval_all = false;
    let mut formulae = Vec::new();

    let mut i = 0;
    while i < args.len() {
        match args[i].as_str() {
            "--help" | "-h" => {
                print_help();
                return Ok(());
            }
            "--compact" => compact = true,
            "--installed" => installed = true,
            "--eval-all" => eval_all = true,
            arg if arg.starts_with('-') => {
                eprintln!("brew options: unknown option: {}", arg);
                return Err(format!("unknown option: {}", arg));
            }
            _ => formulae.push(args[i].clone()),
        }
        i += 1;
    }

    // Check for conflicting flags
    if (installed as i32) + (eval_all as i32) > 1 {
        return Err("--installed and --eval-all are mutually exclusive".to_string());
    }

    if eval_all {
        // Load all formulae and show options
        show_options_for_all(compact)?;
    } else if installed {
        // Show options for installed formulae
        show_options_for_installed(compact)?;
    } else if formulae.is_empty() {
        return Err(
            "`brew options` needs a formula or `--eval-all` passed or `HOMEBREW_EVAL_ALL=1` set!"
                .to_string(),
        );
    } else {
        // Show options for specific formulae
        show_options_for_formulae(&formulae, compact)?;
    }

    Ok(())
}

fn show_options_for_all(compact: bool) -> Result<(), String> {
    let cache_path = paths::homebrew_cache();
    let api_path = cache_path.join("api/formula.jws.json");

    if !api_path.exists() {
        return Err("API cache not found. Run `brew update` first.".to_string());
    }

    let content = std::fs::read_to_string(&api_path)
        .map_err(|e| format!("Failed to read API cache: {}", e))?;

    // Parse the JSON (skip JWS verification)
    let json_start = content.find('{').unwrap_or(0);
    let json_str = &content[json_start..];

    let formulae: Vec<api::FormulaInfo> =
        serde_json::from_str(json_str).map_err(|e| format!("Failed to parse API cache: {}", e))?;

    // Filter to only formulae with options
    let with_options: Vec<_> = formulae.iter().filter(|f| !f.options.is_empty()).collect();

    if with_options.is_empty() {
        // No output when no formulae have options (matching brew behavior)
        return Ok(());
    }

    for formula in &with_options {
        show_formula_options(formula, compact, with_options.len() > 1)?;
    }

    Ok(())
}

fn show_options_for_installed(compact: bool) -> Result<(), String> {
    let opt_path = paths::homebrew_prefix().join("opt");

    if !opt_path.exists() {
        // No installed formulae
        return Ok(());
    }

    let entries =
        std::fs::read_dir(&opt_path).map_err(|e| format!("Failed to read opt directory: {}", e))?;

    let mut installed_formulae = Vec::new();
    for entry in entries.flatten() {
        if let Ok(metadata) = entry.metadata()
            && metadata.is_symlink()
            && let Some(name) = entry.file_name().to_str()
        {
            installed_formulae.push(name.to_string());
        }
    }

    installed_formulae.sort();

    // Load options for each installed formula
    let mut found_any = false;
    for name in installed_formulae {
        if let Ok(formula) = api::get_formula(&name)
            && !formula.options.is_empty()
        {
            show_formula_options(&formula, compact, true)?;
            found_any = true;
        }
    }

    if !found_any {
        // No output when no installed formulae have options
    }

    Ok(())
}

fn show_options_for_formulae(formulae: &[String], compact: bool) -> Result<(), String> {
    let multiple = formulae.len() > 1;

    for name in formulae {
        let formula = api::get_formula(name)
            .map_err(|_| format!("No available formula with the name \"{}\"", name))?;

        if formula.options.is_empty() {
            // No output for formulae without options (matching brew behavior)
            continue;
        }

        show_formula_options(&formula, compact, multiple)?;
    }

    Ok(())
}

fn show_formula_options(
    formula: &api::FormulaInfo,
    compact: bool,
    show_name: bool,
) -> Result<(), String> {
    let stdout = io::stdout();
    let mut handle = stdout.lock();

    if compact {
        // Show all options on one line
        let options_line = formula.options.join(" ");
        writeln!(handle, "{}", options_line).map_err(|e| e.to_string())?;
    } else {
        // Show formula name if multiple formulae
        if show_name {
            writeln!(handle, "{}", formula.full_name).map_err(|e| e.to_string())?;
        }

        // Show each option on its own line
        for option in &formula.options {
            writeln!(handle, "{}", option).map_err(|e| e.to_string())?;
        }
        writeln!(handle).map_err(|e| e.to_string())?;
    }

    Ok(())
}

fn print_help() {
    print!(
        r#"Usage: brew options [options] [formula ...]

Show install options specific to formula.

      --compact                    Show all options on a single line separated
                                   by spaces.
      --installed                  Show options for formulae that are currently
                                   installed.
      --eval-all                   Evaluate all available formulae and casks,
                                   whether installed or not, to show their
                                   options. Enabled by default if
                                   $HOMEBREW_EVAL_ALL is set.
  -d, --debug                      Display any debugging information.
  -q, --quiet                      Make some output more quiet.
  -v, --verbose                    Make some output more verbose.
  -h, --help                       Show this message.
"#
    );
}
