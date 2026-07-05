# huashu-design 与 design-md / hallmark 编排指南

## 三者关系

```
design-md（token 持久化层）
    ↕
hallmark（设计引擎 — 做什么样的设计）
    ↕
huashu-design（渲染引擎 — 怎么实现设计）
```

- **design-md**：`DESIGN.md` 契约管理，存储 color/typography/spacing 等设计 token
- **hallmark**：提供 20 个主题 × 21 种宏观结构 + 57 道 slop 门控 + 设计 DNA 提取
- **huashu-design**：HTML 原生渲染 + 动画/视频 + 品牌资产协议

## 场景 A：有品牌资产

1. `skill_route("design-md")` → `capture` 模式提取现有 token → 读取 `DESIGN.md`
2. 调用 huashu-design 或 hallmark，传入 design token 作为上下文
3. 产出后通过 `visual-review` 验证效果
4. 如需持久化新 token → `design-md update` 更新 `DESIGN.md`

## 场景 B：无品牌资产，从零设计

1. `skill_route("hallmark")` → 品牌优先流程 → 产出调色板 + 字体对标 + 主题选择
2. `skill_route("design-md")` → `capture` 将 hallmark 输出写入 `DESIGN.md`
3. `skill_route("huashu-design")` → 使用 `DESIGN.md` 中的 token 渲染原型/动画/PPT

## 场景 C：已有代码，需改造

1. `skill_route("hallmark")` → `hallmark study <URL/截图>` 提取设计 DNA
2. `skill_route("design-md")` → 将 DNA 输出为 `DESIGN.md`
3. `skill_route("huashu-design")` → 按新版 token 重新渲染

## 避坑

- hallmake 和 huashu 都自带设计系统能力，但 design-md 是**事实来源（source of truth）**
- 当用户已有 `DESIGN.md` 时，始终优先读取它，再决定调用哪个渲染skill
- hallmark 做"设计思辨"，huashu 做"技术产出"——两者不互相替代
