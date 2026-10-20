# AULA F87 PRO: experimental wireless support

Investigation and verification: October 19–20, 2026. Overall audit: **YELLOW**.
Recognition, complete configuration reads, recoverable built-in lighting and
wireless sleep edits, configuration-block backup/restore, and a desktop settings
editor are implemented. Full OEM application replacement
is deferred beyond the current milestone; the limitations below retain its acceptance criteria.

OpenHID recognizes the measured 2.4 GHz receiver control collection as one
standalone keyboard. It never enters Logitech HID++ probing or pairing. The
agent owns device I/O; the desktop and overlay remain IPC clients. New read/set
lighting and settings RPC methods are appended to the service; matching client and agent
builds are required (development protocol version 38).

| Property | Verified value |
| --- | --- |
| Vendor/product | `3554:FA09` |
| Control usage | `FF02/0002` |
| Input/output length | 20 bytes, including report ID `13` |
| Driver | `aula-f87-pro-wireless` |

The neighboring `FF04/0002` feature collection and standard input
collections are excluded. The shared wired identity `258A:010C` is not claimed.
Bluetooth configuration has not been verified.

## Current delivery scope

The current milestone covers the verified wireless receiver, built-in lighting,
idle timeout, recoverable configuration transactions, configuration backup/restore,
and read-only base-matrix inspection/export. Matrix writes, Fn-layer editing,
macros, per-key RGB, wired configuration and firmware tools are deferred to a
later milestone. The acceptance table below retains those future requirements;
it does not describe features included in this delivery.

## OEM replacement acceptance

The target remains replacement of the installed F87 PRO application. A passing
read or an offline test is not evidence that the following whole workflow works.
The installed `Cfg.ini`, `KB.ini` and English/Chinese text catalogs establish the
available concepts; visibility and transport restrictions still need UI/hardware
confirmation rather than assuming every generic catalog entry applies.

| Workflow | Current evidence | Remaining acceptance |
| --- | --- | --- |
| One keyboard in inventory | Strict receiver collection identity, complete live config probe, injected inventory tests, and real-agent CLI/GUI inventory agreement | Native reconnect and sleeping-device recovery |
| Built-in lighting and persistent settings | Exact echoes, complete staged/saved readback; real brightness change and restoration | Human-visible effect/speed/off checks, color/direction controls and power-cycle persistence |
| Wireless sleep | OEM option table; Rust 300-to-600-second SAVE with independent readback and full original restoration; native Chinese editor reads the original 300 seconds | Power-cycle persistence and physical writes initiated from the desktop |
| Debounce | OEM `ShowDebounce` setting only | Isolate the actual field, units, bounds and persistence |
| Board key assignments and combinations | Physical wireless read of 126 entries in 36 frames; all 87 physical keys correlated to OEM slot coordinates; validated matrix-only backup | Normal/Fn layer interpretation, reversible writes and observed key behavior |
| Macro editing and playback | OEM catalog advertises press/release, delay and repeat modes | Determine storage/limits and verify saved playback, cancellation and restoration |
| Custom/per-key RGB | Separate region is indicated by references | Read/backup it, identify transport and verify recoverable edits without replacing unrelated colors |
| Profiles and backup/restore | Validated configuration-block restore and separate base-matrix export | Matrix restore, every Fn/macro/RGB region, and complete restoration |
| Wired configuration | OEM lists shared `258A:010C` and model discriminator `Psd` | Physical descriptor/identity, correct feature report shape and full read/write recovery |
| Bluetooth configuration | No verified control channel | Establish whether supported by this device/OEM workflow before claiming support |
| Firmware tools | Generic OEM catalog mentions USB-only upgrades | Identify applicability and recovery/validation requirements; no firmware writes implemented |
| Desktop usability | IPC editor tests plus real-agent Chinese UI reads, complete effect/sleep menus, dark normal and light 125% layouts | Physical writes initiated from the desktop, live read-failure recovery and debug-build locale stack overflow |

## Commands and desktop behavior

From this checkout:

```sh
cargo run -p openhid -- list
cargo run -p openhid -- diag aula
cargo run -p openhid -- diag aula --matrix # experimental read-only raw matrix
cargo run -p openhid -- diag aula --matrix --backup f87-base-matrix.json
cargo run -p openhid -- diag aula --brightness 2 --preview
cargo run -p openhid -- diag aula --effect 12 --brightness 3
cargo run -p openhid -- diag aula --effect 12 --brightness 3 --save
cargo run -p openhid -- diag aula --sleep 600 --preview
cargo run -p openhid -- diag aula --sleep 300 --save
cargo run -p openhid -- diag aula --backup f87-config.json
cargo run -p openhid -- diag aula --restore f87-config.json --preview
cargo run -p openhid -- diag aula --restore f87-config.json --save
```

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
Effect labels are neutral until OEM names have been mapped independently.

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
key names or interpreting unknown function encodings. It shares the configuration
reader's deadlines, checksum/sequence validation and duplicate handling. It never
writes assignments, SAVE or reset. Physical reads now succeed: 36 frames contain
126 entries, covering all 87 physical key positions in the installed OEM table.
The other 39 entries are retained without assuming their purpose. Earlier reads
and configuration controls timed out, so recovery after inactivity remains open.

`--matrix --backup FILE` exports a separate versioned `wireless-base-matrix`
region with every original response frame. Deserialization verifies the driver,
region, frame order/count, payload bounds and checksum; unknown entries are retained
without padding to the OEM upper bound. Matrix files are rejected by configuration
restore before hardware access. Matrix restoration and Fn/macro/RGB backups are
not implemented yet; this is not a complete device backup.

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
- Windows is the current build and hardware host. Linux/macOS I/O, hotplug,
  sleep/wake, native desktop interactions and cross-platform cancellation require
  additional acceptance evidence.
- Battery, debounce, remapping, macros, media/combo actions,
  custom colors/per-key RGB, profiles, backup/restore of separate storage regions, firmware update,
  and wired/Bluetooth configuration are not supported yet. The original app's
  advertised controls do not prove protocol compatibility or field locations.
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
- The debug desktop currently overflows its Windows main-thread stack while
  initializing the generated locale backend. PE/symbol inspection found a
  1,048,576-byte reserve and a 1,174,960-byte allocation in
  `openhid_desktop::_RUST_I18N_BACKEND::closure$0`. The optimized binary did not
  reproduce the failure. No stack-size workaround or generated-locale refactor
  was added to the AULA change. Debug startup is a separate follow-up here;
  local evidence: `ui-acceptance/debug-stack-probes.json`,
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
