// Copyright (c) 2026 vectorless developers
// SPDX-License-Identifier: Apache-2.0

//! Version migration system for persisted data.
//!
//! This module provides a framework for migrating data between format versions.
//! When the data format changes, migrations can automatically upgrade older data.
//!
//! # Example
//!
//! ```rust,ignore
//! use vectorless::storage::migration::{Migration, Migrator, MigrationContext};
//!
//! // Define a migration from v1 to v2
//! struct V1ToV2;
//!
//! impl Migration for V1ToV2 {
//!     fn from_version(&self) -> u32 { 1 }
//!     fn to_version(&self) -> u32 { 2 }
//!     fn migrate(&self, data: &[u8], ctx: &MigrationContext) -> Result<Vec<u8>> {
//!         // Transform data from v1 to v2 format
//!         // ...
//!     }
//! }
//!
//! // Register migrations
//! let mut migrator = Migrator::new();
//! migrator.register(Box::new(V1ToV2));
//!
//! // Migrate data
//! let migrated = migrator.migrate(data, 1, 2)?;
//! ```

use std::collections::HashMap;

use tracing::{debug, info, warn};

use crate::Error;
use crate::error::Result;

/// Current data format version.
pub const CURRENT_VERSION: u32 = 1;

/// Migration context providing additional information for migrations.
#[derive(Debug, Clone)]
pub struct MigrationContext {
    /// Source version.
    pub from_version: u32,
    /// Target version.
    pub to_version: u32,
    /// Additional metadata.
    pub metadata: HashMap<String, String>,
}

impl MigrationContext {
    /// Create a new migration context.
    pub fn new(from_version: u32, to_version: u32) -> Self {
        Self {
            from_version,
            to_version,
            metadata: HashMap::new(),
        }
    }

    /// Add metadata.
    pub fn with_metadata(mut self, key: impl Into<String>, value: impl Into<String>) -> Self {
        self.metadata.insert(key.into(), value.into());
        self
    }
}

/// Trait for data migrations.
///
/// A migration transforms data from one version to the next.
pub trait Migration: Send + Sync {
    /// Get the source version this migration applies to.
    fn from_version(&self) -> u32;

    /// Get the target version this migration produces.
    fn to_version(&self) -> u32;

    /// Get a human-readable description of this migration.
    fn description(&self) -> &str;

    /// Perform the migration.
    ///
    /// # Arguments
    ///
    /// * `data` - The data to migrate
    /// * `ctx` - Migration context with additional information
    ///
    /// # Returns
    ///
    /// The migrated data in the new format.
    fn migrate(&self, data: &[u8], ctx: &MigrationContext) -> Result<Vec<u8>>;

    /// Check if this migration can be applied to the given data.
    ///
    /// Default implementation always returns true.
    fn can_migrate(&self, _data: &[u8]) -> bool {
        true
    }
}

/// Migration registry and executor.
pub struct Migrator {
    /// Registered migrations, keyed by (from_version, to_version).
    migrations: HashMap<(u32, u32), Box<dyn Migration>>,
}

impl Default for Migrator {
    fn default() -> Self {
        Self::new()
    }
}

impl std::fmt::Debug for Migrator {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Migrator")
            .field("migration_count", &self.migrations.len())
            .finish()
    }
}

impl Migrator {
    /// Create a new migrator.
    pub fn new() -> Self {
        Self {
            migrations: HashMap::new(),
        }
    }

    /// Register a migration.
    pub fn register(&mut self, migration: Box<dyn Migration>) {
        let key = (migration.from_version(), migration.to_version());
        debug!("Registering migration: v{} -> v{}", key.0, key.1);
        self.migrations.insert(key, migration);
    }

    /// Check if a migration path exists between two versions.
    pub fn can_migrate(&self, from_version: u32, to_version: u32) -> bool {
        if from_version == to_version {
            return true;
        }

        // Check if we have a direct migration
        if self.migrations.contains_key(&(from_version, to_version)) {
            return true;
        }

        // Check if we have a path through intermediate versions
        self.find_migration_path(from_version, to_version).is_some()
    }

    /// Find a migration path between two versions.
    ///
    /// Returns a sequence of version numbers to migrate through.
    fn find_migration_path(&self, from_version: u32, to_version: u32) -> Option<Vec<u32>> {
        if from_version == to_version {
            return Some(vec![from_version]);
        }

        // Simple BFS to find a path
        use std::collections::{HashSet, VecDeque};

        let mut visited: HashSet<u32> = HashSet::new();
        let mut queue: VecDeque<u32> = VecDeque::new();
        let mut parent: HashMap<u32, u32> = HashMap::new();

        queue.push_back(from_version);
        visited.insert(from_version);

        while let Some(current) = queue.pop_front() {
            // Find all migrations from current version
            for ((from, to), _) in &self.migrations {
                if *from == current && !visited.contains(to) {
                    visited.insert(*to);
                    parent.insert(*to, current);
                    queue.push_back(*to);

                    if *to == to_version {
                        // Reconstruct path
                        let mut path = vec![to_version];
                        let mut v = to_version;
                        while let Some(&p) = parent.get(&v) {
                            if p == from_version {
                                path.push(p);
                                break;
                            }
                            path.push(p);
                            v = p;
                        }
                        path.reverse();
                        return Some(path);
                    }
                }
            }
        }

        None
    }

    /// Migrate data from one version to another.
    ///
    /// If a direct migration exists, it will be used.
    /// Otherwise, the migrator will try to find a path through intermediate versions.
    pub fn migrate(&self, data: &[u8], from_version: u32, to_version: u32) -> Result<Vec<u8>> {
        if from_version == to_version {
            return Ok(data.to_vec());
        }

        // Find migration path
        let path = self
            .find_migration_path(from_version, to_version)
            .ok_or_else(|| {
                Error::VersionMismatch(format!(
                    "No migration path from v{} to v{}",
                    from_version, to_version
                ))
            })?;

        if path.len() < 2 {
            return Ok(data.to_vec());
        }

        info!(
            "Migrating data from v{} to v{} via path: {:?}",
            from_version, to_version, path
        );

        let mut current_data = data.to_vec();
        let mut current_version = from_version;

        for next_version in path.iter().skip(1) {
            let key = (current_version, *next_version);
            let migration = self.migrations.get(&key).ok_or_else(|| {
                Error::VersionMismatch(format!(
                    "Missing migration from v{} to v{}",
                    current_version, next_version
                ))
            })?;

            let ctx = MigrationContext::new(current_version, *next_version);

            debug!(
                "Applying migration: v{} -> v{} ({})",
                current_version,
                next_version,
                migration.description()
            );

            current_data = migration.migrate(&current_data, &ctx)?;
            current_version = *next_version;
        }

        Ok(current_data)
    }

    /// Get the list of registered migrations.
    pub fn list_migrations(&self) -> Vec<(u32, u32, &str)> {
        self.migrations
            .values()
            .map(|m| (m.from_version(), m.to_version(), m.description()))
            .collect()
    }
}

// ============================================================================
// Built-in migrations
// ============================================================================

/// Placeholder migration for future versions.
/// This is a template that can be copied for actual migrations.
#[derive(Debug)]
pub struct PlaceholderMigration {
    from: u32,
    to: u32,
}

impl PlaceholderMigration {
    /// Create a new placeholder migration.
    pub fn new(from: u32, to: u32) -> Self {
        Self { from, to }
    }
}

impl Migration for PlaceholderMigration {
    fn from_version(&self) -> u32 {
        self.from
    }

    fn to_version(&self) -> u32 {
        self.to
    }

    fn description(&self) -> &str {
        "Placeholder migration (no-op)"
    }

    fn migrate(&self, data: &[u8], _ctx: &MigrationContext) -> Result<Vec<u8>> {
        warn!(
            "Using placeholder migration from v{} to v{} - no changes made",
            self.from, self.to
        );
        Ok(data.to_vec())
    }
}

/// Create a default migrator with all built-in migrations registered.
pub fn default_migrator() -> Migrator {
    Migrator::new()
    // Add migrations as needed when versions change
    // migrator.register(Box::new(V1ToV2::new()));
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_migration_context() {
        let ctx = MigrationContext::new(1, 2).with_metadata("key", "value");

        assert_eq!(ctx.from_version, 1);
        assert_eq!(ctx.to_version, 2);
        assert_eq!(ctx.metadata.get("key"), Some(&"value".to_string()));
    }

    #[test]
    fn test_migrator_no_migration_needed() {
        let migrator = Migrator::new();
        let data = b"test data";

        let result = migrator.migrate(data, 1, 1).unwrap();
        assert_eq!(result, data);
    }

    #[test]
    fn test_migrator_no_path() {
        let migrator = Migrator::new();
        let data = b"test data";

        let result = migrator.migrate(data, 1, 2);
        assert!(result.is_err());
    }

    #[test]
    fn test_migrator_with_placeholder() {
        let mut migrator = Migrator::new();
        migrator.register(Box::new(PlaceholderMigration::new(1, 2)));

        assert!(migrator.can_migrate(1, 2));
        assert!(!migrator.can_migrate(1, 3));

        let data = b"test data";
        let result = migrator.migrate(data, 1, 2).unwrap();
        assert_eq!(result, data);
    }

    #[test]
    fn test_migrator_path_finding() {
        let mut migrator = Migrator::new();
        migrator.register(Box::new(PlaceholderMigration::new(1, 2)));
        migrator.register(Box::new(PlaceholderMigration::new(2, 3)));

        assert!(migrator.can_migrate(1, 3));

        let path = migrator.find_migration_path(1, 3).unwrap();
        assert_eq!(path, vec![1, 2, 3]);

        let data = b"test data";
        let result = migrator.migrate(data, 1, 3).unwrap();
        assert_eq!(result, data);
    }

    #[test]
    fn test_list_migrations() {
        let mut migrator = Migrator::new();
        migrator.register(Box::new(PlaceholderMigration::new(1, 2)));
        migrator.register(Box::new(PlaceholderMigration::new(2, 3)));

        let list = migrator.list_migrations();
        assert_eq!(list.len(), 2);
    }
}
