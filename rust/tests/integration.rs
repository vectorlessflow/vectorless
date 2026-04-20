// Copyright (c) 2026 vectorless developers
// SPDX-License-Identifier: Apache-2.0

//! Integration tests for the Engine client.
//!
//! These tests exercise the full index → persist → query lifecycle
//! without requiring a real LLM endpoint, using the no-LLM pipeline.

use std::path::PathBuf;

use vectorless::__test_support::build_test_engine;
use vectorless::{Engine, IndexContext, IndexMode};

async fn setup() -> (Engine, tempfile::TempDir) {
    let tmp = tempfile::tempdir().unwrap();
    let engine = build_test_engine(tmp.path()).await;
    (engine, tmp)
}

#[tokio::test]
async fn test_index_and_persist_single_markdown() {
    let (engine, tmp) = setup().await;

    // Write a test markdown file
    let md_path = tmp.path().join("test.md");
    std::fs::write(&md_path, "# Hello\n\nWorld content here.").unwrap();

    let ctx = IndexContext::from_path(&md_path).with_mode(IndexMode::Force);
    let result = engine.index(ctx).await.unwrap();

    assert_eq!(result.len(), 1);
    assert!(!result.has_failures());
    let doc_id = result.doc_id().unwrap();
    assert!(!doc_id.is_empty());

    // Verify persisted
    assert!(engine.exists(doc_id).await.unwrap());

    // List should contain 1 doc
    let docs = engine.list().await.unwrap();
    assert_eq!(docs.len(), 1);
    assert_eq!(docs[0].name, "test");

    // Remove
    assert!(engine.remove(doc_id).await.unwrap());
    assert!(!engine.exists(doc_id).await.unwrap());
}

#[tokio::test]
async fn test_index_from_content() {
    let (engine, _tmp) = setup().await;

    let ctx = IndexContext::from_content(
        "# Title\n\nParagraph 1\n\n## Section\n\nParagraph 2",
        vectorless::DocumentFormat::Markdown,
    )
    .with_name("inline-doc");

    let result = engine.index(ctx).await.unwrap();
    assert_eq!(result.len(), 1);
    let doc_id = result.doc_id().unwrap();

    // Verify it's persisted and loadable
    assert!(engine.exists(doc_id).await.unwrap());

    // Clean up
    engine.remove(doc_id).await.unwrap();
}

#[tokio::test]
async fn test_index_multiple_sources_parallel() {
    let (engine, tmp) = setup().await;

    // Create 3 markdown files
    let paths: Vec<PathBuf> = (0..3)
        .map(|i| {
            let p = tmp.path().join(format!("doc{i}.md"));
            std::fs::write(&p, format!("# Doc {i}\n\nContent {i}")).unwrap();
            p
        })
        .collect();

    let ctx = IndexContext::from_paths(paths).with_mode(IndexMode::Force);
    let result = engine.index(ctx).await.unwrap();

    assert_eq!(result.len(), 3);
    assert!(!result.has_failures());

    let docs = engine.list().await.unwrap();
    assert_eq!(docs.len(), 3);

    // Clear all
    let count = engine.clear().await.unwrap();
    assert_eq!(count, 3);
}

#[tokio::test]
async fn test_index_default_mode_skips_existing() {
    let (engine, tmp) = setup().await;

    let md_path = tmp.path().join("existing.md");
    std::fs::write(&md_path, "# Original\n\nOriginal content.").unwrap();

    // First index
    let ctx = IndexContext::from_path(&md_path);
    let result1 = engine.index(ctx).await.unwrap();
    assert_eq!(result1.len(), 1);
    let id1 = result1.doc_id().unwrap().to_string();

    // Second index with Default mode — should skip
    let ctx = IndexContext::from_path(&md_path);
    let result2 = engine.index(ctx).await.unwrap();
    assert_eq!(result2.len(), 1);
    assert!(!result2.has_failures());
    // Same doc ID — not re-indexed
    assert_eq!(result2.doc_id().unwrap(), id1);
}

#[tokio::test]
async fn test_force_mode_reindexes() {
    let (engine, tmp) = setup().await;

    let md_path = tmp.path().join("force.md");
    std::fs::write(&md_path, "# Version 1").unwrap();

    // First index
    let ctx = IndexContext::from_path(&md_path);
    let result1 = engine.index(ctx).await.unwrap();
    let id1 = result1.doc_id().unwrap().to_string();

    // Force re-index — should get a new doc ID
    let ctx = IndexContext::from_path(&md_path).with_mode(IndexMode::Force);
    let result2 = engine.index(ctx).await.unwrap();
    assert_eq!(result2.len(), 1);
    // Different doc ID — re-indexed
    assert_ne!(result2.doc_id().unwrap(), id1);
}

#[tokio::test]
async fn test_clear_empty_workspace() {
    let (engine, _tmp) = setup().await;

    let count = engine.clear().await.unwrap();
    assert_eq!(count, 0);
}

#[tokio::test]
async fn test_remove_nonexistent() {
    let (engine, _tmp) = setup().await;

    let removed = engine.remove("nonexistent-id").await.unwrap();
    assert!(!removed);
}

#[tokio::test]
async fn test_index_from_bytes() {
    let (engine, _tmp) = setup().await;

    let ctx = IndexContext::from_bytes(vec![1, 2, 3, 4], vectorless::DocumentFormat::Pdf)
        .with_name("test-bytes");

    // This will fail at parse (not a real PDF), but should error gracefully
    let result = engine.index(ctx).await;
    assert!(result.is_err());
}
