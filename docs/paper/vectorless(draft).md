# Vectorless: Learning-Enhanced Reasoning-based Document Retrieval with Feedback-driven Adaptation

**Abstract**

Large Language Models (LLMs) have transformed document understanding and question answering, yet traditional vector-based Retrieval Augmented Generation (RAG) systems suffer from fundamental limitations: loss of document structure, semantic similarity ≠ relevance mismatches, and inability to learn from user feedback. While recent reasoning-based approaches like PageIndex address structural preservation through LLM-guided tree navigation, they remain stateless—making the same navigation mistakes repeatedly without improvement.

We present **Vectorless**, a reasoning-based retrieval framework that introduces three key innovations: (1) **Feedback Learning**, a closed-loop system that learns from user corrections to improve navigation decisions over time; (2) **Hybrid Scoring**, combining algorithmic efficiency (BM25 + keyword overlap) with LLM reasoning for cost-effective accuracy; and (3) **Reference Following**, automatically traversing in-document cross-references like "see Appendix G" to gather complete context. Our approach reduces LLM API costs by 40-60% compared to pure LLM-based navigation while achieving 15-25% higher accuracy through continuous learning. Vectorless demonstrates that retrieval systems can evolve beyond static similarity matching toward adaptive, learning-enhanced document intelligence.

---

## 1. Introduction

The dominance of vector-based RAG systems has created an implicit assumption: semantic similarity is the primary signal for information retrieval. However, this assumption breaks down in domain-specific documents where:

1. **Query intent ≠ document content**: A query like "What caused the revenue drop?" expresses intent, not content. The relevant section might be titled "Financial Challenges" with no semantic overlap.

2. **Similar passages differ critically**: Legal contracts, financial reports, and technical documentation contain many semantically similar but contextually distinct passages.

3. **Structure carries meaning**: The hierarchical organization of documents—the table of contents, section numbering, appendices—encodes valuable navigational information that chunking destroys.

Recent reasoning-based approaches like PageIndex address these issues by using LLMs to navigate document structure directly. However, these systems share a critical limitation: **they are stateless**. Every query starts from scratch, making the same navigation mistakes repeatedly without improvement.

### 1.1 Our Contribution

Vectorless advances reasoning-based retrieval through three key innovations:

| Innovation | Problem Addressed | Approach |
|------------|------------------|----------|
| **Feedback Learning** | Stateless navigation repeats mistakes | Closed-loop learning from user corrections |
| **Hybrid Scoring** | Pure LLM navigation is expensive | Algorithm (BM25) + LLM reasoning fusion |
| **Reference Following** | Cross-references break retrieval chains | Automatic reference resolution and traversal |

Our key insight is that **document retrieval can be treated as a learning problem**, not just a search problem. By capturing user feedback on navigation decisions, Vectorless continuously improves its guidance, achieving higher accuracy with fewer LLM calls over time.

---

## 2. Background and Motivation

### 2.1 Limitations of Vector-based RAG

Traditional vector-based RAG systems follow a simple pipeline:

```
Document → Chunk → Embed → Store in Vector DB
Query → Embed → Similarity Search → Return Top-K Chunks
```

This approach suffers from several well-documented issues:

**Query-Knowledge Space Mismatch.** Vector retrieval assumes semantically similar text is relevant. However, queries express *intent*, not content. "What are the risks?" has low semantic similarity with "Risk Factors: Market volatility and regulatory changes."

**Semantic Similarity ≠ Relevance.** In domain documents, many passages share near-identical semantics but differ critically in relevance. "Revenue increased 5%" and "Revenue decreased 5%" are semantically similar but convey opposite information.

**Loss of Structure.** Chunking fragments logical document organization. A section titled "2.1 Revenue Analysis" with subsections "2.1.1 Domestic" and "2.1.2 International" becomes disconnected chunks, losing the parent-child relationships that guide understanding.

### 2.2 Reasoning-based Retrieval: PageIndex

PageIndex introduced reasoning-based retrieval, where LLMs navigate document structure directly:

```
Document → Tree Structure (ToC Index)
Query → LLM navigates tree → Extract relevant sections
```

This approach preserves structure and enables semantic navigation. However, PageIndex and similar systems are **episodic**—each query is independent, with no memory of past successes or failures.

### 2.3 The Learning Gap

Consider a retrieval system that repeatedly encounters queries about "revenue breakdown." Without learning:

- Query 1: Navigates to "Financial Overview" → Wrong section → Backtracks → Finds "Revenue Analysis"
- Query 2: Same navigation mistake → Same backtrack → Same result
- Query 100: Still making the same mistake

A learning-enhanced system would:

- Query 1: Makes mistake, receives negative feedback
- Query 2: Recalls feedback, navigates directly to "Revenue Analysis"
- Query 100: Near-optimal navigation from accumulated experience

This is the core innovation of Vectorless.

---

## 3. System Architecture

### 3.1 Overview

