# Origin and independent maintenance

OpenHID is derived from [AprilNEA/OpenLogi](https://github.com/AprilNEA/OpenLogi)
and maintained independently by [Ben8368](https://github.com/Ben8368) and
OpenHID contributors. This repository owns its development direction, issue
tracking, and future releases. Upstream changes are not automatically synced.
Later reviewed ports and the last reviewed upstream commit are recorded in
[Following OpenLogi upstream](UPSTREAM_SYNC.md).

The recorded upstream base is
[`abc815acf9ebb4df06ccc574016bad2c9b4152ee`](https://github.com/AprilNEA/OpenLogi/commit/abc815acf9ebb4df06ccc574016bad2c9b4152ee)
(2026-10-18). The starting local fork snapshot is
[`11db0bbebe10d761c0830ded6cc9cee39f38e160`](https://github.com/Ben8368/OpenLogi/commit/11db0bbebe10d761c0830ded6cc9cee39f38e160).
The initial OpenHID import includes local fork changes plus the project rename,
new artwork, source notices, and separation from upstream release services.
It must not be described as an exact, unchanged upstream import.

The new Git repository starts with one initial import commit. It does not
contain upstream branches, tags, or earlier commits. Earlier development
history remains with the original projects. The inherited changelog is stored
at [upstream/OpenLogi-CHANGELOG.md](upstream/OpenLogi-CHANGELOG.md); entries there
are upstream history, not OpenHID releases.

## Attribution and licenses

- Retain [LICENSE-MIT](../LICENSE-MIT), [LICENSE-APACHE](../LICENSE-APACHE), and
  [NOTICE](../NOTICE) when distributing source and include the applicable
  notices with binaries. Original copyright notices remain intact.
- `crates/openhid-hidpp` retains its upstream 0BSD license and `hidpp` library
  name. Its own README and LICENSE record the earlier `lus/logy` origin.
- Imported skills retain their original licenses and `skills-lock.json`.
- External dependencies keep their published identities, including
  `openlogi-async-hid`. Renaming this project does not rename that dependency.
- The OpenLogi brand artwork has been replaced by original OpenHID artwork.
  [The original brand notice](upstream/OpenLogi-brand-LICENSE.txt) is retained
  as a record; it grants no permission to use those original assets.

## Scope of the import

The inherited implementation manages Logitech HID++ peripherals and UVC
cameras. The OpenHID name expresses the project's direction; it does not
establish support for every HID device. AULA F87 PRO support has not been
implemented by this rename.

Package names, executable names, configuration directories, IPC endpoints,
application IDs, launch services, and Windows MSI upgrade IDs use OpenHID
identities. OpenLogi settings and permissions are not migrated automatically.
Do not run two agents controlling the same physical device simultaneously.

## Project services

GitHub Release tags may exist in this repository. OpenHID-branded installer
assets use `openhid-…` names (see [INSTALL-linux.md](INSTALL-linux.md)); a tag
that only carries inherited `openlogi-…` filenames is not an OpenHID package
set. The built-in updater has no default OpenHID manifest URL — a release that
enables in-app updates must set `OPENHID_UPDATE_MANIFEST_URL` and its own
`OPENHID_UPDATE_MINISIGN_PUBLIC_KEY` (and its own Sparkle feed/key on macOS).
Inherited Crowdin and upstream-only release workflows are not OpenHID
distribution channels. Every workspace crate is marked `publish = false`.

Device-image mirrors remain third-party sources and keep their original URLs.
They are separate from the removed upstream brand artwork and update feeds.
