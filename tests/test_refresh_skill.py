from pathlib import Path


def test_refresh_skill_stays_available_for_codex_global_entry() -> None:
    skill_path = Path("skills/refresh/SKILL.md")
    content = skill_path.read_text(encoding="utf-8")

    assert skill_path.is_file()
    assert "name: refresh" in content
    assert "$refresh" in content
    assert "cargo run --quiet --manifest-path scripts/router-rs/Cargo.toml -- --framework-recap-json" in content
    assert "下一轮执行 prompt 已准备好，并且已经复制到剪贴板。" in content
