# Initial import verification

Verification date: 2026-10-19. Host: Windows x86_64, Rust 1.99.0.
Source origin and licenses are recorded in [PROVENANCE.md](PROVENANCE.md).

## Automated checks

- `cargo test --locked --workspace`: 2,694 tests passed, no failures;
  two doctests were ignored by their existing annotations.
- `cargo clippy --locked --workspace --all-targets -- -D warnings`: passed.
  After the final desktop corrections, the same check for
  `-p openhid-desktop` also passed.
- `cargo fmt --all -- --check`: passed.
- `ast-grep scan`: passed. The update configuration ownership guard was also
  checked against an allowed owner and a prohibited consumer fixture.
- `cargo build --locked -p openhid -p openhid-desktop -p openhid-agent --bins`:
  passed. The desktop was rebuilt after its final corrections.
- `openhid --help`: exposes the OpenHID CLI name and commands.
- Parsed 56 TOML, 48 JSON, 75 YAML, and 77 XML/SVG/plist files; checked
  syntax of nine Bash files. No parse errors were found.
- All 21 workspace packages use this repository's metadata and are marked
  `publish = false`. Original source licenses, imported guidance, and
  `skills-lock.json` were compared with the starting source and are unchanged.

## Pre-push verification

The complete Windows gate was run with `RUSTFLAGS="-D warnings"` and
`RUSTDOCFLAGS="-D warnings"`: formatting, workspace Clippy, all workspace
tests, and non-GUI rustdoc. The final test count remains 2,694 passed and two
ignored doctests. Additional checks passed for spelling, ast-grep, ShellCheck,
shfmt, cargo-deny, the publish closure (zero publishable workspace packages),
and the wasm portable crates. The configured pre-push hooks were exercised.

The gate exposed a test race: an already queued mock reply can complete a
send on its first poll, while the test required a pending intermediate state.
The test now awaits completion and retains its reply, write-count, and queue
assertions. All eight same-header channel tests passed after the correction.
No production protocol code was changed.

On Windows, the source-import links are materialized as target text because
`core.symlinks=false`. The end-of-file hook excludes those link paths so it
cannot append a newline to a Git symlink target. Their original Git symlink
modes and targets are retained.

The Windows xtask shell wrapper exceeded the native command-line limit;
ShellCheck and shfmt were run over the same tracked script set using a batched
file-discovery step. The publish-closure wrapper hit Windows' running-executable
file lock; its direct `cargo xtask release check-publish` command passed.

## First-push CI correction

The initial GitHub CI run failed in the path-filter action before `cargo fmt`
ran: the first push has no previous commit and the event still named the
empty repository's original default branch, `main`. The action attempted to
fetch that nonexistent branch. The dependent Linux, Windows, and macOS test
jobs were skipped, so that run did not prove those tests passed. Its other
jobs, including Clippy on all three platforms, MSRV, rustdoc, cargo-deny,
wasm, and publish closure, passed.

Default-branch pushes now schedule all test jobs without a history comparison.
Only pull requests use path filtering, against their explicit base commit.
This also supports replacing the initial root commit. GitHub's expression
library verified first-push, rewritten-root, normal-push, source-PR, and
documentation-PR cases; the PR filter patterns were checked unchanged.
`actionlint` validated the corrected CI workflow.

## Native UI observation and limits

A mock agent and an isolated Chinese-language configuration were used for a
Windows desktop smoke check. The device home screen displayed the OpenHID
window title, replacement icon, and five synthetic devices. The Settings and
About screens were not verified interactively.

The normal debug desktop executable overflowed its main-thread stack during
startup. For the home-screen observation only, it was relinked with
`cargo rustc --locked -p openhid-desktop -- -C link-arg=/STACK:16777216`.
This was a temporary debug executable setting, not a source fix. The cause
and normal debug startup remain unresolved; the observation does not prove
that a release installer starts successfully.

Linux/macOS runtime behavior, platform installers, and macOS Icon Composer
output were not validated. No physical-device HID writes or AULA F87 PRO
protocol support were tested. This import does not configure or publish a
release channel.
