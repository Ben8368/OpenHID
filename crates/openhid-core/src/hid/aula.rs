//! Shared, host-free F87 PRO lighting commands and readback state.

use serde::{Deserialize, Serialize};
use thiserror::Error;

/// Current lighting fields; unknown/custom effect identifiers are retained.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct LightingState {
    /// Native effect identifier.
    pub effect: u8,
    /// Brightness when the current effect has a known parameter table.
    pub brightness: Option<u8>,
    /// Speed when the current effect has a known parameter table.
    pub speed: Option<u8>,
}

/// Current verified settings exposed by the F87 PRO wireless control collection.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[non_exhaustive]
pub struct AulaState {
    /// Built-in lighting controls.
    pub lighting: LightingState,
    /// Wireless idle timeout, when its device value is recognized.
    pub sleep_timeout: Option<SleepTimeout>,
}

impl AulaState {
    /// Construct a readback state from independently decoded settings.
    #[must_use]
    pub const fn new(lighting: LightingState, sleep_timeout: Option<SleepTimeout>) -> Self {
        Self {
            lighting,
            sleep_timeout,
        }
    }
}

/// A supported wireless idle timeout, represented in seconds at API boundaries.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(try_from = "u16", into = "u16")]
pub struct SleepTimeout(u8);

const SLEEP_TIMEOUTS: [SleepTimeout; 9] = [
    SleepTimeout(2),
    SleepTimeout(3),
    SleepTimeout(4),
    SleepTimeout(6),
    SleepTimeout(8),
    SleepTimeout(10),
    SleepTimeout(20),
    SleepTimeout(30),
    SleepTimeout(40),
];

impl SleepTimeout {
    /// All supported idle timeouts in ascending order.
    #[must_use]
    pub const fn options() -> &'static [Self; 9] {
        &SLEEP_TIMEOUTS
    }

    /// Construct a timeout from the user-facing duration in seconds.
    #[must_use]
    pub fn from_seconds(seconds: u16) -> Option<Self> {
        Self::options()
            .iter()
            .copied()
            .find(|timeout| timeout.seconds() == seconds)
    }

    /// Decode a device value, leaving unknown values untouched in the snapshot.
    #[must_use]
    pub fn from_wire(value: u8) -> Option<Self> {
        Self::options()
            .iter()
            .copied()
            .find(|timeout| timeout.0 == value)
    }

    /// User-facing duration in seconds.
    #[must_use]
    pub const fn seconds(self) -> u16 {
        (self.0 as u16) * 30
    }

    /// Encoded value used by the verified F87 PRO configuration block.
    #[must_use]
    pub const fn wire_value(self) -> u8 {
        self.0
    }
}

impl TryFrom<u16> for SleepTimeout {
    type Error = InvalidSleepTimeout;

    fn try_from(seconds: u16) -> Result<Self, Self::Error> {
        Self::from_seconds(seconds).ok_or(InvalidSleepTimeout)
    }
}

impl From<SleepTimeout> for u16 {
    fn from(timeout: SleepTimeout) -> Self {
        timeout.seconds()
    }
}

/// A timeout value outside the keyboard's supported wireless options.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Error)]
#[error("unsupported AULA sleep timeout; choose one of the advertised values")]
pub struct InvalidSleepTimeout;

/// A verified setting edit. Unspecified fields and unknown bytes are preserved.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(try_from = "UncheckedAulaChange", into = "UncheckedAulaChange")]
pub struct AulaChange {
    lighting: Option<LightingChange>,
    sleep_timeout: Option<SleepTimeout>,
}

#[derive(Serialize, Deserialize)]
struct UncheckedAulaChange {
    lighting: Option<LightingChange>,
    sleep_timeout: Option<SleepTimeout>,
}

impl AulaChange {
    /// Construct a non-empty edit to one or more independently supported fields.
    pub fn new(
        lighting: Option<LightingChange>,
        sleep_timeout: Option<SleepTimeout>,
    ) -> Result<Self, InvalidAulaChange> {
        if lighting.is_none() && sleep_timeout.is_none() {
            return Err(InvalidAulaChange);
        }
        Ok(Self {
            lighting,
            sleep_timeout,
        })
    }

    /// Lighting fields to change, if selected.
    #[must_use]
    pub const fn lighting(self) -> Option<LightingChange> {
        self.lighting
    }

    /// Sleep timeout to change, if selected.
    #[must_use]
    pub const fn sleep_timeout(self) -> Option<SleepTimeout> {
        self.sleep_timeout
    }
}

impl TryFrom<UncheckedAulaChange> for AulaChange {
    type Error = InvalidAulaChange;

    fn try_from(value: UncheckedAulaChange) -> Result<Self, Self::Error> {
        Self::new(value.lighting, value.sleep_timeout)
    }
}

impl From<AulaChange> for UncheckedAulaChange {
    fn from(value: AulaChange) -> Self {
        Self {
            lighting: value.lighting,
            sleep_timeout: value.sleep_timeout,
        }
    }
}

/// A rejected empty AULA settings edit.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Error)]
#[error("an AULA settings edit must change at least one field")]
pub struct InvalidAulaChange;

/// Resolve only the verified wireless control collection's opaque identity.
/// Other raw collections and HID++ routes must never receive AULA commands.
#[must_use]
pub fn route_identity(route: &super::DeviceRoute) -> Option<&str> {
    match route {
        super::DeviceRoute::RawHid {
            vendor_id,
            product_id,
            usage_page,
            usage_id,
            identity,
        } if openhid_device_registry::aula::matches_control(
            *vendor_id,
            *product_id,
            *usage_page,
            *usage_id,
        ) =>
        {
            Some(identity)
        }
        _ => None,
    }
}

/// A rejected edit, including empty edits and parameters on the off effect.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Error)]
#[error("unsupported AULA lighting edit (effects 0..18, levels 0..4)")]
pub struct InvalidLightingChange;

/// Validated edit; unspecified fields and unknown configuration bytes survive.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(try_from = "UncheckedChange", into = "UncheckedChange")]
pub struct LightingChange {
    effect: Option<u8>,
    brightness: Option<u8>,
    speed: Option<u8>,
}

#[derive(Serialize, Deserialize)]
struct UncheckedChange {
    effect: Option<u8>,
    brightness: Option<u8>,
    speed: Option<u8>,
}

impl TryFrom<UncheckedChange> for LightingChange {
    type Error = InvalidLightingChange;
    fn try_from(value: UncheckedChange) -> Result<Self, Self::Error> {
        Self::new(value.effect, value.brightness, value.speed)
    }
}

impl From<LightingChange> for UncheckedChange {
    fn from(value: LightingChange) -> Self {
        Self {
            effect: value.effect,
            brightness: value.brightness,
            speed: value.speed,
        }
    }
}

impl LightingChange {
    /// Construct an edit. Effect zero is off and has no parameter table.
    pub fn new(
        effect: Option<u8>,
        brightness: Option<u8>,
        speed: Option<u8>,
    ) -> Result<Self, InvalidLightingChange> {
        if effect.is_some_and(|value| !Self::supports_effect(value))
            || brightness.is_some_and(|value| !Self::levels().contains(&value))
            || speed.is_some_and(|value| !Self::levels().contains(&value))
            || (effect == Some(0) && (brightness.is_some() || speed.is_some()))
            || (effect.is_none() && brightness.is_none() && speed.is_none())
        {
            return Err(InvalidLightingChange);
        }
        Ok(Self {
            effect,
            brightness,
            speed,
        })
    }

    /// OEM built-in effects, including zero for off.
    #[must_use]
    pub fn effects() -> std::ops::RangeInclusive<u8> {
        0..=18
    }

    /// Brightness and speed levels offered by built-in effects.
    #[must_use]
    pub fn levels() -> std::ops::RangeInclusive<u8> {
        0..=4
    }

    /// Whether an identifier has supported built-in semantics.
    #[must_use]
    pub fn supports_effect(effect: u8) -> bool {
        Self::effects().contains(&effect)
    }

    /// Requested effect, if changed.
    #[must_use]
    pub fn effect(self) -> Option<u8> {
        self.effect
    }

    /// Requested brightness, if changed.
    #[must_use]
    pub fn brightness(self) -> Option<u8> {
        self.brightness
    }

    /// Requested speed, if changed.
    #[must_use]
    pub fn speed(self) -> Option<u8> {
        self.speed
    }
}

/// Explicit persistence policy for one recoverable lighting transaction.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum LightingMode {
    /// Stage and verify the edit without issuing SAVE.
    Apply,
    /// Verify, SAVE, and read back; includes an already-staged snapshot.
    Save,
    /// Stage/verify and restore/verify the full snapshot without SAVE.
    Preview,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sleep_timeout_and_settings_validate_at_serialization_boundaries() {
        let expected = [
            (60, 2),
            (90, 3),
            (120, 4),
            (180, 6),
            (240, 8),
            (300, 10),
            (600, 20),
            (900, 30),
            (1200, 40),
        ];
        for (seconds, wire) in expected {
            let timeout = SleepTimeout::from_seconds(seconds).unwrap();
            assert_eq!(timeout.wire_value(), wire);
            assert_eq!(SleepTimeout::from_wire(wire), Some(timeout));
            let change = AulaChange::new(None, Some(timeout)).unwrap();
            let encoded = toml::to_string(&change).unwrap();
            assert_eq!(toml::from_str::<AulaChange>(&encoded).unwrap(), change);
        }
        for seconds in [0, 1, 61, 301, 1201, u16::MAX] {
            assert!(SleepTimeout::from_seconds(seconds).is_none());
            toml::from_str::<AulaChange>(&format!("sleep_timeout = {seconds}")).unwrap_err();
        }
        AulaChange::new(None, None).unwrap_err();
        toml::from_str::<AulaChange>("").unwrap_err();
    }

    #[test]
    fn invalid_edits_cannot_be_constructed_or_deserialized() {
        for (effect, brightness, speed, encoded) in [
            (None, None, None, ""),
            (Some(19), None, None, "effect = 19"),
            (None, Some(5), None, "brightness = 5"),
            (None, None, Some(255), "speed = 255"),
            (Some(0), Some(1), None, "effect = 0\nbrightness = 1"),
            (Some(0), None, Some(1), "effect = 0\nspeed = 1"),
        ] {
            LightingChange::new(effect, brightness, speed).unwrap_err();
            assert!(
                toml::from_str::<LightingChange>(encoded).is_err(),
                "{encoded}"
            );
        }
    }

    #[test]
    fn validated_edits_keep_optional_fields_and_roundtrip() {
        for change in [
            LightingChange::new(Some(0), None, None).unwrap(),
            LightingChange::new(Some(18), Some(4), Some(0)).unwrap(),
            LightingChange::new(None, None, Some(4)).unwrap(),
        ] {
            let encoded = toml::to_string(&change).unwrap();
            assert_eq!(toml::from_str::<LightingChange>(&encoded).unwrap(), change);
        }
    }
}
