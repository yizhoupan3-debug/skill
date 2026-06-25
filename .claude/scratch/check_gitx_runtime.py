import json, subprocess

# Get commits that touched RUNTIME_REGISTRY.json
result = subprocess.run(
    ["git", "log", "--oneline", "--", "configs/framework/RUNTIME_REGISTRY.json"],
    capture_output=True, text=True
)
commits = [line.split()[0] for line in result.stdout.strip().split('\n') if line]

for rev in commits[:10]:
    result = subprocess.run(
        ["git", "show", f"{rev}:configs/framework/RUNTIME_REGISTRY.json"],
        capture_output=True, text=True
    )
    if result.returncode != 0:
        continue
    try:
        data = json.loads(result.stdout)
        fwc = data.get("framework_commands", {})
        has_gitx = "gitx" in fwc
        print(f"{rev}: framework_commands keys={list(fwc.keys())}")
    except Exception as e:
        print(f"{rev}: PARSE_ERR {e}")
