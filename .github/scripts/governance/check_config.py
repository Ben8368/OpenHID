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
with (ROOT / ".coderabbit.yaml").open(encoding="utf-8") as source:
    yaml.safe_load(source)


def _globs(node):
    """Every glob string in a labeler match object, at any nesting depth."""
    if isinstance(node, str):
        yield node
    elif isinstance(node, dict):
        for value in node.values():
            yield from _globs(value)
    elif isinstance(node, list):
        for value in node:
            yield from _globs(value)


labeler = yaml.safe_load((github / "labeler.yml").read_text(encoding="utf-8"))
assert isinstance(labeler, dict) and labeler, "labeler.yml must map labels to rules"
label_globs = [glob for rules in labeler.values() for glob in _globs(rules)]
# A crate with no path rule merges without an area label and drops out of
# area-based triage. The labeler cannot warn about this at runtime, so catch it
# here when the crate is added.
for crate in sorted((ROOT / "crates").iterdir()):
    if (crate / "Cargo.toml").exists():
        prefix = f"crates/{crate.name}/"
        assert any(glob.startswith(prefix) for glob in label_globs), (
            f".github/labeler.yml has no rule covering {prefix}"
        )

print("GitHub YAML, labeler coverage, hook stages, and package metadata are valid.")
