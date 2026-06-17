# deep-research 中长尾 trigger hints

> Front matter (`SKILL.md` frontmatter `trigger_hints`) 只保留高信号词；
> 下列 hints 供路由扩展层使用。

## 主题研究类

- 深度研究一下这个话题
- 帮我全面调查一下
- 关于 XX 的详细报告
- 做一个关于 XX 的调研
- 网络调研一下
- 全面分析一下这个问题
- 帮我查一下这方面的资料
- 这方面的最新进展是什么
- 有没有可靠的信息来源
-帮我做一个事实核查

## 事实核查类

- 这个说法对不对
- 验证一下这个信息
- 这个数据是真的吗
- 有没有证据支持
- 交叉验证一下
- claim verification
- fact check this
- is this true that
- source verification

## 报告生成类

- 写一份研究报告
- 生成调研报告
- 做一个信息汇总
- 整理一下相关资料
- 综合分析报告
- research report
- investigation report
- information synthesis

## 行业/市场调研

- 这个行业的情况怎么样
- 市场调研
- 竞品分析
- 技术趋势
- 行业报告
- market research
- competitive analysis
- technology trends

## 维护说明

- frontmatter `trigger_hints` 是**主入口**（高信号、易误触发的反向避免过严）。
- `trigger_hints_long: references/trigger-hints-long.md` 是**长尾扩展**（按场景二次分发）。
- 路由层实现面参考 `skills/SKILL_ROUTING_RUNTIME.json` + `configs/framework/RUNTIME_REGISTRY.json`。
