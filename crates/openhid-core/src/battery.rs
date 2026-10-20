//! Shared interpretation of battery readings for desktop and tray presentation.

use crate::device::{BatteryFreshness, BatteryInfo, BatteryLevel, BatteryStatus};

/// What a battery reading can show.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ChargeReading {
    /// A measured state of charge.
    Percent(u8),
    /// Only the firmware's coarse level: the device has no state-of-charge
    /// gauge, so there is no percentage to show.
    Level(BatteryLevel),
    /// Neither a percentage nor a level yet.
    NoReading,
}

/// Interpret a firmware reading without inventing a percentage.
#[must_use]
pub fn charge_reading(battery: &BatteryInfo) -> ChargeReading {
    match battery.percentage {
        // Some devices cannot gauge charge while under load. On a cold start
        // they report 0% until a later poll, which must not be presented as
        // an empty or critically depleted battery.
        Some(0) if is_charging(battery) => ChargeReading::NoReading,
        Some(percentage @ 0..=100) => ChargeReading::Percent(percentage),
        Some(_) => ChargeReading::NoReading,
        None if battery.level == BatteryLevel::Unknown => ChargeReading::NoReading,
        None => ChargeReading::Level(battery.level),
    }
}

/// Interpret only a measured, current charge. Cache replays and percentages
/// held during charging remain available to last-known displays, not live glyphs.
#[must_use]
pub fn current_charge_reading(battery: &BatteryInfo) -> ChargeReading {
    if battery.freshness == BatteryFreshness::Current {
        charge_reading(battery)
    } else {
        ChargeReading::NoReading
    }
}

/// True when a charging device has supplied neither a usable percentage nor
/// a level yet, so "Charging" is all there is to say.
#[must_use]
pub fn battery_charging_no_reading(battery: &BatteryInfo) -> bool {
    is_charging(battery) && charge_reading(battery) == ChargeReading::NoReading
}

/// Whether a discharging battery should draw the user's attention: 20% or
/// less, or a low or critical level from a device without a percentage.
#[must_use]
pub fn battery_needs_attention(battery: &BatteryInfo) -> bool {
    let low = match current_charge_reading(battery) {
        ChargeReading::Percent(percentage) => percentage <= 20,
        ChargeReading::Level(level) => matches!(level, BatteryLevel::Low | BatteryLevel::Critical),
        ChargeReading::NoReading => false,
    };
    low && !matches!(
        battery.status,
        BatteryStatus::Charging | BatteryStatus::ChargingSlow | BatteryStatus::Full
    )
}

/// Whether the firmware reports active charging.
#[must_use]
pub const fn is_charging(battery: &BatteryInfo) -> bool {
    !matches!(battery.freshness, BatteryFreshness::Cached)
        && matches!(
            battery.status,
            BatteryStatus::Charging | BatteryStatus::ChargingSlow
        )
}

/// Fill for a battery glyph. Coarse levels select a visual band only; these
/// values must never be displayed as measured percentages.
#[must_use]
pub const fn glyph_fill(reading: ChargeReading) -> Option<u8> {
    match reading {
        ChargeReading::Percent(percentage) => Some(percentage),
        ChargeReading::Level(BatteryLevel::Critical) => Some(10),
        ChargeReading::Level(BatteryLevel::Low) => Some(25),
        ChargeReading::Level(BatteryLevel::Good) => Some(70),
        ChargeReading::Level(BatteryLevel::Full) => Some(100),
        ChargeReading::Level(BatteryLevel::Unknown) | ChargeReading::NoReading => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_low_discharging_battery_needs_attention() {
        let battery = |percentage, level, status| BatteryInfo {
            freshness: crate::device::BatteryFreshness::Current,
            percentage,
            level,
            status,
        };
        let good = BatteryLevel::Good;
        let discharging = BatteryStatus::Discharging;

        assert!(battery_needs_attention(&battery(
            Some(20),
            good,
            discharging
        )));
        assert!(battery_needs_attention(&battery(
            Some(0),
            good,
            discharging
        )));
        assert!(!battery_needs_attention(&battery(
            Some(21),
            BatteryLevel::Low,
            discharging
        )));
        // A percentage decides on its own; the level only stands in for one.
        assert!(!battery_needs_attention(&battery(None, good, discharging)));
        assert!(battery_needs_attention(&battery(
            None,
            BatteryLevel::Low,
            discharging
        )));
        assert!(battery_needs_attention(&battery(
            None,
            BatteryLevel::Critical,
            discharging
        )));
        assert!(!battery_needs_attention(&battery(
            None,
            BatteryLevel::Unknown,
            discharging
        )));
        assert!(battery_needs_attention(&battery(
            Some(10),
            good,
            BatteryStatus::Unknown
        )));
        assert!(battery_needs_attention(&battery(
            Some(10),
            good,
            BatteryStatus::Error
        )));
        for powered in [
            BatteryStatus::Charging,
            BatteryStatus::ChargingSlow,
            BatteryStatus::Full,
        ] {
            assert!(
                !battery_needs_attention(&battery(Some(5), BatteryLevel::Critical, powered)),
                "{powered:?} is already being charged"
            );
            assert!(!battery_needs_attention(&battery(
                None,
                BatteryLevel::Critical,
                powered
            )));
        }
    }

    #[test]
    fn charging_zero_is_unknown_but_discharging_zero_is_empty() {
        for status in [BatteryStatus::Charging, BatteryStatus::ChargingSlow] {
            let reading = BatteryInfo {
                freshness: crate::device::BatteryFreshness::Current,
                percentage: Some(0),
                level: BatteryLevel::Critical,
                status,
            };
            assert_eq!(charge_reading(&reading), ChargeReading::NoReading);
            assert!(battery_charging_no_reading(&reading));
            assert!(!battery_needs_attention(&reading));
        }
        let empty = BatteryInfo {
            freshness: crate::device::BatteryFreshness::Current,
            percentage: Some(0),
            level: BatteryLevel::Unknown,
            status: BatteryStatus::Discharging,
        };
        assert_eq!(charge_reading(&empty), ChargeReading::Percent(0));
        assert!(battery_needs_attention(&empty));
    }

    #[test]
    fn coarse_readings_keep_their_level_and_never_invent_a_percentage() {
        for level in [
            BatteryLevel::Critical,
            BatteryLevel::Low,
            BatteryLevel::Good,
            BatteryLevel::Full,
            BatteryLevel::Unknown,
        ] {
            let reading = BatteryInfo {
                freshness: crate::device::BatteryFreshness::Current,
                percentage: None,
                level,
                status: BatteryStatus::Discharging,
            };
            assert_eq!(
                charge_reading(&reading),
                if level == BatteryLevel::Unknown {
                    ChargeReading::NoReading
                } else {
                    ChargeReading::Level(level)
                }
            );
            assert_eq!(
                battery_needs_attention(&reading),
                matches!(level, BatteryLevel::Critical | BatteryLevel::Low)
            );
        }
    }
    #[test]
    fn a_level_only_glyph_fills_inside_its_level_band() {
        assert_eq!(glyph_fill(ChargeReading::Percent(42)), Some(42));
        assert_eq!(
            glyph_fill(ChargeReading::Level(BatteryLevel::Critical)),
            Some(10)
        );
        assert_eq!(
            glyph_fill(ChargeReading::Level(BatteryLevel::Low)),
            Some(25)
        );
        assert_eq!(
            glyph_fill(ChargeReading::Level(BatteryLevel::Good)),
            Some(70)
        );
        assert_eq!(
            glyph_fill(ChargeReading::Level(BatteryLevel::Full)),
            Some(100)
        );
        assert_eq!(glyph_fill(ChargeReading::NoReading), None);
    }
    #[test]
    fn cached_and_held_values_are_last_known_not_current_charge() {
        let mut battery = BatteryInfo {
            percentage: Some(8),
            level: BatteryLevel::Critical,
            status: BatteryStatus::Discharging,
            freshness: BatteryFreshness::Current,
        };
        assert!(battery_needs_attention(&battery));
        battery.freshness = BatteryFreshness::Cached;
        assert_eq!(charge_reading(&battery), ChargeReading::Percent(8));
        assert_eq!(current_charge_reading(&battery), ChargeReading::NoReading);
        assert!(!battery_needs_attention(&battery));
        battery.status = BatteryStatus::Charging;
        assert!(!is_charging(&battery));
        battery.freshness = BatteryFreshness::HeldPercentage;
        assert!(is_charging(&battery));
        assert_eq!(current_charge_reading(&battery), ChargeReading::NoReading);
        assert!(!battery_needs_attention(&battery));
    }

    #[test]
    fn invalid_percentages_never_become_current_charge_or_attention() {
        let battery = BatteryInfo {
            percentage: Some(255),
            level: BatteryLevel::Critical,
            status: BatteryStatus::Discharging,
            freshness: BatteryFreshness::Current,
        };
        assert_eq!(current_charge_reading(&battery), ChargeReading::NoReading);
        assert!(!battery_needs_attention(&battery));
    }
}
