import json
import re
import subprocess
from collections import Counter
from datetime import datetime, timezone
from pathlib import Path
from typing import Any, Dict, List, Optional

class EvolutionEngine:
    """
    Evolution Engine: Optimization and Performance Analytics for Codex Skills.
    Handles health manifests, automated audits, and skill healing.
    """
    def __init__(self, journal_path: str):
        self.journal_path = Path(journal_path)
        self.entries: List[Dict[str, Any]] = []
        self._load_journal()

    def _load_journal(self):
        if not self.journal_path.exists():
            return
        with open(self.journal_path, "r", encoding="utf-8") as f:
            for line in f:
                if line.strip():
                    try:
                        self.entries.append(json.loads(line))
                    except json.JSONDecodeError:
                        continue

    def record_decision(self, task: str, init: str, final: str, conf: float,
                        reroute: bool, struggle: int = 0, reason: str = "",
                        failed_trigger: str = "", notes: str = ""):
        """Record a single routing decision to the log."""
        entry = {
            "ts": datetime.now(timezone.utc).isoformat(),
            "task": task,
            "init": init,
            "final": final,
            "conf": conf,
            "reroute": reroute,
            "struggle": struggle,
            "reason": reason,
            "failed_trigger": failed_trigger,
            "notes": notes
        }
        with open(self.journal_path, "a", encoding="utf-8") as f:
            f.write(json.dumps(entry, ensure_ascii=False) + "\n")
        self.entries.append(entry)

    def audit(self, days: int = 30) -> Dict[str, Any]:
        """Audit the journal using the Rust core for performance."""
        rs_bin = Path(__file__).parent / "evolution-rs" / "target" / "release" / "evolution-rs"
        manifest_path = self.journal_path.parent / "SKILL_MANIFEST.json"

        if rs_bin.exists():
            try:
                cmd = [str(rs_bin), "audit", "--journal", str(self.journal_path), "--days", str(days), "--json"]
                if manifest_path.exists():
                    cmd.extend(["--manifest", str(manifest_path)])

                result = subprocess.run(cmd, capture_output=True, text=True, check=True)
                return json.loads(result.stdout)
            except Exception as e:
                return {"error": f"Rust core failed: {e}", "total_decisions": len(self.entries)}

        return {
            "total_decisions": len(self.entries),
            "reroute_count": len([e for e in self.entries if e.get("reroute")]),
            "message": "Rust core missing, basic stats only.",
            "new_skill_candidates": [],
            "repair_suggestions": []
        }

    def sync(self):
        """Sync journal to feedback table using Rust core."""
        rs_bin = Path(__file__).parent / "evolution-rs" / "target" / "release" / "evolution-rs"
        feedback_path = self.journal_path.parent / ".routing_feedback.md"

        if rs_bin.exists():
            try:
                subprocess.run(
                    [str(rs_bin), "sync", "--journal", str(self.journal_path), "--feedback", str(feedback_path)],
                    check=True
                )
            except Exception:
                pass

    def heal(self):
        """Invoke the Rust Auto-Heal engine."""
        rs_bin = Path(__file__).parent / "evolution-rs" / "target" / "release" / "evolution-rs"
        manifest_path = self.journal_path.parent / "SKILL_MANIFEST.json"
        skills_root = self.journal_path.parent

        if rs_bin.exists():
            try:
                subprocess.run(
                    [str(rs_bin), "heal", "--journal", str(self.journal_path),
                     "--manifest", str(manifest_path), "--skills_root", str(skills_root)],
                    check=True
                )
                print("Auto-Heal completed successfully.")
            except Exception as e:
                print(f"Heal failed: {e}")

    def generate_health_manifest(self, scores_json: Optional[Path] = None) -> Dict[str, Any]:
        """Generate a health manifest using the Rust core for maximum performance."""
        rs_bin = Path(__file__).parent / "evolution-rs" / "target" / "release" / "evolution-rs"
        manifest_path = self.journal_path.parent / "SKILL_MANIFEST.json"

        if rs_bin.exists():
            try:
                cmd = [str(rs_bin), "manifest", "--journal", str(self.journal_path)]
                if scores_json:
                    cmd.extend(["--scores", str(scores_json)])
                if manifest_path.exists():
                    cmd.extend(["--manifest", str(manifest_path)])

                result = subprocess.run(cmd, capture_output=True, text=True, check=True)
                return json.loads(result.stdout)
            except Exception as e:
                return {"error": f"Rust manifest generation failed: {e}"}

        return {"error": "Rust core missing, cannot generate blended manifest."}

if __name__ == "__main__":
    import argparse
    parser = argparse.ArgumentParser(description="Skill Evolution Engine Driver")
    subparsers = parser.add_subparsers(dest="cmd")

    parser_audit = subparsers.add_parser("audit")
    parser_audit.add_argument("--days", type=int, default=30)
    parser_audit.add_argument("--json", action="store_true")

    parser_sync = subparsers.add_parser("sync")

    parser_health = subparsers.add_parser("health")
    parser_health.add_argument("--scores", type=str, help="Path to scores.json")

    args = parser.parse_args()
    engine = EvolutionEngine("skills/.evolution_journal.jsonl")

    if args.cmd == "audit":
        report = engine.audit(args.days)
        if args.json:
            print(json.dumps(report, indent=2, ensure_ascii=False))
        else:
            print(f"Evolution Audit (Last {args.days} days) - Driver: Rust-Powered")
            print("=" * 40)
            print(f"Total Decisions: {report.get('total_decisions', 0)}")
            print(f"Reroutes: {report.get('reroute_count', 0)}")

            if report.get("missed_opportunities"):
                print("\n💡 Missed Opportunities (R12):")
                for m in report["missed_opportunities"]:
                    print(f"  • {m}")

            if report.get("boundary_collisions"):
                print("\n🚨 Boundary Collisions (R14):")
                for c in report["boundary_collisions"]:
                    print(f"  • {c}")

            if report.get("new_skill_candidates"):
                print("\n🌊 Potential New Skills (Candidates):")
                for c in report["new_skill_candidates"]:
                    print(f"  • Candidate: {c['suggested_name']} ({c['count']}x)")

    elif args.cmd == "sync":
        engine.sync()
        print("Feedback table synced via Rust core.")

    elif args.cmd == "heal":
        engine.heal()

    elif args.cmd == "health":
        scores_path = Path(args.scores) if args.scores else None
        manifest = engine.generate_health_manifest(scores_path)
        print(json.dumps(manifest, indent=2))
