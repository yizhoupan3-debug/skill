from pathlib import Path


def test_gitx_skill_exposes_codex_shortcut_and_closeout_flow() -> None:
    skill_path = Path("skills/gitx/SKILL.md")
    content = skill_path.read_text(encoding="utf-8")

    assert "name: gitx" in content
    assert "$gitx" in content
    assert "review、修复、整理、提交、合并 worktree、推送" in content
    assert "git status --short --branch" in content
    assert "git worktree list --porcelain" in content
    assert "git diff --stat" in content
    assert "不要依赖已移除的 Python git helper" in content
    assert "RTK" in content
    assert "git worktree list --porcelain" in content
