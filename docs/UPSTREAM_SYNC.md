# Following OpenLogi upstream

OpenHID has independent Git history. `upstream` is the read-only source remote
`https://github.com/AprilNEA/OpenLogi.git`; `origin` is OpenHID. Fetch upstream
without tags so its releases do not become OpenHID release inputs:

```sh
git remote add upstream https://github.com/AprilNEA/OpenLogi.git # first setup only
git config remote.upstream.tagOpt --no-tags
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

## Reviewed through 2026-10-20

Last reviewed upstream commit:
[`1cabbd1bb3d8d0cd0dc01a9edbcd33122684b0f6`](https://github.com/AprilNEA/OpenLogi/commit/1cabbd1bb3d8d0cd0dc01a9edbcd33122684b0f6).

| Upstream change | Decision and OpenHID adaptation |
| --- | --- |
| [Low-battery notices, #1891](https://github.com/AprilNEA/OpenLogi/pull/1891) | Ported agent-owned notice lifetimes, once-per-discharge tracking, overlay cards and preferences. Reuses `openhid_core::battery::battery_needs_attention`, including freshness validation; stale or invalid readings cannot rearm a discharge. |
| [Lock-key indicators, #1898](https://github.com/AprilNEA/OpenLogi/pull/1898) | Ported HID++ 0x4220 decoding, per-connection baseline, agent observations, overlay indicator and preferences. Appends RPCs after OpenHID's tray and AULA methods under development protocol 38. |
| [Late reconnect response, #1899](https://github.com/AprilNEA/OpenLogi/pull/1899) | Ported connection-generation validation so an old lock-state read cannot populate the new connection's baseline. |
| [Wayland multi-display ring, #1906](https://github.com/AprilNEA/OpenLogi/pull/1906) | Ported output-specific layer-shell windows, shared pointer ownership and remembered compositor refusal. One display's failure does not suppress the other windows; ordinary windows remain the compositor fallback. |

All four changes were authored upstream by Xuan Zhang. OpenHID retains its
battery freshness semantics, native tray behavior, AULA support, independent
identity, and release configuration. This ledger records review decisions;
it does not establish hardware or native UI verification on another OS.
