// Copyright (c) 2026 vectorless developers
// SPDX-License-Identifier: Apache-2.0

//! Codec abstraction for compression and decompression.
//!
//! This module provides a codec trait for compressing/decompressing data,
//! with implementations for:
//!
//! - **Identity**: No compression (pass-through)
//! - **Gzip**: Standard gzip compression
//!
//! # Example
//!
//! ```rust,ignore
//! use vectorless::storage::codec::{Codec, GzipCodec};
//!
//! let codec = GzipCodec::new(6);
//!
//! let data = b"some data to compress";
//! let compressed = codec.encode(data)?;
//! let decompressed = codec.decode(&compressed)?;
//!
//! assert_eq!(data.as_slice(), decompressed.as_slice());
//! ```

use std::fmt::Debug;
use std::io::{Read, Write};

use flate2::Compression;
use flate2::read::GzDecoder;
use flate2::write::GzEncoder;

use crate::Error;
use crate::error::Result;

/// Codec trait for compression/decompression.
pub trait Codec: Debug + Send + Sync {
    /// Encode (compress) data.
    fn encode(&self, data: &[u8]) -> Result<Vec<u8>>;

    /// Decode (decompress) data.
    fn decode(&self, data: &[u8]) -> Result<Vec<u8>>;

    /// Get the codec name.
    fn name(&self) -> &'static str;
}

/// Identity codec (no compression).
///
/// Passes data through unchanged.
#[derive(Debug, Clone, Copy, Default)]
pub struct IdentityCodec;

impl IdentityCodec {
    /// Create a new identity codec.
    pub fn new() -> Self {
        Self::default()
    }
}

impl Codec for IdentityCodec {
    fn encode(&self, data: &[u8]) -> Result<Vec<u8>> {
        Ok(data.to_vec())
    }

    fn decode(&self, data: &[u8]) -> Result<Vec<u8>> {
        Ok(data.to_vec())
    }

    fn name(&self) -> &'static str {
        "identity"
    }
}

/// Gzip codec.
///
/// Uses the `flate2` crate for gzip compression.
#[derive(Debug, Clone)]
pub struct GzipCodec {
    /// Compression level (0-9).
    level: u32,
}

impl GzipCodec {
    /// Create a new gzip codec with the given compression level.
    ///
    /// Level is clamped to 0-9:
    /// - 0: No compression
    /// - 1: Fastest compression
    /// - 6: Default (good balance)
    /// - 9: Best compression (slowest)
    pub fn new(level: u32) -> Self {
        Self {
            level: level.clamp(0, 9),
        }
    }

    /// Create a codec with fast compression (level 1).
    pub fn fast() -> Self {
        Self::new(1)
    }

    /// Create a codec with default compression (level 6).
    pub fn default_level() -> Self {
        Self::new(6)
    }

    /// Create a codec with best compression (level 9).
    pub fn best() -> Self {
        Self::new(9)
    }
}

impl Default for GzipCodec {
    fn default() -> Self {
        Self::default_level()
    }
}

impl Codec for GzipCodec {
    fn encode(&self, data: &[u8]) -> Result<Vec<u8>> {
        let mut encoder = GzEncoder::new(Vec::new(), Compression::new(self.level));
        encoder
            .write_all(data)
            .map_err(|e| Error::Parse(format!("Gzip encode error: {}", e)))?;
        encoder
            .finish()
            .map_err(|e| Error::Parse(format!("Gzip finish error: {}", e)))
    }

    fn decode(&self, data: &[u8]) -> Result<Vec<u8>> {
        let mut decoder = GzDecoder::new(data);
        let mut decoded = Vec::new();
        decoder
            .read_to_end(&mut decoded)
            .map_err(|e| Error::Parse(format!("Gzip decode error: {}", e)))?;
        Ok(decoded)
    }

    fn name(&self) -> &'static str {
        "gzip"
    }
}

/// Create a codec from configuration.
pub fn codec_from_config(
    enabled: bool,
    algorithm: crate::config::CompressionAlgorithm,
    level: u32,
) -> Box<dyn Codec> {
    if !enabled {
        return Box::new(IdentityCodec::new());
    }

    match algorithm {
        crate::config::CompressionAlgorithm::Gzip => Box::new(GzipCodec::new(level)),
        crate::config::CompressionAlgorithm::Zstd => {
            // Zstd not implemented yet, fallback to gzip
            // TODO: Add zstd support when needed
            Box::new(GzipCodec::new(level))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_identity_codec() {
        let codec = IdentityCodec::new();
        let data = b"test data";

        let encoded = codec.encode(data).unwrap();
        let decoded = codec.decode(&encoded).unwrap();

        assert_eq!(data.as_slice(), decoded.as_slice());
        assert_eq!(codec.name(), "identity");
    }

    #[test]
    fn test_gzip_codec_basic() {
        let codec = GzipCodec::default();
        let data = b"Hello, World! This is a test string for compression.";

        let encoded = codec.encode(data).unwrap();
        let decoded = codec.decode(&encoded).unwrap();

        assert_eq!(data.as_slice(), decoded.as_slice());
        assert_eq!(codec.name(), "gzip");

        // Compressed should be smaller for repetitive data
        // Note: For very small data, gzip overhead might make it larger
        let repetitive = b"AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA";
        let compressed = codec.encode(repetitive).unwrap();
        assert!(compressed.len() < repetitive.len());
    }

    #[test]
    fn test_gzip_codec_levels() {
        let data = b"This is test data that should compress well. ".repeat(100);
        let data = data.into_iter().map(|b| b as u8).collect::<Vec<_>>();

        let codec_fast = GzipCodec::fast();
        let codec_best = GzipCodec::best();

        let compressed_fast = codec_fast.encode(&data).unwrap();
        let compressed_best = codec_best.encode(&data).unwrap();

        // Both should decompress to the same data
        assert_eq!(codec_fast.decode(&compressed_fast).unwrap(), data);
        assert_eq!(codec_best.decode(&compressed_best).unwrap(), data);

        // Best compression should be smaller or equal
        assert!(compressed_best.len() <= compressed_fast.len());
    }

    #[test]
    fn test_gzip_empty_data() {
        let codec = GzipCodec::default();
        let data = b"";

        let encoded = codec.encode(data).unwrap();
        let decoded = codec.decode(&encoded).unwrap();

        assert!(decoded.is_empty());
    }

    #[test]
    fn test_codec_from_config() {
        use crate::config::CompressionAlgorithm;

        // Disabled compression
        let codec = codec_from_config(false, CompressionAlgorithm::Gzip, 6);
        let data = b"test";
        let encoded = codec.encode(data).unwrap();
        assert_eq!(encoded, data);

        // Enabled compression
        let codec = codec_from_config(true, CompressionAlgorithm::Gzip, 6);
        let encoded = codec.encode(data).unwrap();
        let decoded = codec.decode(&encoded).unwrap();
        assert_eq!(decoded, data);
    }
}
