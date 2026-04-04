// Copyright (c) 2026 vectorless developers
// SPDX-License-Identifier: Apache-2.0

//! Index pipeline benchmarks.
//!
//! Measures performance of document indexing:
//! - Parsing speed (Markdown, PDF, DOCX)
//! - Tree building
//! - Summary generation (LLM calls)
//! - End-to-end indexing time

use criterion::{black_box, criterion_group, criterion_main, BenchmarkId, Criterion, Throughput};

// TODO: Implement actual benchmarks once the API is stable
//
// use vectorless::client::{Engine, EngineBuilder};
// use vectorless::parser::{MarkdownParser, DocumentParser};

fn bench_markdown_parsing(c: &mut Criterion) {
    let mut group = c.benchmark_group("markdown_parsing");

    // TODO: Create test documents of different sizes
    // let small_doc = generate_markdown(100);    // 100 lines
    // let medium_doc = generate_markdown(500);   // 500 lines
    // let large_doc = generate_markdown(2000);   // 2000 lines

    // TODO: Benchmark parsing
    // group.bench_with_input(BenchmarkId::new("parse", "small"), &small_doc, |b, doc| {
    //     b.iter(|| {
    //         let parser = MarkdownParser::new();
    //         black_box(parser.parse(doc))
    //     })
    // });

    // Placeholder benchmark
    group.bench_function("parse_placeholder", |b| {
        b.iter(|| black_box(1 + 1))
    });

    group.finish();
}

fn bench_tree_building(c: &mut Criterion) {
    let mut group = c.benchmark_group("tree_building");

    // TODO: Benchmark tree construction from parsed content
    // - Node creation
    // - Hierarchy building
    // - Metadata assignment

    group.bench_function("build_placeholder", |b| {
        b.iter(|| black_box(1 + 1))
    });

    group.finish();
}

fn bench_toc_extraction(c: &mut Criterion) {
    let mut group = c.benchmark_group("toc_extraction");

    // TODO: Benchmark ToC extraction
    // - Heading detection
    // - Hierarchy inference
    // - Section boundary detection

    group.bench_function("toc_placeholder", |b| {
        b.iter(|| black_box(1 + 1))
    });

    group.finish();
}

fn bench_full_index_pipeline(c: &mut Criterion) {
    let mut group = c.benchmark_group("full_index");

    // TODO: Benchmark complete indexing pipeline
    // - Parse → Build → Enhance → Enrich → Optimize
    // - With and without LLM summarization
    // - Different document sizes

    group.bench_function("full_pipeline_placeholder", |b| {
        b.iter(|| black_box(1 + 1))
    });

    group.finish();
}

// TODO: Add helper functions for generating test documents
//
// fn generate_markdown(lines: usize) -> String {
//     // Generate markdown with headings, paragraphs, code blocks
// }
//
// fn generate_pdf(pages: usize) -> Vec<u8> {
//     // Generate PDF content
// }

criterion_group!(
    benches,
    bench_markdown_parsing,
    bench_tree_building,
    bench_toc_extraction,
    bench_full_index_pipeline,
);

criterion_main!(benches);
