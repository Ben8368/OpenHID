#!/usr/bin/env python3
"""Install attribution and existing checks using the repository's prek config."""
from pathlib import Path
import shutil
import subprocess
import sys

root = Path(__file__).resolve().parents[3]
prek = shutil.which("prek")
if prek is None:
    print("Install prek first (cargo install --locked prek), or enter the devenv shell.", file=sys.stderr)
    sys.exit(1)
sys.exit(subprocess.run(
    [prek, "install", "--config", "prek.toml", "--hook-type", "pre-commit", "--hook-type", "pre-push",
     "--hook-type", "commit-msg"], cwd=root, check=False,
).returncode)
