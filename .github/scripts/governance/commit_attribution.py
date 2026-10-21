#!/usr/bin/env python3
"""Validate explicit commit attribution without changing the message or author."""
from __future__ import annotations

from pathlib import Path
import re
import subprocess
import sys

IDENTITIES = {
    "Codex": "codex@openai.com",
    "Claude Code": "noreply@anthropic.com",
    "Cursor": "cursoragent@cursor.com",
}


def validate(message_file: Path) -> None:
    message = message_file.read_text(encoding="utf-8")
    result = subprocess.run(
        ["git", "interpret-trailers", "--parse", str(message_file)],
        capture_output=True, text=True, encoding="utf-8", check=True,
    )
    ai = False
    human = False
    parsed_coauthors: set[str] = set()
    for line in result.stdout.splitlines():
        key, _, value = line.partition(":")
        value = value.strip()
        if key.lower() == "co-authored-by":
            parsed_coauthors.add(value)
            name, _, address = value.partition(" <")
            if name in IDENTITIES:
                if address != IDENTITIES[name] + ">":
                    raise ValueError(f"Incorrect identity for {name}.")
                ai = True
        elif key.lower() == "human-authored":
            if value != "true":
                raise ValueError("Human-authored must be exactly true.")
            human = True

    message_coauthors = {
        match.group(1).strip()
        for match in re.finditer(
            r"^[ \t]*Co-authored-by:[ \t]*(.+?)[ \t]*$",
            message,
            re.IGNORECASE | re.MULTILINE,
        )
    }
    if not message_coauthors.issubset(parsed_coauthors):
        raise ValueError(
            "Every Co-authored-by trailer must appear in the final trailer block; "
            "verify with git interpret-trailers --parse."
        )

    if ai and human:
        raise ValueError("AI attribution conflicts with Human-authored: true.")
    if not ai and not human:
        raise ValueError("Declare the actual AI tool or Human-authored: true in the final trailer block.")


def main() -> int:
    try:
        if len(sys.argv) != 2:
            raise ValueError("Expected one commit message file.")
        validate(Path(sys.argv[1]))
    except (ValueError, OSError, subprocess.CalledProcessError) as error:
        print(f"OpenHID: {error}", file=sys.stderr)
        for name, email in IDENTITIES.items():
            print(f"  Co-authored-by: {name} <{email}>", file=sys.stderr)
        print("For entirely human work: Human-authored: true", file=sys.stderr)
        print("See docs/REVIEW_POLICY.md. Attribution is never inferred or inserted.", file=sys.stderr)
        return 1
    return 0


if __name__ == "__main__":
    sys.exit(main())
