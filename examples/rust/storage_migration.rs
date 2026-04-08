// Copyright (c) 2026 vectorless developers
// SPDX-License-Identifier: Apache-2.0

//! Version migration example.
//!
//! This example demonstrates how to use the migration system
//! for upgrading data formats between versions.
//!
//! # Usage
//!
//! ```bash
//! cargo run --example storage_migration
//! ```

use vectorless::storage::{Migration, MigrationContext, Migrator};
use vectorless::{Error, Result};

/// Example migration from v1 to v2.
///
/// Imagine v1 stored data as plain text,
/// and v2 adds a header prefix.
#[derive(Debug)]
struct V1ToV2;

impl Migration for V1ToV2 {
    fn from_version(&self) -> u32 {
        1
    }

    fn to_version(&self) -> u32 {
        2
    }

    fn description(&self) -> &str {
        "Add version header to data format"
    }

    fn migrate(&self, data: &[u8], _ctx: &MigrationContext) -> Result<Vec<u8>> {
        // Add a simple header: "V2:" prefix
        let mut result = b"V2:".to_vec();
        result.extend_from_slice(data);
        Ok(result)
    }
}

/// Example migration from v2 to v3.
///
/// V3 adds compression (simulated with base64-like encoding).
#[derive(Debug)]
struct V2ToV3;

impl Migration for V2ToV3 {
    fn from_version(&self) -> u32 {
        2
    }

    fn to_version(&self) -> u32 {
        3
    }

    fn description(&self) -> &str {
        "Add compression to data format"
    }

    fn migrate(&self, data: &[u8], _ctx: &MigrationContext) -> Result<Vec<u8>> {
        // Simulate compression by adding prefix
        let mut result = b"V3:COMPRESSED:".to_vec();
        result.extend_from_slice(data);
        Ok(result)
    }
}

fn main() -> vectorless::Result<()> {
    println!("=== Version Migration Example ===\n");

    // 1. Create migrator
    println!("1. Creating migrator and registering migrations...");
    let mut migrator = Migrator::new();
    migrator.register(Box::new(V1ToV2));
    migrator.register(Box::new(V2ToV3));

    println!("   Registered migrations:");
    for (from, to, desc) in migrator.list_migrations() {
        println!("   - v{} -> v{}: {}", from, to, desc);
    }
    println!();

    // 2. Check migration paths
    println!("2. Checking migration paths:");
    println!("   Can migrate v1 -> v2: {}", migrator.can_migrate(1, 2));
    println!("   Can migrate v1 -> v3: {}", migrator.can_migrate(1, 3));
    println!("   Can migrate v2 -> v3: {}", migrator.can_migrate(2, 3));
    println!("   Can migrate v1 -> v4: {}", migrator.can_migrate(1, 4));
    println!();

    // 3. Migrate from v1 to v3 (multi-step)
    println!("3. Migrating data from v1 to v3 (via v2):");
    let original_data = b"Hello, World!";
    println!(
        "   Original (v1): {:?}",
        String::from_utf8_lossy(original_data)
    );

    let migrated = migrator.migrate(original_data, 1, 3)?;
    println!("   Migrated (v3): {:?}", String::from_utf8_lossy(&migrated));
    println!();

    // 4. Direct migration
    println!("4. Direct migration v2 -> v3:");
    let v2_data = b"V2:Some data";
    let v3_data = migrator.migrate(v2_data, 2, 3)?;
    println!("   V2: {:?}", String::from_utf8_lossy(v2_data));
    println!("   V3: {:?}", String::from_utf8_lossy(&v3_data));
    println!();

    // 5. No migration needed
    println!("5. Same version (no migration):");
    let data = b"Already v3";
    let result = migrator.migrate(data, 3, 3)?;
    assert_eq!(data.to_vec(), result);
    println!("   ✓ Data unchanged when from == to");
    println!();

    // 6. Error case: no path
    println!("6. Error handling (no migration path):");
    match migrator.migrate(b"test", 1, 99) {
        Err(Error::VersionMismatch(msg)) => {
            println!("   Expected error: {}", msg);
        }
        _ => unreachable!(),
    }
    println!();

    println!("✓ Migration example complete!");
    println!("\nKey points:");
    println!("  - Migrations are registered as v(N) -> v(N+1)");
    println!("  - Migrator finds paths automatically (BFS)");
    println!("  - Multi-step migrations are handled transparently");

    Ok(())
}
