//! Built-in lighting fields within the preserved configuration block.

use super::{AulaChange, ConfigError, ConfigResult, WirelessConfig, checksum};
use crate::backend::HidBackend;
use openhid_core::hid::aula::aula_effect_capabilities;
pub use openhid_core::hid::aula::{
    AulaColorModeChange, AulaColorModeState, LightingChange, LightingMode, LightingState,
};

impl WirelessConfig {
    /// Decode the current built-in lighting fields without assuming capabilities.
    #[must_use]
    pub fn lighting(&self) -> LightingState {
        let effect = self.frames[0][15];
        let params = effect_location(effect).map(|(sequence, offset)| {
            let flags = self.frames[sequence][offset + 1];
            (
                self.frames[sequence][offset],
                flags >> 4,
                match flags & 0x0f {
                    0 => AulaColorModeState::SingleColor,
                    7 => AulaColorModeState::Colorful,
                    palette => AulaColorModeState::Palette(palette),
                },
            )
        });
        LightingState {
            effect,
            brightness: params.map(|value| value.0),
            speed: params.map(|value| value.1),
            color_mode: params.map(|value| value.2),
        }
    }

    pub(super) fn edited(&self, change: LightingChange) -> Result<Self, ConfigError> {
        self.edited_settings(
            AulaChange::new(Some(change), None).map_err(|_| ConfigError::InvalidSetting)?,
        )
    }

    pub(super) fn edited_settings(&self, change: AulaChange) -> Result<Self, ConfigError> {
        let mut target = self.clone();
        if let Some(change) = change.lighting() {
            let effect = change.effect().unwrap_or_else(|| self.lighting().effect);
            if !LightingChange::supports_effect(effect) {
                return Err(ConfigError::InvalidSetting);
            }
            target.frames[0][15] = effect;
            if change.brightness().is_some()
                || change.speed().is_some()
                || change.color_mode().is_some()
            {
                let (sequence, offset) =
                    effect_location(effect).ok_or(ConfigError::InvalidSetting)?;
                if let Some(brightness) = change.brightness() {
                    target.frames[sequence][offset] = brightness;
                }
                if let Some(speed) = change.speed() {
                    let flags = &mut target.frames[sequence][offset + 1];
                    *flags = (*flags & 0x0f) | (speed << 4);
                }
                if let Some(color_mode) = change.color_mode() {
                    let capabilities =
                        aula_effect_capabilities(effect).ok_or(ConfigError::InvalidSetting)?;
                    let mode_bits = match color_mode {
                        AulaColorModeChange::SingleColor(_) if capabilities.has_custom_color() => 0,
                        AulaColorModeChange::Colorful if capabilities.has_random_color() => 7,
                        _ => return Err(ConfigError::InvalidSetting),
                    };
                    let flags = &mut target.frames[sequence][offset + 1];
                    *flags = (*flags & 0xf0) | mode_bits;
                }
            }
        }
        if let Some(timeout) = change.sleep_timeout() {
            target.frames[1][15] = timeout.wire_value();
        }
        if let Some(debounce) = change.debounce() {
            target.frames[0][8] = debounce.wire_value();
        }
        if let Some(effect) = change.side_light_effect() {
            target.set_side_light_effect(effect);
        }
        for frame in &mut target.frames {
            frame[19] = checksum(frame);
        }
        Ok(target)
    }
}

// The 18 built-in effects' two-byte parameter tables span fragments 4..6.
fn effect_location(effect: u8) -> Option<(usize, usize)> {
    match effect {
        1..=6 => Some((4, 7 + usize::from(effect - 1) * 2)),
        7..=13 => Some((5, 5 + usize::from(effect - 7) * 2)),
        14..=18 => Some((6, 5 + usize::from(effect - 14) * 2)),
        _ => None,
    }
}

/// Apply a snapshot-preserving lighting edit using the shared transaction owner.
pub async fn update_lighting(
    backend: &dyn HidBackend,
    identity: Option<&str>,
    change: LightingChange,
    mode: LightingMode,
    cancelled: impl Fn() -> bool,
) -> Result<ConfigResult, ConfigError> {
    super::update_config(
        backend,
        identity,
        super::ConfigChange::Lighting(change),
        mode,
        cancelled,
    )
    .await
}
