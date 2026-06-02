/// Common trait for hook-state structs across all hosts.
#[allow(dead_code)]
pub(crate) trait HookStateVersion {
    const STATE_VERSION: u32;
    fn version(&self) -> u32;
}
