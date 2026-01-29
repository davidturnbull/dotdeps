//! Output formatting for JSON and text modes
//!
//! Provides types for structured output that can be serialized to JSON
//! for machine-readable output, or displayed as text for human consumption.

use crate::cli::Ecosystem;
use serde::Serialize;

/// Result of an add operation
#[derive(Debug, Serialize)]
pub struct AddResult {
    pub ecosystem: String,
    pub package: String,
    pub version: String,
    pub path: String,
    pub cached: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cloned_ref: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub warning: Option<String>,
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    pub dry_run: bool,
}

/// Result of a remove operation
#[derive(Debug, Serialize)]
pub struct RemoveResult {
    pub ecosystem: String,
    pub package: String,
    pub removed: bool,
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    pub dry_run: bool,
}

/// Result of a list operation
#[derive(Debug, Serialize)]
pub struct ListResult {
    pub dependencies: Vec<ListEntry>,
}

/// A single entry in the list output
#[derive(Debug, Serialize)]
pub struct ListEntry {
    pub ecosystem: String,
    pub package: String,
    pub version: String,
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    pub broken: bool,
}

/// Result of a clean operation
#[derive(Debug, Serialize)]
pub struct CleanResult {
    pub cleaned: bool,
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    pub dry_run: bool,
}

/// Result of a context operation
#[derive(Debug, Serialize)]
pub struct ContextResult {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub context: Option<String>,
}

/// Result of skipping a local dependency
#[derive(Debug, Serialize)]
pub struct SkipResult {
    pub ecosystem: String,
    pub package: String,
    pub skipped: bool,
    pub reason: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub path: Option<String>,
}

impl AddResult {
    pub fn new(
        ecosystem: Ecosystem,
        package: &str,
        version: &str,
        path: &str,
        cached: bool,
    ) -> Self {
        Self {
            ecosystem: ecosystem.to_string(),
            package: package.to_string(),
            version: version.to_string(),
            path: path.to_string(),
            cached,
            cloned_ref: None,
            warning: None,
            dry_run: false,
        }
    }

    pub fn with_cloned_ref(mut self, cloned_ref: &str) -> Self {
        self.cloned_ref = Some(cloned_ref.to_string());
        self
    }

    pub fn with_warning(mut self, warning: &str) -> Self {
        self.warning = Some(warning.to_string());
        self
    }

    pub fn with_dry_run(mut self) -> Self {
        self.dry_run = true;
        self
    }
}

impl RemoveResult {
    pub fn new(ecosystem: Ecosystem, package: &str, removed: bool) -> Self {
        Self {
            ecosystem: ecosystem.to_string(),
            package: package.to_string(),
            removed,
            dry_run: false,
        }
    }

    pub fn with_dry_run(mut self) -> Self {
        self.dry_run = true;
        self
    }
}

impl ListEntry {
    pub fn new(ecosystem: Ecosystem, package: &str, version: &str, broken: bool) -> Self {
        Self {
            ecosystem: ecosystem.to_string(),
            package: package.to_string(),
            version: version.to_string(),
            broken,
        }
    }
}

impl SkipResult {
    pub fn local_path(ecosystem: Ecosystem, package: &str, path: &str) -> Self {
        Self {
            ecosystem: ecosystem.to_string(),
            package: package.to_string(),
            skipped: true,
            reason: "local_path".to_string(),
            path: Some(path.to_string()),
        }
    }
}

/// A single action taken during init
#[derive(Debug, Serialize)]
pub struct InitAction {
    pub action: String,
    pub status: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub file: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub message: Option<String>,
}

/// Result of an init operation
#[derive(Debug, Serialize)]
pub struct InitOutput {
    pub initialized: bool,
    pub actions: Vec<InitAction>,
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    pub dry_run: bool,
}

/// Result of an update check operation
#[derive(Debug, Serialize)]
pub struct UpdateCheckOutput {
    pub current_version: String,
    pub latest_version: String,
    pub update_available: bool,
}

/// Result of an update operation
#[derive(Debug, Serialize)]
pub struct UpdateOutput {
    pub updated: bool,
    pub current_version: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub new_version: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub message: Option<String>,
}

impl UpdateCheckOutput {
    pub fn new(current: &str, latest: &str, available: bool) -> Self {
        Self {
            current_version: current.to_string(),
            latest_version: latest.to_string(),
            update_available: available,
        }
    }
}

impl UpdateOutput {
    pub fn up_to_date(version: &str) -> Self {
        Self {
            updated: false,
            current_version: version.to_string(),
            new_version: None,
            message: Some("Already up to date".to_string()),
        }
    }

    pub fn updated(old_version: &str, new_version: &str) -> Self {
        Self {
            updated: true,
            current_version: old_version.to_string(),
            new_version: Some(new_version.to_string()),
            message: None,
        }
    }
}

impl InitAction {
    pub fn new(action: &str, status: &str) -> Self {
        Self {
            action: action.to_string(),
            status: status.to_string(),
            file: None,
            message: None,
        }
    }

    pub fn with_file(mut self, file: &str) -> Self {
        self.file = Some(file.to_string());
        self
    }

    pub fn with_message(mut self, message: &str) -> Self {
        self.message = Some(message.to_string());
        self
    }
}

/// Print JSON output to stdout
pub fn print_json<T: Serialize>(value: &T) {
    match serde_json::to_string_pretty(value) {
        Ok(json) => println!("{}", json),
        Err(e) => {
            eprintln!("Error serializing JSON: {}", e);
            std::process::exit(1);
        }
    }
}
