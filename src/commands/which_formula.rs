use crate::paths;
use std::collections::HashMap;
use std::fs;

pub fn execute(args: &[String]) {
    let mut explain = false;
    let mut _skip_update = false;
    let mut commands = Vec::new();

    for arg in args {
        match arg.as_str() {
            "--explain" => explain = true,
            "--skip-update" => _skip_update = true,
            _ => commands.push(arg.clone()),
        }
    }

    if commands.is_empty() {
        eprintln!("Usage: brew which-formula [--explain] [--skip-update] command [...]");
        std::process::exit(1);
    }

    // Load the executables database
    let db = match load_executables_database() {
        Ok(db) => db,
        Err(e) => {
            eprintln!("Error: Failed to load executables database: {}", e);
            std::process::exit(1);
        }
    };

    for command in commands {
        if let Some(formula) = db.get(&command) {
            if explain {
                println!(
                    "The program '{}' is currently not installed. You can install it by typing:",
                    command
                );
                println!("  brew install {}", formula);
            } else {
                println!("{}", formula);
            }
        }
        // Silently skip commands not in database (matches brew behavior)
    }
}

fn load_executables_database() -> Result<HashMap<String, String>, String> {
    let cache_dir = paths::homebrew_cache();
    let db_path = cache_dir.join("api/internal/executables.txt");

    if !db_path.exists() {
        return Err(format!("Executables database not found at {:?}", db_path));
    }

    let content = fs::read_to_string(&db_path)
        .map_err(|e| format!("Failed to read executables database: {}", e))?;

    let mut db = HashMap::new();

    for line in content.lines() {
        if line.is_empty() {
            continue;
        }

        // Format: formula(version):executable1 executable2 executable3
        if let Some((formula_part, executables_part)) = line.split_once(':') {
            // Extract formula name (remove version in parentheses)
            let formula = if let Some((name, _version)) = formula_part.split_once('(') {
                name
            } else {
                formula_part
            };

            // Parse executables
            for executable in executables_part.split_whitespace() {
                db.insert(executable.to_string(), formula.to_string());
            }
        }
    }

    Ok(db)
}
