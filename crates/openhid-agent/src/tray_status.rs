//! Shared tray state, derived from the agent's authoritative inventory.
//! Native backends only render this state; they never poll hardware.

pub(crate) mod glyph;

use std::sync::{LazyLock, Mutex};

use openhid_core::battery::{
    ChargeReading, battery_needs_attention, current_charge_reading, glyph_fill, is_charging,
};
use openhid_core::config::{Config, canonical_device_key};
use openhid_core::device::{BatteryInfo, BatteryLevel, BatteryStatus, DeviceInventory};
use openhid_core::device_order::{DeviceIdentity, DeviceStableId, PhysicalDeviceKey};
use openhid_core::hid::DeviceRoute;
use openhid_ipc::Observation;
use tokio::sync::watch;

#[derive(Clone, Debug, PartialEq, Eq)]
struct DeviceBattery {
    key: String,
    physical_key: String,
    name: String,
    battery: BatteryInfo,
}

#[derive(Default)]
struct TrayStatus {
    visible: bool,
    show_battery: bool,
    devices: Vec<DeviceBattery>,
    config: Config,
    inventory: Vec<DeviceInventory>,
}

/// Content the native icon needs to draw.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum IconContent {
    Brand,
    Battery(BatteryInfo),
}

/// A complete native tray update, safe to carry between threads.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct Presentation {
    pub(crate) visible: bool,
    pub(crate) icon: IconContent,
    pub(crate) tooltip: String,
    pub(crate) devices: Vec<MenuDevice>,
}

/// Frozen menu entry: the action keeps the identity displayed when the menu opened.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct MenuDevice {
    pub(crate) key: String,
    pub(crate) label: String,
}

static STATUS: LazyLock<Mutex<TrayStatus>> = LazyLock::new(|| Mutex::new(TrayStatus::default()));

/// Apply persisted preferences without discarding inventory or a hidden mode.
pub(crate) fn configure(config: &Config) {
    {
        let mut status = STATUS
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        status.configure(config);
    }
    notify();
}

/// Subscribe once at bootstrap, including the current snapshot before waiting.
pub(crate) fn watch_inventory(mut observations: watch::Receiver<Observation>) {
    tokio::spawn(async move {
        loop {
            let changed = {
                let observation = observations.borrow_and_update();
                let mut status = STATUS
                    .lock()
                    .unwrap_or_else(std::sync::PoisonError::into_inner);
                status.update_inventory(&observation.snapshot.inventory)
            };
            if changed {
                notify();
            }
            if observations.changed().await.is_err() {
                break;
            }
        }
    });
}

/// Read the latest value at native dispatch time so queued updates cannot replay stale state.
pub(crate) fn current() -> Presentation {
    STATUS
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
        .presentation()
}

fn notify() {
    #[cfg(target_os = "windows")]
    crate::tray_windows::refresh();
    #[cfg(target_os = "macos")]
    crate::tray::refresh();
}

impl TrayStatus {
    fn configure(&mut self, config: &Config) {
        self.visible = config.app_settings.show_in_menu_bar;
        self.show_battery = config.app_settings.tray_show_battery;
        self.config = config.clone();
        let inventory = self.inventory.clone();
        self.update_inventory(&inventory);
    }

    fn update_inventory(&mut self, inventory: &[DeviceInventory]) -> bool {
        self.inventory = inventory.to_vec();
        let mut candidates = Vec::new();
        for receiver in inventory {
            for device in receiver.paired.iter().filter(|device| device.online) {
                let Some(battery) = &device.battery else {
                    continue;
                };
                let route = DeviceRoute::for_slot(receiver, device.slot);
                let model = device.model_info.as_ref();
                let serial = model.and_then(|model| model.serial_number.as_deref());
                let unit = model.map_or([0; 4], |model| model.unit_id);
                let identity = DeviceIdentity::from_parts(serial, unit);
                let stable = DeviceStableId::from_parts(route.as_ref(), device.slot, serial, unit);
                let resolved = self.config.resolve_device_key(&stable, Some(&identity));
                let key = canonical_device_key(&stable, Some(&identity))
                    .map_or_else(|| stable.runtime_key(), PhysicalDeviceKey::into_string);
                let physical_key = resolved
                    .as_ref()
                    .map_or_else(|| key.clone(), |resolved| resolved.as_str().to_owned());
                let name = resolved
                    .as_ref()
                    .and_then(|key| self.config.devices.get(key.as_str()))
                    .and_then(|device| device.custom_name.as_ref())
                    .filter(|name| !name.trim().is_empty())
                    .cloned()
                    .or_else(|| device.codename.clone())
                    .unwrap_or_else(|| receiver.receiver.name.clone());
                candidates.push((
                    stable,
                    DeviceBattery {
                        key,
                        physical_key,
                        name,
                        battery: battery.clone(),
                    },
                ));
            }
        }
        candidates.sort_by(|a, b| a.0.cmp(&b.0));
        let mut devices: Vec<DeviceBattery> = Vec::new();
        for (_, candidate) in candidates {
            if let Some(existing) = devices.iter_mut().find(|device| {
                device.key == candidate.key || device.physical_key == candidate.physical_key
            }) {
                // Prefer actual observations over cache replays. A live charging
                // route outranks a simultaneous discharging observation.
                let quality = |battery: &BatteryInfo| {
                    (
                        battery.freshness != openhid_core::device::BatteryFreshness::Cached,
                        is_charging(battery),
                        current_charge_reading(battery) != ChargeReading::NoReading,
                    )
                };
                if quality(&candidate.battery) > quality(&existing.battery) {
                    *existing = candidate;
                }
            } else {
                devices.push(candidate);
            }
        }
        // Attention outranks a lower value on external power. Coarse levels
        // choose visual bands only, never fabricated measured percentages.
        devices.sort_by(|a, b| {
            let priority = |device: &DeviceBattery| {
                (
                    !battery_needs_attention(&device.battery),
                    glyph_fill(current_charge_reading(&device.battery))
                        .map_or((true, 0), |fill| (false, fill)),
                )
            };
            priority(a)
                .cmp(&priority(b))
                .then_with(|| a.name.cmp(&b.name))
                .then_with(|| a.key.cmp(&b.key))
        });
        if self.devices == devices {
            return false;
        }
        self.devices = devices;
        true
    }

    fn presentation(&self) -> Presentation {
        // The same order owns both the displayed glyph and hover details, so
        // the device that needs charging is always the first tooltip entry.
        let lowest = self.devices.iter().find(|device| {
            device.battery.freshness != openhid_core::device::BatteryFreshness::Cached
        });
        let icon = if self.show_battery {
            lowest.map_or(IconContent::Brand, |device| {
                IconContent::Battery(device.battery.clone())
            })
        } else {
            IconContent::Brand
        };
        let mut tooltip = String::from("OpenHID");
        for device in &self.devices {
            tooltip.push('\n');
            tooltip.push_str(&device.name);
            tooltip.push_str(": ");
            tooltip.push_str(&battery_label(&device.battery));
        }
        Presentation {
            visible: self.visible,
            icon,
            tooltip,
            devices: self
                .devices
                .iter()
                .map(|device| MenuDevice {
                    key: device.key.clone(),
                    label: format!("{}: {}", device.name, battery_label(&device.battery)),
                })
                .collect(),
        }
    }
}

fn battery_label(battery: &BatteryInfo) -> String {
    let reading = match current_charge_reading(battery) {
        ChargeReading::Percent(value) => format!("{value}%"),
        ChargeReading::Level(BatteryLevel::Full) => rust_i18n::t!("device.full").to_string(),
        ChargeReading::Level(BatteryLevel::Good) => {
            rust_i18n::t!("device.battery_good").to_string()
        }
        ChargeReading::Level(BatteryLevel::Low | BatteryLevel::Critical) => {
            rust_i18n::t!("device.low_battery").to_string()
        }
        ChargeReading::Level(BatteryLevel::Unknown) | ChargeReading::NoReading => "—".to_owned(),
    };
    let status = if battery.freshness == openhid_core::device::BatteryFreshness::Cached {
        None
    } else if is_charging(battery) {
        Some(rust_i18n::t!("device.charging"))
    } else if battery.status == BatteryStatus::Error {
        Some(rust_i18n::t!("device.battery_error"))
    } else if battery_needs_attention(battery)
        && matches!(current_charge_reading(battery), ChargeReading::Percent(_))
    {
        Some(rust_i18n::t!("device.low_battery"))
    } else {
        None
    };
    status.map_or_else(|| reading.clone(), |status| format!("{reading} · {status}"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use openhid_core::device::{DeviceKind, PairedDevice, ReceiverInfo};

    fn battery_icon(
        percentage: Option<u8>,
        level: BatteryLevel,
        status: BatteryStatus,
    ) -> IconContent {
        IconContent::Battery(BatteryInfo {
            freshness: openhid_core::device::BatteryFreshness::Current,
            percentage,
            level,
            status,
        })
    }

    pub(super) fn inventory(
        readings: &[(bool, Option<u8>, BatteryStatus)],
    ) -> Vec<DeviceInventory> {
        vec![DeviceInventory {
            receiver: ReceiverInfo {
                name: "Receiver".into(),
                vendor_id: 0x046d,
                product_id: 0xc548,
                unique_id: None,
            },
            paired: readings
                .iter()
                .enumerate()
                .map(|(index, &(online, percentage, status))| PairedDevice {
                    slot: u8::try_from(index + 1).unwrap(),
                    codename: Some(format!("Device {index}")),
                    wpid: None,
                    kind: DeviceKind::Mouse,
                    online,
                    battery: Some(BatteryInfo {
                        freshness: openhid_core::device::BatteryFreshness::Current,
                        percentage,
                        level: BatteryLevel::Unknown,
                        status,
                    }),
                    model_info: None,
                    capabilities: None,
                    settings_revision: 0,
                })
                .collect(),
        }]
    }

    #[test]
    fn tray_switches_preserve_mode_and_restore_brand() {
        let mut state = TrayStatus {
            visible: true,
            ..TrayStatus::default()
        };
        state.update_inventory(&inventory(&[(true, Some(85), BatteryStatus::Discharging)]));
        assert_eq!(state.presentation().icon, IconContent::Brand);
        state.show_battery = true;
        assert_eq!(
            state.presentation().icon,
            battery_icon(Some(85), BatteryLevel::Unknown, BatteryStatus::Discharging)
        );
        state.visible = false;
        assert!(!state.presentation().visible);
        assert!(state.show_battery);
        state.visible = true;
        assert_eq!(
            state.presentation().icon,
            battery_icon(Some(85), BatteryLevel::Unknown, BatteryStatus::Discharging)
        );
        state.show_battery = false;
        assert_eq!(state.presentation().icon, IconContent::Brand);
    }

    #[test]
    fn tray_tracks_lowest_online_battery_and_discards_disconnected_readings() {
        let mut state = TrayStatus {
            visible: true,
            show_battery: true,
            ..TrayStatus::default()
        };
        let mut devices = inventory(&[
            (true, Some(85), BatteryStatus::Discharging),
            (true, Some(20), BatteryStatus::Charging),
            (false, Some(1), BatteryStatus::Discharging),
        ]);
        assert!(state.update_inventory(&devices));
        assert_eq!(
            state.presentation().icon,
            battery_icon(Some(20), BatteryLevel::Unknown, BatteryStatus::Charging)
        );
        assert!(state.presentation().tooltip.contains("20%"));
        assert!(!state.presentation().tooltip.contains("Device 2"));
        assert!(!state.update_inventory(&devices));
        devices[0].paired[1].online = false;
        assert!(state.update_inventory(&devices));
        assert_eq!(
            state.presentation().icon,
            battery_icon(Some(85), BatteryLevel::Unknown, BatteryStatus::Discharging)
        );
        state.update_inventory(&[]);
        assert_eq!(state.presentation().icon, IconContent::Brand);
    }

    #[test]
    fn lowest_battery_leads_the_tooltip_including_level_only_devices() {
        let mut state = TrayStatus {
            visible: true,
            show_battery: true,
            ..TrayStatus::default()
        };
        let mut devices = inventory(&[
            (true, Some(85), BatteryStatus::Discharging),
            (true, Some(40), BatteryStatus::Discharging),
            (true, None, BatteryStatus::Discharging),
            (true, Some(0), BatteryStatus::Charging),
        ]);
        devices[0].paired[2].battery.as_mut().unwrap().level = BatteryLevel::Critical;
        state.update_inventory(&devices);
        let presentation = state.presentation();
        assert_eq!(
            presentation.icon,
            battery_icon(None, BatteryLevel::Critical, BatteryStatus::Discharging)
        );
        let names: Vec<_> = presentation
            .tooltip
            .lines()
            .skip(1)
            .map(|line| line.split(':').next().unwrap())
            .collect();
        assert_eq!(names, ["Device 2", "Device 1", "Device 0", "Device 3"]);
        assert!(!presentation.tooltip.contains("10%"));
        assert!(!presentation.tooltip.contains(": 0%"));
        devices[0].paired[1].battery.as_mut().unwrap().percentage = Some(5);
        state.update_inventory(&devices);
        assert_eq!(
            state.presentation().icon,
            battery_icon(Some(5), BatteryLevel::Unknown, BatteryStatus::Discharging)
        );
        assert!(
            state
                .presentation()
                .tooltip
                .lines()
                .nth(1)
                .unwrap()
                .starts_with("Device 1: 5%")
        );
        devices[0].paired.reverse();
        assert!(!state.update_inventory(&devices));
    }

    #[test]
    fn tray_does_not_fabricate_percentage_for_cold_charging_or_coarse_levels() {
        let mut state = TrayStatus {
            visible: true,
            show_battery: true,
            ..TrayStatus::default()
        };
        for reading in [None, Some(0)] {
            state.update_inventory(&inventory(&[(true, reading, BatteryStatus::Charging)]));
            assert_eq!(
                state.presentation().icon,
                battery_icon(reading, BatteryLevel::Unknown, BatteryStatus::Charging)
            );
            assert!(!state.presentation().tooltip.contains(": 0%"));
        }
        let mut coarse = inventory(&[(true, None, BatteryStatus::Discharging)]);
        coarse[0].paired[0].battery.as_mut().unwrap().level = BatteryLevel::Low;
        state.update_inventory(&coarse);
        assert_eq!(
            state.presentation().icon,
            battery_icon(None, BatteryLevel::Low, BatteryStatus::Discharging)
        );
        assert!(!state.presentation().tooltip.contains('%'));
        state.update_inventory(&inventory(&[(true, Some(0), BatteryStatus::Discharging)]));
        assert_eq!(
            state.presentation().icon,
            battery_icon(Some(0), BatteryLevel::Unknown, BatteryStatus::Discharging)
        );
    }
}

#[cfg(test)]
mod projection_regressions {
    use super::tests::inventory;
    use super::*;
    use openhid_core::device::{DeviceModelInfo, DeviceTransports};

    #[test]
    fn one_physical_device_has_one_tray_entry_across_connections() {
        let mut input = inventory(&[(true, Some(30), BatteryStatus::Discharging)]);
        input[0].paired[0].model_info = Some(DeviceModelInfo {
            entity_count: 1,
            unit_id: [1, 2, 3, 4],
            transports: DeviceTransports::default(),
            model_ids: [0xb369, 0, 0],
            extended_model_id: 0,
            serial_number: None,
        });
        let mut cable = input[0].clone();
        cable.receiver.product_id = 0xc343;
        cable.paired[0].slot = 0xff;
        input.push(cable);
        let mut state = TrayStatus::default();
        state.update_inventory(&input);
        assert_eq!(state.presentation().tooltip.lines().count(), 2);
    }

    #[test]
    fn a_charging_device_does_not_hide_a_low_discharging_device() {
        let mut state = TrayStatus {
            show_battery: true,
            ..TrayStatus::default()
        };
        state.update_inventory(&inventory(&[
            (true, Some(5), BatteryStatus::Charging),
            (true, Some(15), BatteryStatus::Discharging),
        ]));
        assert!(matches!(
            state.presentation().icon,
            IconContent::Battery(BatteryInfo {
                percentage: Some(15),
                ..
            })
        ));
    }
    fn identified_inventory() -> Vec<DeviceInventory> {
        let mut input = inventory(&[(true, Some(30), BatteryStatus::Discharging)]);
        input[0].receiver.unique_id = Some("receiver".into());
        input[0].paired[0].model_info = Some(DeviceModelInfo {
            entity_count: 1,
            unit_id: [1, 2, 3, 4],
            transports: DeviceTransports::default(),
            model_ids: [0xb369, 0, 0],
            extended_model_id: 0,
            serial_number: None,
        });
        input
    }

    #[test]
    fn renames_and_legacy_settings_refresh_without_an_inventory_change() {
        let mut input = identified_inventory();
        let mut config = Config::default();
        let route_key = "receiver:receiver:slot:1";
        config
            .devices
            .entry(route_key.into())
            .or_default()
            .custom_name = Some("Work & mouse".into());
        let mut state = TrayStatus::default();
        state.configure(&config);
        state.update_inventory(&input);
        let first = state.presentation();
        assert_eq!(first.devices[0].key, "unit:01020304");
        assert!(first.devices[0].label.starts_with("Work & mouse: "));
        let physical =
            openhid_core::device_order::PhysicalDeviceKey::parse("unit:01020304").unwrap();
        assert!(config.adopt_route(&physical, route_key, None));
        config
            .devices
            .get_mut(physical.as_str())
            .unwrap()
            .custom_name = Some("Renamed mouse".into());
        state.configure(&config);
        assert!(state.presentation().tooltip.contains("Renamed mouse"));
        assert_eq!(state.presentation().devices[0].key, first.devices[0].key);
        input[0].paired[0].model_info.as_mut().unwrap().unit_id = [5, 6, 7, 8];
        state.update_inventory(&input);
        assert_eq!(state.presentation().devices[0].key, "unit:05060708");
        assert!(!state.presentation().tooltip.contains("Renamed mouse"));
    }

    #[test]
    fn live_route_outranks_cache_and_menu_keeps_its_displayed_device_identity() {
        use openhid_core::device::BatteryFreshness;
        let mut input = identified_inventory();
        input[0].paired[0].battery.as_mut().unwrap().freshness = BatteryFreshness::Cached;
        let mut state = TrayStatus {
            show_battery: true,
            ..TrayStatus::default()
        };
        state.update_inventory(&input);
        assert_eq!(state.presentation().icon, IconContent::Brand);
        assert!(!state.presentation().tooltip.contains("30%"));
        let mut cable = input[0].clone();
        cable.receiver.product_id = 0xc343;
        cable.paired[0].slot = 0xff;
        let reading = cable.paired[0].battery.as_mut().unwrap();
        reading.freshness = BatteryFreshness::Current;
        reading.percentage = Some(80);
        input.push(cable);
        state.update_inventory(&input);
        let menu = state.presentation().devices;
        assert_eq!(menu.len(), 1);
        assert!(menu[0].label.contains("80%"));
        input[0].paired[0].model_info.as_mut().unwrap().unit_id = [5, 6, 7, 8];
        input.pop();
        state.update_inventory(&input);
        assert_eq!(menu[0].key, "unit:01020304");
        assert_eq!(state.presentation().devices[0].key, "unit:05060708");
    }

    #[test]
    fn anonymous_devices_in_distinct_slots_are_not_folded_together() {
        let mut state = TrayStatus::default();
        state.update_inventory(&inventory(&[
            (true, Some(50), BatteryStatus::Discharging),
            (true, Some(50), BatteryStatus::Discharging),
        ]));
        assert_eq!(state.presentation().devices.len(), 2);
        assert_ne!(
            state.presentation().devices[0].key,
            state.presentation().devices[1].key
        );
    }
}
