# vectorless-code：基于树遍历的代码搜索

## 1. 现有工具分析

### cocoindex-code

给 AI 编码助手用的**代码语义搜索引擎**。

```
源码 → 分块(~1000字符) → embedding向量 → sqlite-vec
查询 → query embedding → 余弦相似度 → top-k代码块
```

- 依赖：嵌入模型 + 向量数据库
- 搜索速度：~100ms
- 擅长：语义相似匹配（"login" 能匹配 "authenticate"）
- 不擅长：复杂推理查询（"认证流程怎么走"）

### codeindex

和 vectorless 思路相同的代码搜索工具（TypeScript 实现）。

```
源码 → 解析符号 → 构建树(Project>Module>File>Symbol) → LLM生成摘要
查询 → LLM逐层遍历(module→file→symbol, 3次调用) → 返回代码
```

- 依赖：LLM（无 embedding、无向量 DB）
- 索引速度：中（LLM 生成每层摘要）
- 搜索速度：~5-10s（3 次 LLM 调用）
- 擅长：精准定位（LLM 理解语义选择节点）
- 验证了"慢但准"的路线可行

---

## 2. vectorless-code 方案

### 核心思路

复用 vectorless 的编译管线 + 树结构，实现三层查询策略：

| 模式 | 方法 | 速度 | 覆盖场景 |
|---|---|---|---|
| **Fast** | ReasoningIndex 关键词匹配 | ~10ms | 精确查询（函数名、变量名） |
| **标准** | codeindex 式逐层遍历（3次LLM） | ~5s | 语义查询（"认证逻辑在哪"） |
| **Deep** | Worker Agent 推理导航 | ~30s | 复杂查询（"认证流程怎么走"） |

### 查询流程

```
查询 "authentication logic"
    │
    ├─ Step 1: 关键词匹配（~10ms）
    │   extract_keywords → 查 ReasoningIndex
    │   命中 → 返回节点，结束
    │
    ├─ Step 2: 逐层遍历（~5s, 3次LLM）
    │   Level 1: "这8个目录哪些相关？" → LLM 选 2-3 个
    │   Level 2: "这20个文件哪些相关？" → LLM 选 3-5 个
    │   Level 3: "这些代码块哪些相关？" → LLM 选 5-10 个
    │   → 返回，结束
    │
    └─ Step 3: Worker 推理（~30s, 6-15次LLM）
        完整 ls/cd/cat/find/grep 导航
        → 返回带溯源的证据
```

### 三个工具对比

| | cocoindex-code | codeindex | vectorless-code |
|---|---|---|---|
| **方法** | Embedding 向量搜索 | LLM 逐层遍历 | 关键词 + 逐层遍历 + Worker |
| **依赖** | 嵌入模型 + 向量DB | 仅 LLM | 仅 LLM（Fast 模式连 LLM 都不需要） |
| **索引** | 慢（算 embedding） | 中（LLM 生成摘要） | 快（Fast 编译 0 LLM） |
| **搜索速度** | ~100ms | ~5-10s | ~10ms / ~5s / ~30s |
| **语义理解** | 好（向量语义） | 好（LLM 理解） | 好（LLM 理解） |
| **深度查询** | 不支持 | 有限（3层遍历） | 支持（Worker 推理） |
| **精确匹配** | 一般（模糊） | 好（LLM 选择） | 好（关键词精确 + LLM 选择） |
| **跨语言** | 所有语言 | 9种（有语言适配器） | 所有语言（通用分块） |

### 架构

```
源码文件 (*.rs, *.py, *.ts, ...)
    │
    ▼
┌──────────────────────────────────────────┐
│  Code Parser（通用分块）                   │
│  file → Vec<RawNode>                      │
│  Level 0: 项目根                          │
│  Level 1: 文件（path 作为标题）            │
│  Level 2: 代码块（~50行/块，按结构分）      │
└──────────────────────────────────────────┘
    │
    ▼
┌──────────────────────────────────────────┐
│  Compile Pipeline                         │
│  Fast 模式: Build → Enrich → Reasoning    │
│  Standard 模式: + EnhancePass(生成摘要)    │
└──────────────────────────────────────────┘
    │
    ▼
┌──────────────────────────────────────────┐
│  Document IR（代码树）                     │
│  my-project/                              │
│  ├── src/                                 │
│  │   ├── auth.rs                          │
│  │   │   ├── auth.rs:1-48 (imports, ...)  │
│  │   │   └── auth.rs:49-96 (fn login)     │
│  │   ├── parser.rs                        │
│  │   │   └── parser.rs:1-55               │
│  │   └── engine.rs                        │
│  │       └── engine.rs:1-60               │
│  └── tests/                               │
│      └── integration.rs                   │
│          └── integration.rs:1-40          │
└──────────────────────────────────────────┘
    │
    ▼
┌──────────────────────────────────────────┐
│  查询（三层策略）                          │
│  Fast: 关键词 → ReasoningIndex → 节点     │
│  标准: LLM 逐层遍历 (3次调用)              │
│  Deep: Worker Agent 推理导航               │
└──────────────────────────────────────────┘
```

---

## 3. 工作分解

### vectorless 改动

#### 3.1 加 Code 格式（~5 个文件）

| 文件 | 改动 |
|---|---|
| `vectorless-document/src/format.rs` | 加 `Code` variant，映射 `.rs/.py/.ts/.go/.java/.cpp/...` |
| `vectorless-compiler/src/parse/code/` | 新模块：通用代码分块器 |
| `vectorless-compiler/src/parse/mod.rs` | 加 `DocumentFormat::Code =>` match arm |
| `vectorless-engine/src/indexer.rs` | 加 format 映射 |
| `vectorless-engine/src/engine.rs` | 加 pipeline options 映射 |

#### 3.2 通用代码分块器

语言无关的启发式分块：

```rust
fn parse_code(content: &str, file_path: &str) -> Vec<RawNode> {
    let mut nodes = vec![];

    // Level 1: 文件节点（标题 = 相对路径）
    nodes.push(RawNode {
        title: file_path.to_string(),
        level: 1,
        ..Default::default()
    });

    // Level 2: 代码块（按结构分块）
    for chunk in split_by_structure(content, max_lines=50) {
        nodes.push(RawNode {
            title: format!("{}:{}-{}", file_path, chunk.start, chunk.end),
            content: chunk.text,
            level: 2,
            ..Default::default()
        });
    }

    nodes
}
```

分块策略：空行优先 → 缩进变化 → 行数硬切（~50行）。

**可选增强**：tree-sitter 把 Level 2 从"代码块"升级为"函数/类/方法"（50+ 语言）。

#### 3.3 暴露关键词搜索 API

```rust
impl DocumentNavigator {
    /// 关键词检索，毫秒级
    pub fn search_by_keywords(&self, query: &str) -> Vec<SearchResult> {
        let keywords = extract_keywords(query);
        let mut scored: HashMap<NodeId, f32> = HashMap::new();
        for kw in &keywords {
            if let Some(entries) = self.reasoning_index.topic_paths.get(kw) {
                for entry in entries {
                    *scored.entry(entry.node_id).or_default() += entry.weight;
                }
            }
        }
        let mut results: Vec<_> = scored.into_iter()
            .map(|(node_id, score)| SearchResult { node_id, score })
            .collect();
        results.sort_by(|a, b| b.score.partial_cmp(&a.score).unwrap());
        results.truncate(10);
        results
    }
}
```

#### 3.4 逐层遍历查询（codeindex 式）

标准模式的查询策略，3 次 LLM 调用：

```python
async def traverse_search(tree, query, llm):
    # Level 1: 选目录
    modules = tree.children_of_root()
    selected = await llm.select_nodes(query, modules, max_select=3)

    # Level 2: 选文件
    files = [f for m in selected for f in tree.children_of(m)]
    selected = await llm.select_nodes(query, files, max_select=5)

    # Level 3: 选代码块
    chunks = [c for f in selected for c in tree.children_of(f)]
    selected = await llm.select_nodes(query, chunks, max_select=10)

    return selected
```

### vectorless-code 独立项目

```
vectorless-code/           # 新项目，独立仓库
├── pyproject.toml         # 依赖: vectorless, mcp, typer, pathspec
├── src/
│   └── vectorless_code/
│       ├── __init__.py
│       ├── indexer.py     # 遍历文件 → engine.compile(format="code")
│       ├── search.py      # 三层查询策略
│       ├── traversal.py   # 逐层遍历（标准模式）
│       ├── server.py      # MCP server（search tool）
│       ├── cli.py         # CLI: vc init / index / search / mcp
│       └── settings.py    # .gitignore, include/exclude 配置
└── README.md
```

**MCP 接口**：

```python
@mcp.tool()
async def search(query: str, limit: int = 5, mode: str = "auto") -> list[dict]:
    """Search codebase.

    mode: "fast" (keyword), "standard" (traversal), "deep" (worker), "auto"
    """
    if mode == "fast" or mode == "auto":
        results = doc.search_by_keywords(query)
        if results:
            return format_results(results[:limit])

    if mode == "standard" or mode == "auto":
        results = await traverse_search(tree, query, llm)
        if results:
            return format_results(results[:limit])

    # Deep mode
    answer = await engine.ask(query, doc_ids=[doc_id])
    return format_evidence(answer.evidence[:limit])
```

**CLI**：

| 命令 | 功能 |
|---|---|
| `vc init` | 初始化配置 |
| `vc index [--mode fast|standard]` | 编译代码库 |
| `vc search <query> [--mode auto|fast|standard|deep]` | 搜索代码 |
| `vc mcp` | 启动 MCP server |
| `vc status` | 查看索引状态 |

---

## 4. 不需要改的

- `DocumentTree` / arena 结构 — 完全复用
- `BuildPass` — `RawNode.level` 驱动，天然兼容
- `ReasoningIndex` — 关键词倒排索引，Fast 模式核心
- Worker 核心循环 — Deep 模式复用
- PyO3 绑定框架 — 增量添加新方法
- Engine / Workspace / Cache — 完全复用
- SplitPass — 自动处理超大代码文件
- 增量编译 — fingerprint + 增量更新已有

## 5. 优势

1. **无需嵌入模型** — 不需要向量 DB、不需要 embedding API、不需要 GPU
2. **三层速度** — 10ms / 5s / 30s，按需选择
3. **Fast 模式零 LLM** — 索引和查询都不需要 LLM（纯 CPU）
4. **深度查询** — Worker 模式处理 embedding 无法回答的复杂问题
5. **所有语言** — 通用分块器，不依赖 tree-sitter
6. **增量编译** — 代码变更只重编译改动的文件

## 6. 实施步骤

1. **vectorless 加 Code 格式** — 通用分块器 + 关键词搜索 API
2. **vectorless-code CLI** — `vc init / index / search`，验证三层查询
3. **逐层遍历实现** — 标准模式（3 次 LLM），对标 codeindex 效果
4. **vectorless-code MCP server** — 暴露 `search` tool，接入 Claude Code
5. **（可选）tree-sitter 增强** — 精确 AST 分块，替换通用分块器
