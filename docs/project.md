```shell
┌─────────────────────────────────────────────────────────────────┐
│                        Client API                                │
│  DocumentCollection::add(), query(), save(), load()             │
└─────────────────────────────────────────────────────────────────┘
                               │
                               ▼
┌─────────────────────────────────────────────────────────────────┐
│                      Orchestrator                                │
│  协调 Parser → Indexer → Summarizer → Retriever                 │
└─────────────────────────────────────────────────────────────────┘
        │              │              │              │
        ▼              ▼              ▼              ▼
┌──────────────┐ ┌──────────────┐ ┌──────────────┐ ┌──────────────┐
│   Parsers    │ │   Indexers   │ │  Summarizers │ │  Retrievers  │
│  (可插拔)     │ │  (可插拔)    │ │  (可插拔)    │ │  (可插拔)    │
├──────────────┤ ├──────────────┤ ├──────────────┤ ├──────────────┤
│ • Markdown   │ │ • TreeBuild  │ │ • LLM        │ │ • LLM-Navi   │
│ • PDF        │ │ • Thinning   │ │ • Extractive │ │ • MCTS       │
│ • HTML       │ │ • Merging    │ │ • Hybrid     │ │ • Beam       │
│ • DOCX       │ │              │ │              │ │ • Vector     │
└──────────────┘ └──────────────┘ └──────────────┘ └──────────────┘
                               │
                               ▼
┌─────────────────────────────────────────────────────────────────┐
│                        LLM Layer                                 │
│  ChatModel trait: OpenAI, ZAI, Anthropic, Mock                  │
└─────────────────────────────────────────────────────────────────┘


vectorless/
├── Cargo.toml
│
├── src/
│   ├── lib.rs                         # Crate 入口，导出公共 API
│   │
│   ├── core/                          # 核心抽象层
│   │   ├── mod.rs
│   │   ├── traits.rs                  # 核心 Traits 定义
│   │   │   ├── DocumentParser
│   │   │   ├── Retriever
│   │   │   └── Summarizer
│   │   ├── node.rs                    # PageNode 树节点定义
│   │   ├── tree.rs                    # PageTree 文档树定义
│   │   ├── types.rs                   # 公共类型定义
│   │   └── error.rs                   # 错误类型
│   │
│   ├── registry/                      # 注册中心
│   │   ├── mod.rs
│   │   ├── parser_registry.rs         # Parser 注册表
│   │   ├── retriever_registry.rs      # Retriever 注册表
│   │   └── summarizer_registry.rs     # Summarizer 注册表
│   │
│   ├── document/                      # 文档解析 (可插拔实现)
│   │   ├── mod.rs
│   │   ├── parser.rs                  # DocumentParser trait
│   │   ├── types.rs                   # 文档类型定义
│   │   ├── markdown.rs                # Markdown Parser
│   │   ├── pdf.rs                     # PDF Parser
│   │   ├── html.rs                    # HTML Parser (预留)
│   │   └── docx.rs                    # DOCX Parser (预留)
│   │
│   ├── indexer/                       # 索引构建
│   │   ├── mod.rs
│   │   ├── tree_builder.rs            # 树构建
│   │   ├── thinner.rs                 # 树精简
│   │   ├── merger.rs                  # 节点合并
│   │   ├── incremental.rs             # 增量更新 (预留)
│   │   └── config.rs                  # 索引配置
│   │
│   ├── summarizer/                    # 摘要生成 (可插拔实现)
│   │   ├── mod.rs
│   │   ├── summarizer.rs              # Summarizer trait
│   │   ├── llm_summary.rs             # LLM 生成摘要
│   │   ├── extractive.rs              # 抽取式摘要 (预留)
│   │   └── config.rs                  # 摘要配置
│   │
│   ├── retriever/                     # 检索策略 (可插拔实现)
│   │   ├── mod.rs
│   │   ├── retriever.rs               # Retriever trait
│   │   ├── llm_navigate.rs            # LLM 导航检索
│   │   ├── multi_doc.rs               # 多文档检索
│   │   ├── mcts.rs                    # Monte Carlo Tree Search (预留)
│   │   ├── beam_search.rs             # Beam Search (预留)
│   │   ├── hybrid.rs                  # 混合检索 (预留)
│   │   ├── context.rs                 # 上下文构建
│   │   └── config.rs                  # 检索配置
│   │
│   ├── ranking/                       # 结果排序
│   │   ├── mod.rs
│   │   ├── scorer.rs                  # 评分策略
│   │   ├── merger.rs                  # 合并去重
│   │   └── config.rs                  # 排序配置
│   │
│   ├── storage/                       # 存储层
│   │   ├── mod.rs
│   │   ├── workspace.rs               # 工作空间管理
│   │   ├── persistence.rs             # 持久化
│   │   └── cache.rs                   # 缓存
│   │
│   ├── client/                        # 高级客户端 API
│   │   ├── mod.rs
│   │   ├── vectorless.rs              # 主入口，协调所有组件
│   │   └── builder.rs                 # Builder 模式配置
│   │
│   └── utils/                         # 工具函数
│       ├── mod.rs
│       ├── token_count.rs             # Token 计数
│       └── text_process.rs            # 文本处理
│
├── tests/                             # 集成测试
│   ├── markdown_test.rs
│   ├── pdf_test.rs
│   ├── multi_doc_test.rs
│   └── retrieval_test.rs
│
├── benches/                           # 性能基准
│   ├── bench.rs
│   ├── indexing.rs
│   ├── retrieval.rs
│   └── multi_doc.rs
│
└── README.md                          # README
└── config.toml                        # config 文件
└── examples/                          # 使用示例
    ├── basic.rs
    ├── multi_document.rs
    └── custom_retriever.rs

```