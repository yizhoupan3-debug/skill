import json, subprocess, sys

# Get commits that touched SKILL_ROUTING_RUNTIME.json
result = subprocess.run(
    ["git", "log", "--all", "--oneline", "--", "skills/SKILL_ROUTING_RUNTIME.json"],
    capture_output=True, text=True
)
commits = [line.split()[0] for line in result.stdout.strip().split('\n') if line]

for rev in commits[:15]:
    result = subprocess.run(
        ["git", "show", f"{rev}:skills/SKILL_ROUTING_RUNTIME.json"],
        capture_output=True, text=True
    )
    if result.returncode != 0:
        continue
    try:
        data = json.loads(result.stdout)
        for s in data['skills']:
            if isinstance(s, list) and s[0] == 'gitx':
                keys = data['keys']
                ki = keys.index('kind') if 'kind' in keys else -1
                if ki >= 0 and ki < len(s):
                    kind_val = s[ki]
                    print(f"{rev}: kind={kind_val}")
                else:
                    print(f"{rev}: kind NOT IN SKILL ENTRY")
                break
    except Exception as e:
        print(f"{rev}: PARSE_ERR {e}")
