# Browser MCP Skeleton

一个可直接运行的 **browser tool API skeleton**，目标是验证：

- agent-friendly tool surface
- 压缩后的 page state
- 可恢复的错误模型
- 不依赖外部 `mcp` SDK 也能本地迭代

## 当前实现范围

- `browser_open`
- `browser_tabs`
- `browser_close`
- `browser_get_state`
- `browser_get_elements`
- `browser_click`
- `browser_fill`
- `browser_wait_for`

默认 backend 是一个 **deterministic in-memory runtime**，用于：

- 本地联调
- schema 迭代
- agent 调用路径测试
- 错误恢复测试

## 运行

```bash
python3 -m scripts.browser_mcp
```

然后按 **一行一个 JSON-RPC 对象** 发请求：

```json
{"jsonrpc":"2.0","id":1,"method":"initialize","params":{}}
{"jsonrpc":"2.0","id":2,"method":"tools/list","params":{}}
{"jsonrpc":"2.0","id":3,"method":"tools/call","params":{"name":"browser_open","arguments":{"url":"https://example.com/login"}}}
```

## 示例登录流

1. `browser_open("https://example.com/login")`
2. `browser_get_state(include=["summary","interactive_elements"])`
3. `browser_fill(ref="el_email", value="user@example.com")`
4. `browser_fill(ref="el_password", value="secret")`
5. `browser_click(ref="el_signin")`
6. `browser_wait_for(condition={"type":"url_contains","value":"/dashboard"})`

## 下一步扩展

- 接入真实 Playwright backend
- 增加 `browser_get_network`
- 增加局部文本与 screenshot 工具
- 增加 page diff 压缩策略与 stale-ref 重定位
