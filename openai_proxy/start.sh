#!/bin/bash

# 获取脚本所在目录
DIR="$( cd "$( dirname "${BASH_SOURCE[0]}" )" && pwd )"
cd "$DIR"

echo "正在启动 OpenAI Plus 代理服务..."
echo "API 地址: http://localhost:8317/v1"
echo "管理页面: http://localhost:8317/management.html"

: "${OPENAI_PROXY_API_KEY:?请先设置 OPENAI_PROXY_API_KEY，不要把代理访问 key 写进 config.yaml}"

# 确保 auths 目录存在
mkdir -p auths

runtime_config="$(mktemp "${TMPDIR:-/tmp}/openai_proxy_config.XXXXXX.yaml")"
trap 'rm -f "$runtime_config"' EXIT
awk -v key="$OPENAI_PROXY_API_KEY" '{ gsub("__OPENAI_PROXY_API_KEY__", key); print }' config.yaml > "$runtime_config"

# 启动服务器
cliproxyapi server --config "$runtime_config"
