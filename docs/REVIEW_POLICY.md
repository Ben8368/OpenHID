# Review and commit attribution

This document owns the review format and attribution policy. The agent guide
routes here; the PR template collects the evidence. GitHub artifacts remain
English and use conventional commit subjects.

## Traffic-light review

After implementation and before the final verification, publish an audit report
covering the changed boundary: IPC ownership and wire compatibility, device and
input lifecycle, platform behavior, error handling, credentials, test coverage,
and scope as applicable. Record actual results after running the plan.

Use these fields in a PR or completion report:

- **Overall:** RED / YELLOW / GREEN.
- **Red blockers:** concrete blocking defects, or None.
- **Yellow follow-ups:** remaining gaps with an issue or durable document link, or None.
- **Verification plan:** exact commands and the behavior each proves.
- **Evidence and untested paths:** actual outcomes, hardware and OS coverage,
  skipped checks, and reasons.

RED means a verified safety, security, correctness, lifecycle, or contract defect;
it blocks merging and completion. YELLOW means the scoped change is ready with
a documented, non-blocking gap and an owner or follow-up location. GREEN requires
all applicable proofs to pass and no unresolved gap. A successful compilation
alone does not prove hardware behavior; a skipped check is never a pass.

Use the verification commands and pre-push tiers in
[.agents/rules/ci.md](../.agents/rules/ci.md), not a second test checklist here.
CI validates executable checks; the maintainer owns the final review judgment.
Automated dependency and translation PRs also need a maintainer's audit before merge.

## Attribution

Keep the human maintainer as the primary author. When an AI tool substantially
contributes, append its actual tool identity as a Git trailer in the final
paragraph, separated from the body by a blank line. Do not record a model name.
Supported tool identities are enforced by
[commit_attribution.py](../.github/scripts/governance/commit_attribution.py).
The validator prints the accepted trailers on failure. When adopting upstream
work, inspect the source commits for their authors and co-authors, then preserve
each distinct human contributor whose work is included with a `Co-authored-by`
trailer in the final trailer block. Do not infer authorship from the PR owner or
copy attribution from a source commit embedded in the message body. Changes that
are deferred or already implemented locally do not earn a trailer on the current
port commit.

Before pushing an adoption, check the commit that will be pushed:

```powershell
git show -s --format=%B HEAD | git interpret-trailers --parse
```

Every expected contributor must appear in that output. A trailer in a nested
source-commit or PR description is not recognized as a trailer for the OpenHID
commit. The commit-message hook rejects `Co-authored-by` lines that are not also
present in Git's parsed final trailer block. Entirely human commits declare
`Human-authored: true`; this must not coexist with an AI tool trailer.

For example, in PowerShell:

```powershell
git commit -m "fix(agent): restore device session after reconnect" -m "Co-authored-by: Codex <codex@openai.com>"
git show -s --format=%B HEAD | git interpret-trailers --parse
```

This attribution policy is independent of cryptographic commit signing. It does
not enable GPG/SSH signing, invent an author, or rewrite old commits. GitHub bots
commit through their own application identities; local hook installation does
not change those identities. CI runs hook regression tests rather than demanding
new trailers on imported history or automatically generated bot commits.

This policy is the single authority for attribution; upstream-sync procedures
link here instead of duplicating it. That documentation structure follows the
single-source and route-from-one-entry approach in [uNAS's agent guide](https://github.com/Ben8368/uNAS/blob/main/AGENTS.md)
and [governance guide](https://github.com/Ben8368/uNAS/blob/main/docs/GOVERNANCE.md).

## Install and verify hooks

Each clone needs Python 3.11+, Git, and `prek` on PATH:

```sh
python .github/scripts/governance/install_hooks.py
python -m unittest discover -s .github/scripts/governance -p 'test_*.py'
```

The installer registers `pre-commit`, `pre-push`, and `commit-msg` through the
existing `prek.toml`. It keeps the format, lint, and rustdoc hooks. Do not point
`core.hooksPath` at a separate attribution-only directory. Hooks are local opt-in;
contributors must install them, and maintainers still review final squash messages.
