#!/usr/bin/env python3
"""Automated Laziness Auditor for Codex/Antigravity turns."""

import sys
import re

LAZY_PATTERNS = {
    "truncation": r"\.\.\.|// (remains unchanged|same|insert|keep existing)",
    "lazy_phrasing": r"\b(should be|probably|might|maybe|I think|is likely|hopefully|attempt to)\b",
    "context_begging": r"\b(please check|can you check|verify if|let me know if)\b",
    "passive_finish": r"\b(it works now|fix applied|should work|ought to work)\b(?!.*stdout)",
    "complexity_dodging": r"\b(simplified|basic version|placeholder|skeleton|todo)\b",
}

def check_constraint_crossing(text: str, ref_text: str):
    """Check if critical constraints from reference are missing in text."""
    # Heuristic: Check for file paths and specific 'MUST' keywords
    ref_paths = re.findall(r"/[a-zA-Z0-9/_.-]+", ref_text)
    findings = []
    
    for path in set(ref_paths):
        if path not in text:
            # Check if it was justified (e.g. 'instead of', 'removed')
            if not re.search(rf"(removed|deleted|instead of|replaced).*{path}", text, re.IGNORECASE):
                findings.append(f"[CONSTRAINT_MISSING] Reference mentions '{path}' but it's missing in output.")
                
    return findings

def audit_text(text: str, ref_text: str = None):
    score = 0
    findings = []
    
    for category, pattern in LAZY_PATTERNS.items():
        matches = re.findall(pattern, text, re.IGNORECASE)
        if matches:
            score += len(matches)
            # Handle potential tuple groups if they exist
            match_str = matches[0] if isinstance(matches[0], str) else matches[0][0]
            findings.append(f"[{category.upper()}] detected {len(matches)} times: {match_str}")
            
    if ref_text:
        x_findings = check_constraint_crossing(text, ref_text)
        score += len(x_findings) * 2  # Missing constraints are penalized more
        findings.extend(x_findings)
            
    return score, findings

def main():
    import argparse
    parser = argparse.ArgumentParser(description="Automated Laziness Auditor for Codex/Antigravity turns.")
    parser.add_argument("file", help="File to audit")
    parser.add_argument("--ref", help="Reference file (e.g. original prompt or previous turn)")
    args = parser.parse_args()
        
    with open(args.file, 'r') as f:
        content = f.read()
        
    ref_content = None
    if args.ref:
        with open(args.ref, 'r') as f:
            ref_content = f.read()
        
    score, findings = audit_text(content, ref_content)
    
    print(f"--- Laziness Audit Report (PUA v2.2.0) ---")
    print(f"Total Score: {score} (Lower is better)")
    if score > 0:
        print("Findings:")
        for f in findings:
            print(f" - {f}")
        sys.exit(1)
    else:
        print("✅ Clean: No laziness patterns detected.")
        sys.exit(0)

if __name__ == "__main__":
    main()
