from pathlib import Path


def test_gitx_skill_exposes_codex_shortcut_and_closeout_flow() -> None:
    skill_path = Path("skills/gitx/SKILL.md")
    content = skill_path.read_text(encoding="utf-8")

    assert "name: gitx" in content
    assert "$gitx" in content
    assert "review、修复、整理、提交、合并 worktree、推送" in content
    assert "python3 scripts/git_safety.py doctor" in content
    assert "python3 scripts/git_safety.py publish-plan" in content
    assert "python3 scripts/git_safety.py auto-closeout" in content
    assert "python3 scripts/git_safety.py verify-batch" in content
    assert "python3 scripts/git_safety.py list-verify-presets" in content
    assert "python3 scripts/git_safety.py suggest-verify-presets" in content
    assert "--verify-cmd" in content
    assert "--verify-preset" in content
    assert "--use-suggested-verify-presets" in content
    assert "rust-router" in content
    assert "browser-runtime" in content
    assert "label / priority / cwd" in content
    assert "RTK" in content
    assert "python3 scripts/git_safety.py status" in content
    assert "git worktree list --porcelain" in content
