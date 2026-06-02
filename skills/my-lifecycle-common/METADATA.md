# my-lifecycle-common

非 skill 目录。存放 my-lifecycle 阶段（discussx / planx / implementx / verifyx）共用的契约与规范文档。

当前内容：
- `GOAL_STATE_CONTRACT.md` — GOAL_STATE.json 写入契约，禁止直写，所有变更须经 `framework_goal_drive` stdio。

本目录不在 SKILL_MANIFEST.json 中注册，不参与路由，仅供同层 skill 引用。
