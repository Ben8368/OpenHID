#!/usr/bin/env python3
"""Produce a stable required check that rejects failed, cancelled, or skipped lanes."""
import json
import os
from pathlib import Path
import sys


def summarize(needs: dict) -> tuple[str, bool]:
    passed = bool(needs) and all(job.get("result") == "success" for job in needs.values())
    status = "GREEN" if passed else "RED"
    rows = [f"## CI audit: {status}", "", "| Check | Result |", "| --- | --- |"]
    rows.extend(f"| {name} | {job.get('result', 'missing')} |" for name, job in sorted(needs.items()))
    rows += ["", "Skipped or cancelled required lanes do not count as a pass.", ""]
    return "\n".join(rows), passed


def main() -> int:
    report, passed = summarize(json.loads(os.environ["NEEDS_JSON"]))
    print(report)
    if summary := os.environ.get("GITHUB_STEP_SUMMARY"):
        with Path(summary).open("a", encoding="utf-8") as output:
            output.write(report)
    return 0 if passed else 1


if __name__ == "__main__":
    sys.exit(main())
