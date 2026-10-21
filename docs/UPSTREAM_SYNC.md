# Following OpenLogi upstream

OpenHID has independent Git history. `upstream` is the read-only source remote
`https://github.com/AprilNEA/OpenLogi.git`; `origin` is OpenHID. Fetch upstream
without tags so its releases do not become OpenHID release inputs:

```sh
git remote add upstream https://github.com/AprilNEA/OpenLogi.git # first setup only
git config --replace-all remote.upstream.fetch '+refs/heads/master:refs/remotes/upstream/master'
git config remote.upstream.tagOpt --no-tags
git remote set-url --push upstream DISABLED
git fetch upstream master --no-tags
git log --reverse --oneline <last-reviewed-upstream-sha>..upstream/master
git diff <last-reviewed-upstream-sha> upstream/master
```

Review each upstream change against OpenHID's current implementation. Port the
relevant change in a temporary branch/worktree, map the renamed crate paths,
and preserve local features and the original author's credit. Run the repository
local gate and affected boundary checks before pushing; merge after current-head
CI and review. Delete the temporary branch after its changes reach `master`.
The source remote and its tracking ref may remain; they are not OpenHID branches.

Do not merge unrelated upstream history, replace the full source tree, import
upstream tags, or restore its brand assets, release services, credentials, and
update feeds. IPC methods and serialized variants keep OpenHID's existing order;
upstream indices are not OpenHID's indices. The original import base remains
recorded in [PROVENANCE.md](PROVENANCE.md).

## Current queue (2026-10-21)

The fetched tip is [`0cbef3a5`](https://github.com/AprilNEA/OpenLogi/commit/0cbef3a5925a629eec44c8f773426b007b2c6c9a),
34 commits beyond the reviewed checkpoint below. The reviewed checkpoint stays
unchanged until each change has been compared with OpenHID and given a port or
skip decision. Work in this order:

1. **Protocol safety and data fidelity:** review upstream #1933 (preserve
   unmodelled backlight bits), #1936 (safe 0x8070 zone writes), #1937
   (illumination maximum), #1939 (0x2111 torque bounds), #1945 (raw firmware
   bytes), and #1941 (versioned battery status bytes). Then assess #1935
   (thumb-wheel reporting), #1940 (disabled-key reports), #1946 (publish
   0x1b04 reporting changes made by other clients), #1944 (short movements and
   SmartShift), #1947 (DPI calibration broadcasts), and #1942
   (haptic restoration). These device and HID++ boundaries should be reviewed
   before UI behavior that depends on their state.
2. **Permission and input lifecycle:** review #1919 (restart after Input
   Monitoring is granted), #1918 (virtual HID keyboard), #1926 (Secure Input
   status), #1929 (macOS agent signing), #1920 (linked-device host switching),
   and #1921 (desktop switching, with its regression coverage in #1928). Keep
   the macOS permission and signing rules in force for any port.
3. **Dependent UI features:** review #1943 (save DPI only after confirmation),
   then #1922, #1932, #1934, and #1880 (pointer speed, precision mode, DPI LED,
   and pointer-tab visibility). Review #1907 (per-app profile aliases) as a
   separate config and UI change.
4. **Toolchain alignment:** assess #1877's Rust 1.99 floor against OpenHID's
   current stable/MSRV policy before changing the toolchain files.
5. **Upstream-only maintenance:** do not import the upstream Sparkle delta
   release work in #1925 or upstream release #1831. Review README badge edits
   only if OpenHID chooses to change its own project pages.

The upstream webpki advisory fix in #1924 is already covered locally by
OpenHID commit `64aaa23f` (`build(deps): patch the webpki roots past
RUSTSEC-2026-0381`). This is a local adaptation, not a reason to move the
reviewed checkpoint past the other pending changes.

## Reviewed through 2026-10-20

Last reviewed upstream commit:
[`1cabbd1bb3d8d0cd0dc01a9edbcd33122684b0f6`](https://github.com/AprilNEA/OpenLogi/commit/1cabbd1bb3d8d0cd0dc01a9edbcd33122684b0f6).

| Upstream change | Decision and OpenHID adaptation |
| --- | --- |
| [Low-battery notices, #1891](https://github.com/AprilNEA/OpenLogi/pull/1891) | Ported agent-owned notice lifetimes, once-per-discharge tracking, overlay cards and preferences. Reuses `openhid_core::battery::battery_needs_attention`, including freshness validation; cached or invalid percentage values alone cannot rearm a discharge. |
| [Lock-key indicators, #1898](https://github.com/AprilNEA/OpenLogi/pull/1898) | Ported HID++ 0x4220 decoding, per-connection baseline, agent observations, overlay indicator and preferences. Appends RPCs after OpenHID's tray and AULA methods under development protocol 38. |
| [Late reconnect response, #1899](https://github.com/AprilNEA/OpenLogi/pull/1899) | Ported connection-generation validation so an old lock-state read cannot populate the new connection's baseline. |
| [Wayland multi-display ring, #1906](https://github.com/AprilNEA/OpenLogi/pull/1906) | Ported output-specific layer-shell windows, shared pointer ownership and remembered compositor refusal. One display's failure does not suppress the other windows; ordinary windows remain the compositor fallback. |

All four changes were authored upstream by Xuan Zhang. OpenHID retains its
battery freshness semantics, native tray behavior, AULA support, independent
identity, and release configuration. This ledger records review decisions;
it does not establish hardware or native UI verification on another OS.

## Port verification (Windows, 2026-10-20)

The port was rebased onto `6665b282` after the AULA and dependency integrations.
With `RUSTFLAGS=-D warnings`, the full local gate passed:

- `cargo fmt --all -- --check`
- `cargo clippy --workspace --all-targets -- -D warnings`
- `cargo test --workspace`: 2,852 passed, zero failed; two existing documentation examples remained ignored.
- `cargo doc --workspace --no-deps --document-private-items --exclude openhid-ui --exclude openhid-desktop --exclude openhid-overlay --exclude openhid-agent`, with `RUSTDOCFLAGS=-D warnings`.
- `cargo xtask ci ast-grep wasm wire i18n clippy-windows cargo-deny`: six passed, zero failed, zero skipped. Wire-format tests passed all 30 goldens; all 24 locale catalogs passed parity checks.

The 62 structural guards passed. Temporary copies in another crate verified
that battery thresholds, device labels (including renamed functions and
translated labels), and compositor-error classification are rejected outside
their owners. Independent tab labels and unrelated code still passed. The
lockfile contains one GPUI core identity and one Kit source revision.

The stale-battery regression failed before the notice rearm fix and passed
afterward. Overlay tests passed 62 cases, including two-window pointer ownership,
partial opening failures, dismissal, and disconnect cleanup.

The optimized desktop and mock agent started in an isolated development profile.
Native screenshot review was unavailable because the interactive desktop was
locked. Physical-keyboard notices and native macOS/Linux/Wayland behavior remain
unverified; this port was not runtime-tested on hardware. Installer packaging
was not performed. The existing AULA acceptance record and historical follow-ups
remain in [the device notes](devices/aula-f87-pro.md).
