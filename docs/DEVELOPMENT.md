> OpenHID is an independent source import. See [source provenance](PROVENANCE.md).
> The release/signing/Crowdin recipes below describe inherited tooling; these
> services require OpenHID-owned configuration before use. No release feed is
> enabled by default. Build from source using the current README.

# Developing OpenHID

This document covers the local development workflow for OpenHID. For end-user
build instructions, see the [README](../README.md).

## Toolchain

- Stable Rust (Edition 2024, MSRV 1.98 — the floor tracks current stable)
- macOS: Xcode 26+ with the optional **Metal Toolchain** component. The Metal
  Toolchain is what GPUI's `gpui_macos` build script compiles shaders with; the
  version floor is `actool`, which packaging uses to compile the app icon from
  its Icon Composer document. `OPENHID_DEVELOPER_DIR` overrides which Xcode is
  used when several are installed.
- Linux: system libraries — on Debian/Ubuntu:
  `sudo apt-get install libudev-dev gcc g++ clang libfontconfig-dev libwayland-dev libxkbcommon-x11-dev libx11-xcb-dev libssl-dev libzstd-dev pkg-config`
- `create-dmg` for packaging (`brew install create-dmg`); `cargo-bundle` is
  installed automatically by `cargo run -p xtask -- macos bundle`

## Building from source

Nix/devenv is optional. A normal Rust toolchain is enough.

### Without Nix

```sh
# rustup installs the stable toolchain pinned in rust-toolchain.toml
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
# macOS: full Xcode 26+ with the Metal Toolchain (not only Command Line Tools)
# Linux: see system libraries under Toolchain above
# optional helpers: brew install cmake create-dmg sccache
git clone https://github.com/Ben8368/OpenHID
cd OpenHID
cargo run -p openhid --release -- list
cargo run -p openhid-desktop --release
```

If you use [direnv](https://direnv.net) without devenv installed, `.envrc`
prints a notice and leaves your shell alone. Install rustup/cargo yourself
and keep working.

### With devenv (optional)

`devenv.nix` provisions sccache, the stable Rust toolchain, platform libraries,
nfpm on Linux, and the macOS packaging/env helpers GPUI needs
(`create-dmg`, `DEVELOPER_DIR`, and `SDKROOT`). Tasks:

```sh
devenv tasks run openhid:gui      # run the desktop app
devenv tasks run openhid:check    # host-OS gate: fmt + clippy + tests + rustdoc
devenv tasks run openhid:ci       # every GitHub Actions CI job this host can reproduce
devenv tasks run openhid:dmg      # build the macOS DMG
devenv tasks run openhid:i18n-upload    # upload English source strings to Crowdin
devenv tasks run openhid:i18n-download  # download translations and run i18n tests
```

After a `devenv.nix` change, reload direnv so the new env takes effect:

```sh
direnv reload    # or: exit your shell and `cd` back in
```

Without that, GPUI's `gpui_macos` build script can't find Apple's `metal`
shader compiler, and link errors about missing `_write` / `_sysconf` /
`_waitpid` symbols show up because the Nix `apple-sdk-14.4` stub doesn't
expose `libSystem` the way Apple's real linker wants.

### Nix package

The root Flake exposes native `x86_64-linux` and `aarch64-linux` packages plus
the NixOS module. It is separate from the devenv shell:

```sh
nix flake check --all-systems --no-build  # evaluate every output
nix build .#openhid                      # build + test this host's package
nix run .#openhid -- list                # run the packaged CLI
```

The package expression and NixOS module live beside the other Linux packaging
inputs in `packaging/linux/`. `nix fmt` formats all Nix expressions through the
Flake's pinned formatter.

### Dev app bundle (macOS)

On macOS the desktop binary is launched from inside a throwaway
`target/dev/OpenHID.app` — a Cargo `runner` wired in `.cargo/config.toml`
(`.cargo/run-macos.sh`) that hands the build to `xtask macos dev-bundle`. This
makes the dev build show as **OpenHID Dev** in the menu bar and Dock, with the
real app icon; a bare `cargo run` binary has no bundle, so macOS would otherwise
fall back to the `openhid-desktop` executable name and a generic icon. The
binary is hardlinked in (no copy) unless the bundle is being signed, and the
icon is generated on demand. The runner is a transparent passthrough for
everything else (the CLI, tests); set `OPENHID_DEV_BUNDLE=0` to launch the raw
`openhid-desktop` binary instead.

Each run also stops the dev agent and overlay left behind by the previous one,
then starts the freshly built agent and waits for its IPC socket before the
GUI launches — so the window connects immediately instead of sitting on its
connecting frame while the GUI's production fallback re-spawns the agent.
The helpers are launched through LaunchServices so they get their own TCC
identity, which also means they are not children of the GUI: closing its
window or pressing Ctrl-C ends only the GUI, and a surviving dev agent
relaunches itself ~20 s later once its watcher notices the rewritten binary.
Set `OPENHID_DEV_AGENT=0` to run against an agent you started yourself —
nothing is stopped, built, embedded, or started then.

Packaged local dev bundles (`cargo run` and
`cargo run -p xtask -- macos bundle`) use `-dev` bundle identifiers and the
`openhid-dev` XDG profile (`~/.config/openhid-dev`,
`~/.local/share/openhid-dev`, and its own `agent.sock`). That keeps the dev
GUI and agent from sharing the installed production app's Accessibility grant,
single-instance lock, config, or IPC socket.

Those identifiers are a channel, not a guess from the build type:
`macos bundle` takes `--channel dev|production` (dev by default) and verifies
what it stamped, and `macos dmg` refuses a non-production bundle once it is
given a signing identity. Reproduce the shipped layout locally with
`--channel production`, but don't sign and run it — it would take over the
installed app's grants and config, which is exactly what releases
0.6.24–0.6.26 did in reverse.

To install the CLI binary on `PATH`:

```sh
cargo install --path crates/openhid
```

### macOS input-hook safety

The agent's input-hook watchdog gives CoreGraphics permission probes a 10-second budget. After a probe returns, tap servicing and teardown use a fresh 1.5-second budget. Teardown keeps that short budget when Accessibility is revoked or the re-arm limit is reached. The watchdog exits the agent if cleanup stalls, so macOS releases the process-owned tap.

## Developing the GUI without hardware

`openhid-agent-mock` serves the real agent IPC contract from a scripted
in-memory inventory, so the desktop app can be developed with no Logitech
device (or receiver) attached:

```sh
cargo run -p openhid-agent --bin openhid-agent-mock   # then, in another terminal:
OPENHID_DEV_AGENT=0 cargo run -p openhid-desktop
```

The mock defaults itself to the `openhid-dev` profile (as if `OPENHID_PROFILE=dev`
were set), which is the profile the dev app bundle already uses — so it meets the
dev GUI on the dev socket, and an installed *release* build, which is on the
production profile, keeps running untouched. (A locally built bundle installed
into `/Applications` carries `-dev` identifiers and therefore shares the dev
profile: it and the mock contend for the same lock, and whichever starts second
exits.) `OPENHID_DEV_AGENT=0` keeps the runner from building and embedding
the real agent for the GUI to auto-spawn; add `OPENHID_ALLOW_EXTERNAL_AGENT=1`
if your installed production agent is running, since the runner's guard against
it predates the profile split and cannot know the dev GUI is on a separate
socket. Pass `OPENHID_PROFILE=prod` to serve the production socket instead; the
mock then contends for the production agent's single-instance lock and refuses
to start while it is running.

The script covers an online mouse (DPI and SmartShift writes persist and read
back, battery drains so poll-driven repaints are visible), an offline mouse, a
lighting-capable keyboard, a directly-attached device, and a full Bolt pairing
flow (discovery → passkey → paired). Its agent version carries a `-mock` suffix,
so a mock session is identifiable in the UI. It is a dev tool only and is never
bundled.

The proposed architecture for recorded device profiles, raw HID++ replay, and
deterministic hardware scenarios is documented in
[Mock device and hardware record/replay architecture](MOCK_DEVICE_TESTING.md).

### Component gallery

Use the debug-only component gallery to review shared controls across light and
dark themes and every supported interface scale without config, IPC, or hardware:

```sh
OPENHID_COMPONENT_GALLERY=1 cargo run -p openhid-desktop
```

Gallery mode opens one isolated window and bypasses the normal single-instance,
config, agent, asset-sync, and updater startup paths. The environment variable is
ignored by release builds.

## Project layout

```
crates/
  openhid/         the `openhid` binary — a thin wrapper over openhid-cli
  openhid-core/    types, config (TOML), paths, button + action catalog — no HID, no async
  openhid-inject/  OS input synthesis: CGEvent, uinput/MPRIS, and SendInput
  openhid-hidpp/   vendored HID++ protocol crate (lib name `hidpp`)
  openhid-hid/     device discovery, HID++ reads/writes, and control capture over async-hid
  openhid-assets/  device-render registry schema + cached HTTP fetch from OpenHID asset mirrors
  openhid-cli/     CLI implementation: command tree + `run()`, called by the `openhid` binary
  openhid-agent-core/  shared orchestration + the agent/GUI IPC contract
  openhid-agent/   the `openhid-agent` binary — background agent owning device I/O and the hook
  openhid-hook/    OS mouse hook: macOS CGEventTap, Linux evdev/uinput, Windows WH_MOUSE_LL
  openhid-ui/      presentation shared by the two GPUI processes: ring geometry/icons,
                    the GPUI asset source, locale negotiation — gpui, no gpui-component
  openhid-desktop/     the `openhid-desktop` binary — GPUI + gpui-component IPC client
  openhid-overlay/ the `openhid-overlay` binary — the cursor-centred Actions Ring
```

## Agent guidance

Shared rules have one tracked source: [`.agents/rules/`](../.agents/rules/).
Edit and link those `.md` files, not a client-specific copy. The tracked
`.claude/rules` symlink points to `../.agents/rules` inside the checkout.
Existing references to `.claude/rules/<name>.md` still resolve through that alias.
Crate-specific contracts stay in each crate's `AGENTS.md`; task workflows stay
in `.agents/skills/`. No rule generator or install step is required.

The root [AGENTS.md](../AGENTS.md) holds global instructions and the rule index.
`CLAUDE.md` imports only that entrypoint. Claude Code discovers `.md` rules
through `.claude/rules` and uses their `paths` metadata for conditional loading.
Other clients must follow the index; `.agents/rules/` is not a universal
automatic discovery path. Keep the index as ordinary Markdown links: importing
every rule from the root would load unrelated guidance into Claude's context.

### Check the checkout and client loading

From the repository root, in a POSIX shell or Git Bash:

```sh
git ls-files --stage -- .claude/rules .agents/rules
test -L .claude/rules && test -f .claude/rules/rust.md
readlink .claude/rules
```

Expect regular rule files under `.agents/rules/` and one `120000` entry
for `.claude/rules`, whose link target is `../.agents/rules`. The file checks
must succeed too: the index mode alone does not prove a working symlink.

On Windows, enable Developer Mode or obtain symlink creation permission before
cloning with `git clone -c core.symlinks=true <repository-url> <new-directory>`.
With `core.symlinks=false`, Git writes a text file containing the target instead
of a directory link; Claude cannot discover the rules through it. Changing the
config alone does not repair an existing checkout. Preserve local changes and
use a fresh symlink-enabled checkout. Until then, read the canonical rules via
the root index; do not replace the alias with independently maintained copies.

Verify loading in the client, not only the filesystem. In a fresh Claude Code
session, use `/context` or an `InstructionsLoaded` hook to inspect loaded files:

- Read `README.md`: Rust and GUI path rules should not load solely from that read.
- Read `crates/openhid-core/src/lib.rs`: the Rust rule should load, but not GUI.
- Read `crates/openhid-desktop/src/app.rs`: the GUI rule should now load too.

Follow [Claude's loading diagnostics](https://code.claude.com/docs/en/memory#troubleshoot-memory-issues)
if those results differ. Filesystem checks and unchanged `paths` metadata are
not evidence that a particular client version loaded the rules correctly.

## Local CI

The PR test pipeline is `.github/workflows/ci.yml`. To run every job this
machine can reproduce — including typos, the ast-grep guards, MSRV, cargo-deny,
and the Windows cross-lint the host-OS gate does not run:

```sh
cargo xtask ci
cargo xtask ci --list                        # job → command table
devenv tasks run openhid:ci                 # same, from devenv
```

The runner sets `RUSTFLAGS=-D warnings` the way CI does. Jobs that need another
OS are reported as skipped; a skip is not a pass. The full job map (and which
diff requires which job) is [`.agents/rules/ci.md`](../.agents/rules/ci.md).

For structural searches and guard changes, use the [ast-grep skill](../.agents/skills/ast-grep/SKILL.md). The [guard workflow](../.agents/rules/ci.md#ast-grep-rules) defines placement beside the owner, root discovery, and verification.

### Pre-push gate

Before pushing, read [the local gate and push checklist](../.agents/rules/ci.md#local-gate-hard-stop-before-push--scale-it-to-the-affected-graph).
That file owns tier selection, exact commands, and additional checks required by
the diff. `devenv tasks run openhid:check` runs the full host-OS tier, not the
whole CI pipeline. A Rust-bearing rebase or conflict resolution requires the
full tier. Non-Rust changes use the applicable non-Rust checks.

## GitHub workflow

[Review and commit attribution](REVIEW_POLICY.md) owns the traffic-light audit and
commit trailers. [GitHub automation](GITHUB_AUTOMATION.md) describes validation,
dependency bots, optional integrations, and repository activation.

Read this section before preparing, adopting, reviewing, or merging a PR.
These procedures do not authorize remote writes: obtain approval before pushing,
opening or merging PRs, publishing, or approving/rerunning workflows.

### Preparing and merging PRs

- **Always `git fetch upstream master` (or origin) immediately before a rebase.** Rebase
  onto the refreshed tip, not a stale local `master`.
- Merging PRs: **squash by default** with a hand-written subject
  `type(scope): description (#N)` (release-plz parses it; merge commits are disabled).
  Rebase-merge only when every commit on the branch is already release-quality
  conventional. Wait for required review and CI checks before merging — findings get
  fixed, replied to, and resolved, not ignored.
- PR bodies: `## Summary`, `## Changes` (per-crate bullets), `## Testing` listing the
  exact commands run plus hardware-verification status (say "not runtime-tested on
  hardware" when true — real-hardware verification is the maintainer's job, so every
  fix PR states how to test it), and a closing `Fixes #N` line. Screenshots for UI
  changes. Include the audit fields from the PR template and review policy.
- Issues use the bug/feature/device forms and the `type:`/`area:`/`platform:`/`needs:`/
  `status:` label families. Deferred or out-of-scope work becomes a linked issue, not a
  TODO comment.

### Adopting contributor PRs

Contributor PRs are adopted, not rejected: check `maintainerCanModify`, rebase onto
**fresh** master in a worktree, fix review findings, run the applicable local gate
on the rebased tip (a Rust-bearing rebase takes the full tier), **then** push to the
fork branch; preserve authorship (`Co-authored-by` when re-homing work).
Squash-then-rebase is fine when the PR is far behind and commit-by-commit conflicts
thrash.

### CI / Actions when adopting PRs

- CI concurrency is **per branch** (`ci-${{ workflow }}-${{ ref }}` with
  `cancel-in-progress: true`). Approving or re-running an **old SHA** on the same
  branch cancels the current-head run. Only approve / re-run workflows whose
  `head_sha` equals the PR's current head.
- After a force-push, wait for the new runs; do not re-approve stale
  `action_required` jobs from earlier commits on that branch.
- First-time-fork PRs may sit in `action_required` until a maintainer approves the
  workflow run — that is fine; still do not push until the local gate is green.

## Packaging the macOS DMG

```sh
cargo run -p xtask -- macos package    # → target/release/OpenHID.dmg
# Cross-compile a distribution DMG (aarch64 or x86_64):
cargo run -p xtask -- macos package --target x86_64-apple-darwin
```

Environment overrides:

- `OPENHID_BUNDLE_ASSETS=1` — bundle every device render into the `.app` for a
  fully offline build (default: fetched on demand at first launch).
- `OPENHID_SIGN_IDENTITY=<identity>` — codesign the `.app` and `.dmg` with the
  given Developer ID.
- `OPENHID_SPARKLE_PUBLIC_ED_KEY=<base64>` — embed Sparkle.framework and update
  through it (see Release updater publishing). Without it the bundle updates
  through the built-in engine.
- `OPENHID_UPDATE_BASE_URL=<url>` — update host of the embedded Sparkle feed;
  required with `OPENHID_SPARKLE_PUBLIC_ED_KEY`.
- `OPENHID_DMG_BACKGROUND_URL=<url>` — override the branded DMG background
  URL override; by default packaging uses `design/bg/openhid-dmg.png`.

The local packaging command and release workflow both use the same branded DMG
layout: a 760×480 background image in a 760×512 Finder window, with 128px icons
positioned at `(212, 250)` for `OpenHID.app` and `(548, 250)` for
`Applications`.

## Packaging Linux `.deb` / `.rpm` / `.pkg.tar.zst`

Requires [nfpm](https://nfpm.goreleaser.com/) on `PATH`; the package arch is
derived from the host (override with `PKG_ARCH`):

```sh
cargo run -p xtask -- linux package
# → target/release/*.deb / *.rpm / *.pkg.tar.zst
```

The package contents (binaries, udev rules, systemd user unit, desktop entry,
icon) are declared in `packaging/linux/nfpm.yaml`.

`packaging/linux/install.sh` is a POSIX `/bin/sh` frontend for those release
packages, not a second package lifecycle. Its online mode maps the host and
package manager to the release naming contract
`openhid-v<version>-linux-<amd64|arm64>.<format>`, authenticates that file's
detached signature against its embedded minisign public key, verifies only that
file's `SHA256SUMS` entry, and then delegates installation to apt, dnf, yum,
zypper, rpm, or pacman. The embedded public key must stay in sync with
`OPENHID_UPDATE_MINISIGN_PUBLIC_KEY`. The nFPM post-install script remains the
owner of udev and desktop/icon cache reloads. The explicit `--from-source` mode
preserves the checkout installer for the four local `target/release` binaries
and shared resources.

Run the mocked online/source smoke suite directly with dash; it exercises
latest and pinned versions, both architectures, every package-manager mapping,
signature and checksum rejection before sudo, dry-run, and the source resource
set without network or system writes:

```sh
dash -n packaging/linux/install.sh
dash packaging/linux/tests/install-smoke.sh
```

The Nix package uses the same shared resources and is declared in
`packaging/linux/package.nix`; see the Nix package section above for its build
commands.

## Installation-source detection

The desktop app probes once in the background and publishes the typed
`platform::installation::Installation` global: `Detecting`, then
`Detected(InstallationSource)`. It also logs `detected installation source`.
Settings → Updates displays the result as **Installation source**, separately
from the update download source. An open window refreshes when detection
completes or the interface language changes.
This is an ownership snapshot, not download provenance or an update policy;
the updater does not yet change behavior based on it.

- **Homebrew:** matches the installed receipt and Caskroom app back-link to
  the running bundle, distinguishing `openhid` from `openhid@latest`. It
  checks both standard prefixes, `HOMEBREW_PREFIX`, and prefixes discoverable
  from `PATH`, without executing brew. An undiscoverable custom prefix cannot
  be recognized. Other macOS bundles report `MacAppBundle`, not "DMG".
- **Linux:** recognizes a resolved `/nix/store/` executable, or queries dpkg,
  rpm, and pacman for ownership of the exact executable by `openhid`.
  Package queries are read-only, with a two-second timeout per command.
- **Windows:** the MSI writes its `InstallLocation` under
  `HKCU\Software\OpenHID`; only a matching executable is `WindowsMsi`.
  The ZIP carries `openhid-installation.json` next to `OpenHID.exe` and is
  `WindowsPortable`. A matching MSI registration takes precedence.
- **Unknown:** unmarked Windows releases predating these markers, bare
  source/manual installs, or otherwise inconclusive ownership. Missing
  metadata never implies a portable ZIP or a DMG.

## Release updater publishing

Tagged releases attach artifacts and `SHA256SUMS` to GitHub Releases for manual
downloads. When both Linux build legs succeed, the release workflow copies
`packaging/linux/install.sh` to `dist/install.sh`; that exact file is listed in
`SHA256SUMS`, signed as `install.sh.minisig`, and attached as a release asset.
Partial releases without a complete Linux package set omit the installer. The
workflow also publishes the artifacts to Cloudflare R2 and writes a static
updater manifest at:

```text
${OPENHID_UPDATE_BASE_URL}/channels/stable/latest.json
```

The app embeds that manifest URL at build time via
`OPENHID_UPDATE_MANIFEST_URL`, derived from `OPENHID_UPDATE_BASE_URL` in the
release workflow. Release builds also embed `OPENHID_UPDATE_MINISIGN_PUBLIC_KEY`
and run with `Verification::Strict`: an update is installed only if the manifest
asset carries a minisign signature that verifies against that key, plus a
matching SHA-256. A build without the key embedded (local/dev) fails closed —
the update check errors rather than installing an unverified artifact.

### macOS: Sparkle

The signed macOS release bundle embeds
[Sparkle](https://sparkle-project.org) and updates through it instead
(`platform::updater` in `openhid-desktop`). Each architecture ships its own
DMG, so each reads its own feed:

```text
${OPENHID_UPDATE_BASE_URL}/channels/stable/appcast-arm64.xml
${OPENHID_UPDATE_BASE_URL}/channels/stable/appcast-x86_64.xml
```

Each feed carries one item: that architecture's notarized, stapled DMG with an
EdDSA (`sparkle:edSignature`) signature over the exact file, and the release
notes as Markdown. The bundle trusts only the key in its `SUPublicEDKey`, and
`SUVerifyUpdateBeforeExtraction` makes Sparkle check that signature before it
unpacks anything.

Updates stay opt-in. The bundle ships `SUEnableAutomaticChecks = NO`, so
Sparkle never schedules a check or asks to; the app runs the one launch check
itself when **Check for updates** is on, without a window. With **Automatic
updates** also on, a found update downloads in the background and installs on
quit or through **Restart to Update**. "Check for Updates…" opens Sparkle's
update window. A bundle without Sparkle (Windows, Linux, local builds) and a
Sparkle that fails to load fall back to the engine and `latest.json`.

`latest.json` and the DMG minisigs keep being published: bundles from before
Sparkle read only that manifest, and they update into a Sparkle bundle through
it.

Configure the R2/update settings in one 1Password item referenced by the GitHub
secret `OP_R2_SECRET_ITEM`. The item must contain:

- `OPENHID_UPDATE_BASE_URL` — public HTTPS base URL, for example
  `https://updates.example.org`.
- `OPENHID_UPDATE_MINISIGN_PUBLIC_KEY` — base64 minisign public key embedded in
  the app and used to verify updater artifacts.
- `OPENHID_UPDATE_MINISIGN_SECRET_KEY` — the passwordless minisign secret key
  file, **base64-encoded** (`base64 < minisign.key`), used only in the release
  publish job to sign release artifacts before `latest.json` is generated. It
  is stored base64 (not raw) so its two lines survive 1Password's paste
  handling; the workflow decodes it, mirroring the GitHub App key.
- `OPENHID_SPARKLE_PUBLIC_ED_KEY` — Sparkle's base64 EdDSA public key, stamped
  into the macOS bundle as `SUPublicEDKey`.
- `OPENHID_SPARKLE_PRIVATE_ED_KEY` — the matching base64 private key (the
  32-byte seed), used only in the publish job to sign the DMGs into the
  appcasts. `release appcast` refuses a private key that does not belong to the
  public key above.
- `CLOUDFLARE_R2_ACCOUNT_ID` — Cloudflare account ID used for the S3 endpoint.
- `CLOUDFLARE_R2_BUCKET` — bucket name.
- `CLOUDFLARE_R2_ACCESS_KEY_ID` — R2 S3 access key.
- `CLOUDFLARE_R2_SECRET_ACCESS_KEY` — R2 S3 secret key.

The workflow uploads immutable artifacts under `/releases/<tag>/` and only the
channel files under `/channels/stable/` — `latest.json` and the two appcasts —
are mutable. They are uploaded after the artifacts they point at.

Generate the Sparkle key pair once, with the `generate_keys` tool from the
[Sparkle release](https://github.com/sparkle-project/Sparkle/releases) xtask
embeds:

```sh
./bin/generate_keys --account openhid         # prints the public key
./bin/generate_keys --account openhid -x sparkle-private.key
# store the file's contents as OPENHID_SPARKLE_PRIVATE_ED_KEY, then delete it
```

The key cannot be replaced casually: every installed bundle trusts only the one
it shipped with. Rotating it means shipping a bundle with the new key that is
still signed with the old one, or relying on Sparkle's fallback to the DMG's
Developer ID signature.

After a complete Linux release is published, the workflow dispatches
`publish-openhid-installer` with the tag to `openhid-org/get`. That repository
downloads `install.sh`, `install.sh.minisig`, and `SHA256SUMS` from the published
GitHub Release, verifies both the exact checksum and minisign signature, and
commits immutable versioned files plus the latest stable aliases served by
Cloudflare Pages at `installer.example.org`. It mirrors no native package binaries.
The dispatch uses the same 1Password-backed GitHub App as the Homebrew update;
the App must be installed for the `openhid-org/get` repository.

The manifest is generated by the workspace `xtask` helper:

```sh
cargo run -p xtask -- release latest-json \
  --dist dist \
  --tag v0.2.0 \
  --base-url https://updates.example.org \
  --output dist/latest.json
```

and so are the appcasts, after the DMGs are stapled (stapling rewrites them).
The private key is read only from the environment:

```sh
OPENHID_SPARKLE_PRIVATE_ED_KEY=… cargo run -p xtask -- release appcast \
  --dist dist \
  --tag v0.2.0 \
  --base-url https://updates.example.org \
  --public-key "$OPENHID_SPARKLE_PUBLIC_ED_KEY" \
  --release-notes .release/RELEASE_NOTES.md \
  --output-dir .release/appcast
```

The stable feed takes `MAJOR.MINOR.PATCH` releases only: Sparkle's version
comparator stops at the first `-`, so it would read `1.2.0-rc.1` as `1.2.0`.

## Crowdin translation sync

`.github/workflows/crowdin.yml` syncs GUI locales with
[Crowdin](https://crowdin.com/project/openhid) and opens a `crowdin/i18n` PR
when a **real** translation value improved — nightly, and on master pushes that
touch English sources (`en.toml`), `.config/crowdin.yml`, the Crowdin workflow,
the merge script under `.github/scripts/i18n/`, or the shared GitHub App token
action.

**How it helps translation**

| | Role |
|--|--|
| `en.toml` (git) | English source of truth; stable semantic keys grouped by product-domain tables |
| All `locales/*.toml` in git | Same keys as `en.toml` (parity test); seed Crowdin per language |
| Crowdin project | Where people improve non-English **values** |
| Merge script | Applies only values ≠ English; restores keys sparse exports omit |
| Bot PR (`crowdin/i18n`) | Only when a non-English value actually changed |

Call sites use stable keys such as `device.connected`. Feature PRs add
new keys to **every** locale file in the same change. English wording can change
without renaming the key or updating call sites. Crowdin does not invent
translations; it only stores and syncs them. A raw Crowdin download is unsafe:
untranslated strings come back as English (#549), and
`skip_untranslated_strings` overwrites catalogs with sparse files that delete
keys (#552). The workflow always **snapshots → download → merge** via
`.github/scripts/i18n/merge_crowdin_download.py` so catalogs stay complete and only real
translations land in git.

Each run:

1. Snapshots every `locales/*.toml`.
2. Uploads `en.toml` **sources**.
3. Uploads **per-language translations** already in git (`import_eq_suggestions`
   off so `value == English` is not stored as a finished translation).
4. Downloads Crowdin’s export (`skip_untranslated_strings`; sparse is fine).
5. Merges the export into the snapshot (English fill-in ignored; omitted keys
   kept; headers / `_version` preserved).
6. Opens/updates `crowdin/i18n` only when the working tree still differs.

Like the release workflow, the job reads its credentials from one 1Password
item referenced by the GitHub secret `OP_CROWDIN_SECRET_ITEM`. The item must
contain:

- `CROWDIN_PROJECT_ID` — the numeric Crowdin project id.
- `CROWDIN_PERSONAL_TOKEN` — a Crowdin API token with access to the project.

Grant the token only these scopes and restrict its granular access to the
OpenHID project:

- Projects (List, Get, Create, Edit) — Read.
- Translation Status — Read Only.
- Source files & strings — Read and Write.
- Translations — Read and Write.

Missing or invalid credentials fail the workflow. Translation PRs run the
normal CI checks, including the locale key parity test (every catalog must match
`en.toml` key-for-key). The workflow uses the existing `OP_GITHUB_APP_ITEM` to
mint a short-lived token for pushing its translation branch and opening the PR;
the default `GITHUB_TOKEN` remains read-only. Checkout runs with
`persist-credentials: false` and the origin remote is rewritten to the app token
so git push does not inherit the read-only Actions credential.

Local helpers (with Crowdin credentials configured):

```sh
devenv tasks run openhid:i18n-upload    # en.toml sources + per-language translations
devenv tasks run openhid:i18n-download  # download + merge + i18n tests
python3 .github/scripts/i18n/merge_crowdin_download.py --self-test
```
