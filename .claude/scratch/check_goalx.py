import json, subprocess

# Check what goalx commit 2c9dcd9f added
result = subprocess.run(["git", "show", "2c9dcd9f:skills/goalx/SKILL.md"], capture_output=True, text=True)
if result.returncode == 0:
    import re
    m = re.search(r'^kind:\s*(.+)$', result.stdout, re.M)
    print(f"goalx SKILL.md kind: {m.group(1) if m else 'NOT FOUND'}")

result = subprocess.run(["git", "show", "2c9dcd9f:skills/SKILL_ROUTING_RUNTIME.json"], capture_output=True, text=True)
if result.returncode == 0:
    data = json.loads(result.stdout)
    for s in data['skills']:
        if isinstance(s, list) and s[0] == 'goalx':
            ki = data['keys'].index('kind')
            print(f"goalx routing kind: {s[ki] if ki < len(s) else 'NOT FOUND'}")
