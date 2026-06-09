# 文档工程指南（Document Engineering Guide）

> **subagent 执行文档工程 lane 时必须遵循的完整 checklist**

本文件从 `documentation-engineering` skill 提取核心方法论，供 implementx subagent 在文档相关 lane 中参考。

---

## 适用范围

### 适用场景

- 写或改进 README、contributing guide、onboarding document
- API 文档（JSDoc、docstrings、rustdoc、TypeDoc、Swagger/OpenAPI docs）
- 架构决策记录（ADRs）
- Changelog、release notes、version history
- 文档完整性或时效性审计
- 文档生成工具链配置（TypeDoc、Sphinx、rustdoc、Storybook、Docusaurus）
- 典型请求：
  - "帮我写个 README"
  - "补一下这些函数的 JSDoc/docstring"
  - "帮我写个 ADR 记录这次架构决策"
  - "生成 changelog"
  - "这个项目的文档有哪些缺失"
  - "搭一下 TypeDoc/Sphinx 文档生成"

### 不适用

- 学术论文写作 → 使用 `$paper-workbench`
- `.docx` Word 文档编辑 → 使用 `$doc`
- `SKILL.md` 或 skill 文档 → 使用 `$skill-framework-developer`
- 代码注释风格强制（非文档注释）→ 使用当前 code review 或 implementation context
- API 接口设计（非其文档）→ 使用当前 implementation context
- PDF 生成或操作 → 使用 `$pdf`

---

## 五步 Workflow

### 1. Assess（评估）

- 识别现有文档及其结构
- 确定文档受众（users、contributors、maintainers、API consumers）
- 列出缺失项：README sections、API docs、ADRs、changelog、setup guide
- 检查是否已配置文档生成工具链

### 2. Structure（结构设计）

- 设计文档信息架构
- 为重复出现的文档类型定义统一模板（ADR 模板、API doc 模板）
- 建立命名和组织规范
- 规划文档间交叉引用

### 3. Write（撰写）

- 按目标受众撰写各文档段落
- 使用具体示例、代码片段和图表
- 遵循项目已有的语调和术语
- 确保 API docs 与实际代码签名同步

### 4. Automate（自动化）

- 如适用，配置文档生成工具链
- 如项目支持，将文档生成加入 CI/CD
- 如适用，配置 conventional commits 自动 changelog 生成

### 5. Verify（验证）

- 检查所有内部链接和交叉引用
- 验证代码示例可编译/可运行
- 确保 API docs 匹配当前代码
- 按文档完整性检查清单审查

---

## 文档完整性检查清单

```markdown
- README: ✅ / ❌ [缺失 sections]
- API docs: ✅ / ❌
- ADRs: ✅ / ❌
- Changelog: ✅ / ❌
- Onboarding guide: ✅ / ❌
- CONTRIBUTING: ✅ / ❌
- CODE_OF_CONDUCT: ✅ / ❌
```

### 输出模板

```markdown
## Documentation Summary
- Scope: [what was documented]
- Audience: [who this serves]

## Documents Created / Updated
1. [document] — [status: new / updated / audited]

## Completeness Check
- README: ✅ / ❌ [missing sections]
- API docs: ✅ / ❌
- ADRs: ✅ / ❌
- Changelog: ✅ / ❌
- Onboarding guide: ✅ / ❌

## Follow-up
- ...
```

---

## ADR 写作规范

### 模板

```markdown
# ADR-NNN: <标题>

## 状态
Proposed | Accepted | Deprecated | Superseded by ADR-NNN

## 背景
<描述促使此决策的上下文和问题>

## 决策
<描述做出的具体决策>

## 后果
<描述此决策的正面和负面影响>
```

### 硬约束

- ADR 一旦 accepted 即不可修改；如需变更，创建新 ADR 标记 Supersedes
- ADR 编号连续递增，不跳号
- 每个 ADR 聚焦单一决策

---

## README 模板

```markdown
# Project Name

> 一句话描述项目目的

## 功能特性
- Feature 1
- Feature 2

## 快速开始

### 安装
\```bash
# 安装命令
\```

### 使用
\```bash
# 基本用法示例
\```

## 开发

### 前置要求
- 依赖 1
- 依赖 2

### 本地开发
\```bash
# 开发环境搭建命令
\```

### 测试
\```bash
# 测试命令
\```

## 项目结构
\```
├── src/        # 源码
├── tests/      # 测试
└── docs/       # 文档
\```

## API 文档
参见 [API docs](./docs/api.md)

## Contributing
参见 [CONTRIBUTING.md](./CONTRIBUTING.md)

## License
<license type>
```

---

## API 文档模板

### JSDoc / TypeDoc 风格

```javascript
/**
 * 计算两个数的和。
 *
 * @param {number} a - 第一个加数
 * @param {number} b - 第二个加数
 * @returns {number} 两数之和
 * @throws {TypeError} 参数非数字时抛出
 * @example
 * add(1, 2) // => 3
 */
function add(a, b) { ... }
```

### Python docstring 风格（Google style）

```python
def add(a: int, b: int) -> int:
    """计算两个数的和。

    Args:
        a: 第一个加数。
        b: 第二个加数。

    Returns:
        两数之和。

    Raises:
        TypeError: 参数非数字时抛出。

    Examples:
        >>> add(1, 2)
        3
    """
```

### rustdoc 风格

```rust
/// 计算两个数的和。
///
/// # Arguments
///
/// * `a` - 第一个加数
/// * `b` - 第二个加数
///
/// # Returns
///
/// 两数之和。
///
/// # Examples
///
/// ```
/// let result = add(1, 2);
/// assert_eq!(result, 3);
/// ```
fn add(a: i32, b: i32) -> i32 { ... }
```

---

## 文档生成工具链选型

| 工具 | 适用语言 | 输出格式 | 特点 |
|------|---------|---------|------|
| **TypeDoc** | TypeScript | HTML / Markdown | 从 TS 类型注释自动提取，支持模块索引 |
| **Sphinx** | Python（主要） | HTML / PDF / ePub | 强大的交叉引用，Read the Docs 集成 |
| **rustdoc** | Rust | HTML | Rust 官方工具，与 `cargo doc` 集成 |
| **Docusaurus** | 通用 | 静态网站 | Meta 出品，支持 MDX、版本管理、搜索 |
| **Storybook** | 前端组件 | 交互式文档 | 组件 playground + 文档一体化 |
| **Swagger / OpenAPI** | REST API | 交互式 UI | API 设计优先，自动生成客户端 SDK |

### 选型建议

- **纯 TypeScript 项目** → TypeDoc
- **Python 项目** → Sphinx + autodoc
- **Rust 项目** → rustdoc（无需额外配置）
- **多语言或需要品牌化文档站** → Docusaurus
- **前端组件库** → Storybook（组件文档）+ Docusaurus（指南文档）
- **REST API** → OpenAPI spec + Swagger UI 或 Redoc

---

## 硬约束

- 文档必须匹配实际代码行为，而非期望行为
- 示例代码尽可能保持可运行和可测试
- 不跨文档重复信息；使用交叉引用
- 如项目已有文档规范，遵循现有规范
- 标记任何与当前代码库矛盾的文档
- ADR 一旦 accepted 即不可变；创建新 ADR 来取代旧的
- 对生产级文档，参照 `SKILL_FRAMEWORK_PROTOCOLS.md §4` 的 verification gate 标准进行质量审查

---

## 职责边界

### 本指南覆盖

- README、CONTRIBUTING、CODE_OF_CONDUCT、onboarding guides
- API 文档（JSDoc、TypeDoc、docstrings、rustdoc、Swagger UI）
- 架构决策记录（ADRs）
- Changelog 和 release notes（conventional commits、keep-a-changelog）
- 文档完整性审计
- 文档生成工具链配置（Sphinx、TypeDoc、rustdoc、Docusaurus、Storybook docs）
- 内联代码文档策略和标准

### 不覆盖

- 学术论文
- Office 文档格式化
- Skill 元数据文件
- 超越文档注解的代码风格强制
