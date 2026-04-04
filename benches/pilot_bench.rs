// Copyright (c) 2026 vectorless developers
// SPDX-License-Identifier: Apache-2.0

//! Pilot module benchmarks.
//!
//! Measures performance of Pilot (the brain of retrieval):
//! - Intervention decision overhead
//! - Context building
//! - LLM call latency (mocked)
//! - Response parsing
//! - Score merging
//! - Fallback handling

use criterion::{black_box, criterion_group, criterion_main, BenchmarkId, Criterion};

fn bench_intervention_decision(c: &mut Criterion) {
    let mut group = c.benchmark_group("intervention_decision");

    // TODO: Benchmark should_intervene() decision
    // - START point decision
    // - FORK point decision
    // - BACKTRACK point decision
    // - EVALUATE point decision

    // This should be very fast (< 1µs) as it's called frequently

    group.bench_function("should_intervene_placeholder", |b| {
        b.iter(|| black_box(1 + 1))
    });

    group.finish();
}

fn bench_context_building(c: &mut Criterion) {
    let mut group = c.benchmark_group("context_building");

    // TODO: Benchmark ContextBuilder
    // - Token budget allocation
    // - Path context building
    // - Candidate context building
    // - Sibling context building

    // Test different context sizes:
    // - Small: 1-2 candidates, short path
    // - Medium: 3-5 candidates, medium path
    // - Large: 10+ candidates, long path

    group.bench_function("build_context_placeholder", |b| {
        b.iter(|| black_box(1 + 1))
    });

    group.finish();
}

fn bench_response_parsing(c: &mut Criterion) {
    let mut group = c.benchmark_group("response_parsing");

    // TODO: Benchmark ResponseParser
    // - JSON parsing
    // - Regex fallback extraction
    // - Default decision generation

    // let json_response = r#"{"candidates": [...], "direction": "...", "confidence": 0.9}"#;
    //
    // group.bench_with_input("json_parse", json_response, |b, response| {
    //     b.iter(|| {
    //         black_box(ResponseParser::parse(response))
    //     })
    // });

    group.bench_function("parse_json_placeholder", |b| {
        b.iter(|| black_box(1 + 1))
    });

    group.bench_function("parse_regex_placeholder", |b| {
        b.iter(|| black_box(1 + 1))
    });

    group.finish();
}

fn bench_score_merging(c: &mut Criterion) {
    let mut group = c.benchmark_group("score_merging");

    // TODO: Benchmark score merging
    // - final = α × algo + β × llm
    // - Different weight configurations
    // - Batch merging (multiple candidates)

    group.bench_function("merge_scores_placeholder", |b| {
        b.iter(|| black_box(1 + 1))
    });

    group.finish();
}

fn bench_budget_controller(c: &mut Criterion) {
    let mut group = c.benchmark_group("budget_controller");

    // TODO: Benchmark BudgetController
    // - can_call() check
    // - record_usage() update
    // - estimate_cost() calculation
    // - Thread-safe operations

    group.bench_function("budget_check_placeholder", |b| {
        b.iter(|| black_box(1 + 1))
    });

    group.bench_function("budget_record_placeholder", |b| {
        b.iter(|| black_box(1 + 1))
    });

    group.finish();
}

fn bench_fallback_manager(c: &mut Criterion) {
    let mut group = c.benchmark_group("fallback_manager");

    // TODO: Benchmark FallbackManager
    // - Level escalation
    // - Level de-escalation
    // - Retry delay calculation
    // - Action determination

    group.bench_function("fallback_record_placeholder", |b| {
        b.iter(|| black_box(1 + 1))
    });

    group.finish();
}

fn bench_metrics_collector(c: &mut Criterion) {
    let mut group = c.benchmark_group("metrics_collector");

    // TODO: Benchmark MetricsCollector
    // - record_call() with atomic operations
    // - snapshot() generation
    // - Percentile calculation

    group.bench_function("metrics_record_placeholder", |b| {
        b.iter(|| black_box(1 + 1))
    });

    group.bench_function("metrics_snapshot_placeholder", |b| {
        b.iter(|| black_box(1 + 1))
    });

    group.finish();
}

fn bench_full_pilot_decision(c: &mut Criterion) {
    let mut group = c.benchmark_group("full_pilot_decision");

    // TODO: Benchmark complete Pilot.decide() flow
    // - should_intervene check
    // - Context building
    // - LLM call (mocked or skipped)
    // - Response parsing
    // - Decision construction

    // Compare:
    // - With LLM call (real latency)
    // - Without LLM call (algorithm only)
    // - With cached response

    group.bench_function("full_decide_placeholder", |b| {
        b.iter(|| black_box(1 + 1))
    });

    group.finish();
}

// TODO: Add helper functions
//
// fn create_mock_search_state() -> SearchState {
//     // Create mock state for benchmarking
// }
//
// fn create_mock_tree() -> DocumentTree {
//     // Create mock tree for benchmarking
// }

criterion_group!(
    benches,
    bench_intervention_decision,
    bench_context_building,
    bench_response_parsing,
    bench_score_merging,
    bench_budget_controller,
    bench_fallback_manager,
    bench_metrics_collector,
    bench_full_pilot_decision,
);

criterion_main!(benches);
