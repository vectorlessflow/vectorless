// Copyright (c) 2026 vectorless developers
// SPDX-License-Identifier: Apache-2.0

//! Compression example.
//!
//! This example demonstrates compression support in storage:
//! - GzipCodec for compressed storage
//! - IdentityCodec for uncompressed storage
//! - Codec trait for custom compression
//!
//! # Usage
//!
//! ```bash
//! cargo run --example storage_compression
//! ```

use vectorless::Result;
use vectorless::storage::{Codec, GzipCodec, IdentityCodec};

fn main() -> Result<()> {
    println!("=== Compression Example ===\n");

    // Test data
    let original = b"Lorem ipsum dolor sit amet, consectetur adipiscing elit. \
                     Sed do eiusmod tempor incididunt ut labore et dolore magna aliqua. \
                     Ut enim ad minim veniam, quis nostrud exercitation ullamco laboris.";
    println!("Original data ({} bytes):", original.len());
    println!("   {:?}...\n", String::from_utf8_lossy(&original[..50]));

    // 1. Identity codec (no compression)
    println!("1. IdentityCodec (no compression):");
    let identity = IdentityCodec::new();

    let identity_encoded = identity.encode(original)?;
    let identity_decoded = identity.decode(&identity_encoded)?;

    println!("   Encoded size: {} bytes", identity_encoded.len());
    println!(
        "   Compression ratio: {:.1}%",
        (identity_encoded.len() as f64 / original.len() as f64) * 100.0
    );
    assert_eq!(original.to_vec(), identity_decoded);
    println!("   ✓ Roundtrip verified\n");

    // 2. Gzip codec with different levels
    println!("2. GzipCodec with different compression levels:");

    for level in [1, 6, 9] {
        let gzip = GzipCodec::new(level);
        let compressed = gzip.encode(original)?;

        println!(
            "   Level {}: {} bytes ({:.1}% of original)",
            level,
            compressed.len(),
            (compressed.len() as f64 / original.len() as f64) * 100.0
        );
    }
    println!();

    // 3. Gzip roundtrip
    println!("3. Gzip roundtrip verification:");
    let gzip = GzipCodec::new(6);

    let encoded = gzip.encode(original)?;
    let decoded = gzip.decode(&encoded)?;

    assert_eq!(original.to_vec(), decoded);
    println!(
        "   ✓ Encoded {} bytes -> {} bytes",
        original.len(),
        encoded.len()
    );
    println!("   ✓ Decoded back to {} bytes", decoded.len());
    println!("   ✓ Data integrity verified\n");

    // 4. Empty data handling
    println!("4. Edge cases:");
    let empty: &[u8] = &[];

    let empty_encoded = gzip.encode(empty)?;
    let empty_decoded = gzip.decode(&empty_encoded)?;
    assert!(empty_decoded.is_empty());
    println!("   ✓ Empty data handled correctly\n");

    // 5. Comparison
    println!("5. Summary:");
    println!("   Original:    {} bytes", original.len());
    println!("   Identity:    {} bytes (100.0%)", identity_encoded.len());
    println!(
        "   Gzip (lvl6): {} bytes ({:.1}%)",
        encoded.len(),
        (encoded.len() as f64 / original.len() as f64) * 100.0
    );
    println!();

    println!("✓ Compression example complete!");
    println!("\nUsage tips:");
    println!("  - Use GzipCodec for large text documents");
    println!("  - Use IdentityCodec for already-compressed data (PDF, images)");
    println!("  - Level 6 is a good default (balance of speed vs ratio)");

    Ok(())
}
