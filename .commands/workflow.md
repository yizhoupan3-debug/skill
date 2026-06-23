# /workflow（已废弃 → 使用 team 编排）

**Workflow（JS 编排）已彻底移除**。所有多 agent 协作统一使用 team 模型。

替代方案：
- 使用 `session-supervisor` team API 创建团队
- Agent 间通过 `team_send_message` / `team_read_messages` 通信
- Agent 生命周期通过 `agent_register` / `agent_unregister` 跟踪
- 详见 `skills/agent-swarm-orchestration/SKILL.md`

迁移示例：
```
session-supervisor op team_create team_id="research" name="研究团队"
session-supervisor op team_add_member team_id="research" agent_id="searcher" role="search" host_id="claude"
session-supervisor op team_send_message team_id="research" from_agent="supervisor" to_agent="searcher"
```
