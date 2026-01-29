//! File locking for cache operations
//!
//! Provides cross-platform advisory file locking to prevent concurrent
//! processes from interfering during cache population.

use fs2::FileExt;
use std::fs::{File, OpenOptions};
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};
use thiserror::Error;

/// Lock acquisition timeout (5 minutes)
const LOCK_TIMEOUT: Duration = Duration::from_secs(300);

/// Polling interval when waiting for lock (500ms)
const POLL_INTERVAL: Duration = Duration::from_millis(500);

#[derive(Error, Debug)]
pub enum LockError {
    #[error("Failed to create lock file {path}: {source}")]
    CreateFailed {
        path: PathBuf,
        source: std::io::Error,
    },

    #[error("Timeout acquiring lock on {path} after {timeout_secs} seconds")]
    Timeout { path: PathBuf, timeout_secs: u64 },

    #[error("Failed to acquire lock on {path}: {source}")]
    LockFailed {
        path: PathBuf,
        source: std::io::Error,
    },
}

/// An exclusive lock on a cache entry
///
/// The lock is automatically released when this struct is dropped.
/// The lock file is also removed on drop.
pub struct CacheLock {
    _file: File,
    path: PathBuf,
}

impl CacheLock {
    /// Acquire an exclusive lock, blocking until available or timeout
    ///
    /// Uses a 5 minute timeout with 500ms polling interval.
    /// Returns error if the lock cannot be acquired within the timeout.
    pub fn acquire(lock_path: &Path) -> Result<Self, LockError> {
        let start = Instant::now();

        // Ensure parent directory exists
        if let Some(parent) = lock_path.parent() {
            std::fs::create_dir_all(parent).map_err(|source| LockError::CreateFailed {
                path: lock_path.to_path_buf(),
                source,
            })?;
        }

        loop {
            // Try to acquire the lock
            match Self::try_acquire(lock_path)? {
                Some(lock) => return Ok(lock),
                None => {
                    // Check timeout
                    if start.elapsed() >= LOCK_TIMEOUT {
                        return Err(LockError::Timeout {
                            path: lock_path.to_path_buf(),
                            timeout_secs: LOCK_TIMEOUT.as_secs(),
                        });
                    }
                    // Wait before retrying
                    std::thread::sleep(POLL_INTERVAL);
                }
            }
        }
    }

    /// Try to acquire an exclusive lock without blocking
    ///
    /// Returns `Ok(Some(lock))` if acquired, `Ok(None)` if would block,
    /// or `Err` on failure.
    pub fn try_acquire(lock_path: &Path) -> Result<Option<Self>, LockError> {
        // Ensure parent directory exists
        if let Some(parent) = lock_path.parent() {
            std::fs::create_dir_all(parent).map_err(|source| LockError::CreateFailed {
                path: lock_path.to_path_buf(),
                source,
            })?;
        }

        // Open or create the lock file
        let file = OpenOptions::new()
            .write(true)
            .create(true)
            .truncate(true)
            .open(lock_path)
            .map_err(|source| LockError::CreateFailed {
                path: lock_path.to_path_buf(),
                source,
            })?;

        // Try to acquire exclusive lock (non-blocking)
        match file.try_lock_exclusive() {
            Ok(()) => Ok(Some(CacheLock {
                _file: file,
                path: lock_path.to_path_buf(),
            })),
            Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => Ok(None),
            // On some Unix platforms, EAGAIN (11) is returned instead of WouldBlock
            Err(e) if e.raw_os_error() == Some(11) => Ok(None),
            // On some Unix platforms, EACCES (13) can also indicate lock contention
            Err(e) if e.raw_os_error() == Some(13) => Ok(None),
            Err(source) => Err(LockError::LockFailed {
                path: lock_path.to_path_buf(),
                source,
            }),
        }
    }
}

impl Drop for CacheLock {
    fn drop(&mut self) {
        // Unlock is implicit when the file is closed, but we try to remove
        // the lock file for cleanliness. Ignore errors since another process
        // may have already removed it or acquired a new lock.
        let _ = std::fs::remove_file(&self.path);
    }
}

/// Get the lock file path for a cache entry path
///
/// Returns `<cache_path>.lock`
pub fn lock_path_for(cache_path: &Path) -> PathBuf {
    let mut lock_path = cache_path.as_os_str().to_owned();
    lock_path.push(".lock");
    PathBuf::from(lock_path)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;
    use std::sync::atomic::{AtomicU32, Ordering};

    #[test]
    fn test_lock_path_for() {
        let cache_path = Path::new("/home/user/.cache/dotdeps/python/requests/2.31.0");
        let lock = lock_path_for(cache_path);
        assert_eq!(
            lock,
            PathBuf::from("/home/user/.cache/dotdeps/python/requests/2.31.0.lock")
        );
    }

    #[test]
    fn test_acquire_and_release() {
        let temp_dir = std::env::temp_dir().join("dotdeps_lock_test_1");
        let _ = std::fs::create_dir_all(&temp_dir);
        let lock_path = temp_dir.join("test.lock");

        // Acquire lock
        let lock = CacheLock::acquire(&lock_path).unwrap();

        // Lock file should exist
        assert!(lock_path.exists());

        // Drop releases lock
        drop(lock);

        // Lock file should be cleaned up
        assert!(!lock_path.exists());

        // Cleanup
        let _ = std::fs::remove_dir_all(&temp_dir);
    }

    #[test]
    fn test_try_acquire_contention() {
        let temp_dir = std::env::temp_dir().join("dotdeps_lock_test_2");
        let _ = std::fs::create_dir_all(&temp_dir);
        let lock_path = temp_dir.join("test.lock");

        // First lock should succeed
        let lock1 = CacheLock::try_acquire(&lock_path).unwrap();
        assert!(lock1.is_some());

        // Second try should return None (would block)
        let lock2 = CacheLock::try_acquire(&lock_path).unwrap();
        assert!(lock2.is_none());

        // After dropping first lock, should be able to acquire
        drop(lock1);
        let lock3 = CacheLock::try_acquire(&lock_path).unwrap();
        assert!(lock3.is_some());

        // Cleanup
        drop(lock3);
        let _ = std::fs::remove_dir_all(&temp_dir);
    }

    #[test]
    fn test_concurrent_lock_serialization() {
        let temp_dir = std::env::temp_dir().join("dotdeps_lock_test_3");
        let _ = std::fs::remove_dir_all(&temp_dir);
        let _ = std::fs::create_dir_all(&temp_dir);
        let lock_path = temp_dir.join("test.lock");
        let counter = Arc::new(AtomicU32::new(0));

        // Spawn multiple threads that try to increment a counter under lock
        let mut handles = vec![];
        for _ in 0..4 {
            let lock_path = lock_path.clone();
            let counter = Arc::clone(&counter);
            handles.push(std::thread::spawn(move || {
                let _lock = CacheLock::acquire(&lock_path).unwrap();
                // Simulate some work under lock
                let current = counter.load(Ordering::SeqCst);
                std::thread::sleep(Duration::from_millis(10));
                counter.store(current + 1, Ordering::SeqCst);
            }));
        }

        for handle in handles {
            handle.join().unwrap();
        }

        // All increments should have been serialized
        assert_eq!(counter.load(Ordering::SeqCst), 4);

        // Cleanup
        let _ = std::fs::remove_dir_all(&temp_dir);
    }
}
