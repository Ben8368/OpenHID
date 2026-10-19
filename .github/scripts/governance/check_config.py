#!/usr/bin/env python3
"""Parse GitHub YAML, hook TOML, and Node package metadata before running CI."""
import json
from pathlib import Path
import tomllib

import yaml

ROOT = Path(__file__).resolve().parents[3]
github = ROOT / ".github"
yaml_files = list(github.glob("*.yml"))
for directory in ("workflows", "actions", "ISSUE_TEMPLATE"):
    yaml_files.extend((github / directory).rglob("*.yml"))
for path in sorted(yaml_files):
    with path.open(encoding="utf-8") as source:
        yaml.safe_load(source)
with (ROOT / "prek.toml").open("rb") as source:
    config = tomllib.load(source)
assert config["default_stages"] == ["pre-commit"], "commit-msg must not run unrelated default hooks"
attribution = [hook for repo in config["repos"] for hook in repo.get("hooks", [])
               if hook["id"] == "commit-attribution"]
assert len(attribution) == 1 and attribution[0]["stages"] == ["commit-msg"]
json.loads((ROOT / ".github/scripts/release-notes/package.json").read_text(encoding="utf-8"))
print("GitHub YAML, hook stages, and package metadata are valid.")
