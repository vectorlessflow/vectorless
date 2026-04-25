// Copyright (c) 2026 vectorless developers
// SPDX-License-Identifier: Apache-2.0

//! Fingerprint system for content and subtree identification.
//!
//! This module provides a robust fingerprinting system for content identification,
//! enabling precise change detection at both content and subtree levels.
//!
//! # Key Features
//!
//! - **Content Fingerprint**: Hash of node content (title + text)
//! - **Subtree Fingerprint**: Recursive hash including all descendants
//! - **Stable Serialization**: Type-tagged hashing for consistent results
//!
//! # Usage
//!
//! ```rust,ignore
//! use vectorless::fingerprint::{Fingerprint, Fingerprinter};
//!
//! // Create a fingerprint from content
//! let fp = Fingerprinter::new()
//!     .with_str("Hello, world!")
//!     .into_fingerprint();
//!
//! // Compare fingerprints
//! if old_fp == new_fp {
//!     // Content unchanged
//! }
//! ```

use base64::prelude::*;
use blake2::digest::typenum;
use blake2::{Blake2b, Digest};
use serde::{Deserialize, Serialize};
use std::hash::{Hash, Hasher};

/// A 128-bit fingerprint for content identification.
///
/// Uses BLAKE2b-128 for fast, collision-resistant hashing.
/// Displayed as base64 for compact representation.
#[derive(Clone, Copy, PartialEq, Eq)]
pub struct Fingerprint(pub [u8; 16]);

impl Fingerprint {
    /// Create a fingerprint from raw bytes.
    pub fn new(bytes: [u8; 16]) -> Self {
        Self(bytes)
    }

    /// Create a fingerprint from a byte slice (hashes the slice).
    pub fn from_bytes(data: &[u8]) -> Self {
        let mut hasher = Blake2b::<typenum::U16>::default();
        hasher.update(data);
        Self(hasher.finalize().into())
    }

    /// Create a fingerprint from a string.
    pub fn from_str(s: &str) -> Self {
        Self::from_bytes(s.as_bytes())
    }

    /// Encode fingerprint to base64 string.
    pub fn to_base64(self) -> String {
        BASE64_STANDARD.encode(self.0)
    }

    /// Decode fingerprint from base64 string.
    pub fn from_base64(s: &str) -> Result<Self, FingerprintError> {
        let bytes = BASE64_STANDARD
            .decode(s)
            .map_err(|e| FingerprintError::InvalidBase64(e.to_string()))?;
        let bytes: [u8; 16] = bytes
            .try_into()
            .map_err(|e: Vec<u8>| FingerprintError::InvalidLength(e.len()))?;
        Ok(Self(bytes))
    }

    /// Get the raw bytes.
    pub fn as_bytes(&self) -> &[u8] {
        &self.0
    }

    /// Check if this is a zero/null fingerprint.
    pub fn is_zero(&self) -> bool {
        self.0 == [0u8; 16]
    }

    /// Create a zero/null fingerprint (for uninitialized state).
    pub fn zero() -> Self {
        Self([0u8; 16])
    }
}

impl std::fmt::Display for Fingerprint {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        for byte in &self.0 {
            write!(f, "{:02x}", byte)?;
        }
        Ok(())
    }
}

impl std::fmt::Debug for Fingerprint {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Fingerprint({})", self)
    }
}

impl Hash for Fingerprint {
    fn hash<H: Hasher>(&self, state: &mut H) {
        // Fingerprint is already evenly distributed, use first 8 bytes
        state.write(&self.0[..8]);
    }
}

impl Serialize for Fingerprint {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        serializer.serialize_str(&self.to_base64())
    }
}

impl<'de> Deserialize<'de> for Fingerprint {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let s = String::deserialize(deserializer)?;
        Self::from_base64(&s).map_err(serde::de::Error::custom)
    }
}

impl Default for Fingerprint {
    fn default() -> Self {
        Self::zero()
    }
}

/// Error type for fingerprint operations.
#[derive(Debug, thiserror::Error)]
pub enum FingerprintError {
    /// Invalid base64 encoding.
    #[error("Invalid base64: {0}")]
    InvalidBase64(String),

    /// Invalid fingerprint length.
    #[error("Invalid fingerprint length: {0}")]
    InvalidLength(usize),

    /// Serialization error.
    #[error("Serialization error: {0}")]
    Serialization(String),
}

/// Builder for creating fingerprints.
///
/// Provides a fluent API for incrementally building fingerprints
/// from multiple values.
///
/// # Example
///
/// ```rust,ignore
/// let fp = Fingerprinter::new()
///     .with_str("title")
///     .with_str("content")
///     .with_usize(42)
///     .into_fingerprint();
/// ```
#[derive(Clone)]
pub struct Fingerprinter {
    hasher: Blake2b<typenum::U16>,
}

impl Fingerprinter {
    /// Create a new fingerprinter.
    pub fn new() -> Self {
        Self {
            hasher: Blake2b::<typenum::U16>::default(),
        }
    }

    /// Finalize and produce the fingerprint.
    pub fn into_fingerprint(self) -> Fingerprint {
        Fingerprint(self.hasher.finalize().into())
    }

    /// Add a string to the hash.
    pub fn with_str(mut self, s: &str) -> Self {
        self.write_str(s);
        self
    }

    /// Add a string to the hash (mutable).
    pub fn write_str(&mut self, s: &str) {
        self.write_type_tag("s");
        self.write_varlen_bytes(s.as_bytes());
    }

    /// Add bytes to the hash.
    pub fn with_bytes(mut self, bytes: &[u8]) -> Self {
        self.write_bytes(bytes);
        self
    }

    /// Add bytes to the hash (mutable).
    pub fn write_bytes(&mut self, bytes: &[u8]) {
        self.write_type_tag("b");
        self.write_varlen_bytes(bytes);
    }

    /// Add a usize to the hash.
    pub fn with_usize(mut self, n: usize) -> Self {
        self.write_usize(n);
        self
    }

    /// Add a usize to the hash (mutable).
    pub fn write_usize(&mut self, n: usize) {
        self.write_type_tag("u");
        self.hasher.update((n as u64).to_le_bytes());
    }

    /// Add a u64 to the hash.
    pub fn with_u64(mut self, n: u64) -> Self {
        self.write_u64(n);
        self
    }

    /// Add a u64 to the hash (mutable).
    pub fn write_u64(&mut self, n: u64) {
        self.write_type_tag("u8");
        self.hasher.update(n.to_le_bytes());
    }

    /// Add an i64 to the hash.
    pub fn with_i64(mut self, n: i64) -> Self {
        self.write_i64(n);
        self
    }

    /// Add an i64 to the hash (mutable).
    pub fn write_i64(&mut self, n: i64) {
        self.write_type_tag("i8");
        self.hasher.update(n.to_le_bytes());
    }

    /// Add a bool to the hash.
    pub fn with_bool(mut self, b: bool) -> Self {
        self.write_bool(b);
        self
    }

    /// Add a bool to the hash (mutable).
    pub fn write_bool(&mut self, b: bool) {
        self.write_type_tag(if b { "t" } else { "f" });
    }

    /// Add an optional string to the hash.
    pub fn with_option_str(mut self, opt: Option<&str>) -> Self {
        self.write_option_str(opt);
        self
    }

    /// Add an optional string to the hash (mutable).
    pub fn write_option_str(&mut self, opt: Option<&str>) {
        match opt {
            Some(s) => {
                self.write_type_tag("some");
                self.write_str(s);
            }
            None => {
                self.write_type_tag("none");
            }
        }
    }

    /// Add another fingerprint to the hash.
    pub fn with_fingerprint(mut self, fp: &Fingerprint) -> Self {
        self.write_fingerprint(fp);
        self
    }

    /// Add another fingerprint to the hash (mutable).
    pub fn write_fingerprint(&mut self, fp: &Fingerprint) {
        self.write_type_tag("fp");
        self.hasher.update(&fp.0);
    }

    /// Add raw bytes directly (no type tag).
    pub fn write_raw(&mut self, bytes: &[u8]) {
        self.hasher.update(bytes);
    }

    // Internal helpers

    fn write_type_tag(&mut self, tag: &str) {
        self.hasher.update(tag.as_bytes());
        self.hasher.update(b";");
    }

    fn write_varlen_bytes(&mut self, bytes: &[u8]) {
        self.hasher.update((bytes.len() as u32).to_le_bytes());
        self.hasher.update(bytes);
    }
}

/// Node fingerprint containing both content and subtree fingerprints.
///
/// This enables precise change detection:
/// - If `content_fp` changes, the node's content was modified
/// - If `subtree_fp` changes, the node or its descendants were modified
/// - If `content_fp` is same but `subtree_fp` changed, only descendants changed
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub struct NodeFingerprint {
    /// Fingerprint of this node's content (title + text).
    pub content: Fingerprint,

    /// Fingerprint of the entire subtree (including this node).
    /// Computed recursively from all descendants.
    pub subtree: Fingerprint,
}

impl NodeFingerprint {
    /// Create a new node fingerprint.
    pub fn new(content: Fingerprint, subtree: Fingerprint) -> Self {
        Self { content, subtree }
    }

    /// Create a fingerprint for a leaf node (content == subtree).
    pub fn leaf(content: Fingerprint) -> Self {
        Self {
            content,
            subtree: content,
        }
    }

    /// Create a zero/null fingerprint.
    pub fn zero() -> Self {
        Self {
            content: Fingerprint::zero(),
            subtree: Fingerprint::zero(),
        }
    }

    /// Check if this is a zero fingerprint.
    pub fn is_zero(&self) -> bool {
        self.content.is_zero() && self.subtree.is_zero()
    }

    /// Check if content changed compared to another fingerprint.
    pub fn content_changed(&self, other: &Self) -> bool {
        self.content != other.content
    }

    /// Check if subtree changed compared to another fingerprint.
    pub fn subtree_changed(&self, other: &Self) -> bool {
        self.subtree != other.subtree
    }

    /// Check if only descendants changed (content same, subtree different).
    pub fn only_descendants_changed(&self, other: &Self) -> bool {
        self.content == other.content && self.subtree != other.subtree
    }
}

impl Default for NodeFingerprint {
    fn default() -> Self {
        Self::zero()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_fingerprint_from_str() {
        let fp1 = Fingerprint::from_str("hello");
        let fp2 = Fingerprint::from_str("hello");
        let fp3 = Fingerprint::from_str("world");

        assert_eq!(fp1, fp2);
        assert_ne!(fp1, fp3);
    }

    #[test]
    fn test_fingerprint_base64_roundtrip() {
        let fp = Fingerprint::from_str("test content");
        let encoded = fp.to_base64();
        let decoded = Fingerprint::from_base64(&encoded).unwrap();
        assert_eq!(fp, decoded);
    }

    #[test]
    fn test_fingerprinter_chaining() {
        let fp1 = Fingerprinter::new()
            .with_str("title")
            .with_str("content")
            .into_fingerprint();

        let fp2 = Fingerprinter::new()
            .with_str("title")
            .with_str("content")
            .into_fingerprint();

        let fp3 = Fingerprinter::new()
            .with_str("title")
            .with_str("different")
            .into_fingerprint();

        assert_eq!(fp1, fp2);
        assert_ne!(fp1, fp3);
    }

    #[test]
    fn test_fingerprinter_types() {
        let fp1 = Fingerprinter::new()
            .with_str("test")
            .with_usize(42)
            .with_bool(true)
            .into_fingerprint();

        let fp2 = Fingerprinter::new()
            .with_str("test")
            .with_usize(42)
            .with_bool(true)
            .into_fingerprint();

        let fp3 = Fingerprinter::new()
            .with_str("test")
            .with_usize(43) // different number
            .with_bool(true)
            .into_fingerprint();

        assert_eq!(fp1, fp2);
        assert_ne!(fp1, fp3);
    }

    #[test]
    fn test_node_fingerprint() {
        let content = Fingerprint::from_str("content");
        let subtree = Fingerprint::from_str("subtree");

        let fp = NodeFingerprint::new(content, subtree);

        assert!(!fp.is_zero());
        assert_eq!(fp.content, content);
        assert_eq!(fp.subtree, subtree);
    }

    #[test]
    fn test_node_fingerprint_change_detection() {
        let old = NodeFingerprint::new(
            Fingerprint::from_str("content"),
            Fingerprint::from_str("subtree"),
        );

        // Same content, different subtree
        let new1 = NodeFingerprint::new(
            Fingerprint::from_str("content"),
            Fingerprint::from_str("different"),
        );
        assert!(new1.only_descendants_changed(&old));
        assert!(!new1.content_changed(&old));
        assert!(new1.subtree_changed(&old));

        // Different content
        let new2 = NodeFingerprint::new(
            Fingerprint::from_str("different"),
            Fingerprint::from_str("subtree"),
        );
        assert!(!new2.only_descendants_changed(&old));
        assert!(new2.content_changed(&old));
    }

    #[test]
    fn test_fingerprint_serialization() {
        let fp = Fingerprint::from_str("test serialization");
        let json = serde_json::to_string(&fp).unwrap();
        let decoded: Fingerprint = serde_json::from_str(&json).unwrap();
        assert_eq!(fp, decoded);
    }

    #[test]
    fn test_node_fingerprint_serialization() {
        let fp = NodeFingerprint::new(
            Fingerprint::from_str("content"),
            Fingerprint::from_str("subtree"),
        );
        let json = serde_json::to_string(&fp).unwrap();
        let decoded: NodeFingerprint = serde_json::from_str(&json).unwrap();
        assert_eq!(fp, decoded);
    }
}
