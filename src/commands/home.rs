use crate::api;
use std::process::Command;

pub fn run(args: &[String]) -> Result<(), i32> {
    let mut formula_mode = false;
    let mut cask_mode = false;
    let mut names = Vec::new();

    for arg in args {
        match arg.as_str() {
            "--formula" | "--formulae" => formula_mode = true,
            "--cask" | "--casks" => cask_mode = true,
            "-d" | "--debug" => { /* ignored for now */ }
            "-q" | "--quiet" => { /* ignored for now */ }
            "-v" | "--verbose" => { /* ignored for now */ }
            "-h" | "--help" => {
                print_help();
                return Ok(());
            }
            _ => names.push(arg.clone()),
        }
    }

    if names.is_empty() {
        // Open Homebrew's homepage
        open_url("https://brew.sh")?;
    } else {
        // Open each formula/cask's homepage
        for name in names {
            open_formula_homepage(&name, formula_mode, cask_mode)?;
        }
    }

    Ok(())
}

fn open_formula_homepage(name: &str, formula_mode: bool, cask_mode: bool) -> Result<(), i32> {
    // Try to get the homepage URL
    let url = if cask_mode {
        get_cask_homepage(name)
    } else if formula_mode {
        get_formula_homepage(name)
    } else {
        // Try formula first, then cask
        get_formula_homepage(name).or_else(|_| get_cask_homepage(name))
    };

    match url {
        Ok(url) => open_url(&url),
        Err(_) => {
            eprintln!(
                "Error: No available formula or cask with the name \"{}\".",
                name
            );
            Err(1)
        }
    }
}

fn get_formula_homepage(name: &str) -> Result<String, String> {
    let formula = api::get_formula(name)?;
    formula
        .homepage
        .ok_or_else(|| format!("Formula {} has no homepage", name))
}

fn get_cask_homepage(name: &str) -> Result<String, String> {
    let cask = api::get_cask(name)?;
    cask.homepage
        .ok_or_else(|| format!("Cask {} has no homepage", name))
}

fn open_url(url: &str) -> Result<(), i32> {
    #[cfg(target_os = "macos")]
    let command = "open";
    #[cfg(target_os = "linux")]
    let command = "xdg-open";
    #[cfg(target_os = "windows")]
    let command = "start";

    let status = Command::new(command).arg(url).status().map_err(|e| {
        eprintln!("Error: Failed to open URL: {}", e);
        1
    })?;

    if !status.success() {
        eprintln!("Error: Failed to open URL in browser");
        return Err(1);
    }

    Ok(())
}

fn print_help() {
    println!("Usage: brew home, homepage [--formula] [--cask] [formula|cask ...]");
    println!();
    println!("Open a formula or cask's homepage in a browser, or open Homebrew's own");
    println!("homepage if no argument is provided.");
    println!();
    println!("      --formula, --formulae        Treat all named arguments as formulae.");
    println!("      --cask, --casks              Treat all named arguments as casks.");
    println!("  -d, --debug                      Display any debugging information.");
    println!("  -q, --quiet                      Make some output more quiet.");
    println!("  -v, --verbose                    Make some output more verbose.");
    println!("  -h, --help                       Show this message.");
}
