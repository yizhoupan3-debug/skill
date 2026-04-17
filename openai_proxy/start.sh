#!/bin/bash

# 获取脚本所在目录
DIR="$( cd "$( dirname "${BASH_SOURCE[0]}" )" && pwd )"
cd "$DIR"

echo "正在启动 OpenAI Plus 代理服务..."
echo "API 地址: http://localhost:8317/v1"
echo "管理页面: http://localhost:8317/management.html"

# 确保 auths 目录存在
mkdir -p auths

# 启动服务器
cliproxyapi server --config config.yaml
