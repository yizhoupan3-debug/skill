// TODO: HookStateVersion — 版本迁移 trait，待 ReviewGateState 和 CodexLifecycleContextState
// 的版本字段稳定后接入 maybe_migrate 逻辑。
/// Common trait for hook-state structs across all hosts.
#[allow(dead_code)]
pub trait HookStateVersion {
    const STATE_VERSION: u32;
    fn version(&self) -> u32;
}
