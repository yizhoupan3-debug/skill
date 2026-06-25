use std::path::Path;

/// 运行时层 review gate CLI 接口。
///
/// 转发到 host-projection 中注册的宿主特定处理器。
/// 注册在 `register_host_projection_hooks()` 中完成。
pub fn run_review_gate(event: &str, cli_repo_root: Option<&Path>) -> Result<(), String> {
    Ok(host_projection::hooks::run_review_gate(event, cli_repo_root)?)
}
