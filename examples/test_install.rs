use homebrew_rust::api;
use homebrew_rust::install;

#[tokio::main]
async fn main() {
    // Test with libxau which has a cached bottle
    let formula_name = "libxau";

    println!("Loading formula info for {}...", formula_name);
    let formula = match api::get_formula(formula_name) {
        Ok(f) => f,
        Err(e) => {
            eprintln!("Failed to load formula: {}", e);
            return;
        }
    };

    println!("Formula: {}", formula.name);
    println!("Version: {:?}", formula.versions.stable);
    println!("Description: {:?}", formula.desc);

    println!("\nInstalling {}...", formula_name);
    match install::install_bottle(&formula, false).await {
        Ok(keg) => {
            println!("\n✓ Successfully installed {} {}", keg.name, keg.version);
            println!("  Installed to: {}", keg.path.display());
            println!("  Opt link: {}", keg.opt_path().display());
        }
        Err(e) => {
            eprintln!("\n✗ Installation failed: {}", e);
        }
    }
}
