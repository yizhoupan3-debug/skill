/// Goal-drive stdio wrapper: ensures bootstrap + delegates.
pub fn framework_goal_drive(payload: serde_json::Value) -> Result<serde_json::Value, String> {
    crate::kernel_bootstrap::ensure_kernel_bootstrap();
    core_state::state_manager::framework_goal_drive(payload).map_err(|e| e.to_string())
}
