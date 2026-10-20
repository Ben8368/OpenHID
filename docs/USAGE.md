# Usage (CLI)

The `openhid` command-line tool. For install and configuration, see the
[README](../README.md).

```sh
openhid list                 # paired devices: slot, codename, kind, online, battery
openhid assets sync          # pre-fetch device renders from the fastest available mirror
openhid backlight             # read or persistently set keyboard backlight (HID++ 0x1982)
openhid camera               # read or write Logitech UVC image controls
openhid snapshot             # capture one webcam frame to a PNG (default: snapshot.png)
openhid light list           # list standalone lights (Litra) and their controls
openhid fixture contribute   # privacy-safe fixture wizard (see fixtures/README.md)
openhid fixture verify DIR   # strictly verify one on-disk fixture directory
openhid diag aula            # read and validate F87 PRO 2.4 GHz configuration (experimental)
openhid diag aula --brightness 2 --preview # verify lighting and restore the full original snapshot
openhid diag aula --effect 12 --brightness 3 --save # explicitly SAVE and read back
openhid diag aula --matrix # experimental read-only raw key matrix
openhid diag aula --matrix --backup f87-base-matrix.json # matrix-only export; no restore yet
openhid diag aula --sleep 600 --preview # verify a sleep edit, then restore
openhid diag aula --backup f87-config.json # configuration block; excludes keymaps/macros/RGB tables
openhid diag aula --restore f87-config.json --save # restore validated configuration and verify
openhid diag features        # dump every HID++ feature the active device reports
openhid diag controls        # dump reprogrammable controls and capability flags
openhid diag battery          # read the raw battery report (0x1004 / 0x1000 / 0x1F20)
openhid diag dpi             # read → write → read-back → restore DPI (smoke test)
openhid diag pointer-speed             # test 0x2205 pointer scaling and attempt to restore it
openhid diag smartshift                # toggle SmartShift and restore (smoke test)
openhid diag smartshift --torque 60    # set scrolling force / feedback intensity (1-100%)
openhid diag wheel                     # read or set HID++ 0x2121 wheel reporting resolution
openhid diag lighting ff0000           # solid colour for a wired RGB keyboard (any RRGGBB hex)
```

Running `openhid` with no subcommand defaults to `list`. Set
`OPENHID_LOG=debug` for verbose tracing in the CLI, GUI, or agent.

`openhid diag pointer-speed` temporarily changes the selected device's pointer scaling. The command attempts to restore and verify the original scaling even when the test write fails. `--device NAME` selects a device; `--target RAW` sets the temporary 8.8 fixed-point value (`256` is 1×). By default, the command halves the current value, or doubles it when the raw value is `1`. Firmware may accept extreme values without clipping. A restoration error means the original speed could not be confirmed.

Experimental AULA F87 PRO support and hardware acceptance steps are documented in
[the device notes](devices/aula-f87-pro.md).

## Automation API

`openhid api` is the JSON interface for launchers such as Raycast and local
scripts. It requires a running, protocol-compatible OpenHID agent. It never
opens hardware directly, starts the agent, or falls back to diagnostic commands.
Start OpenHID normally before using it. Existing human-readable commands are
unchanged.

```sh
openhid api status
openhid api devices
# Copy an exact, non-null id from api devices; quote it as one argument.
openhid api dpi --device "$DEVICE_ID"
openhid api dpi --device "$DEVICE_ID" --set 1200
openhid api dpi --device "$DEVICE_ID" --set 1200 --save
openhid api smartshift --device "$DEVICE_ID"
openhid api smartshift --device "$DEVICE_ID" --mode free
openhid api smartshift --device "$DEVICE_ID" --mode ratchet --auto-disengage 255
openhid api fn-lock --device "$DEVICE_ID"
openhid api fn-lock --device "$DEVICE_ID" --set on
openhid api os-layout --device "$DEVICE_ID" --set off --save
openhid api report-rate --device "$DEVICE_ID"
openhid api report-rate --device "$DEVICE_ID" --set 1000 --save
openhid api easy-switch --device "$DEVICE_ID"
openhid api easy-switch --device "$DEVICE_ID" --channel 2 --name "Work laptop"
```

### Output and compatibility

After arguments parse, stdout contains exactly one JSON object and a newline.
Success exits with status **0**; runtime failure exits with status **1**:

```json
{"schema_version":1,"ok":true,"data":{"inventory":"ready","devices":[]}}
```

```json
{"schema_version":1,"ok":false,"error":{"code":"agent_unavailable","message":"…"}}
```

Consumers must check `schema_version` and `ok`, ignore unknown object fields,
and handle unknown error codes as failures. Breaking changes to this JSON
contract require a schema version change; it is not the internal IPC version.
Error messages are explanatory text, not stable values to parse. CLI syntax
errors retain standard clap behavior (exit **2**, diagnostics on stderr);
`--help` and `--version` are text. Logging also goes to stderr. Failure to write
stdout cannot provide a JSON envelope.

| Command | `data` fields |
| --- | --- |
| `status` | `agent_version`, `inventory`, `accessibility_granted`, `input_monitoring_granted`, `hook_installed`, `hid_open_failures`, `launch_at_login` |
| `devices` | `inventory`, `devices`: array of `id`, `name`, `kind`, `online`, `battery`, `capabilities`, `light_capabilities` |
| `dpi` | `device`, `persistence`, `current` (integer), `supported` (array of integers) |
| `smartshift` | `device`, `persistence`, `mode` (`free` or `ratchet`), `auto_disengage` (integer), `tunable_torque` (1–100, percent of the wheel's maximum force, or null) |
| `fn-lock` | `device`, `persistence`, `fn_lock`, `default_fn_lock` (boolean, or null when the keyboard reports no firmware default) |
| `os-layout` | `device`, `persistence`, `keep_os_layout` (boolean) |
| `report-rate` | `device`, `persistence`, `current` (integer, hertz), `supported` (array of integers in hertz, for the current connection) |
| `easy-switch` | `device`, `persistence`, `current_channel` (integer from 1, or null), `can_rename` (boolean), `channels`: array of `channel` (integer from 1), `paired` (boolean), `connection` (`unknown`, `unifying`, `usb`, `bluetooth`, or `bolt`), `name` (string or null), `name_max_len` (integer, UTF-8 bytes), `os` (string such as `macOS 15.2`, or null) |

`inventory` is `scanning`, `ready`, or `unavailable`. Only `ready` with an empty
device array means enumeration completed and found no peripherals. Device
operations are refused until inventory is ready. Status queries do not arm a
dormant agent's input stack; opening the desktop app follows the normal lifecycle.

Device IDs are opaque current-route identifiers, not persistent physical-device
keys. Refresh them after reconnects or updates; do not construct IDs, parse them,
or use array positions or names to address devices. Missing routes have a null
ID. Duplicate IDs are returned without deduplication, and operations against
them fail with `ambiguous_device`. In particular, the current transport cannot
distinguish identical direct-attached devices that share one route. IDs can
contain hardware identity information: keep them local and redact before sharing.

`name`, `battery`, and measured `capabilities` may be null; null does not mean
zero charge or an unsupported feature. Battery objects contain `percentage`,
`level`, and `status` (for example `charging` or `discharging`). `percentage`
is null when the device has no state-of-charge gauge and reports only a level,
or when the reading carries no charge figure; read `level` in that case.
Capabilities are the agent's measured feature flags, not guesses based on
device kind.
Not every readable setting has an inventory flag; a setting read can return
`unsupported_feature`. Standalone lights are listed with their light capability
descriptor and null battery. Cameras are not part of this agent inventory API.
The response excludes pairing passkeys, application history, and model serials.

### Immediate control and saved preferences

The DPI, SmartShift, Fn-lock, and report-rate commands read only unless a setting flag is supplied. Without
`--save`, writes return `persistence: "not_saved"`: they do not modify
`config.toml` or request a config reload. Saved preferences may be reapplied on
reconnect, wake, or later configuration changes; firmware may retain some values.

Add `--save` to an explicit DPI, SmartShift, Fn-lock, or report-rate change to save the verified
setting and request an agent config reload. Success returns `persistence: "saved"`.
The device must have a probed physical identity. Existing legacy configuration
entries are respected; CLI route IDs are never used directly as config keys.
Saving edits only the selected setting, preserving unrelated preferences and
comments. A per-link override of that setting is refused with `link_override`
rather than silently saving a default that the override would supersede.
There is no profile-switch API yet.

`os-layout` changes an agent setting rather than the device, so it requires
both `--set` and `--save`; without `--save` it fails with `invalid_value`. The
agent applies the saved value after the reload. `on`, the default for keyboards
that allow it, sets the keyboard's key layout (HID++ `0x4531`) to the host OS
when the keyboard connects, when the setting turns on, and after the keyboard
reports that a layout shortcut switched it. `off` leaves the keyboard's layout
as it is; it does not restore an earlier layout. A keyboard whose measured
capabilities lack `os_layout` returns `unsupported_feature`.

`report-rate --set` accepts only a rate in `supported`. That list belongs to
the device's current connection — a gaming mouse can offer more rates over its
receiver or a cable than over Bluetooth — so any other value fails with
`invalid_value` before anything is written. A saved rate is re-applied on
reconnect only over a connection that lists it. A device without HID++
`0x8060` or `0x8061` returns `unsupported_feature`.

`easy-switch` lists the device's Easy-Switch channels (HID++ `0x1815`) and the
name and OS each paired computer reported. `--channel` and `--name` together
rename the computer on that channel; the device stores the name itself, so
there is no `--save` (it fails with `invalid_value`) and the result is always
`persistence: "not_saved"`. Surrounding whitespace is trimmed. A name that is
empty, contains NUL, or is longer than the channel's `name_max_len` bytes, or a
channel with no paired computer, fails with `invalid_value` before anything is
written. A device that does not let software rename hosts (`can_rename` is
false) returns `unsupported_feature`, and a name the device does not read back
returns `readback_mismatch`. A device without `0x1815` returns
`unsupported_feature`.

The desktop app shows the same channels in an Easy-Switch card on the Device
tab and renames a paired channel under the same name rules. Neither the CLI
nor the app renames a channel on its own: unlike Logi Options+, OpenHID does
not write this computer's name or OS onto the current channel when it starts.

Hardware writes, file persistence, and reload are separate steps, not an atomic
transaction. The config revision is checked under a writer lock; concurrent edits
are not overwritten. A lock conflict is reported immediately, without waiting.
The persistent `config.toml.lock` sidecar must not be deleted while writers run.
External editors that do not honor this lock can still race with a save.

If saving fails after verified hardware readback, the JSON error includes
`persistence: "not_saved"` with `config_conflict`, `config_busy`, or
`config_save_failed`. The live hardware has already changed; no rollback or
automatic retry is attempted. If saving succeeds but reload fails, the error
includes `persistence: "saved"`: `config_reload_failed` means rejection;
`config_reload_unknown` means timeout/disconnection with an unknown reload
outcome. Neither is reported as success. Other errors occur before persistence;
hardware outcomes after a failed write/readback may still be uncertain.

DPI must be in the device-reported supported list; values are never silently
rounded. SmartShift preserves unspecified fields, including wheel torque.
`auto_disengage` is 1–254 in firmware units of 0.25 turn/s; 255 means permanent
ratchet, and 0 is rejected. `--mode ratchet` alone does not disable automatic
release: use `--auto-disengage 255` as well. Fn lock `on` means bare function keys
send F1–F12; `off` means the printed media functions.
With `--save`, SmartShift must use the configuration-supported range 8–255;
this also validates a threshold retained from the device when only mode changes.

DPI and SmartShift are read back after writes; Fn lock returns the firmware
echo. Reads and writes are not a transaction against simultaneous GUI changes.
A timeout, disconnect, or readback error after a write means its outcome may
be uncertain: read the setting again rather than blindly retrying the write.

Stable error codes: `agent_unavailable`, `handshake_failed`, `version_mismatch`,
`timeout`, `disconnected`, `inventory_not_ready`, `device_not_found`,
`device_offline`, `ambiguous_device`, `unsupported_feature`, `invalid_value`,
`readback_mismatch`, `device_error`, `identity_unavailable`, `link_override`,
`config_error`, `config_conflict`, `config_busy`, `config_save_failed`,
`config_reload_failed`, and `config_reload_unknown`. A protocol mismatch requires matching
OpenHID CLI and agent builds; it never triggers direct-hardware fallback.

## Asset synchronization

Asset synchronization probes `assets.openlogi.org`, the versioned Cloudflare
Pages release alias, and the pinned jsDelivr npm release concurrently. The first
mirror with a valid catalog supplies every file for that synchronization run.
Set `OPENHID_ASSETS` or pass `openhid assets sync --base <URL>` to use one
uniform asset origin instead of automatic mirror selection.

## Tray battery display

On Windows and macOS, Settings > General can show a battery glyph in place of
OpenHID's tray icon. Devices needing attention take priority over a lower
reading on a charging device. Hover for device readings, or open the tray menu
and choose a device to open its settings, even when the settings window is closed.

The menu uses your custom device names and combines connections belonging to the
same physical device. Cached or held charge values are not displayed as current
percentages in the tray. The settings app retains last-known readings and labels
them accordingly. Devices reporting only coarse battery levels show those levels,
not invented percentages. This display does not send low-battery notifications.
