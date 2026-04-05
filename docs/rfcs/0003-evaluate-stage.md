# RFC-0003: Evaluate Stage Naming

## Summary

Rename the `JudgeStage` to `EvaluateStage` to better reflect its purpose in the retrieval pipeline.

## Motivation

The term "judge" implies a binary verdict, while the stage actually:
1. Aggregates content from candidates
2. Evaluates sufficiency levels (Sufficient, Partial, Insufficient)
3. Can trigger additional search iterations
4. Builds the final response

"Evaluate" better captures the nuanced assessment process.

## Design

### Changes

| Before | After |
|--------|-------|
| `JudgeStage` | `EvaluateStage` |
| `judge.rs` | `evaluate.rs` |
| `judge_time_ms` | `evaluate_time_ms` |
| `"judge"` stage name | `"evaluate"` stage name |

### Preserved Names

The following are intentionally preserved:
- `LlmJudge` - The sufficiency checker that "judges" sufficiency
- `llm_judge` - Field name for the LLM-based sufficiency judge

These remain as they specifically make a judgment call on sufficiency.

## Pipeline Flow Update

```
Before: Analyze → Plan → Search → Judge
After:  Analyze → Plan → Search → Evaluate
```

## Implementation

1. Rename `src/retrieval/stages/judge.rs` to `evaluate.rs`
2. Update struct name from `JudgeStage` to `EvaluateStage`
3. Update all references in pipeline and retriever code
4. Update documentation and diagrams

## Status

**Implemented** - 2026-04-05
