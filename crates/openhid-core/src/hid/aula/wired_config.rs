//! Validated wired F87 Pro configuration block and setting edits.

use thiserror::Error;

use super::{
    AulaChange, AulaEffectCapabilities, AulaSideLightEffect, AulaState, LightingChange,
    LightingState, SleepTimeout, aula_effect_capabilities,
};

/// Wired F87 Pro configuration block length, in bytes and in the protocol's `u16` length field.
pub const AULA_WIRED_CONFIG_BLOCK_LENGTH: u16 = 128;
/// Byte length used by fixed-size arrays for the wired configuration block.
pub const AULA_WIRED_CONFIG_BLOCK_BYTE_LENGTH: usize = AULA_WIRED_CONFIG_BLOCK_LENGTH as usize;

const CONFIG_EFFECT_OFFSET: usize = 10;
const CONFIG_SLEEP_OFFSET: usize = 24;
const CONFIG_SIDE_LIGHT_OFFSET: usize = 18;
const CONFIG_EFFECT_SETTINGS_OFFSET: usize = 56;
const CONFIG_END_MARKER_OFFSET: usize = 126;

/// A complete healthy wired F87 Pro config block; unrecognized bytes are retained.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AulaWiredConfigBlock {
    bytes: [u8; AULA_WIRED_CONFIG_BLOCK_BYTE_LENGTH],
}

impl AulaWiredConfigBlock {
    /// Validate a complete healthy 128-byte block and retain every byte.
    pub fn from_bytes(bytes: &[u8]) -> Result<Self, InvalidAulaWiredConfigBlock> {
        let bytes: [u8; AULA_WIRED_CONFIG_BLOCK_BYTE_LENGTH] = bytes
            .try_into()
            .map_err(|_| InvalidAulaWiredConfigBlock::Length)?;
        if bytes[1..3] != [0x03, 0x03]
            || bytes[6..9] != [0x04, 0x04, 0x07]
            || bytes[11] != 0x20
            || bytes[CONFIG_END_MARKER_OFFSET..] != [0x5a, 0xa5]
        {
            return Err(InvalidAulaWiredConfigBlock::Markers);
        }
        Ok(Self { bytes })
    }

    /// The complete configuration block, including preserved unknown bytes.
    #[must_use]
    pub const fn as_bytes(&self) -> &[u8; AULA_WIRED_CONFIG_BLOCK_BYTE_LENGTH] {
        &self.bytes
    }

    /// Decode supported lighting, sleep and side-light settings from the block.
    #[must_use]
    pub fn aula_state(&self) -> AulaState {
        let effect = self.bytes[CONFIG_EFFECT_OFFSET];
        let capabilities = aula_effect_capabilities(effect);
        let settings_offset = usize::from(effect) * 2;
        let brightness = capabilities
            .filter(|value| value.has_brightness())
            .map(|_| self.bytes[CONFIG_EFFECT_SETTINGS_OFFSET + settings_offset])
            .filter(|value| LightingChange::levels().contains(value));
        let speed = capabilities
            .filter(|value| value.has_speed())
            .map(|_| self.bytes[CONFIG_EFFECT_SETTINGS_OFFSET + settings_offset + 1] >> 4)
            .filter(|value| LightingChange::levels().contains(value));
        let lighting = LightingState {
            effect,
            brightness,
            speed,
            color_mode: None,
        };
        AulaState::new(
            lighting,
            SleepTimeout::from_wire(self.bytes[CONFIG_SLEEP_OFFSET]),
        )
        .with_side_light_effect(AulaSideLightEffect::from_wire(
            self.bytes[CONFIG_SIDE_LIGHT_OFFSET],
        ))
    }

    /// Apply only settings whose wired-block fields are understood, preserving
    /// every other byte. Color palette changes use a separate block.
    pub fn apply_change(&mut self, change: AulaChange) -> Result<(), InvalidAulaWiredConfigChange> {
        if change.debounce().is_some() {
            return Err(InvalidAulaWiredConfigChange::Debounce);
        }
        if let Some(lighting) = change.lighting() {
            if lighting.color_mode().is_some() {
                return Err(InvalidAulaWiredConfigChange::ColorMode);
            }
            let effect = lighting
                .effect()
                .unwrap_or(self.bytes[CONFIG_EFFECT_OFFSET]);
            if !LightingChange::supports_effect(effect) {
                return Err(InvalidAulaWiredConfigChange::Effect);
            }
            if effect == 0 && (lighting.brightness().is_some() || lighting.speed().is_some()) {
                return Err(InvalidAulaWiredConfigChange::OffParameters);
            }
            if let Some(brightness) = lighting.brightness() {
                let capabilities = aula_effect_capabilities(effect);
                if !capabilities.is_some_and(AulaEffectCapabilities::has_brightness) {
                    return Err(InvalidAulaWiredConfigChange::Brightness);
                }
                self.bytes[CONFIG_EFFECT_SETTINGS_OFFSET + usize::from(effect) * 2] = brightness;
            }
            if let Some(speed) = lighting.speed() {
                let capabilities = aula_effect_capabilities(effect);
                if !capabilities.is_some_and(AulaEffectCapabilities::has_speed) {
                    return Err(InvalidAulaWiredConfigChange::Speed);
                }
                let offset = CONFIG_EFFECT_SETTINGS_OFFSET + usize::from(effect) * 2 + 1;
                self.bytes[offset] = (self.bytes[offset] & 0x0f) | (speed << 4);
            }
            if let Some(effect) = lighting.effect() {
                self.bytes[CONFIG_EFFECT_OFFSET] = effect;
            }
        }
        if let Some(timeout) = change.sleep_timeout() {
            self.bytes[CONFIG_SLEEP_OFFSET] = timeout.wire_value();
        }
        if let Some(effect) = change.side_light_effect() {
            self.bytes[CONFIG_SIDE_LIGHT_OFFSET] = effect.wire_value();
        }
        Ok(())
    }
}

/// Why a byte sequence is not a healthy F87 Pro wired configuration block.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Error)]
pub enum InvalidAulaWiredConfigBlock {
    /// The block is not exactly 128 bytes.
    #[error("wired AULA configuration block must be exactly 128 bytes")]
    Length,
    /// A documented healthy-block marker is absent or changed.
    #[error("wired AULA configuration block is missing a known healthy-block marker")]
    Markers,
}

/// Why an edit cannot safely be represented by the current wired config protocol.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Error)]
pub enum InvalidAulaWiredConfigChange {
    /// Debounce is not yet verified for the wired config block.
    #[error("wired AULA debounce edits are not supported")]
    Debounce,
    /// Custom color and colorful mode require the separate palette block.
    #[error("wired AULA color-mode edits require the palette block")]
    ColorMode,
    /// The current effect identifier has no supported parameter row.
    #[error("unsupported wired AULA effect")]
    Effect,
    /// Brightness is not supported by the selected effect.
    #[error("brightness is not supported by this wired AULA effect")]
    Brightness,
    /// Speed is not supported by the selected effect.
    #[error("speed is not supported by this wired AULA effect")]
    Speed,
    /// The off effect does not have brightness or speed settings.
    #[error("the wired AULA off effect does not accept brightness or speed")]
    OffParameters,
}
