---
paths:
  - "crates/openhid-hook/**"
  - "crates/openhid-inject/**"
  - "crates/openhid-hid/**"
  - "crates/openhid-agent/src/autostart.rs"
  - "crates/openhid-agent/src/autostart/**"
  - "crates/openhid-agent/src/resume_windows.rs"
  - "crates/openhid-camera/**"
  - "crates/openhid-permissions/**"
---

# Platform / cfg-gated code — macOS-green is a trap

macOS-green proves **nothing** about `#[cfg(target_os = "linux")]` /
`windows` code. Recent agent failures that only showed up on CI Linux:

- Shadowing a crate-level constant with a local `const` of a different type
  (e.g. `LOGITECH_VENDOR_ID: u16` next to `use crate::LOGITECH_VENDOR_ID`
  which is `u32`) — E0255 / E0308, **only compiles on Linux**.
- Importing a name that only exists on another OS, or redefining one that
  master already exports from `lib.rs`.

When the diff touches any of:

- `crates/openhid-hook/src/linux.rs` / `windows/**`
- `crates/openhid-inject/src/inject/linux.rs` / `windows.rs`
- `crates/openhid-agent/src/autostart/linux.rs` / `windows.rs`, or
  `crates/openhid-agent/src/resume_windows.rs`
- `crates/openhid-camera/src/capture_linux.rs`, `capture_windows.rs`,
  `com_windows.rs`, `uvc_windows.rs`, `uvc_linux.rs`, `linux.rs`
- `crates/openhid-hid/src/transport.rs` or `transport/windows.rs`
- any `#[cfg(target_os = …)]` block, in any crate

you MUST either:

1. Cross-check with devenv when available:
   `devenv tasks run openhid:check-windows` (also
   `cargo xtask ci clippy-windows`), or
2. Manually re-read every changed cfg-gated file against **current master** for:
   - name collisions with existing `pub use` / `pub const` items
   - type mismatches (`u16` vs `u32`, `Option` arity, new enum fields)
   - call sites that gained args on master (e.g. `AppState::new`, `build_device_list`,
     `dispatch_action`) but the PR still uses the old signature

Do not claim "cross-platform green" without CI (or a local cross-lint) having
actually run those targets. `RUSTFLAGS=-D warnings` is global in CI — plain
warnings fail there too.

There is no Linux equivalent of the Windows task, and it cannot be complete if
there ever is: `openhid-camera`'s Linux backend needs kernel headers
(`v4l2-sys` wants `linux/videodev2.h`), so it does not cross-compile from macOS
at all. For a Linux-only change outside camera, rustup's
`aarch64-unknown-linux-musl` target covers the rest — but leave out `openhid`,
`openhid-cli` and `openhid-assets`, whose `ureq → ring` dependency needs a
cross C toolchain:

```sh
cargo clippy --target aarch64-unknown-linux-musl \
  -p openhid-hook -p openhid-inject -p openhid-hid -p openhid-hidpp \
  -p openhid-core -p openhid-agent -p openhid-agent-core -p openhid-ipc \
  -p openhid-permissions --all-targets -- -D warnings
```

Everything that recipe skips — camera on Linux above all — is CI's alone to
catch.
