//! System detection for platform, architecture, and macOS version.

use std::process::Command;

/// macOS version codenames mapped to major version numbers.
const MACOS_CODENAMES: &[(&str, u32)] = &[
    ("tahoe", 26),
    ("sequoia", 15),
    ("sonoma", 14),
    ("ventura", 13),
    ("monterey", 12),
    ("big_sur", 11),
    ("catalina", 10), // Actually 10.15
];

/// Get the current macOS version as a string (e.g., "26.2").
#[cfg(target_os = "macos")]
pub fn macos_version() -> Option<String> {
    let output = Command::new("sw_vers")
        .arg("-productVersion")
        .output()
        .ok()?;

    if output.status.success() {
        Some(String::from_utf8_lossy(&output.stdout).trim().to_string())
    } else {
        None
    }
}

#[cfg(not(target_os = "macos"))]
pub fn macos_version() -> Option<String> {
    None
}

/// Get the major macOS version number (e.g., 26 for Tahoe).
pub fn macos_major_version() -> Option<u32> {
    let version = macos_version()?;
    let major = version.split('.').next()?;
    major.parse().ok()
}

/// Get the macOS codename for the current version (e.g., "tahoe").
pub fn macos_codename() -> Option<&'static str> {
    let major = macos_major_version()?;

    // Special case: macOS 10.x versions
    if major == 10 {
        return Some("catalina");
    }

    for (name, ver) in MACOS_CODENAMES {
        if *ver == major {
            return Some(name);
        }
    }

    None
}

/// Get the current architecture (arm64 or x86_64).
pub fn arch() -> &'static str {
    #[cfg(target_arch = "aarch64")]
    {
        "arm64"
    }
    #[cfg(target_arch = "x86_64")]
    {
        "x86_64"
    }
    #[cfg(not(any(target_arch = "aarch64", target_arch = "x86_64")))]
    {
        "unknown"
    }
}

/// Get the bottle tag for the current system (e.g., "arm64_tahoe" or "sonoma").
pub fn bottle_tag() -> Option<String> {
    let codename = macos_codename()?;

    #[cfg(target_os = "macos")]
    {
        let arch = arch();
        if arch == "arm64" {
            Some(format!("arm64_{codename}"))
        } else {
            // x86_64 macOS bottles don't have arch prefix
            Some(codename.to_string())
        }
    }

    #[cfg(target_os = "linux")]
    {
        let arch = arch();
        if arch == "arm64" {
            Some("arm64_linux".to_string())
        } else {
            Some("x86_64_linux".to_string())
        }
    }

    #[cfg(not(any(target_os = "macos", target_os = "linux")))]
    {
        None
    }
}

/// Check if the current system is macOS.
#[allow(dead_code)]
pub fn is_macos() -> bool {
    cfg!(target_os = "macos")
}

/// Check if the current system is Linux.
#[allow(dead_code)]
pub fn is_linux() -> bool {
    cfg!(target_os = "linux")
}

/// Get the CPU family string (e.g., "arm64_sonoma" for ARM Mac).
#[allow(dead_code)]
pub fn cpu_family() -> String {
    let arch = arch();
    if arch == "arm64" {
        if let Some(codename) = macos_codename() {
            format!("arm64_{}", codename)
        } else {
            "arm64".to_string()
        }
    } else {
        arch.to_string()
    }
}
