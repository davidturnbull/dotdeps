use homebrew_rust::api;
use homebrew_rust::download::extract_tar_gz;
use homebrew_rust::install::{Keg, Tab};
use std::fs;
use std::path::PathBuf;

fn main() {
    // Test with a cached bottle for bat
    let formula_name = "bat";
    let cached_bottle = PathBuf::from(
        "/Users/david/Library/Caches/Homebrew/downloads/a3e00bcd5538fcff81ae95ca3937b485036bc37c2d1f57606673937b704d22f4--bat--0.26.1.arm64_tahoe.bottle.tar.gz",
    );

    if !cached_bottle.exists() {
        eprintln!("Cached bottle not found: {}", cached_bottle.display());
        eprintln!("This test requires a pre-downloaded bottle file.");
        return;
    }

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

    // Manual installation test
    println!("\nTesting bottle extraction and installation...");

    // Create temporary test cellar
    let test_cellar = PathBuf::from("/tmp/homebrew-rust-test/Cellar");
    fs::create_dir_all(&test_cellar).unwrap();

    println!("==> Extracting bottle to {}", test_cellar.display());
    if let Err(e) = extract_tar_gz(&cached_bottle, &test_cellar) {
        eprintln!("✗ Extraction failed: {}", e);
        return;
    }

    println!("✓ Extraction successful!");

    // List extracted files
    println!("\nExtracted structure:");
    if let Ok(entries) = fs::read_dir(&test_cellar) {
        for entry in entries.flatten() {
            println!("  {}", entry.path().display());
        }
    }

    // Try to create keg and tab
    let version = "0.26.1";
    let keg = Keg::new(formula.name.clone(), version.to_string());

    println!("\nKeg info:");
    println!("  Path: {}", keg.path.display());
    println!("  Opt path: {}", keg.opt_path().display());
    println!("  Exists: {}", keg.exists());

    // Test Tab creation
    println!("\nTesting Tab creation...");
    let tab = Tab::new(&formula, false);
    println!("  Homebrew version: {}", tab.homebrew_version);
    println!("  Arch: {}", tab.arch);
    println!("  Installed on request: {}", tab.installed_on_request);

    if let Ok(json) = serde_json::to_string_pretty(&tab) {
        println!("\nTab JSON:");
        println!("{}", json);
    }

    // Clean up
    println!("\nCleaning up test directory...");
    let _ = fs::remove_dir_all("/tmp/homebrew-rust-test");
    println!("Done!");
}
