#!/usr/bin/env python3
"""Fix crate:: imports for the split crates.

For each crate, self-references (crate::SELF_MODULE::) stay as crate::.
External references (crate::OTHER_MODULE::) become vectorless_other::.
Also handles bare `crate::Error` -> `vectorless_error::Error`.
"""
import os
import re
import sys

# Mapping: crate_dir -> (self_module, [external_deps])
CRATES = {
    "vectorless-error": ("error", []),
    "vectorless-document": ("document", []),
    "vectorless-config": ("config", []),
    "vectorless-utils": ("utils", ["error", "document"]),
    "vectorless-scoring": ("scoring", []),
    "vectorless-graph": ("graph", ["document"]),
    "vectorless-events": ("events", ["error", "document"]),
    "vectorless-metrics": ("metrics", ["config", "error"]),
    "vectorless-llm": ("llm", ["config", "error", "metrics", "utils"]),
    "vectorless-storage": ("storage", ["config", "document", "error", "utils"]),
    "vectorless-query": ("query", ["error", "llm", "scoring"]),
    "vectorless-index": ("index", ["config", "document", "error", "llm", "metrics", "scoring", "storage", "utils"]),
    "vectorless-agent": ("agent", ["document", "error", "llm", "query", "scoring"]),
    "vectorless-retrieval": ("retrieval", ["agent", "document", "error", "llm", "query", "storage", "utils"]),
    "vectorless-rerank": ("rerank", ["agent", "error", "query"]),
    "vectorless-engine": ("client", ["agent", "config", "document", "error", "events", "index", "llm", "metrics", "retrieval", "rerank", "storage"]),
}

MODULE_TO_CRATE = {
    "error": "vectorless_error",
    "document": "vectorless_document",
    "config": "vectorless_config",
    "utils": "vectorless_utils",
    "scoring": "vectorless_scoring",
    "graph": "vectorless_graph",
    "events": "vectorless_events",
    "metrics": "vectorless_metrics",
    "llm": "vectorless_llm",
    "storage": "vectorless_storage",
    "query": "vectorless_query",
    "index": "vectorless_index",
    "agent": "vectorless_agent",
    "retrieval": "vectorless_retrieval",
    "rerank": "vectorless_rerank",
    "client": "vectorless_engine",
}

BASE = "/home/ztgx/Desktop/vectorless/vectorless-core"

def fix_file(filepath, self_module):
    with open(filepath, 'r') as f:
        content = f.read()

    original = content

    # Replace crate::OTHER_MODULE:: with vectorless_other::
    # But keep crate::SELF_MODULE:: as crate::SELF_MODULE::
    for module, crate_name in MODULE_TO_CRATE.items():
        if module == self_module:
            continue
        # Match crate::module:: (with word boundary to avoid partial matches)
        pattern = r'crate::' + re.escape(module) + r'::'
        replacement = crate_name + '::'
        content = re.sub(pattern, replacement, content)

    # Replace bare crate::Error (without any module prefix) with vectorless_error::Error
    # But only if self_module is not "error"
    if self_module != "error":
        # Match "crate::Error" that isn't followed by :: (i.e., not crate::error::)
        content = re.sub(r'crate::Error(?!::)', 'vectorless_error::Error', content)
        # Match "crate::Result" -> "vectorless_error::Result"
        content = re.sub(r'crate::Result(?!::)', 'vectorless_error::Result', content)

    if content != original:
        with open(filepath, 'w') as f:
            f.write(content)
        return True
    return False

changed_files = 0
for crate_dir, (self_module, deps) in CRATES.items():
    src_dir = os.path.join(BASE, crate_dir, "src")
    if not os.path.isdir(src_dir):
        continue
    for root, dirs, files in os.walk(src_dir):
        for fname in files:
            if fname.endswith('.rs'):
                fpath = os.path.join(root, fname)
                if fix_file(fpath, self_module):
                    changed_files += 1
                    print(f"  Fixed: {os.path.relpath(fpath, BASE)}")

print(f"\nTotal files changed: {changed_files}")
