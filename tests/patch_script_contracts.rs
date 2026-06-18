//! Contract tests for Claude maintenance shell scripts.

use std::path::PathBuf;
use std::process::Command;

fn framework_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

#[test]
fn patch_scripts_pass_bash_syntax_check() {
    {
        let name = "install-claude.sh";
        let path = framework_root().join("scripts").join(name);
        let status = Command::new("bash")
            .args(["-n", path.to_str().unwrap()])
            .status()
            .expect("bash -n");
        assert!(status.success(), "bash -n failed for {name}");
    }
}

