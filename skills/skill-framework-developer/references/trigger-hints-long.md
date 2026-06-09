# skill-framework-developer 中长尾 trigger hints

> Front door 只保留 ≤15 高信号词。下列供路由扩展 / 同义词索引 / 离线检索使用。

## 沉到底部的长尾短语

下列短语从 `SKILL.md` frontmatter `trigger_hints` 下沉到此处，原因是它们**同义/低信息密度/容易误触发**，更适合作为"人工维护时的备忘录"，而不是 runtime 首轮路由的主触发面。

如需重新上收，优先要求：
- 该短语能稳定区分"框架治理/路由诊断"与"单个 skill 内容编辑/新建 skill"的请求
- 该短语在真实对话中出现频率高，且不会把大量无关任务误路由到本 owner

### 路由系统同义/变体

- skill框架
- framework review
- 路由 review
- routing framework
- 路由没触发
- 路由诊断（已上收；保留备忘）

### 抽象 / 减法视角

- 不必要抽象
- 减法视角
- 第一性原理
- 减少 token 消耗
- 行为驱动
- 沉到 runtime
- runtime 轻量化
- 兼容层
- 胶水层
- 多余入口
- 减少入口
- 减入口
- 不损害功能

### skill 库维护 / 规范

- skill 维护
- skill 核查
- skill 精简
- 旧口径清理
- contract 清理
- 历史文件清理
- skill 合并
- 写一个 skill
- 批量规范 skill
- validate skills
- sync health checks
- registry drift cleanup
- skill library maintenance
- owner gate overlay（已标准化为 `owner / gate / overlay`）

### 反馈驱动 / 持续优化

- skill 不好用
- skill不好用
- 科研 skill 不好用
- 写作 skill 不好用
- 持续优化 skill
- 外部调研优化 skill
