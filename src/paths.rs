use std::env;
use std::path::PathBuf;

/// Detect the Homebrew prefix based on platform and architecture.
/// - macOS ARM (Apple Silicon): /opt/homebrew
/// - macOS Intel: /usr/local
/// - Linux: /home/linuxbrew/.linuxbrew
pub fn homebrew_prefix() -> PathBuf {
    // Check environment variable override first
    if let Ok(prefix) = env::var("HOMEBREW_PREFIX") {
        return PathBuf::from(prefix);
    }

    #[cfg(target_os = "macos")]
    {
        #[cfg(target_arch = "aarch64")]
        {
            PathBuf::from("/opt/homebrew")
        }
        #[cfg(target_arch = "x86_64")]
        {
            PathBuf::from("/usr/local")
        }
        #[cfg(not(any(target_arch = "aarch64", target_arch = "x86_64")))]
        {
            PathBuf::from("/usr/local")
        }
    }

    #[cfg(target_os = "linux")]
    {
        PathBuf::from("/home/linuxbrew/.linuxbrew")
    }

    #[cfg(not(any(target_os = "macos", target_os = "linux")))]
    {
        PathBuf::from("/usr/local")
    }
}

/// Get the Homebrew repository path.
/// This is where Homebrew itself is installed.
pub fn homebrew_repository() -> PathBuf {
    if let Ok(repo) = env::var("HOMEBREW_REPOSITORY") {
        return PathBuf::from(repo);
    }

    // On macOS ARM, the repository is at the prefix
    // On macOS Intel/Linux, the repository might be in a subdirectory
    let prefix = homebrew_prefix();

    #[cfg(target_os = "macos")]
    {
        prefix
    }

    #[cfg(target_os = "linux")]
    {
        prefix.join("Homebrew")
    }

    #[cfg(not(any(target_os = "macos", target_os = "linux")))]
    {
        prefix.join("Homebrew")
    }
}

/// Get the Cellar path where formulas are installed.
pub fn homebrew_cellar() -> PathBuf {
    if let Ok(cellar) = env::var("HOMEBREW_CELLAR") {
        return PathBuf::from(cellar);
    }

    // Check if there's a Cellar in the repository first
    let repo = homebrew_repository();
    let repo_cellar = repo.join("Cellar");
    if repo_cellar.is_dir() {
        return repo_cellar;
    }

    // Otherwise use the prefix
    homebrew_prefix().join("Cellar")
}

/// Get the Caskroom path where casks are installed.
pub fn homebrew_caskroom() -> PathBuf {
    if let Ok(caskroom) = env::var("HOMEBREW_CASKROOM") {
        return PathBuf::from(caskroom);
    }

    homebrew_prefix().join("Caskroom")
}

/// Get the cache path for downloads.
pub fn homebrew_cache() -> PathBuf {
    if let Ok(cache) = env::var("HOMEBREW_CACHE") {
        return PathBuf::from(cache);
    }

    #[cfg(target_os = "macos")]
    {
        if let Ok(home) = env::var("HOME") {
            return PathBuf::from(home).join("Library/Caches/Homebrew");
        }
    }

    #[cfg(target_os = "linux")]
    {
        if let Ok(xdg_cache) = env::var("XDG_CACHE_HOME") {
            return PathBuf::from(xdg_cache).join("Homebrew");
        }
        if let Ok(home) = env::var("HOME") {
            return PathBuf::from(home).join(".cache/Homebrew");
        }
    }

    // Fallback
    PathBuf::from("/tmp/homebrew-cache")
}

/// Get the logs path.
#[allow(dead_code)]
pub fn homebrew_logs() -> PathBuf {
    if let Ok(logs) = env::var("HOMEBREW_LOGS") {
        return PathBuf::from(logs);
    }

    #[cfg(target_os = "macos")]
    {
        if let Ok(home) = env::var("HOME") {
            return PathBuf::from(home).join("Library/Logs/Homebrew");
        }
    }

    #[cfg(target_os = "linux")]
    {
        // On Linux, logs go in the cache directory
        return homebrew_cache().join("Logs");
    }

    // Fallback
    PathBuf::from("/tmp/homebrew-logs")
}

/// Get the taps directory.
pub fn homebrew_taps() -> PathBuf {
    homebrew_library().join("Taps")
}

/// Get the Library directory.
pub fn homebrew_library() -> PathBuf {
    if let Ok(library) = env::var("HOMEBREW_LIBRARY") {
        return PathBuf::from(library);
    }

    homebrew_repository().join("Library")
}
