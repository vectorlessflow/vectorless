# PDF TOC 处理模块设计

## 一、问题讨论

### 1.1 TOC 检测策略：正则 vs LLM

**采用混合策略**

```
┌─────────────────────────────────────────────────────────┐
│                    TOC 检测流程                          │
├─────────────────────────────────────────────────────────┤
│                                                         │
│   PDF 前N页文本                                         │
│        │                                                │
│        ▼                                                │
│   ┌─────────────┐                                       │
│   │ 正则匹配    │  典型模式:                            │
│   │ (快速筛选)  │  • "Chapter X ....... N"             │
│   │             │  • "1.1 标题 ... 15"                 │
│   │             │  • "目录 / Contents"                 │
│   └──────┬──────┘                                       │
│          │                                              │
│    ┌─────┴─────┐                                        │
│    │           │                                        │
│   明确        不确定                                     │
│    │           │                                        │
│    ▼           ▼                                        │
│  返回结果   ┌─────────────┐                             │
│            │ LLM 判断    │  只对前5-10页                │
│            │ (置信度低时)│  成本可控                    │
│            └──────┬──────┘                              │
│                   │                                     │
│                   ▼                                     │
│                 返回结果                                │
│                                                         │
└─────────────────────────────────────────────────────────┘
```

**理由**：
- 正则覆盖 70-80% 常见 TOC 格式，零成本
- LLM 作为 fallback，只处理正则不确定的情况
- 不需要所有情况都用 LLM

### 1.2 页码偏移处理

**采用锚点验证 + 偏移众数**

```
┌─────────────────────────────────────────────────────────┐
│                   页码偏移计算流程                        │
├─────────────────────────────────────────────────────────┤
│                                                         │
│   TOC 条目 (含页码)          PDF 物理页                  │
│        │                        │                       │
│        ▼                        ▼                       │
│   ┌─────────────────────────────────────┐              │
│   │ 选取 N 个锚点 (如第1、5、10章节)     │              │
│   └──────────────────┬──────────────────┘              │
│                      │                                  │
│                      ▼                                  │
│   ┌─────────────────────────────────────┐              │
│   │ LLM 验证：标题是否在该页出现         │              │
│   │ 得到: (TOC页码, 物理页码) 配对       │              │
│   └──────────────────┬──────────────────┘              │
│                      │                                  │
│                      ▼                                  │
│   ┌─────────────────────────────────────┐              │
│   │ 计算每个配对的偏移: offset = phy - toc│             │
│   │ 取众数 (mode) 作为最终偏移           │              │
│   └──────────────────┬──────────────────┘              │
│                      │                                  │
│                      ▼                                  │
│            全部 TOC 页码 + offset                       │
│                                                         │
└─────────────────────────────────────────────────────────┘
```

**特殊情况处理**：
- 前言、目录部分偏移可能不同（通常 TOC 页码不计入）
- 如果偏移方差过大 → 偏移不可靠，改用 LLM 逐个定位

### 1.3 无 TOC 的 PDF 处理

**需要支持，作为备选模式**

```
┌─────────────────────────────────────────────────────────┐
│                   处理模式选择                           │
├─────────────────────────────────────────────────────────┤
│                                                         │
│                    ┌───────────┐                        │
│                    │ 检测 TOC  │                        │
│                    └─────┬─────┘                        │
│                          │                              │
│              ┌───────────┼───────────┐                  │
│              │           │           │                  │
│            有TOC      有TOC        无TOC                │
│          有页码      无页码          │                  │
│              │           │           │                  │
│              ▼           ▼           ▼                  │
│       ┌──────────┐ ┌──────────┐ ┌──────────┐          │
│       │ 模式A    │ │ 模式B    │ │ 模式C    │          │
│       │ 偏移校正 │ │ LLM定位  │ │ LLM提取  │          │
│       └──────────┘ └──────────┘ └──────────┘          │
│                                                         │
│  优先级: A > B > C (成本和准确率考量)                    │
│                                                         │
└─────────────────────────────────────────────────────────┘
```

**模式说明**：
- **模式A (有TOC有页码)**：计算偏移后直接应用
- **模式B (有TOC无页码)**：LLM 逐个定位章节位置
- **模式C (无TOC)**：LLM 分块提取文档结构 (P2 优先级)

---

## 二、整体架构

### 2.1 架构图

```
┌─────────────────────────────────────────────────────────────────┐
│                        PdfParser                                 │
│  (现有 pdf.rs 重构)                                              │
└───────────────────────────────┬─────────────────────────────────┘
                                │
                                ▼
┌─────────────────────────────────────────────────────────────────┐
│                      TocProcessor                                │
│  (新增: src/document/toc/)                                      │
│                                                                 │
│  ┌──────────────────────────────────────────────────────────┐  │
│  │                      处理流水线                           │  │
│  │                                                          │  │
│  │   Pages ──▶ Detect ──▶ Extract ──▶ Parse ──▶ Assign    │  │
│  │               │           │          │          │        │  │
│  │            (正则+LLM)   (文本)    (LLM)    (偏移/LLM)   │  │
│  │                                                          │  │
│  └──────────────────────────────────────────────────────────┘  │
│                                │                                │
│                                ▼                                │
│                    ┌───────────────────┐                       │
│                    │   IndexVerifier   │                       │
│                    │   (验证+修复)      │                       │
│                    └─────────┬─────────┘                       │
│                              │                                 │
│                              ▼                                 │
│                      Vec<TocEntry>                             │
│                                                                 │
└─────────────────────────────────────────────────────────────────┘
```

### 2.2 模块结构

```
src/document/
├── mod.rs
├── types.rs              # 现有: RawNode, ParseResult 等
├── markdown.rs           # 现有
│
├── pdf/
│   ├── mod.rs
│   ├── parser.rs         # PDF 解析，输出 Vec<PdfPage>
│   └── page.rs           # PdfPage 类型定义
│
└── toc/
    ├── mod.rs            # 导出
    ├── types.rs          # TocEntry, TocDetection, VerificationReport
    │
    ├── detector.rs       # TocDetector (正则 + LLM fallback)
    ├── parser.rs         # TocParser (LLM 解析 TOC 文本)
    ├── assigner.rs       # PageAssigner (偏移计算 + LLM 定位)
    │
    ├── verifier.rs       # IndexVerifier (抽样验证)
    ├── repairer.rs       # IndexRepairer (错误修复)
    │
    └── processor.rs      # TocProcessor (流水线整合)
```

---

## 三、处理流程

### 3.1 完整流程图

```
                            ┌─────────────────┐
                            │   PDF 文件      │
                            └────────┬────────┘
                                     │
                                     ▼
                            ┌─────────────────┐
                            │  PdfParser      │
                            │  提取全部页面    │
                            └────────┬────────┘
                                     │
                                     ▼
                            ┌─────────────────┐
                            │  Vec<PdfPage>   │
                            │  (页码, 文本)    │
                            └────────┬────────┘
                                     │
                                     ▼
┌────────────────────────────────────────────────────────────────┐
│                        TocProcessor                             │
│                                                                │
│  ┌──────────────────────────────────────────────────────────┐ │
│  │ Step 1: TOC 检测                                          │ │
│  │                                                          │ │
│  │   前10页 ──▶ 正则匹配 ──▶ [确定?] ──▶ 返回               │ │
│  │                   │                                      │ │
│  │               [不确定]                                    │ │
│  │                   │                                      │ │
│  │                   ▼                                      │ │
│  │               LLM判断                                    │ │
│  │                                                          │ │
│  │   输出: TocDetection { found, pages, has_page_numbers } │ │
│  └──────────────────────────────────────────────────────────┘ │
│                         │                                     │
│              ┌──────────┴──────────┐                          │
│              │                     │                          │
│          [有TOC]               [无TOC]                        │
│              │                     │                          │
│              ▼                     ▼                          │
│  ┌─────────────────────┐  ┌─────────────────────┐            │
│  │ Step 2a: 提取TOC文本 │  │ Step 2b: LLM提取   │            │
│  │                     │  │        结构 (P2)    │            │
│  │ 合并TOC页面文本      │  │                     │            │
│  └──────────┬──────────┘  └──────────┬──────────┘            │
│             │                        │                        │
│             ▼                        │                        │
│  ┌─────────────────────┐             │                        │
│  │ Step 3: 解析TOC结构  │             │                        │
│  │                     │             │                        │
│  │ LLM → Vec<TocEntry> │             │                        │
│  │ (title, level,      │             │                        │
│  │  toc_page)          │             │                        │
│  └──────────┬──────────┘             │                        │
│             │                        │                        │
│             ▼                        │                        │
│  ┌─────────────────────┐             │                        │
│  │ Step 4: 分配物理页码 │             │                        │
│  │                     │             │                        │
│  │ [有页码]            │             │                        │
│  │   计算偏移 → 应用   │             │                        │
│  │                     │             │                        │
│  │ [无页码]            │             │                        │
│  │   LLM逐个定位       │             │                        │
│  └──────────┬──────────┘             │                        │
│             │                        │                        │
│             └──────────┬─────────────┘                        │
│                        │                                      │
│                        ▼                                      │
│             ┌─────────────────────┐                           │
│             │ Step 5: 验证 & 修复 │                           │
│             │                     │                           │
│             │ 抽样验证 ─▶ [错误?] │                           │
│             │      │              │                           │
│             │  [准确率OK]         │                           │
│             │      │              │                           │
│             │      ▼              │                           │
│             │    完成             │                           │
│             │                     │                           │
│             │ [错误多] ─▶ 修复 ──▶│                           │
│             │      │              │                           │
│             │      └──▶ 重验 ────▶│                           │
│             └──────────┬──────────┘                           │
│                        │                                      │
└────────────────────────┼──────────────────────────────────────┘
                         │
                         ▼
                ┌─────────────────┐
                │ Vec<TocEntry>   │
                │                 │
                │ title           │
                │ level           │
                │ physical_page   │
                │ confidence      │
                └─────────────────┘
                         │
                         ▼
                ┌─────────────────┐
                │ TreeBuilder     │
                │ (现有)          │
                │ TocEntry → 树   │
                └─────────────────┘
```

---

## 四、核心类型定义

### 4.1 类型概览

```
┌─────────────────────────────────────────────────────────────────┐
│                          核心类型                                │
├─────────────────────────────────────────────────────────────────┤
│                                                                 │
│  PdfPage                                                        │
│  ├── number: usize         // 页码 (1-based)                   │
│  ├── text: String          // 页面文本                         │
│  └── token_count: usize    // token 估算                       │
│                                                                 │
│  ─────────────────────────────────────────────────────────────  │
│                                                                 │
│  TocEntry                                                       │
│  ├── title: String              // 标题                        │
│  ├── level: usize               // 层级 (1, 2, 3...)           │
│  ├── toc_page: Option<usize>    // TOC中的页码                 │
│  ├── physical_page: Option<usize> // 实际物理页码              │
│  └── confidence: f32            // 置信度                       │
│                                                                 │
│  ─────────────────────────────────────────────────────────────  │
│                                                                 │
│  TocDetection                                                   │
│  ├── found: bool                // 是否存在TOC                 │
│  ├── pages: Vec<usize>          // TOC所在页码                 │
│  ├── has_page_numbers: bool     // TOC是否含页码               │
│  └── confidence: f32            // 检测置信度                   │
│                                                                 │
│  ─────────────────────────────────────────────────────────────  │
│                                                                 │
│  VerificationReport                                             │
│  ├── total: usize               // 检查总数                    │
│  ├── correct: usize             // 正确数                      │
│  ├── errors: Vec<Error>         // 错误列表                    │
│  └── accuracy: f32              // 准确率                       │
│                                                                 │
└─────────────────────────────────────────────────────────────────┘
```

### 4.2 类型详细说明

#### PdfPage

```rust
/// PDF 单页内容
pub struct PdfPage {
    /// 页码 (1-based)
    pub number: usize,
    /// 页面文本内容
    pub text: String,
    /// Token 估算值
    pub token_count: usize,
}
```

#### TocEntry

```rust
/// TOC 条目
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TocEntry {
    /// 标题文本
    pub title: String,
    /// 层级深度 (1 = 顶级章节, 2 = 子章节, ...)
    pub level: usize,
    /// TOC 中标注的页码 (可能存在偏移)
    pub toc_page: Option<usize>,
    /// 实际物理页码 (验证/分配后)
    pub physical_page: Option<usize>,
    /// 置信度 (0.0 - 1.0)
    pub confidence: f32,
}
```

#### TocDetection

```rust
/// TOC 检测结果
#[derive(Debug, Clone)]
pub struct TocDetection {
    /// 是否检测到 TOC
    pub found: bool,
    /// TOC 所在的页码列表
    pub pages: Vec<usize>,
    /// TOC 中是否包含页码信息
    pub has_page_numbers: bool,
    /// 检测置信度
    pub confidence: f32,
}
```

#### VerificationReport

```rust
/// 验证报告
#[derive(Debug, Clone)]
pub struct VerificationReport {
    /// 验证的条目总数
    pub total: usize,
    /// 正确的条目数
    pub correct: usize,
    /// 准确率 (0.0 - 1.0)
    pub accuracy: f32,
    /// 错误列表
    pub errors: Vec<VerificationError>,
}

/// 单个验证错误
#[derive(Debug, Clone)]
pub struct VerificationError {
    /// 条目索引
    pub index: usize,
    /// 标题
    pub title: String,
    /// 预期页码
    pub expected_page: usize,
    /// 错误类型
    pub error_type: ErrorType,
}

/// 错误类型
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ErrorType {
    /// 标题未在指定页出现
    TitleNotFound,
    /// 标题出现但不在页首
    NotAtPageStart,
    /// 页码超出文档范围
    PageOutOfRange,
}
```

---

## 五、组件职责

### 5.1 组件列表

```
┌─────────────────────────────────────────────────────────────────┐
│  组件                   │  职责                                 │
├─────────────────────────────────────────────────────────────────┤
│                                                                 │
│  PdfParser              │  输入PDF → 输出Vec<PdfPage>          │
│                         │  (使用 lopdf 分页提取)               │
│                                                                 │
│  TocDetector            │  输入Pages → 输出TocDetection        │
│                         │  (正则优先，LLM fallback)            │
│                                                                 │
│  TocParser              │  输入TOC文本 → 输出Vec<TocEntry>     │
│                         │  (LLM解析)                           │
│                                                                 │
│  PageAssigner           │  输入Entries+Pages → 分配physical    │
│                         │  (偏移计算 或 LLM定位)               │
│                                                                 │
│  IndexVerifier          │  输入Entries+Pages → Verification    │
│                         │  (抽样LLM验证)                       │
│                                                                 │
│  IndexRepairer          │  输入Errors → 修复Entries            │
│                         │  (LLM重新定位)                       │
│                                                                 │
│  TocProcessor           │  整合上述组件为流水线                │
│                         │  (对外统一接口)                      │
│                                                                 │
└─────────────────────────────────────────────────────────────────┘
```

### 5.2 组件详细说明

#### PdfParser

- **输入**: PDF 文件路径
- **输出**: `Vec<PdfPage>`
- **职责**: 使用 lopdf 库分页提取 PDF 文本
- **实现要点**:
  - 处理 PDF 编码问题
  - 保留页码信息
  - 估算每页 token 数量

#### TocDetector

- **输入**: `&[PdfPage]`
- **输出**: `TocDetection`
- **职责**: 检测 PDF 中是否存在 TOC
- **策略**:
  1. 正则匹配典型 TOC 模式 (快速)
  2. 置信度不足时调用 LLM 判断
- **正则模式示例**:
  - `Chapter\s+\d+.*?\d+`
  - `\d+\.\d+\s+.+?\.\s*\d+`
  - `^目录|Contents|Table of Contents`

#### TocParser

- **输入**: TOC 文本 (字符串)
- **输出**: `Vec<TocEntry>`
- **职责**: 将 TOC 文本解析为结构化条目
- **实现**: LLM 解析，输出 JSON 格式

#### PageAssigner

- **输入**: `&mut [TocEntry]`, `&[PdfPage]`
- **输出**: 为每个 entry 分配 `physical_page`
- **策略**:
  - **有页码**: 计算偏移 → 应用偏移
  - **无页码**: LLM 逐个定位

#### IndexVerifier

- **输入**: `&[TocEntry]`, `&[PdfPage]`
- **输出**: `VerificationReport`
- **职责**: 抽样验证条目页码是否正确
- **方法**: LLM 检查标题是否在指定页出现

#### IndexRepairer

- **输入**: `&mut [TocEntry]`, `&[VerificationError]`, `&[PdfPage]`
- **输出**: 修复后的条目
- **职责**: 修复验证失败的条目
- **方法**: LLM 在错误页附近重新定位

#### TocProcessor

- **输入**: `&[PdfPage]`
- **输出**: `Vec<TocEntry>`
- **职责**: 整合所有组件为完整流水线
- **对外接口**: 统一的 `process()` 方法

---

## 六、对外接口

### 6.1 极简接口设计

只有 `TocProcessor` 对外暴露，其他组件为内部实现。

```rust
impl TocProcessor {
    /// 创建默认配置的处理器
    pub fn new() -> Self;
    
    /// 使用自定义配置创建处理器
    pub fn with_config(config: TocProcessorConfig) -> Self;
    
    /// 处理 PDF 页面，返回结构化 TOC
    pub async fn process(&self, pages: &[PdfPage]) -> Result<Vec<TocEntry>>;
}
```

### 6.2 使用示例

```rust
use vectorless::document::pdf::PdfParser;
use vectorless::document::toc::{TocProcessor, TocEntry};

async fn example() -> Result<()> {
    // 1. 解析 PDF
    let parser = PdfParser::new();
    let pages = parser.parse_file("document.pdf").await?;
    
    // 2. 提取 TOC
    let processor = TocProcessor::new();
    let entries = processor.process(&pages).await?;
    
    // 3. 打印结果
    for entry in &entries {
        let indent = "  ".repeat(entry.level - 1);
        println!("{}{} - Page {:?}", indent, entry.title, entry.physical_page);
    }
    
    // 4. 转换为 DocumentTree (使用现有 TreeBuilder)
    let tree = TreeBuilder::new()
        .with_root_title("Document")
        .build_from_toc(entries);
    
    Ok(())
}
```

---

## 七、实现优先级

### 7.1 优先级矩阵

| 优先级 | 组件 | 说明 | 预估工时 |
|--------|------|------|----------|
| P0 | PdfParser (分页) | 基础能力，使用 lopdf | 1天 |
| P0 | TocDetector (正则) | 覆盖 70% 场景 | 0.5天 |
| P0 | TocParser (LLM) | 核心解析能力 | 1天 |
| P0 | PageAssigner (偏移) | 有页码 TOC 的处理 | 0.5天 |
| P1 | TocDetector (LLM fallback) | 提升覆盖率 | 0.5天 |
| P1 | IndexVerifier | 质量保证 | 1天 |
| P1 | PageAssigner (LLM定位) | 无页码 TOC 的处理 | 1天 |
| P2 | IndexRepairer | 错误修复 | 1天 |
| P2 | 无TOC结构提取 (模式C) | LLM 直接提取结构 | 2天 |

### 7.2 实现阶段

**阶段一 (P0)**: 基础能力 - 3天
- PDF 分页解析
- TOC 正则检测
- TOC LLM 解析
- 页码偏移计算

**阶段二 (P1)**: 质量提升 - 2.5天
- LLM 检测 fallback
- 索引验证
- 无页码 TOC 定位

**阶段三 (P2)**: 完整功能 - 3天
- 错误修复
- 无 TOC 结构提取

---

## 八、依赖项

### 8.1 新增依赖

```toml
[dependencies]
# PDF 处理
lopdf = "0.33"           # PDF 解析 (替代 pdf-extract)
pdf-extract = "0.7"      # 保留作为备选

# LLM 调用 (复用现有)
async-openai = "0.34"

# 工具
regex = "1.10"           # 正则匹配 (已有)
serde_json = "1.0"       # JSON 解析 (已有)
```

### 8.2 复用现有模块

- `config::LlmConfig` - LLM 配置 (需新建)
- `core::Error` - 错误类型
- `indexer::TreeBuilder` - 树构建器

---

## 九、测试策略

### 9.1 单元测试

- 每个组件独立测试
- Mock LLM 响应
- 正则模式覆盖测试

### 9.2 集成测试

- 端到端流程测试
- 使用真实 PDF 文件
- 准确率基准测试

### 9.3 测试用例

```
tests/
├── toc/
│   ├── test_detector.rs      # 检测器测试
│   ├── test_parser.rs        # 解析器测试
│   ├── test_assigner.rs      # 分配器测试
│   ├── test_verifier.rs      # 验证器测试
│   └── test_processor.rs     # 整体流程测试
│
└── fixtures/
    ├── with_toc_numbers.pdf      # 有 TOC 有页码
    ├── with_toc_no_numbers.pdf   # 有 TOC 无页码
    └── without_toc.pdf           # 无 TOC
```

---

## 十、风险与缓解

| 风险 | 影响 | 缓解措施 |
|------|------|----------|
| LLM 解析不稳定 | 中 | 多次重试 + 完整性检查 |
| 正则覆盖率不足 | 低 | LLM fallback |
| 页码偏移不一致 | 中 | 众数统计 + 方差检测 |
| PDF 编码问题 | 低 | 多种解码尝试 |

---

## 十一、后续扩展

- 并发处理 (Phase 2)
- 分布式部署支持
- 更多文档格式 (HTML, DOCX)
- 缓存 LLM 结果
