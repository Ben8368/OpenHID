//! Capabilities copied from the installed F87 PRO `LedOpt` table.

/// Controls offered for one of the OEM's built-in F87 PRO effects.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AulaEffectCapabilities(u8);

const SPEED: u8 = 1 << 0;
const BRIGHTNESS: u8 = 1 << 1;
const DIRECTION: u8 = 1 << 2;
const RANDOM_COLOR: u8 = 1 << 3;
const CUSTOM_COLOR: u8 = 1 << 4;

const fn capabilities(bits: u8) -> AulaEffectCapabilities {
    AulaEffectCapabilities(bits)
}

impl AulaEffectCapabilities {
    /// Whether the effect accepts an animation-speed value.
    #[must_use]
    pub const fn has_speed(self) -> bool {
        self.0 & SPEED != 0
    }

    /// Whether the effect accepts a brightness value.
    #[must_use]
    pub const fn has_brightness(self) -> bool {
        self.0 & BRIGHTNESS != 0
    }

    /// Whether the OEM exposes a direction control for the effect.
    #[must_use]
    pub const fn has_direction(self) -> bool {
        self.0 & DIRECTION != 0
    }

    /// Whether the OEM exposes a random-color control for the effect.
    #[must_use]
    pub const fn has_random_color(self) -> bool {
        self.0 & RANDOM_COLOR != 0
    }

    /// Whether the OEM exposes a custom single-color control for the effect.
    #[must_use]
    pub const fn has_custom_color(self) -> bool {
        self.0 & CUSTOM_COLOR != 0
    }
}

// `LedOpt1..18` in `F87PRO/KB.ini`; columns are speed, light, direct,
// random, color after the hardware and effect IDs.
const EFFECT_CAPABILITIES: [AulaEffectCapabilities; 18] = [
    capabilities(BRIGHTNESS | RANDOM_COLOR | CUSTOM_COLOR),
    capabilities(SPEED | BRIGHTNESS | RANDOM_COLOR | CUSTOM_COLOR),
    capabilities(SPEED | BRIGHTNESS),
    capabilities(SPEED | BRIGHTNESS | RANDOM_COLOR | CUSTOM_COLOR),
    capabilities(SPEED | BRIGHTNESS | RANDOM_COLOR | CUSTOM_COLOR),
    capabilities(SPEED | BRIGHTNESS | RANDOM_COLOR | CUSTOM_COLOR),
    capabilities(SPEED | BRIGHTNESS | RANDOM_COLOR | CUSTOM_COLOR),
    capabilities(SPEED | BRIGHTNESS | RANDOM_COLOR | CUSTOM_COLOR),
    capabilities(SPEED | BRIGHTNESS | RANDOM_COLOR | CUSTOM_COLOR),
    capabilities(SPEED | BRIGHTNESS | RANDOM_COLOR | CUSTOM_COLOR),
    capabilities(SPEED | BRIGHTNESS | RANDOM_COLOR | CUSTOM_COLOR),
    capabilities(SPEED | BRIGHTNESS | RANDOM_COLOR | CUSTOM_COLOR),
    capabilities(SPEED | BRIGHTNESS | RANDOM_COLOR | CUSTOM_COLOR),
    capabilities(SPEED | BRIGHTNESS | RANDOM_COLOR | CUSTOM_COLOR),
    capabilities(SPEED | BRIGHTNESS),
    capabilities(SPEED | BRIGHTNESS),
    capabilities(SPEED | BRIGHTNESS),
    capabilities(SPEED | BRIGHTNESS | RANDOM_COLOR | CUSTOM_COLOR),
];

/// Return OEM parameter capabilities for a hardware effect ID (1–18).
#[must_use]
pub fn aula_effect_capabilities(effect: u8) -> Option<AulaEffectCapabilities> {
    EFFECT_CAPABILITIES
        .get(usize::from(effect.checked_sub(1)?))
        .copied()
}
