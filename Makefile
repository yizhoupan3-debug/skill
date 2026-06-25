# Dev iteration helpers — 重构期避免全 workspace 构建
# Usage: make check-runtime  (cargo check 仅 runtime 相关 crate)

# 重构期常用 crate 集（按需增删）
RUNTIME_CRATES = \
	-p runtime-core \
	-p runtime-core-contracts \
	-p runtime-exit-gate \
	-p runtime-infra \
	-p runtime-storage

.PHONY: check check-runtime test-runtime check-all

# 快速检查：修改 tool_handlers / stdio_dispatch 等高频变更区域时用
check-runtime:
	cargo check $(RUNTIME_CRATES)

# 快速测试：修改逻辑后验证
test-runtime:
	cargo test $(RUNTIME_CRATES)

# 重构期最终验收 / 提交前（全量检查）
check-all:
	cargo check

check:
	cargo check
