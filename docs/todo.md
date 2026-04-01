# Vectorless TODO

## Future Enhancements

### Storage

- [ ] **Sled storage backend for large-scale deployments**
  - **When**: Document count > 10,000
  - **Why**: Better performance, ACID transactions, concurrent access
  - **How**: 
    1. Add `sled` dependency
    2. Create `SledWorkspace` trait implementation
    3. Add storage backend selection in config: `storage.backend = "sled"`
  - **Trade-offs**:
    | Aspect | JSON Files | Sled |
    |--------|-----------|------|
    | Readability | ✅ Human-readable | ❌ Binary |
    | Dependencies | ✅ None | ❌ Extra crate |
    | Performance | Good for <10K docs | Better for >10K |
    | Concurrency | File locking needed | ✅ Built-in |
    | ACID | ❌ No | ✅ Yes |

### Retrievers

- [ ] **Implement BeamSearch retriever**
- [ ] **Implement MCTS retriever**
- [ ] **Implement MultiDoc retriever**
- [ ] **Implement Hybrid retriever**

### Document Formats

- [ ] **PDF parsing** (currently returns "not implemented")
- [ ] **HTML parsing** (currently returns "not implemented")
- [ ] **DOCX parsing** (currently returns "not implemented")
