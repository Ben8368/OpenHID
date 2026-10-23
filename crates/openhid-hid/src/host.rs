//! The route-addressed device API, wired to this host's HID stack.
//!
//! Resolving a [`DeviceRoute`] means enumerating and opening, so every function
//! here needs a backend. The layer that implements them takes one explicitly —
//! that is what lets it be driven by a scripted device tree, or by another
//! host's HID stack. These are the same functions with *this* host's backend
//! supplied, for the overwhelmingly common caller who means "this machine".
//!
//! Channel-addressed operations (the `_on` family) need no backend and are not
//! wrapped: they act on a channel the caller already holds.

use std::future::Future;
use std::sync::Arc;

use openhid_core::device::{DeviceInventory, LightState, StandaloneDevice};
use openhid_core::hid::{FnLockState, LightCommand, PairingError, WriteError};

use crate::probe_cache::FileProbeCacheStore;
use crate::transport::native_backend;
use openhid_core::hid::smartshift::{
    SmartShiftAutoDisengage, SmartShiftMode, SmartShiftStatus, TunableTorque,
};
use openhid_device::ChannelPool;
use openhid_device::LightWatch;
use openhid_device::backend::{HidBackend, HotplugStream};
use openhid_device::backlight::{BacklightChange, BacklightState};
use openhid_device::inventory::{Enumerator, InventoryError};
use openhid_device::pairing::PairingReceiver;
use openhid_device::write::{
    self as device, Dpi, DpiInfo, FeatureEntry, FirmwareEntity, HapticWaveform, LightingMethod,
    LitraModel, PointerScaling, ReprogControlEntry, ScrollResolution, ScrollWheelMode,
};
use openhid_device::{DeviceIoGate, DeviceIoSignal, DeviceRoute};

/// This host's HID stack.
///
/// Public because the entry points with a wide parameter list — a capture
/// session, a pairing flow — are clearer passed a backend than wrapped again;
/// an agent that drives them names its backend once and hands it down.
#[must_use]
pub fn backend() -> Arc<dyn HidBackend> {
    native_backend()
}

/// Control the process-wide native HID activity gate from the host lifecycle
/// observer.
#[must_use]
pub fn device_io_signal() -> DeviceIoSignal {
    crate::transport::device_io_signal()
}

/// Subscribe to the process-wide native HID activity gate.
#[must_use]
pub fn device_io_gate() -> DeviceIoGate {
    crate::transport::device_io_gate()
}

/// Read the sensor DPI of the device `route` reaches.
pub async fn get_dpi(route: &DeviceRoute) -> Result<Dpi, WriteError> {
    device::get_dpi(&*native_backend(), route).await
}

/// Read the DPI range and capabilities of the device `route` reaches.
pub async fn get_dpi_info(route: &DeviceRoute) -> Result<DpiInfo, WriteError> {
    device::get_dpi_info(&*native_backend(), route).await
}

/// Write a new sensor DPI to the device `route` reaches.
pub async fn set_dpi(route: &DeviceRoute, dpi: Dpi) -> Result<(), WriteError> {
    device::set_dpi(&*native_backend(), route, dpi).await
}

/// Read the pointer-motion scaling of the device `route` reaches.
pub async fn get_pointer_scaling(route: &DeviceRoute) -> Result<PointerScaling, WriteError> {
    device::get_pointer_scaling(&*native_backend(), route).await
}

/// Write a pointer-motion scaling to the device `route` reaches and return the
/// value it applied.
pub async fn set_pointer_scaling(
    route: &DeviceRoute,
    scaling: PointerScaling,
) -> Result<PointerScaling, WriteError> {
    device::set_pointer_scaling(&*native_backend(), route, scaling).await
}

/// Read the SmartShift mode, threshold and torque of the device `route` reaches.
pub async fn get_smartshift_status(route: &DeviceRoute) -> Result<SmartShiftStatus, WriteError> {
    device::get_smartshift_status(&*native_backend(), route).await
}

/// Write a full SmartShift status to the device `route` reaches.
pub async fn set_smartshift(
    route: &DeviceRoute,
    status: SmartShiftStatus,
) -> Result<(), WriteError> {
    device::set_smartshift(&*native_backend(), route, status).await
}

/// Flip the device `route` reaches between free-spin and ratchet.
pub async fn toggle_smartshift(route: &DeviceRoute) -> Result<SmartShiftMode, WriteError> {
    device::toggle_smartshift(&*native_backend(), route).await
}

/// Set the SmartShift auto-disengage sensitivity of the device `route` reaches.
pub async fn set_smartshift_sensitivity(
    route: &DeviceRoute,
    value: SmartShiftAutoDisengage,
) -> Result<SmartShiftStatus, WriteError> {
    device::set_smartshift_sensitivity(&*native_backend(), route, value).await
}

/// Set the SmartShift tunable torque (scrolling force) of the device `route` reaches.
pub async fn set_smartshift_torque(
    route: &DeviceRoute,
    value: TunableTorque,
) -> Result<SmartShiftStatus, WriteError> {
    device::set_smartshift_torque(&*native_backend(), route, value).await
}

/// Read the scroll-wheel resolution and inversion of the device `route` reaches.
pub async fn get_scroll_wheel_mode(route: &DeviceRoute) -> Result<ScrollWheelMode, WriteError> {
    device::get_scroll_wheel_mode(&*native_backend(), route).await
}

/// Set the scroll-wheel resolution of the device `route` reaches.
pub async fn set_scroll_resolution(
    route: &DeviceRoute,
    resolution: ScrollResolution,
) -> Result<ScrollWheelMode, WriteError> {
    device::set_scroll_resolution(&*native_backend(), route, resolution).await
}

/// Set the scroll-wheel inversion of the device `route` reaches.
pub async fn set_scroll_inversion(route: &DeviceRoute, inverted: bool) -> Result<(), WriteError> {
    device::set_scroll_inversion(&*native_backend(), route, inverted).await
}

/// Set both scroll-wheel resolution and inversion in one pass.
pub async fn set_scroll_wheel_mode(
    route: &DeviceRoute,
    resolution: ScrollResolution,
    inverted: bool,
) -> Result<ScrollWheelMode, WriteError> {
    device::set_scroll_wheel_mode(&*native_backend(), route, resolution, inverted).await
}

/// Read the Fn-lock state of the keyboard `route` reaches.
pub async fn get_fn_lock(route: &DeviceRoute) -> Result<FnLockState, WriteError> {
    device::get_fn_lock(&*native_backend(), route).await
}

/// Set the Fn-lock of the keyboard `route` reaches and read it back.
pub async fn set_fn_lock(route: &DeviceRoute, on: bool) -> Result<FnLockState, WriteError> {
    device::set_fn_lock(&*native_backend(), route, on).await
}

/// Read the backlight state of the keyboard `route` reaches.
pub async fn get_backlight(route: &DeviceRoute) -> Result<BacklightState, WriteError> {
    device::get_backlight(&*native_backend(), route).await
}

/// Apply one change to the backlight of the keyboard `route` reaches and
/// read it back; see [`device::update_backlight`].
pub async fn update_backlight(
    route: &DeviceRoute,
    change: BacklightChange,
) -> Result<BacklightState, WriteError> {
    device::update_backlight(&*native_backend(), route, change).await
}

/// Set every key of the keyboard `route` reaches to one colour.
pub async fn set_keyboard_color(
    route: &DeviceRoute,
    r: u8,
    g: u8,
    b: u8,
) -> Result<(), WriteError> {
    set_keyboard_color_with(route, LightingMethod::Auto, r, g, b).await
}

/// Set every key to one colour over a chosen lighting feature.
pub async fn set_keyboard_color_with(
    route: &DeviceRoute,
    method: LightingMethod,
    r: u8,
    g: u8,
    b: u8,
) -> Result<(), WriteError> {
    let target = route.clone();
    let gate = device_io_gate();
    crate::lighting::LightingJob::spawn(route, move |cancel| async move {
        device::LightingWrite {
            method,
            color: openhid_core::color::Rgb::new(r, g, b),
        }
        .apply(
            &*native_backend(),
            &target,
            || cancel.is_cancelled(),
            || gate.allows_io(),
        )
        .await
        .map(drop)
    })?
    .finish()
    .await
}

/// Play a haptic waveform on the device `route` reaches.
pub async fn play_haptic(route: &DeviceRoute, waveform: HapticWaveform) -> Result<(), WriteError> {
    device::play_haptic(&*native_backend(), route, waveform).await
}

/// Apply a light command to the Litra `route` reaches, returning the state
/// it reports afterwards (`None` when only its raw report reaches it).
pub async fn apply_litra(
    route: &DeviceRoute,
    model: LitraModel,
    command: LightCommand,
) -> Result<Option<LightState>, WriteError> {
    device::apply_litra(&*native_backend(), route, model, command).await
}

/// Read the state of the Litra `route` reaches (`None` when only its raw
/// report reaches it, which reads nothing back).
pub async fn read_litra_state(
    route: &DeviceRoute,
    model: LitraModel,
) -> Result<Option<LightState>, WriteError> {
    device::read_litra_state(&*native_backend(), route, model).await
}

/// Watch the Litra `route` reaches for changes made on the light itself
/// (`None` when only its raw report reaches it, which announces nothing).
pub async fn watch_litra(
    route: &DeviceRoute,
    model: LitraModel,
) -> Result<Option<LightWatch>, WriteError> {
    device::watch_litra(&*native_backend(), route, model).await
}

/// Walk the HID++ feature table of the device `route` reaches.
pub async fn dump_features(route: &DeviceRoute) -> Result<Vec<FeatureEntry>, WriteError> {
    device::dump_features(&*native_backend(), route).await
}

/// Walk the firmware entities of the device `route` reaches.
pub async fn dump_firmware_entities(
    route: &DeviceRoute,
) -> Result<Vec<FirmwareEntity>, WriteError> {
    device::dump_firmware_entities(&*native_backend(), route).await
}

/// Walk the reprogrammable controls of the device `route` reaches.
pub async fn dump_reprog_controls(
    route: &DeviceRoute,
) -> Result<Vec<ReprogControlEntry>, WriteError> {
    device::dump_reprog_controls(&*native_backend(), route).await
}

/// Read the raw battery report of the device `route` reaches.
pub async fn read_battery_raw(route: &DeviceRoute) -> Result<String, WriteError> {
    device::read_battery_raw(&*native_backend(), route).await
}

/// An enumerator over this host's HID stack, with a memory-only probe cache.
///
/// One-shot callers (the CLI) want exactly this: nothing to warm-start from and
/// nothing left behind.
#[must_use]
pub fn enumerator() -> Enumerator {
    Enumerator::with_backend(native_backend())
}

/// An enumerator over this host's HID stack whose probe cache is the on-disk
/// one, so a device fully probed once keeps its identity across restarts.
///
/// Falls back to memory-only when no data dir resolves — a warm start is an
/// optimization, never a requirement.
#[must_use]
pub fn persisted_enumerator() -> Enumerator {
    let enumerator = enumerator();
    match FileProbeCacheStore::in_data_dir() {
        Some(store) => enumerator.with_probe_cache(Arc::new(store)),
        None => enumerator,
    }
}

/// A channel pool over this host's HID stack.
#[must_use]
pub fn channel_pool() -> ChannelPool {
    ChannelPool::with_backend(native_backend())
}

/// Enumerate this host's recognized standalone devices.
pub async fn enumerate_standalone() -> Result<Vec<StandaloneDevice>, InventoryError> {
    openhid_device::inventory::standalone::enumerate_standalone(&*native_backend()).await
}

/// Start a wireless lighting edit in the existing owned lighting worker.
/// CLI callers must await `finish`; GUI/agent callers may await `wait` without
/// destroying the transaction's compensation when their request is cancelled.
pub async fn start_aula_lighting(
    identity: Option<&str>,
    change: openhid_device::aula::LightingChange,
    mode: openhid_device::aula::LightingMode,
) -> Result<crate::lighting::LightingJob<openhid_device::aula::ConfigResult>, WriteError> {
    start_aula_lighting_on(native_backend(), device_io_gate(), identity, change, mode).await
}

/// Start a recoverable transaction over the caller's backend, including injected hosts.
pub async fn start_aula_lighting_on(
    backend: Arc<dyn openhid_device::backend::HidBackend>,
    device_io: DeviceIoGate,
    identity: Option<&str>,
    change: openhid_device::aula::LightingChange,
    mode: openhid_device::aula::LightingMode,
) -> Result<crate::lighting::LightingJob<openhid_device::aula::ConfigResult>, WriteError> {
    let change = openhid_device::aula::ConfigChange::Lighting(change);
    start_aula_config_on(backend, device_io, identity, change, mode).await
}

/// Start a recoverable edit of the verified F87 PRO settings.
pub async fn start_aula_settings(
    identity: Option<&str>,
    change: openhid_device::aula::AulaChange,
    mode: openhid_device::aula::LightingMode,
) -> Result<crate::lighting::LightingJob<openhid_device::aula::ConfigResult>, WriteError> {
    start_aula_settings_on(native_backend(), device_io_gate(), identity, change, mode).await
}

/// Start a recoverable F87 PRO settings edit over an injected backend.
pub async fn start_aula_settings_on(
    backend: Arc<dyn openhid_device::backend::HidBackend>,
    device_io: DeviceIoGate,
    identity: Option<&str>,
    change: openhid_device::aula::AulaChange,
    mode: openhid_device::aula::LightingMode,
) -> Result<crate::lighting::LightingJob<openhid_device::aula::ConfigResult>, WriteError> {
    start_aula_config_on(
        backend,
        device_io,
        identity,
        openhid_device::aula::ConfigChange::Settings(change),
        mode,
    )
    .await
}

/// Restore a validated configuration backup with owned recovery and route locking.
pub async fn start_aula_restore(
    identity: Option<&str>,
    config: openhid_device::aula::WirelessConfig,
    mode: openhid_device::aula::LightingMode,
) -> Result<crate::lighting::LightingJob<openhid_device::aula::ConfigResult>, WriteError> {
    start_aula_config_on(
        native_backend(),
        device_io_gate(),
        identity,
        openhid_device::aula::ConfigChange::Restore(config),
        mode,
    )
    .await
}

/// Start a complete wireless per-key RGB table operation in the owned lighting worker.
pub async fn start_aula_per_key_rgb(
    identity: Option<&str>,
    colors: openhid_core::hid::aula::AulaPerKeyRgb,
) -> Result<crate::lighting::LightingJob<openhid_core::hid::aula::AulaState>, WriteError> {
    start_aula_per_key_rgb_on(native_backend(), device_io_gate(), identity, colors).await
}

/// Start per-key RGB over an injected backend, retaining the route lock through completion.
pub async fn start_aula_per_key_rgb_on(
    backend: Arc<dyn openhid_device::backend::HidBackend>,
    device_io: DeviceIoGate,
    identity: Option<&str>,
    colors: openhid_core::hid::aula::AulaPerKeyRgb,
) -> Result<crate::lighting::LightingJob<openhid_core::hid::aula::AulaState>, WriteError> {
    let operation_backend = Arc::clone(&backend);
    start_aula_worker_on(
        backend,
        device_io,
        identity,
        move |identity, cancel| async move {
            openhid_device::aula::apply_selected_per_key_rgb(
                &*operation_backend,
                Some(&identity),
                &colors,
                || cancel.is_cancelled(),
            )
            .await
            .map_err(|error| WriteError::Hid(error.to_string()))
        },
    )
    .await
}

/// Start a complete wireless keymap write in the owned worker with recovery.
pub async fn start_aula_keymap_on(
    backend: Arc<dyn openhid_device::backend::HidBackend>,
    device_io: DeviceIoGate,
    identity: Option<&str>,
    keymap: openhid_core::hid::aula::AulaKeymap,
) -> Result<crate::lighting::LightingJob<openhid_core::hid::aula::AulaKeymap>, WriteError> {
    start_aula_keymap_with_expected_on(backend, device_io, identity, None, keymap).await
}

/// Start a wireless keymap write after checking the caller's loaded snapshot.
pub async fn start_aula_keymap_if_unchanged_on(
    backend: Arc<dyn openhid_device::backend::HidBackend>,
    device_io: DeviceIoGate,
    identity: Option<&str>,
    expected: openhid_core::hid::aula::AulaKeymap,
    keymap: openhid_core::hid::aula::AulaKeymap,
) -> Result<crate::lighting::LightingJob<openhid_core::hid::aula::AulaKeymap>, WriteError> {
    start_aula_keymap_with_expected_on(backend, device_io, identity, Some(expected), keymap).await
}

async fn start_aula_keymap_with_expected_on(
    backend: Arc<dyn openhid_device::backend::HidBackend>,
    device_io: DeviceIoGate,
    identity: Option<&str>,
    expected: Option<openhid_core::hid::aula::AulaKeymap>,
    keymap: openhid_core::hid::aula::AulaKeymap,
) -> Result<crate::lighting::LightingJob<openhid_core::hid::aula::AulaKeymap>, WriteError> {
    let operation_backend = Arc::clone(&backend);
    start_aula_worker_on(
        backend,
        device_io,
        identity,
        move |identity, cancel| async move {
            match expected {
                Some(expected) => {
                    openhid_device::aula::update_selected_keymap_if_unchanged(
                        &*operation_backend,
                        Some(&identity),
                        expected,
                        keymap,
                        || cancel.is_cancelled(),
                    )
                    .await
                }
                None => {
                    openhid_device::aula::update_selected_keymap(
                        &*operation_backend,
                        Some(&identity),
                        keymap,
                        || cancel.is_cancelled(),
                    )
                    .await
                }
            }
            .map_err(|error| WriteError::Hid(error.to_string()))
        },
    )
    .await
}

async fn start_aula_config_on(
    backend: Arc<dyn openhid_device::backend::HidBackend>,
    device_io: DeviceIoGate,
    identity: Option<&str>,
    change: openhid_device::aula::ConfigChange,
    mode: openhid_device::aula::LightingMode,
) -> Result<crate::lighting::LightingJob<openhid_device::aula::ConfigResult>, WriteError> {
    let operation_backend = Arc::clone(&backend);
    start_aula_worker_on(
        backend,
        device_io,
        identity,
        move |identity, cancel| async move {
            openhid_device::aula::update_config(
                &*operation_backend,
                Some(&identity),
                change,
                mode,
                || cancel.is_cancelled(),
            )
            .await
            .map_err(|error| WriteError::Hid(error.to_string()))
        },
    )
    .await
}

async fn start_aula_worker_on<T, Operation, OperationFuture>(
    backend: Arc<dyn openhid_device::backend::HidBackend>,
    device_io: DeviceIoGate,
    identity: Option<&str>,
    operation: Operation,
) -> Result<crate::lighting::LightingJob<T>, WriteError>
where
    T: Send + 'static,
    Operation:
        FnOnce(String, crate::lighting::LightingCancellation) -> OperationFuture + Send + 'static,
    OperationFuture: Future<Output = Result<T, WriteError>> + Send + 'static,
{
    device_io
        .ensure_allowed()
        .map_err(|_| WriteError::DeviceNotFound)?;
    let node = openhid_device::aula::select_node(&*backend, identity)
        .await
        .map_err(|error| WriteError::Hid(error.to_string()))?;
    let identity = node.identity();
    let address = openhid_core::device::RawDeviceAddress {
        vendor_id: node.vendor_id,
        product_id: node.product_id,
        usage_page: node.usage_page,
        usage_id: node.usage_id,
        identity,
    };
    let route = DeviceRoute::from(&address);
    crate::lighting::LightingJob::spawn(&route, move |cancel| async move {
        device_io
            .ensure_allowed()
            .map_err(|_| WriteError::DeviceNotFound)?;
        operation(address.identity, cancel).await
    })
}

/// Read one F87 PRO wireless configuration without changing settings.
/// Pass an exact raw-route identity to select among distinct serial-bearing devices.
pub async fn read_aula_config(
    identity: Option<&str>,
) -> Result<openhid_device::aula::WirelessConfig, openhid_device::aula::ReadError> {
    openhid_device::aula::read_selected_config(&*native_backend(), identity).await
}

/// Read the six-byte OEM model discriminator from one wireless F87 PRO without changing settings.
pub async fn read_aula_psd(
    identity: Option<&str>,
) -> Result<[u8; 6], openhid_device::aula::ReadError> {
    openhid_device::aula::read_selected_psd(&*native_backend(), identity).await
}

/// Read the F87-family wired model response using Feature Report `0x06/0x82`.
pub async fn read_aula_wired_model_probe(
    identity: Option<&str>,
) -> Result<openhid_device::aula::WiredModelProbe, openhid_device::aula::WiredProbeError> {
    openhid_device::aula::read_wired_model_probe(&*native_backend(), identity).await
}

/// Read one complete, validated wired F87 Pro configuration block.
pub async fn read_aula_wired_config(
    identity: Option<&str>,
) -> Result<openhid_device::aula::WiredConfigBlock, openhid_device::aula::WiredConfigReadError> {
    openhid_device::aula::read_wired_config(&*native_backend(), identity).await
}

/// Experimentally read the opaque wired F87 Pro macro-table candidate without writing macro data.
pub async fn read_aula_wired_macro_table(
    identity: Option<&str>,
) -> Result<openhid_device::aula::WiredMacroTable, openhid_device::aula::WiredMacroReadError> {
    openhid_device::aula::read_wired_macro_table(&*native_backend(), identity).await
}

/// Experimentally replace a complete wired F87 Pro macro-table candidate after a persisted backup.
pub async fn write_aula_wired_macro_table(
    identity: Option<&str>,
    expected_original: &openhid_device::aula::WiredMacroTable,
    target: &openhid_device::aula::WiredMacroTable,
) -> Result<(), openhid_device::aula::WiredMacroWriteError> {
    openhid_device::aula::write_wired_macro_table(
        &*native_backend(),
        identity,
        expected_original,
        target,
    )
    .await
}

/// Replace and verify one complete wired F87 Pro configuration block.
pub async fn write_aula_wired_config(
    identity: Option<&str>,
    block: &openhid_device::aula::WiredConfigBlock,
) -> Result<openhid_device::aula::WiredConfigBlock, openhid_device::aula::WiredConfigWriteError> {
    openhid_device::aula::write_wired_config(&*native_backend(), identity, block).await
}

/// Upload one validated wired F87-family keymap layer to an exact collection identity.
pub async fn write_aula_wired_keymap(
    identity: Option<&str>,
    blob: &openhid_core::hid::aula::AulaWiredKeymapBlob,
) -> Result<(), openhid_device::aula::WiredKeymapWriteError> {
    openhid_device::aula::write_wired_keymap(&*native_backend(), identity, blob).await
}

/// Experimental read of raw F87 PRO wireless matrix entries; never writes assignments.
pub async fn read_aula_matrix(
    identity: Option<&str>,
) -> Result<openhid_device::aula::WirelessMatrix, openhid_device::aula::ReadError> {
    openhid_device::aula::read_selected_matrix(&*native_backend(), identity).await
}

/// Read the F87 PRO base matrix with the OEM's unadvertised `0x15` request carrier.
/// This read-only diagnostic tests Linux hidraw and Windows raw-HID report-ID delivery.
pub async fn read_aula_matrix_oem_carrier(
    identity: Option<&str>,
) -> Result<openhid_device::aula::WirelessMatrix, openhid_device::aula::ReadError> {
    ensure_aula_oem_raw_report_transport()?;
    openhid_device::aula::read_selected_matrix_with_oem_carrier(&*native_backend(), identity).await
}

/// Read both keymap layers through the OEM's unadvertised `0x15` carrier.
/// This diagnostic is read-only and does not prove that the keyboard accepts the reports.
pub async fn read_aula_keymap_oem_carrier(
    identity: Option<&str>,
) -> Result<openhid_core::hid::aula::AulaKeymap, openhid_device::aula::ReadError> {
    ensure_aula_oem_raw_report_transport()?;
    openhid_device::aula::read_selected_keymap_with_oem_carrier(&*native_backend(), identity).await
}

/// Read both raw F87 PRO wireless keymap layers without changing the device.
pub async fn read_aula_keymap(
    identity: Option<&str>,
) -> Result<openhid_core::hid::aula::AulaKeymap, openhid_device::aula::ReadError> {
    openhid_device::aula::read_selected_keymap(&*native_backend(), identity).await
}

fn ensure_aula_oem_raw_report_transport() -> Result<(), openhid_device::aula::ReadError> {
    if cfg!(any(target_os = "linux", target_os = "windows")) {
        Ok(())
    } else {
        Err(openhid_device::aula::ReadError::UnsupportedTransport)
    }
}

/// Read the validated 126-entry F87 PRO wireless base matrix through the host backend.
pub async fn read_aula_base_matrix(
    identity: Option<&str>,
) -> Result<openhid_core::hid::aula::AulaBaseMatrix, openhid_device::aula::ReadError> {
    openhid_device::aula::read_selected_base_matrix(&*native_backend(), identity).await
}

/// Apply and verify a complete F87 PRO keymap, restoring the snapshot on failure.
pub async fn update_aula_keymap(
    identity: Option<&str>,
    target: openhid_core::hid::aula::AulaKeymap,
) -> Result<openhid_core::hid::aula::AulaKeymap, openhid_device::aula::KeymapWriteError> {
    openhid_device::aula::update_selected_keymap(&*native_backend(), identity, target, || false)
        .await
}

/// Read the currently supported F87 PRO settings without writing.
pub async fn read_aula_state(
    identity: Option<&str>,
) -> Result<openhid_device::aula::AulaState, openhid_device::aula::ReadError> {
    openhid_device::aula::read_selected_state(&*native_backend(), identity).await
}

/// Subscribe to this host's HID hotplug events.
pub fn watch_hotplug() -> Result<HotplugStream, InventoryError> {
    openhid_device::inventory::hotplug::watch_hotplug(&*native_backend())
}

/// Enumerate the HID++ receivers and paired devices on this host, once.
pub async fn enumerate() -> Result<Vec<DeviceInventory>, InventoryError> {
    openhid_device::inventory::enumerate(native_backend()).await
}

/// List the pairing-capable receivers connected to this host.
pub async fn list_pairing_receivers() -> Result<Vec<PairingReceiver>, PairingError> {
    openhid_device::pairing::list_pairing_receivers(&*native_backend()).await
}
