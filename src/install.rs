use crate::api::FormulaInfo;
use crate::download::{download_and_verify, extract_tar_gz};
use crate::paths;
use crate::system;
use std::fs;
use std::os::unix;
use std::path::PathBuf;

/// Represents an installed formula (a "keg" in Homebrew terminology)
pub struct Keg {
    pub name: String,
    pub version: String,
    pub path: PathBuf,
}

impl Keg {
    /// Create a new Keg reference for a formula
    pub fn new(name: String, version: String) -> Self {
        let path = paths::homebrew_cellar().join(&name).join(&version);
        Keg {
            name,
            version,
            path,
        }
    }

    /// Get the path to the opt symlink for this formula
    pub fn opt_path(&self) -> PathBuf {
        paths::homebrew_prefix().join("opt").join(&self.name)
    }

    /// Check if this keg exists in the Cellar
    pub fn exists(&self) -> bool {
        self.path.exists()
    }

    /// Create the opt symlink pointing to this keg
    pub fn link_opt(&self) -> Result<(), String> {
        let opt_path = self.opt_path();

        // Create opt directory if it doesn't exist
        if let Some(parent) = opt_path.parent() {
            fs::create_dir_all(parent)
                .map_err(|e| format!("Failed to create opt directory: {}", e))?;
        }

        // Remove existing symlink if present
        if opt_path.exists() || opt_path.symlink_metadata().is_ok() {
            fs::remove_file(&opt_path)
                .map_err(|e| format!("Failed to remove existing opt symlink: {}", e))?;
        }

        // Create new symlink
        unix::fs::symlink(&self.path, &opt_path)
            .map_err(|e| format!("Failed to create opt symlink: {}", e))?;

        Ok(())
    }

    /// Write installation metadata (INSTALL_RECEIPT.json)
    pub fn write_tab(
        &self,
        formula: &FormulaInfo,
        installed_as_dependency: bool,
    ) -> Result<(), String> {
        let tab_path = self.path.join("INSTALL_RECEIPT.json");
        let tab = Tab::new(formula, installed_as_dependency);

        let json = serde_json::to_string_pretty(&tab)
            .map_err(|e| format!("Failed to serialize tab: {}", e))?;

        fs::write(&tab_path, json).map_err(|e| format!("Failed to write tab file: {}", e))?;

        Ok(())
    }
}

/// Installation metadata (Homebrew calls this a "Tab")
#[derive(serde::Serialize)]
pub struct Tab {
    pub homebrew_version: String,
    pub used_options: Vec<String>,
    pub unused_options: Vec<String>,
    pub built_as_bottle: bool,
    pub poured_from_bottle: bool,
    pub installed_as_dependency: bool,
    pub installed_on_request: bool,
    pub time: i64,
    pub source_modified_time: i64,
    pub compiler: String,
    pub stdlib: Option<String>,
    pub runtime_dependencies: Option<Vec<RuntimeDependency>>,
    pub arch: String,
    pub built_on: BuiltOn,
}

#[derive(serde::Serialize)]
pub struct RuntimeDependency {
    pub full_name: String,
    pub version: String,
    pub revision: i64,
    pub pkg_version: String,
    pub declared_directly: bool,
}

#[derive(serde::Serialize)]
pub struct BuiltOn {
    pub os: String,
    pub os_version: String,
    pub cpu_family: String,
    pub xcode: Option<String>,
    pub clt: Option<String>,
}

impl Tab {
    pub fn new(_formula: &FormulaInfo, installed_as_dependency: bool) -> Self {
        Tab {
            homebrew_version: get_homebrew_version(),
            used_options: vec![],
            unused_options: vec![],
            built_as_bottle: true,
            poured_from_bottle: true,
            installed_as_dependency,
            installed_on_request: !installed_as_dependency,
            time: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_secs() as i64,
            source_modified_time: 0,
            compiler: "clang".to_string(),
            stdlib: None,
            runtime_dependencies: None,
            arch: system::arch().to_string(),
            built_on: BuiltOn {
                os: "Macintosh".to_string(),
                os_version: system::macos_version().unwrap_or_else(|| "unknown".to_string()),
                cpu_family: system::cpu_family(),
                xcode: None,
                clt: None,
            },
        }
    }
}

fn get_homebrew_version() -> String {
    // Try to get Homebrew version from git
    let output = std::process::Command::new("git")
        .arg("describe")
        .arg("--tags")
        .current_dir(paths::homebrew_repository())
        .output();

    if let Ok(output) = output
        && output.status.success()
        && let Ok(version) = String::from_utf8(output.stdout)
    {
        return version.trim().to_string();
    }

    "unknown".to_string()
}

/// Install a formula from a bottle
pub async fn install_bottle(
    formula: &FormulaInfo,
    installed_as_dependency: bool,
) -> Result<Keg, String> {
    let version = formula
        .versions
        .stable
        .as_ref()
        .ok_or("Formula has no stable version")?;

    // Get bottle information for current platform
    let bottle_tag =
        system::bottle_tag().ok_or("Unable to determine bottle tag for current system")?;
    let bottle = formula
        .bottle
        .as_ref()
        .and_then(|b| b.stable.as_ref())
        .and_then(|s| s.files.get(&bottle_tag))
        .ok_or_else(|| format!("No bottle available for {}", bottle_tag))?;

    let url = &bottle.url;
    let sha256 = &bottle.sha256;

    // Calculate cache path for bottle
    let cache_dir = paths::homebrew_cache().join("downloads");
    fs::create_dir_all(&cache_dir)
        .map_err(|e| format!("Failed to create cache directory: {}", e))?;

    let filename = url.split('/').next_back().unwrap_or("bottle.tar.gz");
    let cache_path = cache_dir.join(filename);

    // Download bottle if not cached
    if !cache_path.exists() {
        println!("==> Downloading {}", url);
        let client = reqwest::Client::new();
        download_and_verify(&client, url, &cache_path, sha256).await?;
    } else {
        println!("==> Using cached bottle");
    }

    // Create keg
    let keg = Keg::new(formula.name.clone(), version.clone());

    // Extract bottle to Cellar
    println!("==> Pouring {}", filename);

    // Create formula rack directory (parent of version directory)
    let rack_dir = paths::homebrew_cellar().join(&formula.name);
    fs::create_dir_all(&rack_dir).map_err(|e| format!("Failed to create rack directory: {}", e))?;

    // Extract to Cellar
    extract_tar_gz(&cache_path, &paths::homebrew_cellar())?;

    // Verify extraction
    if !keg.exists() {
        return Err(format!(
            "Bottle extraction failed: {} does not exist",
            keg.path.display()
        ));
    }

    // Write installation metadata
    keg.write_tab(formula, installed_as_dependency)?;

    // Create opt symlink
    keg.link_opt()?;

    Ok(keg)
}
