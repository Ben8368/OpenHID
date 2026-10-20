# GitHub automation

The default branch is `master`. Normal PR validation needs no repository secrets.
All external actions are pinned to commit SHAs and Dependabot maintains them.

## Validation

| Workflow | Trigger and purpose |
| --- | --- |
| Checks | Every PR, master push, manual dispatch, and weekly full run. Validates workflow/configuration syntax, attribution hooks on Linux and Windows, CI status aggregation, translation merge logic, and the Node tooling lockfile. Calls the reusable Rust checks. |
| Rust checks (`ci.yml`) | Called by Checks. Keeps the `cargo xtask ci` job IDs and commands. Rust/build/config changes run Clippy, rustdoc, dependency policy, publish closure, MSRV, wasm portability, Windows/Linux tests, and both macOS architectures. Docs-only PRs keep rustfmt, typos, ast-grep, and shell checks. Non-PR runs always run every Rust lane. |
| Dependency review | Every PR. Rejects newly introduced high/critical vulnerabilities; the Cargo dependency policy remains in `.cargo/deny.toml`. |
| CodeQL | PRs, master pushes, weekly, and manual. Analyzes GitHub Actions and TypeScript. This is not Rust source analysis. |
| Build | Manual unsigned installers, or a PR with the `needs: build` label. Signing is explicit. No secrets are needed for unsigned artifacts. |
| Nix CI / devenv CI | Existing path-filtered platform/environment checks; Nix also runs weekly. |
| Stale allows | Existing weekly three-platform lint-suppression audit. |

`Checks / CI result` is the stable required check to select in a branch ruleset
after its first successful run. It always runs after its validation dependencies;
failure, cancellation, or a skipped required lane produces RED. Expected
docs-only skips inside the reusable Rust workflow are governed by its path decision.
Security checks run separately and can also be required after their first runs.
A human traffic-light audit is still required by [REVIEW_POLICY.md](REVIEW_POLICY.md).

The executable Rust job map and local commands remain in
[.agents/rules/ci.md](../.agents/rules/ci.md). Workflow/config validation is:

```sh
python -m pip install -r .github/scripts/governance/requirements.txt
python .github/scripts/governance/check_config.py
python -m unittest discover -s .github/scripts/governance -p 'test_*.py' -v
actionlint
```

The Checks workflow downloads the pinned actionlint release and verifies its
checksum before execution. Tooling dependencies use the pinned pnpm version in
`.github/scripts/release-notes/package.json`, Node 24, and a frozen lockfile.

## PR and issue automation

| Automation | Trigger | Behavior |
| --- | --- | --- |
| Labeler | `pull_request_target` on opened, synchronize, reopened | Applies `.github/labeler.yml` labels read from the base revision. It never checks out or runs the head revision, receives no secrets, and requests only `pull-requests: write`. |
| Stale | Weekly schedule and manual dispatch | Marks issues inactive for 120 days and PRs inactive for 90 days with `status: stale`. Both close delays are `-1`, so it never closes anything. Hardware (`area: hid`, `area: hidpp`, `area: camera`), platform, blocked/upstream, and decision issues, milestone items, and `dependencies` PRs are exempt. |
| CodeRabbit | Only after the App is installed | Advisory review. `commit_status` and `request_changes_workflow` are off, so it creates no check and no merge gate. Clippy, ast-grep, actionlint, shellcheck, and formatting stay in CI; CodeRabbit runs none of them. |

The labeler uses `pull_request_target` because `pull_request` receives a
read-only token on fork PRs and cannot apply labels there. The job checks out
only `github.event.pull_request.base.sha`; do not add a head-revision checkout,
run PR-provided scripts, or widen its permissions.

Labels use the `area:` and `type:` families from the issue conventions in
DEVELOPMENT.md, plus the repository's existing `dependencies` and
`documentation` labels. Dependabot applies `dependencies`; the labeler adds the
path labels. No label approves, merges, or closes anything. `check_config.py`
fails when a workspace crate has no labeler rule.

Deliberately not configured:

- **Renovate**: Dependabot owns all four ecosystems; a second updater would open
  duplicate PRs.
- **Release Please**: release-plz, git-cliff, and `release.yml` own releases.
- **Mergify**: no bot may auto-merge. Release, dependency, hardware, and
  cross-platform changes keep a maintainer review.
- **pre-commit.ci**: `prek.toml` plus Checks already run the hooks; a second
  service would duplicate them.
- **Codecov**: there is no maintainable coverage pipeline, and adding
  hardware-dependent tests just to produce a percentage was rejected.
- **CLA**: no external-contribution licence agreement is managed yet. Adopt one
  only if the project starts accepting substantial external code.

## Dependency updates

Dependabot covers only ecosystems present here:

| Ecosystem | Directory | Schedule (Asia/Shanghai) |
| --- | --- | --- |
| GitHub Actions | `/` | Monday 09:00 |
| Cargo workspace | `/` | Monday 09:15 |
| pnpm release tooling | `/.github/scripts/release-notes` | Monday 09:30 |
| Python configuration validator | `/.github/scripts/governance` | Monthly 09:45 |

Compatible minor/patch updates are grouped per ecosystem; major updates remain
separate. A seven-day cooldown and small PR limits bound update noise. Security
alerts/updates should be enabled in the repository's Code security settings;
security updates are not delayed by version-update cooldown. Cargo git/path
dependencies still need maintainer review. Each update PR
carries the `dependencies` label, and the Cargo and Actions ecosystems keep
their `rust` and `github_actions` labels; the labeler adds `area: ci` for
changes under `.github/**` and `dependencies` for Cargo manifest and lockfile
changes. Labels
are triage metadata: no dependency PR is auto-approved or auto-merged, and
hardware support issues are never closed by the stale workflow.

## Optional services

With no variables configured, these workflows skip their external integrations.
Set a flag only after preparing the corresponding credentials and target service.

| Repository variable | Prerequisites |
| --- | --- |
| `OPENHID_RELEASE_ENABLED=true` | Release-plz publishing authorization, 1Password GitHub App, registry and signing credentials, release environment, and this repository's updater storage. |
| `OPENHID_CROWDIN_ENABLED=true` | This project's Crowdin project and token through the existing 1Password integration. |
| `OPENHID_FLAKE_UPDATES_ENABLED=true` | `AUTOMATION_TOKEN` secret with contents/PR write access to this repository. Use a fine-grained PAT that can trigger CI on its generated PRs. |
| `OPENHID_INSTALLER_SITE_ENABLED=true` | `OPENHID_INSTALLER_SITE_OWNER` and `OPENHID_INSTALLER_SITE_REPO`, plus GitHub App access to that repository. |
| `OPENHID_HOMEBREW_ENABLED=true` | `OPENHID_HOMEBREW_OWNER` and `OPENHID_HOMEBREW_REPO`, plus GitHub App access to that repository. |

Release/Crowdin retain the credential contract documented in the workflows:
`OP_SERVICE_ACCOUNT_TOKEN` and the applicable `OP_GITHUB_APP_ITEM`,
`OP_APPLE_SECRET_ITEM`, `OP_AZURE_SECRET_ITEM`, `OP_R2_SECRET_ITEM`,
`OP_CODEX_SECRET_ITEM`, and `OP_CROWDIN_SECRET_ITEM` item references. Inspect the
chosen workflow before enabling it. These flags do not provision external services
or authorize publishing. Only release-plz creates version tags; published releases
remain immutable. Existing release prerequisites in DEVELOPMENT.md still apply.

CodeRabbit is optional and needs no repository variable. Install its GitHub App
only after confirming the current free plan for public repositories; installing
an App is an external action and is not performed by merging this repository.
`.coderabbit.yaml` is committed and inert until then. It is advisory: it adds no
status check, cannot request changes, and is not a required check. Its review
instructions cover unsafe/FFI safety, platform `cfg` behavior, HID session
lifetime, IPC compatibility, permissions boundaries, and concurrency.

## Repository activation

Merging the files installs the workflow and version-update configuration. Separately:

1. Enable Actions, Dependabot alerts/security updates, dependency graph, and CodeQL
   code scanning in GitHub settings. Public repositories can use these workflows.
2. Create the `needs: build` label if unsigned PR installers are wanted.
3. Create the labels referenced by `.github/labeler.yml` and the stale workflow
   when they are missing: `area: ipc`, `area: agent`, `area: input`, `area: hid`,
   `area: hidpp`, `area: gui`, `area: cli`, `area: core`, `area: camera`,
   `area: permissions`, `area: assets`, `area: ci`, `area: tooling`,
   `type: packaging`, `status: stale`, `status: blocked`, `status: upstream`,
   `needs: decision`, and `platform: macos` / `platform: linux` /
   `platform: windows` / `platform: all`. `dependencies` and `documentation`
   already exist.
   `check_config.py` verifies crate coverage, not that a label exists on GitHub;
   a missing label is skipped by the labeler and fails the stale job.
4. After a successful first run, require `CI result` in the `master` ruleset and
   choose the security checks that should block merging. Do not require
   path-filtered Nix/devenv/installer jobs on every PR.
5. Each contributor runs the hook installer documented in REVIEW_POLICY.md.
6. Enable optional services individually once their credentials and destinations
   belong to this project. Use a protected `release` environment for signing.
7. Optionally install the CodeRabbit App for advisory reviews. Do not add it to
   the required checks.

Repository settings, credentials, first cloud results, and ruleset activation are
external state; editing these files does not change or verify them.
