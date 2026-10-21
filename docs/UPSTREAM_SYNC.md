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

The fetched tip is [`d2e66e7d`](https://github.com/AprilNEA/OpenLogi/commit/d2e66e7d),
35 commits beyond the reviewed checkpoint below. The disposition table below records
the review of this batch. Changes requiring broader product or lifecycle work remain
explicitly deferred; they are not implied to be ported.

1. **Protocol safety and data fidelity:** review is complete. Ported #1933,
   #1935, #1937, #1938, #1939, #1940, #1941, #1944, #1945, and #1947;
   #1936 was already ported in OpenHID PR #36. #1942 and #1946 are deferred
   for the lifecycle reasons in the disposition table below.
2. **Permission and input lifecycle:** #1919 is already implemented locally.
   #1918, #1926, #1929, #1920, #1921, and #1928 are deferred or skipped as
   documented below.
3. **Dependent UI features:** #1943 and #1880 are already covered locally.
   #1907, #1922, #1932, and #1934 are separate product features and were
   skipped for this protocol-focused sync.
4. **Toolchain alignment:** #1877 was reviewed and skipped; OpenHID follows its
   own current-stable floor policy.
5. **Upstream-only maintenance:** do not import the upstream Sparkle delta
   release work in #1925 or upstream release #1831. README badge edits are
   project-specific and were skipped.

The upstream webpki advisory fix in #1924 is already covered locally by
OpenHID commit `64aaa23f` (`build(deps): patch the webpki roots past
RUSTSEC-2026-0381`). This local dependency patch is recorded as the disposition
for upstream #1924.

## Review disposition (2026-10-21)

| Upstream change | Decision and OpenHID adaptation |
| --- | --- |
| [#1933](https://github.com/AprilNEA/OpenLogi/pull/1933), [#1935](https://github.com/AprilNEA/OpenLogi/pull/1935), [#1937](https://github.com/AprilNEA/OpenLogi/pull/1937), [#1938](https://github.com/AprilNEA/OpenLogi/pull/1938), [#1939](https://github.com/AprilNEA/OpenLogi/pull/1939), [#1940](https://github.com/AprilNEA/OpenLogi/pull/1940), [#1941](https://github.com/AprilNEA/OpenLogi/pull/1941), [#1944](https://github.com/AprilNEA/OpenLogi/pull/1944), [#1945](https://github.com/AprilNEA/OpenLogi/pull/1945), [#1947](https://github.com/AprilNEA/OpenLogi/pull/1947) | Ported protocol fidelity and device-state fixes. #1937 adds the effective-maximum owner guard. #1944 updates desktop settings reads when ratchet reports arrive. #1938 reads legacy battery capability before mapping levels. |
| [#1936](https://github.com/AprilNEA/OpenLogi/pull/1936) | Already ported in [OpenHID PR #36](https://github.com/Ben8368/OpenHID/pull/36); no duplicate changes. |
| [#1919](https://github.com/AprilNEA/OpenLogi/pull/1919) | Already implemented locally: agent relaunches after Input Monitoring is granted. |
| [#1920](https://github.com/AprilNEA/OpenLogi/pull/1920) | Skipped: OpenHID uses its own host-link and device-ordering model; the upstream workaround does not map to the local ordering contract. |
| [#1943](https://github.com/AprilNEA/OpenLogi/pull/1943) | Already covered locally: DPI writes wait for device confirmation before persisting. |
| [#1880](https://github.com/AprilNEA/OpenLogi/pull/1880) | Already handled by OpenHID's pointer-tab capability policy, including software scroll inversion. |
| [#1946](https://github.com/AprilNEA/OpenLogi/pull/1946) | Deferred: applying reports from other clients requires reconciling device session ownership, capture restoration, and inventory revision semantics. The upstream patch does not apply cleanly to these boundaries. |
| [#1942](https://github.com/AprilNEA/OpenLogi/pull/1942) | Deferred: haptic ownership spans the Actions Ring worker, device session lifecycle, and serialized HID operations. The upstream session-restore patch does not map cleanly onto OpenHID's worker/operation ownership and needs a dedicated lifecycle design. |
| [#1907](https://github.com/AprilNEA/OpenLogi/pull/1907) | Skipped for this sync: `[apps]` aliases add a new config/UI feature and need a separate migration and cross-platform UX review. |
| [#1918](https://github.com/AprilNEA/OpenLogi/pull/1918), [#1929](https://github.com/AprilNEA/OpenLogi/pull/1929) | Skipped: virtual HID keyboard typing and its signing profile are upstream-specific macOS input architecture and packaging changes. OpenHID's injection contract needs a separate macOS permission/signing review. |
| [#1921](https://github.com/AprilNEA/OpenLogi/pull/1921), [#1928](https://github.com/AprilNEA/OpenLogi/pull/1928) | Skipped: desktop switching is a new platform-specific gesture feature, not a protocol fix; it needs a standalone input/injection design and native verification. |
| [#1926](https://github.com/AprilNEA/OpenLogi/pull/1926) | Skipped: Secure Input status requires a separate macOS permission UI and localization review. |
| [#1922](https://github.com/AprilNEA/OpenLogi/pull/1922), [#1932](https://github.com/AprilNEA/OpenLogi/pull/1932), [#1934](https://github.com/AprilNEA/OpenLogi/pull/1934) | Skipped: pointer speed, precision mode, and DPI LED add a coordinated device/config/UI feature set; keep them for a separate product change. |
| [#1877](https://github.com/AprilNEA/OpenLogi/pull/1877) | Skipped: OpenHID tracks its current stable floor by its own release/toolchain policy; no need to raise it in this sync. |
| [#1925](https://github.com/AprilNEA/OpenLogi/pull/1925), [#1831](https://github.com/AprilNEA/OpenLogi/pull/1831) | Skipped: upstream Sparkle delta releases and release metadata are not part of OpenHID's release system. |
| [`7310a112`](https://github.com/AprilNEA/OpenLogi/commit/7310a112), [`cd8ed411`](https://github.com/AprilNEA/OpenLogi/commit/cd8ed411), [`5ca7a754`](https://github.com/AprilNEA/OpenLogi/commit/5ca7a754), [`15a43a1c`](https://github.com/AprilNEA/OpenLogi/commit/15a43a1c), [`b83454ad`](https://github.com/AprilNEA/OpenLogi/commit/b83454ad) | Skipped: README badge ordering and language-switcher changes are project-specific. |

The reviewed checkpoint for this batch is [`d2e66e7d`](https://github.com/AprilNEA/OpenLogi/commit/d2e66e7d). Deferred items remain listed above for focused follow-up work.

The newly ported protocol and device fixes in this batch were authored upstream
by Xuan Zhang. OpenHID's adaptations retain their original upstream links above;
no upstream commit objects or release history were imported.

## Port verification (Windows, 2026-10-21)

- `cargo test -p openhid-hidpp`: 346 passed; one documentation example ignored.
- `cargo test -p openhid-device`: 597 passed.
- `cargo test -p openhid-desktop state::tests::smartshift`: 2 passed.
- `cargo clippy -p openhid-cli -p openhid-desktop -p openhid-device -p openhid-hidpp --all-targets -- -D warnings`: passed.
- `cargo fmt --all -- --check`, `git diff --check`, and `cargo xtask ci ast-grep`: passed.
- Final push gate: `cargo clippy --workspace --all-targets -- -D warnings`,
  `cargo test --workspace`, and non-GUI workspace rustdoc all passed with
  `RUSTFLAGS=-D warnings` (and `RUSTDOCFLAGS=-D warnings` for rustdoc).
- `cargo xtask ci wasm ast-grep`: both jobs passed. The new illumination guard
  rejected a temporary prohibited copy and accepted unrelated code.

The final workspace gate passed on Windows. Hardware behavior and native
macOS/Linux paths were not exercised.

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
