// Copyright (c) 2026 vectorless developers
// SPDX-License-Identifier: Apache-2.0

//! File locking for workspace safety.
//!
//! Provides cross-process file locking to prevent data corruption
//! when multiple processes access the same workspace.

// File locking inherently requires unsafe FFI calls
#![allow(unsafe_code)]
//!
//! Provides cross-process file locking to prevent data corruption
//! when multiple processes access the same workspace.

use std::fs::{File, OpenOptions};
use std::path::Path;

use crate::Error;
use crate::error::Result;

/// A file lock that is automatically released when dropped.
///
/// Uses the `flock` on Unix and `LockFileEx` on Windows.
#[derive(Debug)]
pub struct FileLock {
    /// The locked file handle.
    file: Option<File>,
    /// Path to the lock file (for debugging).
    path: std::path::PathBuf,
    /// Whether the lock is held exclusively.
    exclusive: bool,
}

impl FileLock {
    /// Try to acquire an file lock.
    ///
    /// # Arguments
    ///
    /// * `path` - Path to the lock file (will be created if it doesn't exist)
    /// * `exclusive` - If true, acquires an exclusive (write) lock; otherwise a shared (read) lock
    ///
    /// # Errors
    ///
    /// Returns `Error::WorkspaceLocked` if the lock is held by another process.
    pub fn try_lock(path: impl Into<std::path::PathBuf>, exclusive: bool) -> Result<Self> {
        let path = path.into();

        // Ensure parent directory exists
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).map_err(Error::Io)?;
        }

        // Open or create the lock file
        let file = OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .truncate(false)
            .open(&path)
            .map_err(Error::Io)?;

        // Try to acquire the lock
        #[cfg(unix)]
        {
            let fd = std::os::unix::io::AsRawFd::as_raw_fd(&file);

            let result = if exclusive {
                // LOCK_EX | LOCK_NB
                unsafe { libc::flock(fd, 0x02 | 0x04) }
            } else {
                // LOCK_SH | LOCK_NB
                unsafe { libc::flock(fd, 0x01 | 0x04) }
            };

            if result != 0 {
                return Err(Error::WorkspaceLocked);
            }

            Ok(Self {
                file: Some(file),
                path,
                exclusive,
            })
        }

        #[cfg(windows)]
        {
            use std::os::windows::fs::OpenOptionsExt;
            use windows_sys::Win32::Storage::FileSystem::{
                LOCKFILE_EXCLUSIVE_LOCK, LOCKFILE_FAIL_IMMEDIATELY, LockFileEx,
            };

            let handle = std::os::windows::io::AsRawHandle::as_raw_handle(&file);

            let mut overlapped = std::mem::MaybeUninit::zeroed();
            let result = unsafe {
                LockFileEx(
                    handle,
                    if exclusive {
                        LOCKFILE_EXCLUSIVE_LOCK
                    } else {
                        0
                    } | LOCKFILE_FAIL_IMMEDIATELY,
                    0,
                    0xFFFFFFFF,
                    0xFFFFFFFF,
                    overlapped.as_mut_ptr(),
                )
            };

            if result == 0 {
                return Err(Error::WorkspaceLocked);
            }

            Ok(Self {
                file: Some(file),
                path,
                exclusive,
            })
        }

        #[cfg(not(any(unix, windows)))]
        {
            // Fallback: No file locking available
            // Just keep the file open, which provides some protection
            Ok(Self {
                file: Some(file),
                path,
                exclusive,
            })
        }
    }

    /// Try to acquire a lock without blocking.
    ///
    /// Returns `Ok(FileLock)` if the lock was acquired, or `Ok(None)` if it would block.
    pub fn try_lock_no_wait(
        path: impl Into<std::path::PathBuf>,
        exclusive: bool,
    ) -> Result<Option<Self>> {
        match Self::try_lock(&path.into(), exclusive) {
            Ok(lock) => Ok(Some(lock)),
            Err(Error::WorkspaceLocked) => Ok(None),
            Err(e) => Err(e),
        }
    }

    /// Check if the lock file is locked by another process.
    ///
    /// This is useful for checking without acquiring a lock.
    pub fn is_locked(path: impl Into<std::path::PathBuf>) -> bool {
        Self::try_lock(&path.into(), false).is_err()
    }

    /// Release the lock.
    pub fn unlock(mut self) {
        if let Some(file) = self.file.take() {
            // File will be unlocked when dropped
            drop(file);
        }
    }

    /// Get the lock file path.
    pub fn path(&self) -> &Path {
        &self.path
    }

    /// Check if this is an exclusive lock.
    pub fn is_exclusive(&self) -> bool {
        self.exclusive
    }
}

impl Drop for FileLock {
    fn drop(&mut self) {
        if let Some(file) = self.file.take() {
            // File descriptor closed, lock automatically released
            drop(file);
        }
    }
}

/// A scoped lock guard that releases the lock when dropped.
///
/// This is useful for ensuring the lock is released even on panic.
pub struct ScopedLock {
    lock: Option<FileLock>,
}

impl ScopedLock {
    /// Acquire a scoped lock.
    pub fn new(path: impl Into<std::path::PathBuf>, exclusive: bool) -> Result<Self> {
        let lock = FileLock::try_lock(path, exclusive)?;
        Ok(Self { lock: Some(lock) })
    }

    /// Release the lock early.
    pub fn release(mut self) {
        if let Some(lock) = self.lock.take() {
            lock.unlock();
        }
    }
}

impl Drop for ScopedLock {
    fn drop(&mut self) {
        // Lock automatically released when FileLock is dropped
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn test_file_lock_acquire_release() {
        let temp = TempDir::new().unwrap();
        let lock_path = temp.path().join("test.lock");

        let lock = FileLock::try_lock(&lock_path, true).unwrap();
        assert!(lock.is_exclusive());

        // Should be able to unlock
        lock.unlock();
    }

    #[test]
    fn test_file_lock_conflict() {
        let temp = TempDir::new().unwrap();
        let lock_path = temp.path().join("conflict.lock");

        // Acquire exclusive lock
        let _lock1 = FileLock::try_lock(&lock_path, true).unwrap();

        // Try to acquire another exclusive lock - should fail
        let result = FileLock::try_lock(&lock_path, true);
        assert!(matches!(result, Err(Error::WorkspaceLocked)));
    }

    #[test]
    fn test_file_lock_shared() {
        let temp = TempDir::new().unwrap();
        let lock_path = temp.path().join("shared.lock");

        // Acquire shared lock
        let lock1 = FileLock::try_lock(&lock_path, false).unwrap();
        assert!(!lock1.is_exclusive());

        // Should be able to acquire another shared lock
        let lock2 = FileLock::try_lock(&lock_path, false).unwrap();
        assert!(!lock2.is_exclusive());

        // But exclusive lock should fail
        let result = FileLock::try_lock(&lock_path, true);
        assert!(matches!(result, Err(Error::WorkspaceLocked)));

        lock1.unlock();
        lock2.unlock();
    }

    #[test]
    fn test_scoped_lock() {
        let temp = TempDir::new().unwrap();
        let lock_path = temp.path().join("scoped.lock");

        {
            let _scoped = ScopedLock::new(&lock_path, true).unwrap();
            // Lock held here

            // Another lock should fail
            let result = FileLock::try_lock(&lock_path, true);
            assert!(matches!(result, Err(Error::WorkspaceLocked)));
        }
        // Lock released here

        // Now should succeed
        let _lock = FileLock::try_lock(&lock_path, true).unwrap();
    }
}
