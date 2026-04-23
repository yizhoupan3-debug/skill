from pathlib import Path


def test_refresh_skill_stays_available_for_codex_global_entry() -> None:
    skill_path = Path("skills/refresh/SKILL.md")
    content = skill_path.read_text(encoding="utf-8")

    assert skill_path.is_file()
    assert "name: refresh" in content
    assert "$refresh" in content
    assert 'PROJECT_DIR="${CLAUDE_PROJECT_DIR:-$(git rev-parse --show-toplevel 2>/dev/null || pwd)}"' in content
    assert (
        '"$PROJECT_DIR"/scripts/router-rs/target/release/router-rs --framework-refresh-json --claude-hook-max-lines 4 --repo-root "$PROJECT_DIR"'
        in content
    )
    assert (
        '"$PROJECT_DIR"/scripts/router-rs/target/debug/router-rs --framework-refresh-json --claude-hook-max-lines 4 --repo-root "$PROJECT_DIR"'
        in content
    )
    assert (
        'cargo run --manifest-path "$PROJECT_DIR"/scripts/router-rs/Cargo.toml --release -- --framework-refresh-json --claude-hook-max-lines 4 --repo-root "$PROJECT_DIR"'
        in content
    )
    assert "下一轮执行 prompt 已准备好，并且已经复制到剪贴板。" in content
    assert "--framework-refresh-verbose" in content
    assert "manual next-turn execution prompt" not in content
