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
dependencies still need maintainer review. No dependency PR is auto-approved or
auto-merged, and hardware support issues are not automatically closed as stale.

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

## Repository activation

Merging the files installs the workflow and version-update configuration. Separately:

1. Enable Actions, Dependabot alerts/security updates, dependency graph, and CodeQL
   code scanning in GitHub settings. Public repositories can use these workflows.
2. Create the `needs: build` label if unsigned PR installers are wanted.
3. After a successful first run, require `CI result` in the `master` ruleset and
   choose the security checks that should block merging. Do not require
   path-filtered Nix/devenv/installer jobs on every PR.
4. Each contributor runs the hook installer documented in REVIEW_POLICY.md.
5. Enable optional services individually once their credentials and destinations
   belong to this project. Use a protected `release` environment for signing.

Repository settings, credentials, first cloud results, and ruleset activation are
external state; editing these files does not change or verify them.
