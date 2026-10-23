# AULA F87 PRO: experimental support

Investigation and verification: October 19–23, 2026. Overall audit: **YELLOW**.
Recognition, complete configuration reads, recoverable built-in lighting,
wireless sleep, debounce and side-light effect edits, configuration-block
backup/restore, and a desktop settings editor are implemented. Full OEM
application replacement remains in progress;
the acceptance table below tracks the missing and not-yet-verified workflows.

OpenHID recognizes the measured 2.4 GHz receiver control collection as one
standalone keyboard. It never enters Logitech HID++ probing or pairing. The
agent owns device I/O; the desktop and overlay remain IPC clients. AULA lighting,
settings, per-key RGB and base/Fn keymap calls are appended to the service;
matching client and agent builds are required (development protocol version 41).

| Property | Verified value |
| --- | --- |
| Vendor/product | `3554:FA09` |
| Control usage | `FF02/0002` |
| Input/output length | 20 bytes, including report ID `13` |
| Driver | `aula-f87-pro-wireless` |

The neighboring `FF04/0002` feature collection and standard input
collections are excluded. The shared wired VID/PID `258A:010C` is never
claimed from identity alone: standalone inventory probes its `FF00` vendor
collections and includes only the collection(s) that answer with the F87 Pro
model code.
The keyboard also advertises Bluetooth pairing, but the installed F87 Pro
application profile identifies only wired `258A:010C` and the 2.4 GHz receiver
`3554:FA09`; no Bluetooth `FA08` profile or Bluetooth configuration workflow is
present in the installed package. The manufacturer's [F87 three-mode card](https://www.aulastar.com/uploads/soft/20240622/1-240622141341305.pdf)
confirms that Bluetooth is a keyboard connection mode, but does not establish
that this OEM application can configure the keyboard over Bluetooth. Current
replacement parity therefore covers the two transports the installed profile
supports; Bluetooth configuration remains outside that demonstrated OEM scope.

For wired diagnostics, the read-only `0x06/0x82` probe now accepts only the
F87 Pro model byte (`0x0B` at response offset 13), in addition to the echoed
selector and family checks. This keeps the keymap upload gate from treating
the related F87 TK model (`0x66`) as an F87 Pro. The model byte is documented
as hardware-verified by the [independent F87 controller protocol](https://github.com/RadianceADM/aula-f87-control/blob/main/docs/PROTOCOL.md);
OpenHID has not yet observed that response from this keyboard.
The wired protocol layer can read and replace the full 128-byte healthy
configuration block. Persistent writes capture the original block, verify
readback and attempt exact restoration on failure. Wired candidates now enter
standalone inventory only after the model query returns the F87 Pro code. The
desktop lighting panel uses the Agent/IPC path for supported built-in lighting,
sleep, and side-light fields; it hides wireless-only editors and the unverified
debounce control. No wired candidate has been connected for physical validation,
so discovery, settings, visible output, and persistence remain unverified.

## Current delivery scope

The current milestone covers the recognized wireless receiver, built-in lighting,
idle timeout, debounce and side-light effect selection, recoverable configuration
transactions, configuration backup/restore, local base/Fn keymap editing,
experimental base/Fn keymap writing with readback/recovery, matrix inspection/export, and base/Fn keymap
migration between OpenHID profiles and OEM local profile files. Built-in lighting
controls are
filtered by the OEM `LedOpt` capability table, including single-color and colorful
mode selection where the OEM table advertises them. The desktop color picker stages
a custom color and applies it with the selected effect. Palette upload currently
uses the 37-report template from the MIT-licensed F75-family Aula-Manager reference;
the palette has no readback and this transfer has not been verified on the physical
F87 PRO. A color-mode change cannot use Preview because the palette cannot be restored.
The desktop also exposes an
experimental per-key RGB table editor on a drawn F87 layout. Its draft starts
black, supports assigning palette colors to individual physical keys, then
writes the complete 126-entry table and switches to custom mode. Ctrl-click
(Command-click on macOS) selects several keys to stage one color across them.
The OEM game-key preset selects Esc, WASD and the four arrows and stages the
installed red `clrGaoshou` value into those nine local table entries; applying
the complete RGB table remains a separate action.
The other 39
unmapped table indexes are also available in a separate index picker. The 87
OEM key indexes match the captured base-matrix slots; physical RGB output for
those indexes is not yet verified. The desktop can save, select, overwrite,
import, export and delete named local RGB profiles in the same versioned JSON
format accepted by the CLI; selecting or importing a profile only changes the
draft. Neither path can read the device colors back. The F87 write protocol is
not yet proven on hardware.
The wireless keymap editor reads both layers into a draft and supports individual
standard keyboard, modifier, media and Fn actions, common modifier/key chords,
plus an advanced four-byte raw
  action editor for unclassified encodings. A read-only Agent RPC on an awake
  receiver has physically validated both 126-entry layers. The desktop can now write both
layers through the receiver's advertised `0x13` report, after checking the live
F87 Pro PSD and reading a complete snapshot. It rejects a stale draft unless
the live map still matches the map loaded for editing. It waits for each
fragment ACK, waits 15 ms before each fragment and 70 ms after the last, verifies
both layers by readback, and attempts to restore the snapshot after a failure. The pacing
matches the same-model Linux reference. The direct write path remains
experimental; the installed Windows OEM's `0x15` frame remains offline-only
because that report ID is absent from the receiver descriptor. The direct
writer and physical key behavior have not been validated; the Fn table read
path has now been confirmed.
The editor and CLI now label the two Alt+Esc/Alt+Tab action records confirmed in
the installed OEM Fn profile; every other unresolved action stays raw.
Read-only inspection of the installed `profile.dct` found 15 of its 87 Fn
  records tagged with OEM action page `0x08` (four-byte values such as
`00020308`, `01000208`, and `02000408`). The nearby OEM action classifier at
`0x425c60` recognizes only page `0x02` records in the inspected path; a bounded
static scan found no page-`0x08` comparison in the adjacent keymap code. The
records' source-key positions do not identify their assigned combinations, so
OpenHID preserves these values as raw actions and does not label them from the
  OEM slot names. Portable keymap profile version 4 retains each source OEM Fn
  action alongside its converted matrix value, so ambiguous page-`0x08` bytes
  can be exported unchanged unless the corresponding Fn key is explicitly
  edited. Versions 1–3 remain readable; their two trailing Fn slots are dropped
  only when both are zero, and profiles with assignments in those nonexistent
  wireless slots are rejected instead of losing data. When older versions lack
  source-action metadata, export retains the selected template's original Fn
  action wherever it still matches the matrix value. The advanced editor accepts exactly four bytes for inspection
and local-profile editing; this does not establish that the keyboard accepts
those bytes over the unresolved write protocol.
Further static tracing found the OEM validation log's internal field names:
`Major`, `Minor`, `Combo{modify,vk1,vk2}`, and `Parameter`. Its action-label
helper recognizes `Major=0x08, Minor=0x0000A2` as `disable`; none of the 15
sample page-`0x08` actions has that value. A separate OEM dialog path handles
`Major=0x08, Minor=0x0000A4`: it populates three selectors from the following
parameter field (one byte, one byte, then a 16-bit value). The local `.dct`
stores a 16-byte record per key, so the four-byte wireless action does not
describe the full OEM row; the sampled page-`0x08` records have either a zero
parameter or `0x09000001`, but none uses the `0xA4` action code. This establishes
that some OEM actions carry data beyond the four-byte matrix entry, without
proving how the 15 sampled actions map to the selector fields or how that
extended action is transported to the wireless device. OpenHID now exposes the
confirmed `0x08/0xA2` disable action by name; other unresolved four-byte values
remain raw. It retains the other OEM row bytes as opaque metadata through local
and portable keymap profiles; export reapplies them to a validated template
while regenerating the four-byte action field.
The `0x08/0xA4` dialog handler packs its parameters as `modify:u8`, `vk1:u8`,
and `vk2:u16` little-endian in the separate four-byte parameter field. The
desktop OEM-row editor now exposes those fields when the selected action is
`0x08/0xA4`, while retaining the raw 12-byte row editor. This edits local OEM
profile metadata for export only; the parameter-bearing action is not applied
to the keyboard because its wireless representation remains unresolved.
Named local keymap profiles can be saved,
selected, imported, exported and deleted; those profile actions never access the
device. Local macro profiles can be imported, inspected and edited, but applying
macros to the keyboard and playback controls remain unavailable. Wired
configuration now has experimental inventory, Agent/IPC and desktop lighting
paths, but no `258A:010C` device has been connected for validation. The desktop
can import an OEM base profile or validated report, edit representable wired
single-key actions, export the report and upload it through the Agent after an
explicit warning; the keyboard has no reliable keymap readback or rollback.
Bluetooth configuration and firmware tools remain
deferred. The installed F87 profile blocks the OEM audio worker, so
music-synced lighting is not currently treated as a replacement requirement.
The acceptance table below tracks the remaining gaps.

## OEM replacement acceptance

The target remains replacement of the installed F87 PRO application. A passing
read or an offline test is not evidence that the following whole workflow works.
The installed `Cfg.ini`, `KB.ini` and English/Chinese text catalogs establish the
available concepts; visibility and transport restrictions still need UI/hardware
confirmation rather than assuming every generic catalog entry applies.

| Workflow | Current evidence | Remaining acceptance |
| --- | --- | --- |
| One keyboard in inventory | Strict receiver collection identity, complete live config probe, injected inventory tests, and real-agent CLI/GUI inventory agreement | Native reconnect and sleeping-device recovery |
| Built-in lighting and persistent settings | Exact echoes, complete staged/saved readback; real brightness change and restoration; effect-specific level and color-mode controls use OEM `LedOpt` capabilities; Agent settings and keymap reads now share the per-route queue used by writes | Human-visible effect/speed/off checks, physical custom-color and colorful-mode writes, power-cycle persistence |
| Wireless sleep | OEM option table; Rust 300-to-600-second SAVE with independent readback and full original restoration; native Chinese editor reads the original 300 seconds | Power-cycle persistence and physical writes initiated from the desktop |
| Debounce | OEM `ShowDebounce=0x04000101`; an independent F87 controller reference maps config fragment 0, byte 8 values 0–4 to 1–5 ms; OpenHID decodes and stages that byte through the complete snapshot transaction | Confirm the field and units against a physical F87 OEM edit, then verify desktop and CLI writes and power-cycle persistence |
| Side-light effect | The [independent F87 controller protocol](https://github.com/RadianceADM/aula-f87-control/blob/main/docs/PROTOCOL.md) verifies configuration byte 18 values 0–4 (off, rainbow wave, rainbow together, solid, breathing); OpenHID decodes, stages and exposes the value in both the desktop panel and CLI through the complete snapshot transaction | Compare readback and visible output on an awake F87 Pro, then verify desktop/CLI writes and power-cycle persistence; side-light color, brightness and speed are not exposed |
| Battery | The [independent F87 controller protocol](https://github.com/RadianceADM/aula-f87-control/blob/main/docs/PROTOCOL.md) documents a read-only `0x13/0x4a` command returning 1–100% and a cable/on-battery marker; AULA settings readback and standalone inventory now feed the existing battery indicators | Confirm report values and power-source transitions on the physical receiver; the protocol does not distinguish active charging from full while cable-powered |
| Board key assignments and combinations | Physical wireless read of both 126-entry layers in 36 frames each; all 87 physical keys correlated to OEM slot coordinates; local base+Fn editor and named profiles; standard keyboard, keypad and consumer usages plus an advanced raw four-byte editor; the current writer verifies the live F87 Pro PSD, rejects stale editor snapshots, writes 20-byte `0x13/0x01` fragments with per-fragment ACKs and reference pacing, reads both layers back, and attempts restoration on failure. A same-model Linux implementation uses this advertised wireless carrier; the installed Windows OEM's `0x15` carrier remains an offline plan | Verify ACK framing, visible key behavior, wireless writes, recovery and persistence on hardware. The 520-byte feature-report capture in [aula-keybind](https://github.com/vndarkblue/aula-keybind/blob/main/PROTOCOL.md) targets `258A:010C/010D`; it does not establish the wireless protocol for this `3554:FA09` receiver |
| Macro library, editing and playback | Desktop imports `.mcr`, `.dct`, and portable JSON into named local profiles; tree profiles retain all node headers, support offline folder/macro creation, rename and leaf-node deletion, and let the user select/edit macro nodes by decoded OEM node path (with ordinal fallback) while exporting the full tree. CLI creates folders and empty macros, deletes leaf nodes, inspects node and parent IDs, names, kinds, extracts `.mcr` blocks, and edits raw events by node index. The installed `mac.dct` is zero-byte and imports as an empty tree; a locally created two-node tree round-tripped byte-for-byte. A read-only inventory of the installed F87 PRO directory and AULA user-data roots found no additional `.mcr` or `.dct` samples. The OEM-editor-created 128-byte `.mcr` sample round-trips byte-for-byte through OpenHID (SHA-256 `0E0F8B248A017EDFC982B2884687C14289DC84396B8FCC5B51E6A5D274CD37DE`); inspection shows one `0xF0` mouse-left record (`value=0x0045`) followed by one all-zero row, while the OEM editor displays one event. This validates a real non-empty container and one event code, not event timing semantics or the inferred non-empty `.dct` tree. Static tracing confirms the OEM writes 20-byte `0x15` frames with `WriteFile` and matches responses by command byte. After the F87 was awakened, an Agent-owned experimental `0x13/0x43` read collected all 37 expected fragments and returned a 512-byte all-zero prefix. The wired CLI reads the candidate table as eight `0x06/0x85` chunks and now has an experimental `0x06/0x05` writer: it saves a full 4096-byte backup, checks that the table has not changed, verifies full readback and attempts restoration on failure. The candidate follows a same-model community 520-byte report format that conflicts with the installed OEM's `0x407`-byte macro frame; it remains unverified on attached wired hardware | Validate a non-empty OEM `mac.dct` tree and the wired macro table read/write, recovery, key binding and physical playback on an attached F87 Pro. The wireless prefix remains incomplete; resolve its schema and full backup path before considering any wireless macro write |
| Custom/per-key RGB | Complete 126-color model, full-table wireless writer, Agent/IPC path, desktop keyboard editor, local named profile save/select/import/export/delete, and versioned CLI profile import; all 87 OEM key indexes match their captured base-matrix slots | Verify `0x02/0x1c` and effect 21 on F87 hardware, visible per-key output and persistence; establish behavior of 39 unmapped table indexes, RGB read/backup or reliable recovery; exercise desktop profile lifecycle |
| Profiles and backup/restore | Validated configuration-block restore, base-matrix export, local portable base+Fn keymap profiles, and OEM DCT base+Fn export that preserves every other template byte | Decode remaining OEM profile fields for lighting and macros; reliable device read/restore of all Fn/macro/RGB regions and complete device restoration |
| Music-synced lighting | Installed driver contains `CMusicDlg`, `CAudioClient` and `CMusicDlg::StartMusic`; English catalog lists gain, color/colorful mode, effect, smoothness and 11 music-mode labels. All three installed F87 profile files omit `MusicUI` and `LightBarMusicCmd` (both default to 0). In `StartMusic`, worker creation is gated on `ShowPower=1` and `MaxFnNum=0`; F87 sets `ShowPower=0`, while `MaxFnNum` defaults to `0x30` | The installed F87 configuration keeps the OEM audio worker disabled, so audio-reactive lighting is not currently an F87 replacement requirement. Revisit only if direct OEM UI or hardware evidence shows this model enables it |
| Wired key assignments | The installed app has separate `F87PRO` and `wired` profiles for the same `258A:010C` VID/PID; their six-byte `Psd` values differ (`03 00 00 00 01 0b` vs. `03 00 00 00 00 a9`). Static disassembly confirms its matcher compares VID/PID and all six `Psd` bytes. The OEM discovery query is `0x13/0x05`; `openhid diag aula --psd` implements the same wireless request shape and returned the wireless fingerprint (`03 00 00 00 01 0b`) after wake. The `0x13/0x44` config read still times out, and neither response establishes the wired fingerprint. A read-only `HidD_GetFeature` on the current wireless receiver's `FF04/0002` returned report `06 00 02 00 e2 00 56 29`; this does not match either profile fingerprint. Desktop and CLI can import the OEM base profile or a 520-byte `0x06` report, edit only supported single-key outputs, and save the report; the desktop uploads through the Agent only after an exact wired-route model gate and explicit confirmation. The keyboard has no reliable keymap readback or rollback. The report geometry comes from [aula-keybind](https://github.com/vndarkblue/aula-keybind), whose protocol prose overcounts slots: 128 slots plus header and trailer exceed 520 bytes, while its [implementation](https://github.com/vndarkblue/aula-keybind/blob/main/src/aula_keybind/protocol.py) allows complete slot indexes only through 126 | Confirm the `0x06/0x82` response against the installed F87 Pro wired profile, then verify upload behavior and visible key actions on a directly attached F87 Pro; this path has not been exercised on hardware and does not apply to the current `3554:FA09` receiver |
| Wired lighting/configuration | The wired protocol layer reads and validates all 128 config bytes and can write one complete healthy block after the F87 Pro model query, preserving unknown bytes and restoring the original block if write/readback fails; the `aula-wired` CLI can export and restore binary backups; Agent/IPC read and write RPCs route through the Agent per-route worker; standalone inventory probes `FF00` candidate collections and exposes only validated F87 Pro responses; the desktop panel uses the RPCs for supported lighting effect/level, sleep and side-light edits while suppressing unsupported/unverified wireless-only controls | On a directly attached F87 Pro, confirm inventory, read settings, verify visible output, readback and persistence; compare the queried model response with the installed wired profile's OEM discriminator |
| Bluetooth configuration | The keyboard supports Bluetooth pairing, but the installed OEM package has profiles only for wired `258A:010C` and 2.4 GHz `3554:FA09`; no Bluetooth configuration path is present | No Bluetooth workflow is required for parity with this installed application unless direct OEM behavior shows otherwise |
| Firmware tools | Both F87 `KB.ini` profiles comment out `FwVerNeedToUpdate` (wireless threshold `0x11ff`, wired `0x1ff`); the driver reads the active value with default `0`, and the profile sets `Fw=24`. The user-click handler checks `FwVerNeedToUpdate` only in its local-update branch (`UpdateByLocal != 0`), compares it with the connected firmware version, and skips when the current version is greater. Installed `Cfg.ini` sets `UpdateByLocal=0` and `EnableYun=0`. The generic executable builds a remote `update_version.xml` URL, but no F87 firmware image is installed; a read-only GET to the default constructed manifest path returned HTTP 404 on 2026-10-23, which does not prove the updater is permanently disabled | Determine whether the remote updater exposes an F87 firmware offer under the installed profile/config, then establish its artifact source, model/version checks and recovery procedure before treating firmware update as F87 parity; no firmware writes implemented |
| Desktop usability | IPC editor tests plus real-agent Chinese UI reads, complete effect/sleep menus, dark normal and light 125% layouts; Windows debug desktop now links with a 4 MiB stack reserve for the measured generated-locale initializer frame | Physical writes initiated from the desktop, live read-failure recovery and runtime confirmation of the updated debug startup |

## Commands and desktop behavior

From this checkout:

```sh
cargo run -p openhid -- list
cargo run -p openhid -- diag aula
cargo run -p openhid -- diag aula --psd # read-only OEM wireless model query
cargo run -p openhid -- diag aula --wired-model-probe # read-only wired Feature Report 0x06/0x82 query
cargo run -p openhid -- aula-wired read-config --identity 'IDENTITY_FROM_WIRED_MODEL_PROBE' --output f87-wired-config.bin
cargo run -p openhid -- aula-wired write-config --identity 'IDENTITY_FROM_WIRED_MODEL_PROBE' --input f87-wired-config.bin --apply # validates, snapshots, writes, reads back and recovers on failure
cargo run -p openhid -- aula-wired read-macros --identity 'IDENTITY_FROM_WIRED_MODEL_PROBE' --output f87-wired-macros.bin # experimental opaque 4096-byte table read
cargo run -p openhid -- aula-wired write-macros --identity 'IDENTITY_FROM_WIRED_MODEL_PROBE' --input edited-macros.bin --backup f87-wired-macros.original.bin --apply # experimental, full backup + readback + restore on failure
cargo run -p openhid -- diag aula --side-light-effect 1 --save
cargo run -p openhid -- diag aula --matrix # experimental read-only raw matrix
cargo run -p openhid -- diag aula --matrix --backup f87-base-matrix.json
cargo run -p openhid -- diag aula --oem-matrix-probe --backup f87-oem-carrier-matrix.json # Linux/Windows raw-HID, read-only
cargo run -p openhid -- diag aula --oem-keymap-probe --backup f87-oem-keymap.json # Linux/Windows raw-HID, read-only base+Fn probe
cargo run -p openhid -- diag aula --import-oem-profile profile.dct --export-keymap f87-keymap.json
cargo run -p openhid -- aula-keymap export-oem-profile --input f87-keymap.json --template profile.dct --output f87-edited.dct # offline; preserves non-keymap fields
cargo run -p openhid -- aula-keymap factory-template --layer base --output f87-base.bin
cargo run -p openhid -- aula-keymap import-oem-base --input profile.dct --output f87-base.bin # offline base-layer conversion; rejects unsupported actions
cargo run -p openhid -- aula-keymap set-action --input f87-base.bin --output f87-base-edited.bin --key A --action f24
cargo run -p openhid -- aula-keymap write-wired --input f87-base-edited.bin --identity 'IDENTITY_FROM_WIRED_MODEL_PROBE' --apply # persistent Feature Report write; no reliable readback
cargo run -p openhid -- diag aula --keymap
cargo run -p openhid -- diag aula --keymap --backup f87-keymap.json
cargo run -p openhid -- diag aula --macro-prefix --macro-prefix-output macro-prefix.bin # Agent-owned incomplete read-only dump
cargo run -p openhid -- diag aula --macro-table-candidate --macro-table-candidate-output macro-table-candidate.bin # repeated read-only OEM-shaped query; not a backup
cargo run -p openhid -- aula-macro import --input OEM.mcr --output macro.json
cargo run -p openhid -- aula-macro inspect --input macro.json
cargo run -p openhid -- aula-macro replace-event --input macro.json --index 0 --record 010001000200000003000000 --output edited.json
cargo run -p openhid -- aula-macro insert-event --input edited.json --index 1 --record 010001000200000003000000 --output inserted.json
cargo run -p openhid -- aula-macro remove-event --input inserted.json --index 1 --output removed.json
cargo run -p openhid -- aula-macro swap-events --input macro.json --first-index 0 --second-index 1 --output reordered.json
cargo run -p openhid -- aula-macro export --input macro.json --output exported.mcr
cargo run -p openhid -- aula-macro encode-transfer-block --input exported.mcr --output macro-block.bin
cargo run -p openhid -- aula-macro tree import --input mac.dct --output macro-tree.json
cargo run -p openhid -- aula-macro tree inspect --input macro-tree.json
cargo run -p openhid -- aula-macro tree rename-node --input macro-tree.json --node-index 2 --name "Office shortcuts" --output renamed-tree.json
cargo run -p openhid -- aula-macro tree create-folder --input macro-tree.json --parent-id 0xFFFF0100 --name "Office" --output with-folder.json
cargo run -p openhid -- aula-macro tree create-macro --input with-folder.json --parent-id 0xFFFF0100 --name "Launch editor" --output with-macro.dct
cargo run -p openhid -- aula-macro tree delete-node --input with-macro.dct --node-index 1 --output without-macro.dct
cargo run -p openhid -- aula-macro tree replace-event --input macro-tree.json --node-index 2 --event-index 0 --record 010001000200000003000000 --output edited-tree.json
cargo run -p openhid -- aula-macro tree insert-event --input edited-tree.json --node-index 2 --event-index 1 --record 010001000200000003000000 --output inserted-tree.dct
cargo run -p openhid -- aula-macro tree remove-event --input inserted-tree.dct --node-index 2 --event-index 1 --output removed-tree.json
cargo run -p openhid -- aula-macro tree swap-events --input macro-tree.json --node-index 2 --first-event-index 0 --second-event-index 1 --output reordered-tree.dct
cargo run -p openhid -- aula-macro tree extract-macro --input removed-tree.json --node-index 2 --output extracted.mcr
cargo run -p openhid -- aula-macro tree encode-transfer-block --input removed-tree.json --node-index 2 --output macro-block.bin
cargo run -p openhid -- aula-macro tree encode-transfer-table --input macro-tree.json --node-index 2 --node-index 5 --output macro-table.bin # order follows first key reference
cargo run -p openhid -- aula-macro tree export --input removed-tree.json --output restored-mac.dct
cargo run -p openhid -- diag aula --plan-keymap-write f87-keymap.json # offline JSON frame plan; no device I/O
cargo run -p openhid -- aula-keymap restore-fn-defaults --input f87-base-matrix.json --output f87-oem-fn.json # combine base backup with installed OEM Fn1 defaults
cargo run -p openhid -- diag aula --restore-keymap f87-keymap.json --apply # currently refused: writer protocol unresolved
cargo run -p openhid -- diag aula --brightness 2 --preview
cargo run -p openhid -- diag aula --effect 12 --brightness 3
cargo run -p openhid -- diag aula --effect 12 --brightness 3 --save
cargo run -p openhid -- diag aula --effect 12 --color '#FF0080' --save
cargo run -p openhid -- diag aula --effect 12 --colorful --save
cargo run -p openhid -- diag aula --sleep 600 --preview
cargo run -p openhid -- diag aula --sleep 300 --save
cargo run -p openhid -- diag aula --debounce 3 --preview
cargo run -p openhid -- diag aula --debounce 3 --save
cargo run -p openhid -- diag aula --backup f87-config.json
cargo run -p openhid -- diag aula --rgb-profile f87-rgb.json --save
cargo run -p openhid -- diag aula --restore f87-config.json --preview
cargo run -p openhid -- diag aula --restore f87-config.json --save
```

The installed F87 English catalog exposes Email, Home page, My computer,
Calculator, Favorites, Media Player, and display brightness up/down as key
assignments. The shared action list includes standard Consumer-page usages for
those choices, including 16-bit IDs such as Email Reader `0x018A` and
Bookmarks `0x022A`, plus Display Brightness Increment/Decrement `0x006F`/`0x0070`;
OEM profile conversion preserves the high usage byte. The values follow the
[USB-IF HID Usage Tables 1.7](https://www.usb.org/sites/default/files/hut1_7.pdf)
and are not yet validated as F87 device writes.

The OEM `F87PRO/KB.ini` also contains the confirmed Alt+Esc and Alt+Tab action
records. The desktop custom-action field and CLI accept modifier/key chords
such as `ctrl+shift+a`, `alt+f4`, and `windows+r`, using that record shape. The
shared list also includes common presets. Only Alt+Esc and Alt+Tab have F87
profile evidence; the additional chord encodings still need an OEM profile or
device readback check.

`aula-keymap import-oem-base` converts representable key assignments from the
installed `profile.dct` into the wired 520-byte base report. It preserves the
factory template's unassigned slots and F1–F12 reserved slots, maps standalone
modifier usages, and stops with the slot/action bytes if an OEM action cannot be
encoded. It does not import the Fn layer; the OEM profile contains proprietary
Fn actions whose wired representation has not been established. Conversion is
offline and does not access the keyboard.

Run `aula-keymap inspect-oem-fn --input profile.dct` before migrating Fn
assignments. It lists each OEM action that does not fit the wired report's
single-usage slot format and reports how many of the 87 physical assignments
are representable. This is a read-only preflight; it does not create a partial
Fn report that could silently clear the listed actions.

`aula-keymap restore-fn-defaults` can combine a captured base-matrix JSON backup
with the factory Fn assignments packed in the installed F87 `KB.ini`, producing
a new local wireless keymap profile. It also accepts an existing complete
wireless keymap profile and retains its base layer and opaque OEM Fn fields.
This reconstructs a local profile only; it is not a full device backup and
does not itself write to the keyboard. The desktop can apply the resulting
profile through the experimental wireless writer; verify the resulting mapping
on the physical F87 Pro before relying on it.

The installed F87 `KB.ini` also defines a red gaming-key group for Esc, WASD,
and the four arrow keys. The desktop RGB editor's **Select game keys** action
selects those nine LEDs and stages red as the chosen color; the user still
stages the color into the local table and separately chooses whether to save it
to the keyboard.

`aula-macro import/export` converts OEM `.mcr` files to/from the versioned
`openhid-aula-oem-macro` JSON profile. It preserves the OEM header and each
12-byte event, and `aula-macro inspect --input FILE` prints the recovered event
class plus every raw field for either format. An OEM-editor-created sample
renders raw code `0xF0` as a mouse-left action with press/release timing, so
OpenHID now labels `0xF0` as mouse-left and classifies the remaining disk codes
in `0x00`–`0xF4` as mapped input actions instead of cycle or movement
selectors. The decoder's special handling for `0xF5`,
`0xF6`, `0xF8`, and `0xF9` remains; their payload meanings and units remain
opaque. `aula-macro replace-event` replaces exactly one raw 12-byte row at the
zero-based index shown by `inspect`; it preserves the macro header and every
other event. `insert-event` and `remove-event` change the raw event sequence
while preserving the macro header and adjusting its declared record count. Each
edited result may be written as JSON or `.mcr`, and the output path must not
already exist. These commands operate on exact 12-byte records: they do not
validate event semantics or create valid key/timing sequences. They are local
file operations and never open the keyboard. The `.mcr` container round trip
has been checked against the sample above; non-empty `.dct` compatibility and
device macro writes remain unverified.

`aula-macro tree import/export` converts the local, unencrypted `mac.dct`
node sequence to/from the versioned `openhid-aula-oem-macro-tree` JSON profile.
The codec preserves each 0x58-byte node header and optional 0x68-byte macro
header with its raw 12-byte event rows. `tree inspect` reports node order, ID,
parent ID, decoded name, kind and event count; `tree extract-macro` exports one
node's macro as an encrypted `.mcr`. `tree rename-node` changes one node name
while preserving every other header field. Names are limited to 29 UTF-16 code
units, matching the OEM constructor's bounded copy. `tree delete-node` removes a
leaf macro or folder; folders with children are rejected to avoid implicit subtree
deletion. `tree replace-event`, `tree insert-event` and `tree remove-event` edit a
raw event by node and event index, preserving all other tree bytes. These remain offline file operations.
The installed user's `%LOCALAPPDATA%\BYCOMBO4\mac.dct` is zero bytes; OpenHID
imports it as a valid empty tree. A local workflow created a folder and empty
macro from that input, exported a 280-byte two-node DCT, and imported/exported
that generated DCT again byte-for-byte (SHA-256
`339832582D7DA54556180C5B6CA337A43269E460FA35AAFD7E4EBC95105C8BBE`). This
confirms the empty-tree workflow and internal codec round trip, but the inferred
node layout still needs validation against a non-empty OEM file before treating
it as fully compatible with the official macro manager.

`--rgb-profile` reads JSON with format `openhid-aula-per-key-rgb`, version `1`,
and exactly 126 `{ "red", "green", "blue" }` entries in LED-index order. It
requires `--save`, switches the device to custom effect 21, sends the complete
table, and saves. The color table has no verified readback, so the CLI reports
configuration readback separately and does not call it color verification.
This wireless table protocol remains unverified on the physical F87 PRO; profile
import is an experimental write path. OpenHID cannot export the current device
table until a reliable RGB read path exists.

The desktop Lighting editor also keeps named profiles under the OpenHID local
data directory in `aula-rgb-profiles/`. Save writes the current complete draft
as the same versioned JSON; saving an existing name replaces that file. Select
loads into the draft without touching the keyboard. Import uses a native file
picker, validates the versioned JSON, and chooses a unique local name rather
than replacing an existing profile. Export writes the selected profile to a
user-chosen JSON path; Delete asks for confirmation before removing the local
file. These operations do not provide device readback.

The same editor provides offline macro profiles under `aula-macro-profiles/`
and full local macro trees under `aula-macro-tree-profiles/`. It imports OEM
`.mcr` files, local `.dct` trees, or portable JSON, and can create a new empty
macro library directly in the desktop editor. Duplicate library names receive
a numeric suffix. A tree keeps its original
node headers and event order; selecting a macro node allows replacing,
inserting, or removing exact 12-byte records through fields for the event code,
auxiliary value, value, reserved bytes and special payload. The editor validates
hex width, keeps reserved bytes visible, and preserves the OEM byte layout on
save; field meanings and delay units remain unassigned. Export writes the full
`.dct` tree without replacing an existing destination. Folder and macro nodes
appear in the selector; the selected node can be renamed without changing its ID,
parent, macro block or other header bytes. Macro choices display the decoded
folder/name path and node ordinal. The selected event shows its recovered class,
including the confirmed mouse-left event, and each raw storage field separately,
including the two reserved bytes.
When inserting a new event, choosing mouse-left fills the exact 12-byte row
observed in the OEM sample (`F00000004500000000000000`) only while the other
fields still contain their zero defaults; edited payload fields are preserved.
Individual macros export as
`.mcr`. Deletion requires confirmation. The CLI supports the same raw-record
edits. Desktop macro playback and device writes from the `.mcr`/`.dct` editor
remain unavailable. A separate wired CLI command can write an opaque 4096-byte
macro-table candidate only after `--apply`; its transfer and recovery path
remain experimental until verified on the physical keyboard.

`--color` selects one custom color for an effect whose OEM `LedOpt` row permits
custom color; `--colorful` selects its colorful mode. These settings upload a full
37-report palette, each report requiring an exact echo. The F87 configuration block
is read back, but the palette itself is not; a failure can leave its palette partly
updated, and later configuration recovery cannot restore it. The report layout is
adapted from an MIT-licensed F75-family reference and still needs physical F87
verification. Preview is disabled for these edits for that reason.

For the Windows desktop in this milestone, use an optimized build:

```sh
cargo build --release -p openhid-desktop --bin openhid-desktop
cargo build --release -p openhid-agent --bin openhid-agent -p openhid --bin openhid
```

Then launch `target/release/openhid-desktop.exe`. Keep the matching
`openhid-agent.exe` beside it, and use `target/release/openhid.exe` for the CLI.
The debug GUI startup limitation is recorded in the final audit below.

Use `--identity` with the exact raw-route identity from `list` when more
than one keyboard is present. Ambiguous collections are rejected before opening.
The diagnostic accesses hardware directly. Coordinate access with the OEM app;
competing requests can affect results.

Selecting an online F87 PRO exposes the Lighting tab. The editor reads a fresh
configuration, offers built-in effect IDs 0–18, brightness 0–4 and speed 0–4,
and offers the OEM wireless idle timeouts: 60, 90, 120, 180, 240, 300, 600, 900
and 1200 seconds. One editor owns the read and Apply/Save lifecycle for both
settings. Unknown/custom effects remain visible and require an explicit built-in
selection before lighting edits; sleep-only edits preserve them. Unknown sleep
values are retained unless the user explicitly chooses a supported timeout. Switching effects
preserves their existing parameter tables unless levels are explicitly selected.
A failed operation requires a fresh successful read before another write.
Effect labels use the installed OEM English and Chinese names for IDs 1–18;
the names identify the OEM menu entries, not independently verified visual
behavior on this F87 PRO.

Agent-backed configuration, base-matrix and base+Fn reads use the same
per-route worker lock as AULA settings, RGB-table and keymap writes. Concurrent
requests within the Agent therefore cannot interleave raw HID exchanges for one
F87 route. Direct diagnostics and the OEM application are separate processes
and do not participate in this lock.

Apply stages and verifies without SAVE. Save stages, verifies, issues SAVE,
then verifies again, even if the snapshot was already staged. Preview stages,
verifies, restores the complete original snapshot, and verifies restoration;
it does not SAVE. Readback verifies all configuration bytes, not only lighting.
These operations preserve unknown fields and the low nibble of speed flags.

Backups are versioned JSON containing exactly ten validated configuration frames.
Existing files are never overwritten. Restore checks family, format version,
fragment count/order, payload sizes, checksums and terminator before opening
hardware, and requires `--preview` or `--save`. Restoration uses the same verified
transaction and recovery as ordinary settings edits. This backs up the configuration
block only: separate keymap, macro and per-key RGB storage is not included, so this
is not a complete device backup. No USB path or serial is exported in this format.

`--matrix` is a separate experimental read-only diagnostic for wireless opcode
`0x41`. It accepts at most 128 complete four-byte raw entries, without assigning
meaning to unknown function encodings. It shows OEM key labels for the 87 physical
slots correlated to the captured base matrix, and marks the remaining 39 entries
unmapped while retaining all four raw bytes. It shares the configuration reader's
deadlines, checksum/sequence validation and duplicate handling. It never writes
assignments, SAVE or reset. Physical reads now succeed: 36 frames contain 126
entries. Earlier reads and configuration controls timed out, so recovery after
inactivity remains open.

`--matrix --backup FILE` exports a separate versioned `wireless-base-matrix`
region with every original response frame. Deserialization verifies the driver,
region, frame order/count, payload bounds and checksum; unknown entries are retained
without padding to the OEM upper bound. Matrix files are rejected by configuration
restore before hardware access. Matrix restoration and macro/RGB backups are
not implemented yet. Base+Fn keymaps have a separate snapshot/restore workflow;
these formats together still do not provide a complete device backup.

`--oem-matrix-probe` is a Linux/Windows raw-HID, read-only transport diagnostic.
It sends the OEM-shaped base `GetMatrix` request with report ID `0x15` and
accepts the same validated `0x13` response fragments as `--matrix`. Linux
hidraw passes raw report bytes through without descriptor parsing. Windows
tries the same 20-byte request through the collection's raw output path even
though `0x15` is absent from the HID descriptor. This checks query delivery,
not whether matrix writes work. Add `--backup FILE` to save the returned base
matrix in the existing versioned format. macOS remains unsupported for this
probe.

On October 23, 2026, Windows tried `WriteFile`, `HidD_SetOutputReport` and
`HidD_SetFeature` for this request. The first two returned error 87 and the
last returned error 1, so it was rejected before transmission. This does not
test Linux hidraw or a directly connected wired `258A:010C` collection.

`--keymap` reads and prints both keymap layers; the base layer is physically
verified, while Fn layer selection is inferred from the installed OEM method
and remains unverified on hardware. `--keymap --backup FILE` exports the
current base+Fn snapshot. `--restore-keymap FILE --apply` is currently refused
until the F87 wireless writer is verified. The desktop editor offers keymap
drafts as local named profiles under
`aula-keymap-profiles/`; importing or selecting a profile only changes the
draft. Its versioned JSON is the same portable format used by the CLI backup,
so a profile exported from either interface can be imported by the other.
Export uses create-new semantics and does not overwrite a chosen file.
These keymap profiles do not include macros or the separate RGB table and are
not a complete device backup.

The sleep mapping comes from the installed OEM `KB.ini` (`SleepUI`/`SleepHW`).
A physical OEM 300-to-600-second experiment changed configuration fragment 1,
byte 15, from 10 to 20, and was restored and saved. That experiment also wrote a
cached brightness value in fragment 5; the OpenHID sleep-only path preserves the
lighting bytes. Rust sleep editing subsequently passed a physical 300-to-600-second
SAVE, independent full readback and exact original restoration. Only fragment 1,
byte 15 and its checksum changed. A power-cycle check remains acceptance work.

## Read failures and recovery

The wireless receiver can remain enumerated while a full configuration request
receives no complete reply. A read timeout does not establish whether the cause
is keyboard sleep, radio delivery, or a competing client. Keep the keyboard awake
and connected, coordinate access with the OEM app, then use **Read from keyboard**
or `diag aula` again. A successful complete read is required before the editor
allows another write after failure. Do not restore factory defaults to diagnose
this condition. Earlier timeouts and later successful reads are retained in the
hardware acceptance evidence below.

A read-only CLI attempt on October 23 to load both keymap layers timed out on
the initial base-matrix query with zero response fragments. This confirms the
receiver was enumerated but the keyboard was not responding to that query at
the time; it provides no evidence about the Fn-layer selector.

The per-key writer requires the full validated 126-LED table. Following the
field-tested F75-family wireless reference, it first stages and verifies effect
21, sends all 28 `0x02/0x1c` reports with exact echoes, then issues SAVE and checks
the configuration readback. It refuses to begin while custom mode is active.
If the operation fails, it attempts to restore and save the original lighting
configuration; this cannot restore RGB colors because the table has no verified
readback. The F87 hardware has not confirmed that command, its effect-21
selection, visible output, persistence or recovery behavior. No RGB table write
has been sent to the keyboard.

## Protocol and lifetime boundaries

The read request is:

```text
13 44 01 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 58
```

A complete result has ten frames with prefix `13 44 0a`, sequences 0–9,
14 payload bytes except the final two-byte payload `5a a5`, and a wrapping
sum of bytes 0–18 in byte 19. Only complete validated snapshots establish online
status. Malformed/conflicting frames fail the read; unrelated traffic and
identical duplicates cannot complete missing fragments. A read-only retry is
permitted for timeout; settings writes are never blindly retried.

Lighting writes clone the snapshot, change command 44 to 04, change only the
requested fields, and recalculate checksums. Each fragment requires an exact echo,
followed by complete readback. SAVE command 0a/01 also requires an exact echo.
Measured SAVE acknowledgement latency was approximately 609 ms; its deadline is
three seconds, versus one second per configuration echo. A write/recovery phase
has a twelve-second budget.

Windows overlapped I/O owns stable buffer and OVERLAPPED allocations plus a live
handle. Cancellation requests CancelIoEx and waits for completion before freeing
storage. The owned worker retains the session and route lock through recovery,
even if the requesting future disappears. CLI operations wait for worker cleanup
before process exit. Native cancellation completion may exceed a protocol deadline.
The portable transport uses async-hid. The native I/O gate is checked at entry
and within the owned operation.

## Hardware evidence and remaining acceptance

- Rust has read all ten physical-device fragments successfully. Built-in lighting
  staging and preview/restoration have been exercised against full snapshots.
- An unchanged original snapshot was written with ten exact echoes, SAVE was
  acknowledged, and complete pre/post-SAVE readbacks matched. An independent
  subsequent read also matched the original saved snapshot byte for byte.
- A subsequent Rust changed-brightness SAVE completed: level 3 to 2, independent
  complete readback, restore level 3 with SAVE, and another independent readback.
  All ten final fragments matched the original baseline. Evidence is retained in
  the local investigation `rust-changed-save-restore-20261020.json`.
  Power-cycle persistence has not been demonstrated.
- Initial reads still occasionally time out. The cause is unresolved; keyboard
  sleep, OEM ownership, and radio delivery remain hypotheses, not diagnoses.
- On October 23, 2026, a fresh Windows CLI session found no running AULA or
  OpenHID process, enumerated one `3554:FA09` receiver, then received zero
  fragments for both settings command `0x44` and base-matrix command `0x41`.
  The matrix read therefore failed before any entries arrived. This confirms
  current hardware reads are unavailable in that session; it does not
  distinguish a sleeping/unpaired keyboard from a receiver or transport issue.
- Windows is the current build and hardware host. Linux/macOS I/O, hotplug,
  sleep/wake, native desktop interactions and cross-platform cancellation require
  additional acceptance evidence.
- Battery and macro device writes/playback, advanced media/combo actions,
  backup/restore of separate storage regions, firmware update, and Bluetooth
  configuration are not supported. Wired configuration and keymap upload have
  desktop/Agent implementation paths but lack connected-device validation;
  wired keymaps remain write-only because reliable readback is unavailable.
  Debounce, wireless remapping and per-key RGB have experimental implementation
  paths, but their writes and persistence still need physical acceptance. The
  original app's advertised controls do not prove protocol compatibility or field
  locations.
- Duplex raw recording/replay is not represented by the current write-only
  fixture schema. Recording rejects duplex before opening hardware rather than
  producing an apparently complete partial capture. Fixture schema/privacy work
  remains separate.

## Provenance

The installed OEM application at `C:\Program Files (x86)\AULA\F87 PRO` was
examined as local evidence. Original OpenHID protocol code uses measured frame
facts; no OEM binaries or assets are included.

Reference snapshots examined during investigation:

- `free-soldier28/Aula-Manager`, tree
  `b98c0afad7ae57bbd4f073230c1ee1cb87006318` (MIT): wireless frame/protocol and
  driver behavior cross-checked against the physical response.
- `marcoslor/Aula-F87-Controller`, tree
  `de3cbef7d6149c31877ac96bd60d8c71581d301f`: reference only; no license was
  detected and no implementation was copied.
- `yhuikzdtguioaert/AulaSofwareLinux`, tree
  `2b7d1c8e13c9725a5a398a5702ac2dafcb454e72` (MIT code; OEM artwork excluded):
  wireless matrix read opcode `0x41` cross-checked against its `src/proto.rs`.
  That implementation refuses wireless matrix/RGB writes and requires wired
  feature reports for those operations. The generic model layout/effect examples
  are not accepted as proof of this F87 PRO's effect names, custom-mode field or
  wired report shape. No code or artwork from that project is bundled.
- `vndarkblue/aula-keybind`, tree `d45213a56bc72308019eb3254b34650fc2a95356`
  (MIT): its protocol notes document a 520-byte HID Feature Report `0x06` keymap
  for captured F87 hardware, with separate base/Fn layers and a write-only
  readback limitation. The notes list wired `258A:010C` and a `258A:010D`
  dongle; neither establishes support for this F87 PRO's observed `3554:FA09`
  receiver. This is protocol evidence to validate on the matching USB interface,
  not a wireless write implementation. No code or templates are bundled.

The protocol tests contain report bytes rather than USB paths, serials, host
identifiers, keystrokes, or a newly contributed hardware cassette.

## Focused verification update (Windows, 2026-10-20)

- Final affected-package tests passed: 160 CLI, 569 device, 18 registry and 52 HID
  tests. The first HID run exposed two worker tests using the same route; the
  new test now uses its own route and the whole HID suite passes in parallel.
  Production lock/time budgets were not weakened.
- `cargo test -p openhid-ipc -p openhid-ui`: passed, including 10 IPC unit tests,
  all 28 wire-format goldens, 4 UI unit tests and locale catalog parity.
- `cargo test -p openhid-desktop aula`: 8 passed, covering shared initial reads,
  stale replies, busy/failed writes, explicit sleep selection and Save dispatch.
  These tests do not prove real window layout or physical hardware behavior.
- `cargo test -p openhid-agent-core aula`: 4 passed, including injected-backend,
  wrong-collection and suspended-I/O behavior for both lighting and settings.
- Core validation tests passed, including duration roundtrips and refusal of
  unsupported/empty settings before dispatch. Matrix tests use clearly marked
  synthetic examples, exercise all 128 entries, and reject oversized data.
- `cargo clippy` with `--all-targets -- -D warnings` passed for all ten changed
  crates: core, device-registry, device, hid, ipc, agent-core, agent, cli, ui and
  desktop. `cargo fmt --all -- --check` and `git diff --check` passed.
- Portable checks passed for registry/hidpp/device on `wasm32-unknown-unknown`
  and core with `--no-default-features` on that target. Native Linux/macOS
  behavior is not proven by these checks.
- `ast-grep scan --inspect summary`: all 59 rules loaded and the tree passed.
  Temporary prohibited-copy probes failed the framing, settings lifecycle and
  IPC request guards; intended owners and unrelated probes passed. Probes removed.
- A physical `diag aula --matrix` read timed out; the immediate configuration
  control read also timed out. Neither sent assignments, SAVE or reset.
  Evidence is in local investigation files `rust-matrix-read-20261020-134020.json`
  and `rust-matrix-control-20261020-134128.json`. The cause remains unresolved.
- A subsequent control read still timed out with the receiver's control collection
  present. Repeating the same read with the OEM process stopped also timed out;
  the OEM executable was restarted afterwards. This comparison does not support
  concurrent OEM ownership as the sole explanation, and it does not establish
  whether the keyboard was awake. No settings, matrix, SAVE or reset were sent.
  Local evidence: `rust-control-resume-20261020-135845.json` and
  `rust-control-oem-comparison-20261020-135924.json`.
- On October 23, `cargo run -p openhid -- list` reported no reachable agent and
  fell back to direct hardware enumeration. It found the Bolt receiver and its
  mouse, plus the F87 PRO receiver collection, but marked the keyboard offline.
  The configuration probe (`0x44`) received zero fragments. Separate read-only
  `--keymap` and `--matrix` attempts also received zero `0x41` fragments. This
  confirms that the current device state cannot validate the Fn selector; it
  does not distinguish a sleeping keyboard from a disconnected one. No setting
  write was sent. The keyboard must respond to a read before protocol conclusions
  can be drawn.
- The OEM UI was not operated in this update. Computer Use had been stopped
  by Esc; no running-app screenshots or visual acceptance are claimed.

## Matrix and sleep acceptance update (Windows, 2026-10-20)

- The same receiver later answered both configuration and matrix reads. Matrix
  evidence `rust-matrix-awake-20261020-140105.json` contains 126 raw entries;
  `matrix-physical-slot-correlation-20261020.json` correlates all 87 OEM key slots.
  This proves the base read path; it does not prove Fn data or a matrix write opcode.
- A rebuilt CLI exported 36 matrix response frames to
  `rust-matrix-backup-20261020-141509.json`. The separate `.result.json` records
  successful execution. No assignment, SAVE or reset was sent for this export.
- Rust sleep SAVE/readback/restoration passed. Evidence:
  `rust-sleep-verified-20261020-141553.json`, with before/after/restored snapshots.
  The experiment changed only the sleep byte (10 to 20) and its checksum, then
  restored all ten original frames exactly. Two earlier attempts failed during
  baseline reads and performed no settings writes.
- Complete package tests passed: 570 device and 162 CLI tests. The new backup
  coverage rejects damaged/incomplete/wrong-region data and matrix-as-config
  restore before I/O. Clippy for both changed packages and formatting passed.
- Portable `openhid-device` compilation on `wasm32-unknown-unknown` passed.
  The 59-rule ast-grep scan passed; a duplicated canonical frame-order check
  failed its owner guard, while the owner and an unrelated field check passed.
- No native GUI acceptance or power-cycle persistence is claimed by these checks.

## Final native acceptance and audit (Windows, 2026-10-20)

Overall: **YELLOW** for the current delivery scope. Red blockers: **None** for
that scope when using the optimized build. The full OEM replacement remains
outside this milestone. No installation or packaging was performed.

- The optimized desktop launched successfully and connected to the matching real
  agent on the isolated `dev` profile (protocol 38). Both CLI and GUI inventories
  showed the AULA keyboard alongside the existing MX Master 3S.
- The Chinese Lighting panel completed a physical configuration read and showed
  effect 12 and the restored 300-second timeout. The effect menu exposed Off and
  IDs 1–18; the sleep menu exposed all nine supported timeouts. The busy panel
  visibly disabled its controls while reading.
- Native screenshots verified the dark theme at normal scale and the light theme
  at 125% application scale. The panel fit the available window without clipped
  controls in both cases. Local evidence is under `ui-acceptance/`, including
  `native-ui-result.json`, `aula-dark-normal.jpg`, `aula-effects-menu.jpg`,
  `aula-sleep-menu.jpg`, `aula-light-125-busy.jpg` and `aula-light-125.jpg`.
- GUI hardware writes were not repeated during visual acceptance. Save dispatch
  is covered by the focused desktop tests; the physical SAVE/readback/restoration
  proof came from the Rust CLI described above. Live failure/reconnect behavior,
  power-cycle persistence and native Linux/macOS operation remain untested.
- The debug desktop previously overflowed its Windows main-thread stack while
  initializing the generated locale backend. PE/symbol inspection measured a
  1,174,960-byte frame against the default 1,048,576-byte reserve; optimized
  builds did not reproduce the issue. The desktop build script now reserves
  4 MiB for the Windows debug executable only. PE inspection confirms the new
  `SizeOfStackReserve`; runtime startup has not yet been rechecked. Local
  evidence: `ui-acceptance/debug-stack-probes.json`,
  `ui-acceptance/debug-stack-symbols.json` and `ui-acceptance/desktop.stderr.log`.
- Intermittent receiver read timeouts remain unexplained. A failed baseline read
  sends no settings write; the editor requires a fresh successful read before
  enabling another write. The later native reads succeeded, which does not by
  itself establish a fix for those earlier timeouts.

Final source verification (after the matrix backup change):

- `cargo test -p openhid-device`: 570 passed.
- `cargo test -p openhid-cli`: 162 passed.
- Focused AULA suites: 35 device, 5 CLI, 8 desktop and 4 agent-core tests passed.
- The locale parity integration test passed (1 test).
- `cargo clippy -p openhid-device -p openhid-cli --all-targets -- -D warnings`
  passed; affected consumers `openhid-hid`, `openhid-agent-core`, `openhid-agent`
  and `openhid-desktop` passed `cargo check`.
- `cargo check -p openhid-device --target wasm32-unknown-unknown` and device/CLI
  rustdoc with `RUSTDOCFLAGS=-D warnings` passed.
- `cargo fmt --all -- --check`, diff whitespace validation and the 59-rule
  ast-grep scan passed. Temporary guard probes were removed.
- `cargo build --release -p openhid-desktop --bin openhid-desktop` passed and
  supplied the executable used for native acceptance.
- `cargo build --release -p openhid-agent --bin openhid-agent -p openhid --bin openhid`
  passed. Matching desktop, agent and CLI executables are available together in
  `target/release`. The optimized CLI's `list` used the real agent snapshot and
  showed the online AULA keyboard alongside the Logitech mouse; `diag aula --help`
  exposed the current lighting, sleep and backup commands.

Push verification on the final source tree (Windows, 2026-10-20):

- The full local gate ran with `RUSTFLAGS="-D warnings"`: formatter check,
  `cargo clippy --workspace --all-targets -- -D warnings`,
  `cargo test --workspace`, and the non-GUI private-item rustdoc command from
  `.agents/rules/ci.md` with `RUSTDOCFLAGS="-D warnings"` all passed.
- Workspace tests reported 2,758 passed and zero failed. The two existing ignored
  documentation examples in `openhid-hidpp` and `openhid-hidpp-derive` were not run.
- `cargo xtask ci ast-grep wasm wire i18n clippy-windows` passed all five named
  jobs. Wire-format goldens passed 28 tests. Locale parity and the desktop
  localization tests passed; Windows Clippy used the native host.
- The commit hooks passed whitespace/EOF, YAML/TOML, conflict markers, added-file
  size, typos, formatting and structural guards. Hook governance tests passed
  five cases. Unchanged shell checks had no matching files.
- The refreshed `origin/master` was still `2997eb7`. The repository had no release
  tags; the existing protocol was 37 and this change advances it to 38. The
  changed platform seam was audited against that master; no native Linux/macOS
  execution or cross-lint is claimed.
- Local gate logs are under `target/investigations/aula-f87-pro/push-gate` in the
  investigation checkout. The device document filename matches the lowercase
  usage link, including on case-sensitive systems.

## OEM effect names and live read check (Windows, 2026-10-22)

- The desktop effect picker now shows the installed OEM English/Chinese names
  beside IDs 1–18. Other locale catalogs use the English names until reviewed
  translations are available. This improves identification only; it does not
  establish the visible behavior of each effect on this F87 PRO.
- Focused verification passed: `cargo test -p openhid-desktop aula` (8 tests),
  `cargo test -p openhid-ui locale_files_have_the_same_keys` (1 test),
  `cargo fmt --all -- --check` and `git diff --check`.
- The receiver enumerated as `3554:FA09` and the OEM process was not running.
  A fresh read-only `cargo run -p openhid -- diag aula` timed out before the
  complete configuration arrived. It sent no settings write, SAVE or reset.
  This is consistent with the previously recorded intermittent read timeouts;
  it does not establish their cause or negate the earlier successful reads.
- Per-key RGB model/encoder checks passed: `cargo test -p openhid-core
  per_key_rgb` (1 test) and `cargo test -p openhid-device per_key_wire_table`
  (1 test). These tests validate data shape and generated bytes only.
- Full-table apply flow passed synthetic device tests, Agent injected-backend
  routing tests, the append-only IPC golden, and desktop AULA tests. They verify
  ordering, exact echoes, refusal while custom mode is active and honest partial
  failure reporting; they do not validate an F87 physical RGB write.

## Per-key transaction order recheck (Windows, 2026-10-22)

- The per-key implementation now stages and verifies effect 21 before sending
  the 28 wireless RGB fragments, then SAVE/readbacks the configuration. This
  matches the sequence in the locally retained field-tested F75-family
  reference; it is not F87 hardware evidence.
- On a failed stage, RGB fragment or SAVE, the device layer attempts to restore
  and save the original lighting configuration. The previous RGB table cannot
  be restored because no table readback is known. Recovery failure is reported
  separately.
- The installed receiver still enumerated, and the OEM process was not running,
  but `cargo run -p openhid -- diag aula` timed out before returning the baseline
  configuration. It sent no settings write, SAVE, reset or RGB table report.
- `cargo check -p openhid-device`, `cargo clippy -p openhid-device --all-targets
  -- -D warnings`, formatting and `git diff --check` passed. Tests were not run
  after this order change; F87 physical RGB behavior remains unverified.

## RGB keyboard layout mapping (Windows, 2026-10-22)

- Parsed the installed `F87PRO/KB.ini` base-key layout and compared all 87 OEM
  key LED indexes against `matrix-physical-slot-correlation-20261020.json`.
  There were no missing keys or index mismatches. The core now exports this
  87-key layout, and the desktop editor selects keys directly on a drawn board.
- The layout/index match is offline evidence. It does not prove that a custom
  RGB write lights the corresponding physical key on this F87 PRO. The other 39
  RGB table slots have no physical key label in the OEM base layout; the UI now
  exposes them through a separate index picker instead of leaving them uneditable.
- `cargo check -p openhid-desktop` passed after the layout and UI change. No tests
  were run and no hardware write was sent.

## Effect-specific controls (Windows, 2026-10-22)

- Added one core capability table from `LedOpt1..18` in the installed F87 PRO
  `KB.ini`. The desktop now hides unsupported brightness/speed controls; the
  fixed effect no longer offers a speed selector. Direction, random-color and
  custom-color capabilities are retained in the same model for the next UI and
  write-path steps.
- All 18 capability masks match the installed `LedOpt` columns (`speed`, `light`,
  `direct`, `random`, `color`). `cargo clippy -p openhid-core -p openhid-desktop
  --all-targets -- -D warnings`, `cargo fmt --all -- --check`, and `git diff
  --check` passed. No tests were run and no hardware write was sent; capability
  metadata does not verify visual effect behavior on the physical keyboard.
- The OEM comment defines the full row as `hw,effect,speed,light,direct,random,color`.
  The `F87PRO`, `1`, and `wired` copies agree: `hw` is 1–18, while the second
  `effect` field includes 19, 20, 28, 29, and 30. The UI capability table is keyed
  by `hw`; do not treat the second field as a wire selector without correlating
  it against an F87 configuration change. The raw configuration effect and OEM
  menu selector still need that direct correlation.

## OEM command surface from static inspection (Windows, 2026-10-22)

- Read-only x86 disassembly of the installed `OemDrv.exe` located `CDev3632`
  wrappers for matrix, LED, macro, and onboard settings. The matrix-read wrapper
  calls operation `0x41`; LED read/write wrappers call `0x44`/`0x04`; onboard
  read/write wrappers call `0x4A`/`0x0A`. The matrix-write wrapper routes through
  a chunked report builder, and macro writes use a separate data writer.
- These are static call-site observations, not packet captures or proof that
  every operation is available through the F87 PRO wireless collection. Matrix
  writes, macro framing, offsets, checksums, and readback semantics remain to be
  correlated before an OpenHID writer can safely claim support. No device write
  was sent during this investigation.

## Wired keymap protocol lead (Windows, 2026-10-22)

- A separate MIT-licensed F87/F75 reverse-engineering project documents keymap
  uploads as a 520-byte Feature Report `0x06` on HID interface 1. It identifies
  independent base and Fn layer blobs, and warns that the report is generally
  not readable as a keymap. Its protocol page says the slot data spans `0x008`
  through `0x207` (128 four-byte slots) and places a two-byte trailer at
  `0x208..0x209`; those ranges require 522 bytes, so they cannot describe the
  reported 520-byte transfer. The same project's [protocol implementation](https://github.com/vndarkblue/aula-keybind/blob/main/src/aula_keybind/protocol.py)
  sets `PAYLOAD_SIZE = 520`, computes the highest complete slot index as 126,
  and validates the trailer at the end of that buffer. With that length, the
  trailer is at `0x206..0x207`, leaving `0x204..0x205` outside the complete
  slots. Treat the raw report length as the implementation-backed value, and do
  not infer a 128th complete slot from the prose until a capture resolves the
  discrepancy.
  Wired keymap slots store an AULA usage page and one 8-bit usage ID, unlike the
  wireless matrix's action encoding. The CLI and desktop editors translate only
  unbound, single keyboard usages, and one-byte Consumer usages into the wired
  slot layout; modifier chords and OEM actions remain unsupported by this report
  editor. This format-level distinction does not validate writes
  to an attached F87 Pro. OpenHID now has an explicit `aula-keymap write-wired`
  upload path. It requires an exact collection identity and one validated
  520-byte layer blob; it sends one Feature Report `0x06` and reports that the
  transport accepted the transfer, without claiming keymap readback or key
  behavior. The CLI can export the packaged factory base template or an
  unbound Fn template before editing; these 520-byte captures are pinned to
  `vndarkblue/aula-keybind` commit
  `d45213a56bc72308019eb3254b34650fc2a95356`. The source blobs hash to
  `a5c1b631c7935cb986fd305f5357ad1ae0407d80d81060ab632007728dc27d55`
  (base) and
  `aecfdb89dfdf07e229018a71c1f04bfe3852a04a672e60a4fa5f0add74189d3f`
  (Fn). The Fn template is unbound and overwrites existing Fn assignments if
  uploaded; keep a copy of every source layer.
- The documented USB IDs (`258A:010C` wired and `258A:010D` dongle) do not match
  the currently observed `3554:FA09` receiver. Do not route this table through
  the wireless `0x41` matrix protocol or write it to that receiver. Its mapping
  state can be seeded from the packaged factory template or a user-provided base
  because the device does not reliably return the current table. The upload is
  persistent according to the F87 wired protocol reference; preserve the source
  blob so the same layer can be restored.
  A Windows Plug and Play inventory on 2026-10-23 found the `3554:FA09` receiver and no
  `258A:010C` wired node, so this checkout has no attached wired interface to
  identify or inspect.

## Built-in color modes (Windows, 2026-10-22)

- Added the OEM `LedOpt`-gated single-color and colorful controls to the desktop
  and CLI. The color picker stages an RGB value; Apply/Save sends the selected
  effect configuration and the 37-report wireless palette. The appended color
  state and change fields join unreleased IPC protocol v39.
- Adapted the palette report template and RGB slot from the MIT-licensed
  `free-soldier28/Aula-Manager` reference recorded under Provenance. Every palette
  report requires an exact echo, but the device exposes no verified palette
  readback. A partial transfer may leave the palette changed even when OpenHID
  restores the original configuration, so Preview is refused for color edits.
- `cargo check` for the affected core, device, CLI, IPC, HID, agent and desktop
  crates passed. Clippy with `--all-targets -- -D warnings`, formatting and
  `git diff --check` passed. Tests were not run, and no device write was sent;
  custom color and colorful mode remain unverified on the physical F87 PRO.

## Wireless read timeout investigation (Windows, 2026-10-22)

- A fresh direct diagnostic found the receiver control collection, opened the
  verified `FF02/0002` raw session and completed the `0x44/0x01` output write,
  then timed out while waiting for the first input report. No settings write or
  SAVE was sent. This is the first evidence that localizes the current timeout
  below selection, open and output-write stages.
- Compared the C# reference script in the ignored local investigation folder:
  it arms an overlapped input read before sending the request. The old Rust path
  wrote first and only then started reading, allowing a fast first response to
  be missed. Added an explicit raw request/response exchange that arms Windows
  input before output and uses concurrent reader/writer operations on other
  native backends. Config, SAVE, palette and per-key report echoes use it too.
- The next read-only diagnostic still timed out without an input report after
  the exchange-order change. The pre-arm race is corrected in code, but it is
  not established as the cause of this device's current no-response state. The
  keyboard may be asleep or unpaired; the receiver's presence alone does not
  prove the keyboard is online. No retry loop was added to mask that distinction.
- The installed Chinese OEM text catalog's device-search prompt explicitly asks
  the user to press a key on a wireless keyboard to wake it. This supports the
  sleep hypothesis but does not prove the current keyboard state. A wake-and-
  re-read is still needed for physical verification.

## Bounded read retries (Windows, 2026-10-23)

- Ordinary idempotent AULA reads now share one owner that retries a timed-out
  query up to five times, with a one-second attempt deadline and a six-second
  single-query deadline. A base+Fn read has a twelve-second combined deadline.
  This covers configuration, matrix, battery and OEM
  model-discriminator reads; malformed matching responses and transport errors
  still fail immediately. Settings, matrix and RGB writes are never retried.
- The retry count and request reuse follow the installed OEM driver's shared
  reader behavior. The previous separate PSD retry loop and one-off transaction
  readback retry now call the same owner. An ast-grep guard prevents other device
  modules from calling the one-attempt collector directly; the macro-prefix
  stream keeps its independent full-transfer deadline.
- `cargo check -p openhid-cli`, `cargo clippy -p openhid-device -p openhid-cli
  --all-targets -- -D warnings`, `ast-grep scan`, formatting and diff checks
  passed. Rust tests were not run. No hardware settings were written.
- A fresh standard `0x13/0x41` matrix diagnostic exhausted all five attempts
  without returning a complete matrix. The retry behavior therefore does not
  resolve the current receiver's no-response state; the device still needs a
  physical wake/re-pair check before further wireless protocol conclusions.

## Desktop RGB profile transfer and live read retry (Windows, 2026-10-22)

- The Lighting editor now imports and exports versioned per-key RGB JSON through
  native file pickers. Import validates the profile, saves it locally with a
  collision-free name and loads only the draft. Export writes the selected full
  126-color table in the same format used by `diag aula --rgb-profile`.
- Saving, loading, importing, exporting and deleting profiles remain local file
  operations; only the separate **Save to all 126 LEDs** control sends the
  experimental device write. No hardware write was sent in this update.
- `cargo check -p openhid-desktop`, `cargo clippy -p openhid-desktop --all-targets
  -- -D warnings`, `cargo fmt --all -- --check`, and `git diff --check` passed.
  The workspace test suite was not run.
- The installed OEM application was launched but its targetable accessibility
  tree and captured screenshot did not correspond, so no UI controls were
  operated. A subsequent read-only `diag aula` again found the receiver and
  opened the raw control session but received zero configuration fragments.
  The keyboard still needs a physical key press to wake it before device-level
  acceptance can continue.
- The RGB board now uses checkbox semantics and supports Ctrl/Command-click
  multi-selection; staging a color updates every selected LED in the local draft.
  This mirrors the OEM catalog's multi-key selection workflow without changing
  the experimental device write path. `cargo check -p openhid-desktop`, desktop
  Clippy with `-D warnings`, formatting, locale-key presence in all 24 catalogs,
  and `git diff --check` passed. Tests were not run.

Suggested guidance changes: None; the current ownership and verification rules
covered this milestone.

## Earlier verification record (Windows, 2026-10-19)

- `cargo test -p openhid-device-registry -p openhid-device -p openhid-hid -p openhid-cli`:
  passed, including existing protocol/inventory and recording regressions.
- `cargo test -p openhid-device aula`: 12 passed after adding the stalled-write
  deadline case; checks exact single read request, independent captured frames,
  invalid and missing data, route rejection, cancellation, and recovery.
- `cargo test -p openhid-agent-core watchers::inventory`: 17 passed, including
  standalone failure isolation, omission grace and recovery.
- `cargo clippy -p openhid-device-registry -p openhid-device -p openhid-hid -p openhid-cli --all-targets -- -D warnings`:
  passed. `cargo fmt --all -- --check` passed.
- `cargo check -p openhid-agent-core -p openhid-agent`: passed.
- `cargo check -p openhid-device-registry -p openhid-hidpp -p openhid-device --target wasm32-unknown-unknown`
  and `cargo check -p openhid-core --no-default-features --target wasm32-unknown-unknown`:
  passed. This proves host-free compilation, not native Linux/macOS behavior.
- `cargo doc -p openhid-device-registry -p openhid-device -p openhid-hid -p openhid-cli --no-deps`
  with `RUSTDOCFLAGS=-D warnings`: passed.
- `ast-grep scan --inspect summary`: passed, 55 rules. A temporary duplicated
  identity in the CLI crate failed the owner guard; unrelated identity code and
  the registry owner passed. Temporary probe removed.
- Changed native/cfg code was compared with local master `2997eb7`; no duplicate
  exports or platform type/name collisions found. Linux/macOS checks were not run.
- `cargo run -p openhid -- list`: direct enumeration succeeded, showed one F87 PRO
  offline, and still listed the connected Logitech mouse. The Rust diagnostic
  timed out without returning partial frames. GUI was not run.

No settings were changed during the October 19 verification above. Recoverable
physical-device experiments on October 20 are recorded separately in the hardware
evidence section. No packaging or installation was performed during the hardware investigation.

## Wireless keymap read/write path (Windows, 2026-10-22)

- Added a validated raw keymap model for 126 base entries and 126 Fn entries,
  wireless read requests, and local named profiles. The Fn table was initially
  modeled as 128 entries; a live hardware read on October 23 confirmed the
  receiver returns 36 fragments of 14 bytes (504 bytes, or 126 entries) for
  both layers. The CLI can
  inspect/export with `openhid diag aula --keymap --backup f87-keymap.json`.
  The backup contains only these two layers; it is not a complete OEM profile
  and does not include macros.
- The installed Windows OEM uses report `0x15` for matrix writes, while the
  receiver descriptor advertises `0x13`. Static analysis establishes the OEM
  method but cannot make Windows accept an unadvertised report ID. A separate
  same-model [F87 Pro implementation](https://github.com/meursault03/aula-f87pro-linux/tree/825a91a46d9b4573c7bfebdeb90a607b7638b709)
  uses the advertised `0x13` carrier with write command `0x01`, fragment count,
  layer nibble, and checksum; it labels receiver support experimental.
- OpenHID's wireless writer now has a distinct direct-`0x13` path. It checks the
  live six-byte PSD, reads the complete original base/Fn keymap, sends one
  acknowledged fragment at a time, and verifies both layers by readback. A
  failed update or cancellation attempts to restore and verify the original
  mapping. The older OEM `0x15` write-frame planner remains offline-only. This
  direct path has compiled but has not been run on the physical F87 Pro; radio
  write acceptance, persistence and recovery still need hardware verification.
  No keymap write has been sent to the physical keyboard.
- A read-only Agent RPC on the awake `3554:FA09` receiver returned and validated
  both keymap layers: 126 base entries and 126 Fn entries. Its 36 Fn response
  fragments each carried 14 bytes. Response byte 4 packs the layer selector in
  its upper nibble and the payload length in its lower nibble (`0x1e` for a
  14-byte Fn fragment); treating the whole byte as a length caused the earlier
  parser failure. The same correction fixed the exact-multiple final-fragment
  calculation (504 bytes is 36 full fragments). No device data was written.
- Source compilation and formatting are tracked in the current development
  update; they do not count as hardware acceptance.

## F87 OEM matrix method table (Windows, 2026-10-22–23)

- The installed `CDevComboFilm` virtual-method table identifies
  `SetMatrix layer=%d` and `GetMatrix layer=%d`. Static disassembly shows the
  matrix write command is `0x01` (wired variant `0x11`), with the layer passed
  separately. Matrix read uses `0x41` (wired variant `0x51`), also with a
  separate layer argument. The Fn layer therefore is not established as a
  distinct `0x03` write or `0x43` read command.
- The selected `CDevComboFilm` vtable's slots 20 and 21 resolve to
  `SetMatrix` (`0x48A660`) and `GetMatrix` (`0x48A6B0`). `SetMatrix` calls the
  OEM packet helper (`0x48A190`) with report ID `0x15`, command `0x01` for the
  wireless path, and a maximum fragment length of 14 bytes. Rechecking the x86
  stack offsets shows the copy destination is frame offset 5 (the `lea` occurs
  after two arguments are pushed); payload bytes occupy offsets 5–18 in the
  fixed 20-byte `WriteFile` transfer. Offset 19 is an additive checksum over
  the frame with that byte initially zero. The static call arguments populate
  byte 1 with the wireless/wired command (`0x01`/`0x11`), byte 2 with `0x01`,
  byte 3 with the fragment index, and byte 4 with the layer nibble plus fragment
  length. The frame length and copy boundary are internally consistent. The
  receiver still advertises output ID `0x13`, not `0x15`, so this does not
  establish that Windows or the physical receiver accepts the OEM writer's
  report ID. After each write, the OEM waits for a 20-byte reply; its matcher
  rejects report ID `0xff` and byte-2 error bit `0x80`, accepts a reply when
  byte 1 matches the expected command, and discards up to 20 unrelated reports.
  This matcher does not validate a reply checksum.
- Static tracing of OEM discovery shows it obtains the standard HID class GUID
  with `HidD_GetHidGuid`, enumerates interface paths through
  `SetupDiGetClassDevsW` / `SetupDiEnumDeviceInterfaces` /
  `SetupDiGetDeviceInterfaceDetailW`, then opens each path with `CreateFileW`
  using read/write access and shared read/write before querying HID attributes,
  preparsed data and capabilities. This confirms the OEM transport begins from
  ordinary Windows HID collection handles; this pass has not yet tied the
  selected F87 `CDevComboFilm` object to a particular collection path. It
  therefore does not resolve why the OEM-shaped `0x15` transfer is rejected by
  Windows on OpenHID's selected collection, or establish that another
  collection accepts it.
- A read-only SetupAPI/HID inventory on this Windows host returned the F87
  `FF02/0002` collection first (20-byte input/output, output capability ID
  `0x13`) and `FF04/0002` later (8-byte Feature report, ID `0x06`). Static
  disassembly of the OEM capability helper checks Feature, Input, then Output
  caps and its outer enumerator stops at the first path that produces a device
  object. Together these findings strongly indicate that the OEM's wireless
  F87 object uses the same `FF02/0002` collection OpenHID selects. Interface
  ordering is host-specific, and the live OEM handle was not inspected, so this
  is not direct proof of its runtime handle. It makes a different-collection
  explanation less likely; report ID `0x15` remains absent from the selected
  descriptor. `HidD_SetOutputReport` is not a safe workaround to try blindly:
  [Microsoft documents](https://learn.microsoft.com/en-us/windows-hardware/drivers/ddi/hidsdi/nf-hidsdi-hidd_setoutputreport)
  that some devices may become unresponsive when that API is used. The OEM's
  static `WriteFile` sender remains the relevant comparison, but no successful
  `0x15` transfer has been observed.
- A later read-only present-device Plug and Play inventory confirmed the complete HID
  collection topology for this receiver: interface `MI_00` exposes the standard
  keyboard collection; `MI_01` exposes Consumer Control (`COL02`), System
  Control (`COL03`), Keyboard (`COL04`), Mouse (`COL05`), and exactly two
  vendor-defined collections, `FF02/0002` (`COL01`) and `FF04/0002` (`COL06`).
  This rules out an unenumerated third vendor collection on the current Windows
  device instance as the missing `0x15` carrier. It does not prove which opened
  path the OEM selects at runtime or whether a raw Linux hidraw path would pass
  the OEM request.
- A descriptor-only call to `HidP_InitializeReportForID` on the current
  `FF02/0002` preparsed data returns `HIDP_STATUS_SUCCESS` (`0x00110000`) for
  output report `0x13` and `HIDP_STATUS_REPORT_DOES_NOT_EXIST` (`0xC0110010`)
  for output report `0x15`. The adjacent `FF04/0002` parser accepts Feature
  report `0x06` and has no output report type. This call only asks Windows to
  initialize a report from the already-read descriptor; it sends no report to
  the keyboard. Microsoft's [`HidP_InitializeReportForID` documentation](https://learn.microsoft.com/en-us/windows-hardware/drivers/ddi/hidpi/nf-hidpi-hidp_initializereportforid)
  defines the latter status as an invalid/nonexistent report ID. That makes
  descriptor rejection the leading explanation for the earlier Windows error
  87, but it does not prove whether the receiver would accept an unadvertised
  ID through a different driver or transport. No such transport has been found.
- Linux remains a candidate for testing that transport boundary. OpenHID's
  pinned `openlogi-async-hid` dependency selects `HidRawBackend` on Linux, and
  its output writer calls `write(fd, buf)` without rewriting the report. The
  [Linux hidraw documentation](https://github.com/torvalds/linux/blob/master/Documentation/hid/hidraw.rst)
  describes reports as raw/unparsed and says `write()` passes the report to the
  device. This avoids Windows' descriptor parser, but does not prove that the
  F87 receiver accepts `0x15`. A base-layer `GetMatrix` request on Linux would
  be a read-only discriminator; keep keymap and macro writes disabled until
  the request/reply path and readback are verified on that hardware.
- The adjacent `CDevComboFilm::SetMacro` method calls the packet writer with
  command `0x03` (wired variant `0x13`); it is separate from `SetMatrix`.
  This explains why reusing the F99 reference's `0x03` Fn-write command for
  this F87 is unsafe.
- The OEM packet helper builds report ID `0x15` frames and fragments payloads;
  the current physical base-matrix read instead uses the verified `0x13`
  collection. The OEM `GetMatrix` helper also constructs a `0x15` request: byte
  1 is `0x41`/`0x51`, byte 2 is `0x01`, and byte 4 is the selected layer in its
  high nibble. OpenHID's Fn request uses the same selector field (`0x10`) in the
  physically verified `0x13` read carrier. At the time this method table was
  first analyzed, its Fn reply had not yet been observed; the later live read
  confirmed the selector and packed response length as recorded above. The
  receiver's `FF02/0002` descriptor advertises 20-byte
  input/output reports with output ID `0x13`, not `0x15`. The writer's static
  20-byte frame is now understood; report-ID acceptance and live readback
  remain unverified, so OpenHID uses the direct `0x13` candidate below instead
  of sending the OEM `0x15` frame. No hardware write was sent. The current
  Windows Plug and Play inventory lists the `3554:FA09` composite device
  and its HID collections as present; this does not establish that the keyboard
  is awake. The earlier read-only Fn-layer attempt timed out before accepting a
  fragment. Static code matches the current layer-selector encoding, so that
  timeout alone does not identify a request-format defect or prove the Fn layer
  unsupported. An independent [F99 Pro protocol description](https://github.com/dnviti/aula-x99-pro-driver-linux/blob/master/PROTOCOL.md)
  reports Fn reads on command `0x43` and labels several `0x08` actions as LED
  controls. Its [implementation](https://github.com/dnviti/aula-x99-pro-driver-linux/blob/master/aula_f99pro.py#L2227-L2258)
  uses wireless report ID `0x13` and declares write commands `0x01`/`0x03`.
  That model's reported PSD ends in `0x58`, while the installed F87 profile uses
  `0x0B`; the local F87 OEM `GetMatrix` path selects command `0x41`/`0x51`, passes
  the layer in the high nibble, and builds report ID `0x15`. Treat the F99 report
  as a cross-model lead, not a replacement for the F87 method table or proof of
  F87 behavior.

- The independent [Aula-F87-Controller protocol notes](https://github.com/marcoslor/Aula-F87-Controller/blob/main/docs/PROTOCOL.md)
  cover the same `3554:FA09` receiver and corroborate lighting report ID
  `0x13`, fragment checksums, configuration, palette, and per-key RGB command
  families. They also claim both `FF02` and `FF04` wireless collections accept
  commands. That collection claim conflicts with the local read-only HID
  preparsed-data query: `FF02/0002` advertises 20-byte input/output reports,
  while `FF04/0002` exposes only an 8-byte Feature report `0x06`. OpenHID
  currently selects the `FF02` control collection; no write has been attempted
  through `FF04`. Keep the independent collection claim unresolved until an
  F87 capture or safe hardware observation reconciles it. Its lighting captures
  do not establish the OEM keymap or macro report-ID `0x15` paths.

- A separate [F87 Pro Linux implementation](https://github.com/meursault03/aula-f87pro-linux/tree/825a91a46d9b4573c7bfebdeb90a607b7638b709)
  provides a useful same-model comparison. Its current source reads the 10-byte
  identity response and matches the six-byte F87 Pro PSD (`03 00 00 00 01 0b`);
  it reads 504-byte matrices with wireless `0x13/0x41`, stages matrix writes as
  `0x13/0x01`, reads a 512-byte macro prefix with `0x13/0x43`, and explicitly
  refuses wireless macro writes. Its apply flow reads a fresh complete snapshot
  and compares it with the caller's expected snapshot before writing; a
  mismatch aborts. The README labels 2.4 GHz support experimental,
  limits documented receiver key assignments to layers 0–1, and says macros
  require USB. This supports investigating a direct `0x13` matrix path as a
  separate F87 transport from the installed Windows driver's `0x15` carrier;
  it does not establish that path on this Windows receiver or resolve the
  current timeout. OpenHID does read a fresh rollback snapshot, but its desktop
  write request does not include the keymap the user originally loaded, so it
  cannot reject a stale editor draft yet. The repo's simulated tests and preview
  screenshots are not physical evidence for this device. Do not enable OpenHID
  wireless writes from this cross-check alone.

## Wireless debounce control (2026-10-22)

- Added a bounded 1–5 ms debounce type, decoding from configuration fragment 0,
  byte 8, with unknown values preserved in the raw configuration snapshot.
  Desktop settings and `diag aula --debounce` use the same verified transaction
  as sleep and lighting: preview restores the complete original snapshot, and
  save requires full readback before reporting success.
- The field location and value mapping are an implementation lead from the
  [Aula-F87-Controller reference](https://github.com/marcoslor/Aula-F87-Controller).
  The OEM `KB.ini` exposes `ShowDebounce=0x04000101`, but this update did not
  compare an OEM debounce edit or write the physical keyboard. Treat the field
  mapping and persistent behavior as pending physical confirmation.

## OEM local profile container and keymap migration (Windows, 2026-10-23)

- The installed app stores its local device profile in
  `%LOCALAPPDATA%\BYCOMBO4\AULA F87 PRO\profile.dct`; macro data uses the
  separate `%LOCALAPPDATA%\BYCOMBO4\mac.dct`, which is currently zero bytes;
  neither the install tree nor the local `BYCOMBO4` tree contains an `.mcr`
  sample. The local `profile.dct` is
  `0x3400` bytes. Static file-I/O code reads and writes a 4-byte leading field
  followed by a fixed `0x33fc`-byte record. The record starts with magic
  `0xFCAA1245` and repeats the leading field in its next four bytes. A
  read-only local sample confirms the file is populated. Within it, the 87
  physical base assignments begin at offset `0x3c`; these 16-byte rows store
  the four-byte assignment at record offset 4. Their slot, type and ordinary
  key-usage fields match the installed `F87PRO/KB.ini` `[KEY]` entries; seven
  modifier/combo rows use the other action bytes. The importer and DCT exporter
  validate the base-record type and duplicated matrix slot for all 87 keys
  before mapping or replacing actions. The 87 physical Fn assignments begin at
  offset `0x940`, use 16-byte records, and store each four-byte action at record
  offset 4. All 37 configured actions match the installed `F87PRO/KB.ini`
  `[FN1]` entries at their expected key positions. `diag aula
  --import-oem-profile` maps both tables to F87 wireless matrix slots and retains the 12
  non-action bytes in each mapped Fn record and writes a new OpenHID JSON
  keymap profile. Running that offline import against the current 13,312-byte
  local sample produced validated 126-entry base and Fn tables, with 87 base
  assignments, 37 Fn assignments, and all 87 opaque Fn rows retained; the OEM
  file reserves 128 Fn record slots, of which 41 are outside the 87-key layout;
  no HID reports were sent. The current CLI then exported that profile against
  the same OEM template; the resulting 13,312-byte file matched the source
  byte-for-byte (SHA-256 `6CA47EC7F66201F4D8C2C1D36B72BDD51E56E020545109C2730329B27E261EBA`),
  including the opaque Fn action pages. A fresh CLI import/export on October 23
  reproduced the same hash. The
  same template round-trip also remained byte-identical when the portable profile
  was downgraded to its version 2 shape without source-action metadata. The
  desktop's **Import keymap profile** also
  accepts `profile.dct` without a connected keyboard and imports both layers from
  the local file. Non-empty Fn records outside the 87-key layout are rejected
  during import. **Export keymap to OEM profile** takes a
  selected OpenHID keymap and a validated OEM DCT template, overlays retained
  non-action Fn row bytes when available, replaces the 87 mapped base and Fn action fields,
  and preserves the template outside those row fields. Known
  USB keyboard-page and consumer-page actions convert between the OEM profile
  and matrix-editor byte layouts. The seven installed OEM modifier records
  also convert to the editor's modifier-bit actions and back (left/right Ctrl,
  Shift and Alt, plus left Win); their OEM modifier-mask/Windows-VK tuples are
  held in one core conversion table. Zero and unrecognized actions remain raw.
  This mapping follows the installed profile format and the editor's action
  catalog, but physical Fn readback is still unavailable. The exporter does not
  interpret the retained OEM row fields or convert unrelated OEM
  profile sections. For imported OEM profiles, the desktop now exposes the
  selected physical Fn row's retained 12 bytes in an exact hexadecimal editor.
  The edit stays in the local profile and is included when that profile is
  saved or exported; it is not sent to the keyboard.

## OEM macro sender static analysis (Windows, 2026-10-22–23)

- The installed English `Text/en/text.xml` exposes the OEM editor scope: macro
  folders and named macros, record/stop, key events with start and hold times,
  preview and insertion, repeat counts and two repeat-until-stop modes, X/Y
  movement, and delay events. This is UI evidence for the replacement feature
  requirements; it does not define the serialized macro schema.
- The OEM executable reads `ShowMacro` and `MacroInsertMask` from the global
  `[OPT]` section in `Cfg.ini`; static call arguments use a default of `1` for
  both settings. `ShowMacro` is checked by the macro editor layout, while the
  installed `Cfg.ini` does not override it. The F87 PRO wireless and wired
  `KB.ini` profiles contain no macro-visibility key. This confirms that the
  installed profile does not hide the generic macro editor for this model;
  `MacroInsertMask` controls insertion behavior and is not a device capability
  declaration. The commented `;MacroInsertMask=3` line is not active config.
- The installed OEM `OemDrv.exe` is a 32-bit PE. Static strings include
  `CDevComboFilm::SetMacro`, `SetMacroData`, `MacroBufferSize` and the macro
  transfer diagnostics. The selected wireless `CDevComboFilm` vtable's slot 22
  is `SetMacro` (`0x48A7A0`) and slot 23 is `GetMacro` (`0x48A7E0`). Static
  disassembly selects write command `0x03` (wired variant `0x13`) and read
  command `0x43` (wired variant `0x53`), respectively. `SetMacro` routes macro
  data through `0x48A6F0` in `0x200`-byte source chunks, then the shared `0x15`
  packet helper (`0x48A190`). That helper caps its fragment-length field at 14
  bytes and copies source data at frame offset 5, fitting the payload in
  offsets 5–18 of the 20-byte frame; byte 19 carries the additive checksum.
  Its transport wrapper at `0x486EB0` calls `Kernel32!WriteFile` with the
  complete 20-byte frame and the device object's handle, using overlapped I/O.
  The response matcher at `0x489C30` reads 20-byte reports, discards report ID
  `0xff` and frames with bit 7 set in byte 2, and accepts a response when byte
  1 matches the expected command. It retries mismatched responses up to 20
  reads; this matcher does not compare a fragment index. These findings
  confirm the OEM sender's API and response filter, but do not explain why the
  equivalent read-only OpenHID `0x15` output attempt is rejected by Windows on
  the selected receiver collection, or prove that a setting-changing
  transfer would be accepted by the F87 receiver.
  At each `SetMacro` call, the helper receives the mode-dependent command,
  chunk index, source pointer, chunk length, and the constants `0x06` and
  `0x0f` as separate arguments. The helper only checks `0x06` for nonzero when
  deciding whether to retry two response-error cases; its retry counter allows
  up to five additional attempts. It passes `0x0f` to a wait helper that uses
  `QueryPerformanceCounter` and a 1000× conversion, establishing a 15 ms
  minimum interval between output frames. Neither value is copied into the
  packet fields. `SetMacro` itself passes its caller's byte buffer and length
  straight to this chunker; event encoding happens upstream. A separate OEM
  aggregate path at `0x48DB20` calls the builder at `0x48D230`, which walks
  macro nodes and invokes `0x48CE60` for each node. RTTI and both call sites
  establish that `0x48DB20` is `CDevG5KB` virtual slot 13: they construct that
  temporary object at `0x48Aef0` (vtable `0x6100b4`, product field `0x0916`)
  and pass it as `this`. It converts action rows through slot 33, then sends
  the aggregate through the same object's slot 22 (`0x48B8D0`) at `0x48DF63`.
  This is not `CDevComboFilm::SetMacro` (`0x48A7A0`), so the recovered aggregate
  serializer does not establish the F87 upload format. The other observed slot
  22 sender call at `0x491D66` runs on a temporary class with vtable `0x610A84`
  and product field `0x8805`, also distinct from the selected F87 class.
  `0x48CE60` copies the macro name from node
  offset `0x14` as UTF-16LE, writes a one-byte marker containing the UTF-16
  name's byte length, then visits
  event rows beginning at `+0x5c` with 12-byte stride and count at `+0x58`.
  Each nonzero-code row becomes a four-byte packed action; zero-code rows are
  skipped. The builder's output length is `2 × UTF-16 code-unit count + 1 +
  4 × encoded event count`. The action byte is chosen through a code dispatch
  and, for ordinary keys, an embedded Windows virtual-key-to-HID-usage table;
  the row's value bytes and auxiliary field supply the other action bytes.
  Static recovery covers the dispatch categories and all 115 key-map pairs.
  Modifier keys emit HID modifier usages; `0xF0`–`0xF2` use three fixed action
  selectors; `0xF5` passes the low payload byte directly, `0xF6` negates it,
  and `0xF8`/`0xF9` pass it directly using their respective classes. `0xF3`/`0xF4`
  depend on an OEM context object and are rejected by the offline encoder. The
  aggregate builder's table layout is also recovered, but its four-bank
  key-action inputs still need reconstruction. None of this
  establishes that the advertised
  wireless `0x13` carrier accepts these writes or that the separate `0x06`
  feature-report sender applies to this receiver. The local editor format and
  F87 transfer buffer do not yet have a proven conversion path. OpenHID's
  host-free core can encode
  the recovered per-node format, and `aula-macro encode-transfer-block` writes
  that block to a new local file for inspection. It supports the recovered
  virtual-key table and static action classes, rejects unknown codes and the
  context-dependent `0xF3`/`0xF4` cases, and is not a HID report or device write.
  The matching `aula-macro tree encode-transfer-block` command accepts a macro
  node from a local DCT tree. The aggregate builder at `0x48D230` scans four
  banks of 144 key positions for macro actions, assigns an index to each unique
  macro node, and emits a table of 16-bit offsets and lengths followed by the
  encoded blocks; its caller rejects totals above `0x2800` bytes. This recovers
  the generic aggregate layout. OpenHID now assembles that macro directory and payload
  from an explicit node order; repeated node IDs are deduplicated and conflicting
  contents are rejected. `aula-macro tree encode-transfer-table` exposes this
  offline operation and prints each node's assigned index, offset and length.
  Its input order must be the OEM aggregate path's first-key-reference order.
  The four-bank key-action inputs and their relationship to F87 device state,
  including whether F87 uses this aggregate path at all, still need
  reconstruction before generating a complete device macro configuration.
  The two-byte record formatter at `0x488F10` is not the missing macro event
  encoder: its caller is in the `CDev916KB::ApplySetting` matrix-save path,
  and it walks `0x24`-byte records to form the matrix transfer buffer. Do not
  infer macro event semantics from that formatter.
  The read helper rejects `0xff` report IDs and a set error bit in byte 2. A
  separate macro-data serializer rejects assembled buffers above `0x2800` bytes
  before checking the device-reported
  capacity. The `SetMacroData` call path reads `0x400`-byte source chunks and
  passes a `0x407`-byte buffer to `HidD_SetFeature` with report ID `0x06`;
  static IAT mapping confirms the API. Its call site is inside the OEM's
  `Macro(%X)` transfer loop: it advances through pre-encoded `0x400`-byte blocks
  and calls the report wrapper for each block. This confirms the upload path,
  but not how macro events are encoded into those source blocks. The physical
  receiver's `FF04/0002`
  collection advertises only an 8-byte Feature report `0x06`. The relationship
  between this sender, the selected wireless class, and that collection is
  unresolved; neither path establishes macro support on this `3554:FA09` device. A
  read-only scan of the running OEM process's model registry found the exact
  wireless `3554:FA09` entry and `Psd=03 00 00 00 01 0B` from the installed
  `F87PRO/KB.ini`. The wireless discovery path matches that entry's model
  discriminator and constructs the device object with the `CDevComboFilm`
  vtable. This resolves class selection for the installed F87 PRO wireless
  profile, but not which macro sender it uses, device capacity, or persistence.
- The local `%LOCALAPPDATA%\BYCOMBO4\mac.dct` is empty, so it provides no macro
  sample. Static analysis of the tree reader at `0x46D340` shows it reads a
  `0x58`-byte root and checks magic `0xA8B83333`; when root field `0x14` is
  nonzero, it reads a `0x68`-byte child, checks magic `0xFACB99A2`, and sizes
  that child's entry block as `0x68 + count × 12`. The corresponding tree
  writer at `0x46D780` opens the literal `mac.dct`; its node writer emits a
  `0x58`-byte header and, when present, the `0x68 + count × 12` macro block,
  while `0x46D730` walks nested node lists. The TEA routines are not called on
  this `.dct` path. These calls establish recursive node serialization, but
  the empty local file still prevents validating the complete tree layout and
  record ordering. The host-free core codec reads and writes the inferred
  concatenated node format while preserving each `0x58`-byte node header and
  optional unencrypted `0x68`-byte macro block. Desktop and CLI import/export;
  the desktop selects and edits macro nodes while the CLI also supports
  inspection and extraction. Raw event edits preserve other node contents. This
  advances the static reconstruction, but it has not yet been round-trip checked
  against a non-empty OEM `mac.dct`. The
  OEM node constructor at `0x46D0F0` copies a supplied name as UTF-16LE into
  header bytes `0x18..0x58`, with a 30-code-unit bounded copy. The codec exposes
  that name for CLI and desktop labels while preserving the complete raw header.
  This field layout also awaits validation against a non-empty OEM tree.
  The loader at `0x46D4E0` resolves each node's parent from header bytes `8..12`
  and creates a nested child list only when the kind field at `12..16` equals 1.
  The host-free tree model now exposes node and parent IDs, resolves root-to-node
  paths with cycle protection, and the desktop uses those paths in macro labels.
  Static analysis of folder creation at `0x46D0F0` confirms that a folder uses
  kind 1, stores the resolved parent ID at bytes `8..12`, and writes its UTF-16LE
  name at `0x18..0x58`; the fresh `0x58`-byte header is zeroed before those
  fields are populated. The internal root uses sentinel ID `0xFFFF0100` and is
  not serialized as a record. The node ID generator waits 20 ms and combines
  local time fields, while recursive lookup compares IDs. OpenHID's offline
  folder creation instead picks a collision-free local ID, and rejects missing
  or non-folder parents. Tree imports now reject duplicate node IDs, the root
  sentinel as a serialized node ID, cyclic parent chains, and known parents that
  are not folders. They also verify each macro block's node ID against its
  containing tree-node ID, which the OEM creator copies into both headers.
  Missing parent records remain importable as orphan paths. This prevents
  ambiguous or invalid path resolution, but the model has not been validated
  by loading a non-empty tree in the OEM app.
  Desktop folder creation edits and saves only the selected local profile; CLI
  `tree create-folder` writes a new DCT or JSON output without replacing files.
  The OEM's new-macro UI handler at `0x43CA70` builds a `0x68`-byte macro
  record through `0x46CF40`, applies the entered name, and passes that record to
  `0x46D160` to attach it to the selected parent. That confirms the constructor
  call path. A bounded x86 emulation of `0x46CF40`, with deterministic stubs for
  its allocator, ID generator and UTF-16 formatting helper, produced a 0x68-byte
  header with magic `0xFACB99A2`, generated ID at `+4`, value 1 at `+0x0C`, the
  UTF-16 name at `+0x14`, value 1 at `+0x54`, and zero records at `+0x58`; the
  other bytes were zero. `0x46D160` places the macro block on a zeroed tree
  node, copies the same ID, and sets the parent ID. Its macro-presence field is
  nonzero in memory; the loader only branches on zero versus nonzero before
  reading the block, so the offline codec uses marker 1. The tree node's own
  name area remains empty for this path; macro names come from the macro block.
  OpenHID now creates empty named macros from these reconstructed fields and
  reads/renames macro names in that block. This is emulated/static evidence;
  the inferred record still needs comparison with a non-empty OEM `mac.dct` or
  successful OEM load before claiming round-trip compatibility.
  The separate OEM `.mcr` import/export path wraps a `0x68`-byte child node plus its
  entries: export encrypts its buffer before `WriteFile`, and import decrypts
  the bytes read from the selected file before checking child magic
  `0xFACB99A2`. The transform is standard 32-round TEA over two little-endian
  32-bit words, using `delta=0x9E3779B9`; the executable's initial key words
  are `0x00010022`, `0x0060F420`, `3`, and `4`. Its buffer wrapper copies first
  and skips transformation unless the length is divisible by 8. A 128-byte
  non-empty sample exported by the installed OEM editor is 8-byte aligned and
  round-trips byte-for-byte through OpenHID; other alignments and event schemas
  remain unverified. The
  plaintext `.dct` tree parser
  validates a `0x58`-byte root with magic `0xA8B83333`; a child has a `0x68`-
  byte header with magic `0xFACB99A2`, followed by `count × 12` bytes of entries.
  The local macro-tree loader and serializer both consume and produce these
  12-byte entries. The local macro-file load path validates the child magic at
  `0x43E88F` and then calls the row builder at `0x43DE50` from `0x43E91E`; UI
  edit handlers call the same row builder. Static disassembly of that path
  reads the recovered fields as a
  16-bit event code at offset 0, 16-bit auxiliary field at offset 2, 16-bit
  value at offset 4, two unclassified bytes at offsets 6–7, and a 32-bit special
  value at offset 8. Before decoding, the row builder skips a record when its
  event code is zero and the 32-bit word at offset 4 is zero; this check includes
  bytes 6–7. `AulaOemMacroFile` owns the OEM-visible row indexes and count. The
  desktop hides that row like the OEM editor, while CLI inspection reports both
  raw records and visible rows and labels each empty row; both paths preserve the
  raw record.
  The desktop macro editor can move a visible event one position earlier or
  later by swapping the complete 12-byte records; hidden empty rows stay at
  their original raw indexes.
  For other ordinary rows the decoder uses offsets 0–5; the four special codes
  also consume offsets 8–11. This is a property of this UI decode path, not proof
  that the serialized bytes are globally unused. This function is reached from the
  local macro-tree load path and UI edit paths; its branches establish
  row-decoding behavior, not
  device playback semantics. It routes codes below `0xF5` through the auxiliary
  low bit and value using the Windows-layout-dependent key table; `0xF5`,
  `0xF6`, `0xF8`, and `0xF9` instead consume the special value. The editor's
  selector/model cross-reference maps choices `0xF0`–`0xF4` to cycle and
  movement labels, but the OEM-generated serialized sample with event code
  `0xF0` is displayed as mouse-left. Those editor labels therefore cannot be
  applied directly to the serialized event-code field. The selector
  initialization registers `0xF0`–`0xF6`, `0xF8`, and `0xF9`; the new-row
  handler at `0x43C0D0` stores the selected event code in its UI row object and
  separately assigns an internal dispatch value: `0xF0`–`0xF4` and `0xF8`/`0xF9`
  use the `0x10000` class, while `0xF5`/`0xF6` use `0x50000`. These values belong
  to the in-memory editor object, not the serialized 12-byte record, and do not
  establish device playback encoding. The macro editor maps selector choice 6
  to `0xF5`, after the five
  selector entries corresponding to cycle count, two cycle conditions, and
  X/Y movement. The OEM English/Chinese macro catalogs place “Delay” after
  those five entries, so OpenHID classifies `0xF5` as Delay. Its stored unit and
  playback timing semantics remain unknown; `0xF6`, `0xF8`, and `0xF9` remain
  special-payload events. The selector labels establish event names, not the
  meaning or units of their stored payload fields. The key
  conversion helper at `0x475990` calls
  `GetKeyboardLayout(0)` and dispatches only low-byte language IDs `0x07`,
  `0x0C`, `0x0A`, and `0x1F` to separate mapping tables. A semantic editor must
  preserve the recorded values and account for the active Windows layout; the
  current evidence does not justify converting them directly to USB HID
  usages. OpenHID exposes the recovered storage fields as `AulaOemMacroEvent`,
  classifies ordinary disk codes `0x00`–`0xF4` as mapped input actions based on
  the sample and decoder path, and classifies `0xF5`, `0xF6`, `0xF8`, and
  `0xF9` by their special decoder behavior while retaining the two unknown
  bytes. The desktop event details expose the raw fields separately while
  keeping payload meanings and timing unassigned. The desktop event editor
  offers selector codes `0xF5`, `0xF6`, `0xF8`, and `0xF9` and
  preserves auxiliary/value/reserved/payload fields when a code is selected;
  `0xF6`, `0xF8` and `0xF9` are labeled as opaque special payloads. The selector
  list and event classification share the core model's single owner. A second
  editor menu lists all 115 virtual-key codes in the recovered OEM key-to-usage
  table, labels common keys and shows the raw VK for every entry; choosing one
  changes only the event code, preserving all other raw fields. This is an
  offline editing aid and does not validate the transfer table on the F87 Pro.
  This improves offline editing only and does not establish payload units or device
  playback. The empty local `mac.dct` still prevents checking the tree format
  against OEM-generated nodes or assigning meanings to special event payloads.
  The serializer's source loop is capped
  at `0x3F7` records (1015);
  the offline `.mcr` and JSON profile codecs now reject larger event lists so an
  export cannot silently exceed the OEM app's observed local serialization
  bound. This is not the keyboard's macro capacity. The installed English and
  Chinese text catalogs describe
  editor rows for key start/hold/end times, delay, X/Y movement, fixed-count
  cycles, and cycles gated by key release or another key press. These labels
  describe the editor surface; they do not establish how each row maps to the
  12-byte record. The read-only `openhid diag aula --macro-prefix` probe
  currently tries `[0x13, 0x43, 0x01, …, 0x57]` on the advertised wireless
  report ID, following the same-model Linux implementation's `GetMacro`
  command. It expects a 512-byte prefix assembled from 37 `0x13` input
  fragments (36 × 14 bytes, then 8 bytes). By default it reports only fragment
  and byte counts; `--macro-prefix-output PATH` can save the incomplete raw
  prefix to a new file without printing its contents. This dump is not a
  complete or restorable backup. The CLI resolves the target from the running
  Agent's inventory and sends the request through its per-route HID queue; it
  refuses to open a direct session when the Agent is unavailable or the device
  is offline. The Agent's response budget covers the full prefix transfer. The
  first `0x13` attempt sent the descriptor's report ID with the wrong command
  and returned no fragments. While the keyboard was awake, the later `0x13/0x43`
  attempt returned all 37 expected fragments and a 512-byte all-zero prefix.
  This validates the response framing, but does not distinguish empty macro
  memory from a blank or unsupported region. This is
  an experimental prefix read, not a complete backup or decoded macro format.
  The installed OEM `GetMacro` method selects command `0x43` wirelessly
  (and `0x53` in wired mode), but its shared request helper builds report ID
  `0x15`, which the receiver descriptor does not advertise. A read-only attempt
  to send that OEM-shaped report through the selected Windows output collection
  was rejected by Windows with error 87 before a frame was sent. The earlier
  `0x13` request query returned zero fragments before its deadline.
  Static disassembly of this installed `OemDrv.exe` now confirms the wireless
  request fields: the `GetMacro` wrapper forwards its output buffer and requested
  byte length to the shared reader; that reader sends `[0x15, 0x43, 0x01, 0x00,
  0x00, …, 0x59]` and copies at most 14 payload bytes per response fragment.
  The final `0x59` is the reader's additive frame checksum for those request
  fields. The installed Windows OEM's 20-byte `0x15` request was rejected by
  the selected receiver collection before transmission (output error 87;
  Feature error 1). OpenHID's experimental 20-byte `0x13/0x43` request returned
  all expected responses while the keyboard was awake, but every payload byte
  was zero. The transport and framing work; non-empty macro readback remains
  unverified. OpenHID now has a separate read-only
  `--macro-table-candidate` path that repeats the same `0x13/0x43` query twenty
  times, using the generic OEM aggregate encoder's 0x2800-byte ceiling as a
  diagnostic bound; that limit is not established for F87. The output can be
  saved for analysis; the CLI reports distinct response rounds and all-zero
  rounds while preserving the original round order in the dump. This is
  explicitly
  not treated as complete or restorable: the F87 has not yet shown that each
  repeated query advances to a new region.
  The macro prefix command now shares the Linux/Windows raw-report platform gate
  with the OEM-carrier keymap probe; this enables a discriminating Windows
  `WriteFile` check without enabling macro writes. On 2026-10-23, after the user
  woke the keyboard, the read-only `openhid diag aula --psd` request returned
  `[03, 00, 00, 00, 01, 0b]`, confirming the F87 Pro wireless discriminator.
  The OEM-shaped macro request failed before transmission: the selected Windows
  `FF02/0002` collection rejected the 20-byte `0x15` frame through all tried
  `WriteFile`/`HidD_SetOutputReport` paths with error 87, and
  `HidD_SetFeature` with error 1. No macro fragments were sent and no dump file
  was created. A separate OpenHID `0x13/0x43` attempt later completed through
  the rebuilt Agent, collected all 37 fragments, and saved a 512-byte all-zero
  prefix. The successful transport does not confirm stored macro contents. A
  stale Agent had first rejected the new RPC because its request
  enum predated the appended method; the IPC version is now 41 and the rebuilt
  Agent accepts the request. The earlier `0x13/0x44` inventory timeouts and
  AULA search prompt were observed before wake and do not establish the current
  device state. The live model response confirms identity, but the Windows
  report-descriptor/send-path mismatch prevents validating the OEM macro read.
  The alternate advertised carrier has confirmed complete response framing,
  but no non-zero macro data or complete macro table has been recovered. The independent
  [aula-f87pro-linux reference](https://github.com/meursault03/aula-f87pro-linux)
  also describes only a 512-byte wireless read and declines wireless writes
  without a complete backup; this is a cross-project clue, not hardware
  validation for OpenHID. It proposes a 4096-byte table and four-byte events,
  but its cited helper addresses do not match this installed binary: local
  `0x48A6B0` is `CDevComboFilm::GetMatrix`, while `0x48A310` is inside the
  shared `0x15` packet helper, not an event encoder. Treat its format as an
  unverified lead. That repository also describes a wired Feature Report
  candidate: eight 512-byte chunks requested with command `0x85`, and command
  `0x05` for writes. The public [MIT-licensed AULA F87 Pro Linux
  implementation](https://github.com/meursault03/aula-f87pro-linux) offers USB
  macro editing, but its README says physical macro timing and playback modes
  remain unverified. Its 520-byte `0x06/0x05` write candidate conflicts with
  the installed Windows OEM's `0x407`-byte `HidD_SetFeature` macro-data frame;
  the relationship between these paths is unknown, so the community packet
  shape is not validated for this OEM build. OpenHID's
  `openhid aula-wired read-macros` reads the opaque
  4096-byte candidate after the exact wired F87 Pro model check. The new
  `openhid aula-wired write-macros` path requires an explicit `--apply`, saves
  the complete original table to a new backup file first, rejects a stale
  backup, writes eight `0x06/0x05` chunks, verifies the full readback, and
  attempts to restore and verify the original table on failure. The raw table
  format and these wired transfers remain unverified on attached F87 Pro
  hardware; the candidate writer must not be treated as compatible with the
  installed OEM until the frame-length conflict is resolved. No macro event
  editor or playback test uses this candidate yet.
  The full wireless macro command fields, transfer-buffer
  schema, device capacity and persistence remain unresolved; reply matching
  has only been recovered from static code, not a live exchange.
  The separate F87/F75 wired keymap
  reference's 520-byte Feature Report `0x06` does not resolve the discrepancy:
  it matches the community macro candidate but differs from the OEM's
  `0x407`-byte macro-data frame, and neither transfer is validated against
  this wired device.
  A read-only Windows HID preparsed-data query of the connected receiver's
  `FF02/0002` control collection reports 20-byte input/output reports and one
  output button-cap entry with report ID `0x13`. Its separate `FF04/0002`
  collection exposes an 8-byte Feature report with ID `0x06`; neither collection
  advertises output report ID `0x15`. The query opened collection handles with
  shared read/write access only to inspect HID metadata; it sent no HID reports.
  This descriptor does not establish whether Windows `WriteFile` or the device
  accepts an unadvertised report ID. Windows rejected the read-only `0x15`
  request; no setting-changing report was attempted. The report descriptor
  mismatch and unresolved command/data schema still need to be resolved before
  implementing wireless macro writes.
  Public cross-checks have not supplied a sender: the [Aula-Manager protocol
  notes](https://github.com/free-soldier28/Aula-Manager/blob/main/docs/PROTOCOL.md)
  document lighting and per-key RGB, while [AULA F87 Control](https://pypi.org/project/aula-f87-control/0.3.2/)
  lists lighting, per-key colors, battery, sleep, side-light and music-sync
  features without a macro transfer path. This absence does not show that the
  F87 firmware lacks macro support; it leaves the installed OEM driver as the
  only available macro-sender evidence in this investigation.
- Do not use the separate [F108 Pro macro notes](https://github.com/parsiya/f108-pro/blob/main/ai-docs/macro-protocol.md)
  as the F87 wire format: that document itself says its protocol was not
  implemented or hardware-tested.
  Resolve the F87 sender's full call sequence and data schema, then verify a
  backed-up macro edit and playback on awake hardware before implementing macro
  writes.

## OEM audio-reactive lighting lead (Windows, 2026-10-23)

- Read-only inspection of the installed driver finds RTTI for `CMusicDlg`,
  `CAudioClient`, and `CMMNotificationClient`, plus the string
  `CMusicDlg::StartMusic bStart=%d`. The installed English catalog contains a
  Music panel with gain factor, color/colorful mode, effect, smoothness, and 11
  mode labels. This establishes an audio-reactive feature in the installed
  driver package, but does not prove the F87 PRO model enables the panel or
  identify its HID streaming protocol.
- Static disassembly of `CMusicDlg::StartMusic` (`0x44b390`) resolves the worker
  gate: `ShowPower` must be set and `MaxFnNum` must equal zero. The F87 PRO
  `KB.ini` sets `ShowPower=0`; it omits `MaxFnNum`, whose driver default is
  `0x30`. Both conditions therefore block worker creation for this model.
  `MusicUI` and `LightBarMusicCmd` are also absent from all three installed F87
  profile files and default to zero. Static code shows `MusicUI` being loaded
  into the driver config but no other direct field reference; this alone does
  not establish whether a generic panel can be displayed.
- The installed F87 configuration therefore provides no evidence that the
  OEM exposes audio-reactive lighting on this model. Treat the music panel as
  generic package content, not a required F87 parity feature, unless direct
  OEM UI or hardware evidence contradicts these gates. No audio capture or
  streaming protocol has been implemented in OpenHID.

## OEM wireless model discriminator query (Windows, 2026-10-23)

- Static analysis of the installed `OemDrv.exe` discovery call at `0x40BDA1`
  confirms it requests six bytes with report ID `0x13`, command `0x05`, then
  supplies those bytes and the HID VID/PID to the six-byte `Psd` profile matcher.
  The shared reader at `0x486FE0` constructs a 20-byte frame with one requested
  page and the additive checksum. Its response path matches report ID and
  command, validates page sequencing, and copies up to the requested length
  from response payloads. On a one-second input timeout it resends the same
  query, up to five attempts.
- OpenHID now exposes this as `openhid diag aula --psd`, a read-only query on the
  already verified wireless control collection. Initial attempts on the
  connected Windows receiver timed out after five one-second attempts with zero
  input reports while the keyboard was asleep. After the user awakened it, the
  query succeeded and returned the installed wireless profile's six-byte
  fingerprint (`03 00 00 00 01 0b`). A same-session read of the established
  `0x13/0x44` configuration command timed out without input. A successful query
  reports six bytes for comparison with the installed wireless profile
  (`03 00 00 00 01 0b`) and wired profile (`03 00 00 00 00 a9`). The diagnostic
  does not yet identify the wired collection or enable wired support; those
  require a separately verified wired transport and response.
- The earlier `FF04/0002` Feature report `0x06` read returned
  `06 00 02 00 e2 00 56 29`, which is not either profile discriminator. That
  feature read is separate from this OEM `0x13/0x05` input/output transaction.
- A separate wired discovery probe is available as
  `openhid diag aula --wired-model-probe`. It targets only the direct USB
  `258A:010C` candidate and sends the documented Feature Report `0x06/0x82`
  model query, then prints the first 14 response bytes. The query matches the
  F75/F87 protocol reference. OpenHID now also checks response byte 13 for the
  documented F87 Pro code `0x0B`, and the diagnostic prints the exact identity
  of the collection that answered. It still does not match the installed app's
  six-byte wired `Psd` fingerprint. No settings are written. Multiple valid
  collections are rejected as ambiguous; specify `--identity` to select one.
  A fresh Windows run on 2026-10-23 found no `258A:010C` collection, so no
  wired query or wired keymap transfer was sent. An attached `3554:FA09`
  wireless receiver is not a wired candidate.
- `aula-keymap write-wired` uses that same model query as a pre-write gate on
  the explicitly selected collection. It checks the response ID, command, and
  echoed selector bytes, plus family code `0x03` and F87 Pro model code `0x0B`,
  before sending the 520-byte keymap report. The
  independent [Aula-Manager protocol sample](https://github.com/free-soldier28/Aula-Manager/blob/main/docs/PROTOCOL.md)
  documents a 14-byte `0x06/0x82` response on an F75 and describes F87 as the
  reference model; the response-to-F87-Pro `Psd` mapping remains unverified.
