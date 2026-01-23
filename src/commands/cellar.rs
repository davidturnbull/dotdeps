use crate::commands::{Command, CommandResult};
use crate::formula;
use crate::paths;

pub struct Cellar;

impl Command for Cellar {
    fn run(&self, args: &[String]) -> CommandResult {
        // Filter out flags (--debug, --quiet, --verbose, --help)
        let formula_args: Vec<&String> = args.iter().filter(|a| !a.starts_with('-')).collect();

        // No formula arguments - just output the cellar
        if formula_args.is_empty() {
            println!("{}", paths::homebrew_cellar().display());
            return Ok(());
        }

        // Handle formula arguments
        let cellar = paths::homebrew_cellar();

        for formula_name in &formula_args {
            // Normalize the formula name (strip tap prefix if present)
            let normalized = formula::normalize_name(formula_name);

            // Validate formula exists
            if !formula::exists(formula_name) {
                return Err(
                    format!("No available formula with the name \"{}\".", normalized).into(),
                );
            }

            // Output the cellar path for this formula
            println!("{}", cellar.join(normalized).display());
        }

        Ok(())
    }
}
