//! Compatibility re-exports for semantic keyboard-backlight types.
//!
//! The pure domain types live in `openhid-core`; HID++ conversion and
//! transport remain in this crate's write layer.

pub use openhid_core::hid::backlight::{
    BacklightChange, BacklightControls, BacklightMode, BacklightState, BacklightStatus,
};
