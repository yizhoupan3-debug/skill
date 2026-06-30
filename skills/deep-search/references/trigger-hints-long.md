# deep-search 中长尾 trigger hints

> Front matter (`SKILL.md` frontmatter `trigger_hints`) 只保留高信号词；
> 下列 hints 供路由扩展层使用。
>
> **核心信号：** 用户要的是「查信息/找资料/做汇总」，不是学术调研或手稿。

## 网络检索类

- 帮我查一下
- 网上搜一下
- 查资料
- 搜索信息
- 找资料
- 在线查询
- 查查这个
- 有没有这个相关的信息
- 帮我搜索一下
- 搜索结果
- 网页搜索
- 搜一下
- 上网查
- 查查

## 事实核查类

- 这个说法对不对
- 验证一下这个信息
- 这个数据是真的吗
- 有没有证据支持
- 交叉验证一下
- claim verification
- fact check this
- is this true
- source verification

## 信息汇总类

- 做个信息汇总
- 整理一下相关内容
- 汇总报告
- 信息整理
- 搜索并汇总
- research report
- investigation report
- information synthesis

## 行业/产品调研

- 竞品分析
- market research
- competitive analysis
- technology trends
- 这个行业的前景
- 哪个产品好
- 产品对比
- 优缺点对比

## 维护说明

- frontmatter `trigger_hints` 是**主入口**（高信号、易误触发的反向避免过严）。
- `trigger_hints_long` 是**长尾扩展**（按场景二次分发）。
- 路由层实现面参考 `skills/SKILL_ROUTING_RUNTIME.json` + `configs/framework/RUNTIME_REGISTRY.json`。
