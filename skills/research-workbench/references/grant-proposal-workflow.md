# Grant proposal lane（基金标书）

吸收 **Grant Writer** 精华；五宿主同一契约，无独立 `$grant-writer` 热入口（避免 skill 拓扑膨胀）。

## 何时启用

- 用户写 NSF/NIH/国自然/省部级标书、Specific Aims、研究计划、预算论证。
- 对象是可资助**项目方案**，不是已成型手稿（后者 → `$paper-workbench` 仅借叙事 discipline）。

## 结构模板（可裁剪）

```markdown
# Grant Proposal Card

- Agency / program:
- Deadline / page limits:
- Specific aims (3 max, each testable):
- Significance (why agency should care):
- Innovation (what changes vs state of practice):
- Approach (aim-by-aim: methods, milestones, go/no-go):
- Preliminary data (decisive figures only):
- Team / environment / resources:
- Data management / sharing (link data-availability-fair if relevant):
- Budget justification (personnel, equipment, travel):
- Risks & alternatives:
```

## Lane 顺序

1. **冻结 aims card**（一页）；未冻结不写正文。
2. **`experiment_design`**：每个 aim 的可检验终点、对照、样本量/功效 concerns → `$statistical-analysis` 按需。
3. **`reproducibility`**：预注册/数据计划 → `$experiment-reproducibility`。
4. **叙事润色**：仅边界内；不抬口径超过 preliminary data。
5. **审稿模拟**：用 NIH reviewer guidance 视角（`paper-workbench` review rubric 的 grant 口径）做 findings-only 预审，**不改** agency 模板硬性章节。

## 硬约束

- 不得编造 preliminary results 或合作者承诺。
- 与手稿 reuse 须披露（self-plagiarism 红线见 `citation-management` integrity）。
